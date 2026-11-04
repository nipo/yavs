# Install or update yavs on Windows x86_64 without administrator rights:
#
#   powershell -c "irm https://raw.githubusercontent.com/ru551n/yavs/main/install.ps1 | iex"
#
# Options are environment variables, set before the line above (docs/getting-started.rst, "One-line install"):
#   YAVS_VERSION=0.14.0      install that version (default: the newest release); older ones are allowed
#   YAVS_COMPAT=1            use the -compat archive (x86_64 CPUs without AVX2; needs yavs 0.14.0 or newer)
#   YAVS_FORCE=1             install even if that version is already installed
#   YAVS_INSTALL_DIR=DIR     install into DIR (default: %LOCALAPPDATA%\Programs\yavs)
#   YAVS_NO_MODIFY_PATH=1    never offer to add the directory to your user PATH
#   GITHUB_TOKEN             used for the lookup of the newest version only, to avoid GitHub's rate limit
# Read this file first if you like: it downloads a release archive from github.com, checks it against the
# release's SHA256SUMS and unpacks it into a directory of yours. If that directory is not on your user PATH and
# you are at a console, it asks before adding it (default: no). All logic is in functions and Install-Yavs
# runs on the last line, so a download that stops early runs nothing.

function Install-Yavs {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    } catch { }

    $repo = 'ru551n/yavs'
    $releases = "https://github.com/$repo/releases"
    $guide = 'https://yavs.readthedocs.io/en/latest/getting-started.html'

    function Say($text) { Write-Host $text }
    function Fail($text) { throw [System.InvalidOperationException]::new($text) }
    function Flag($name) { $v = [Environment]::GetEnvironmentVariable($name); return ($v -and $v -ne '0') }

    # Download URL to PATH; returns the HTTP status. A transport failure stops here.
    function Get-File($url, $path, $token) {
        $headers = @{}
        if ($token) { $headers['Authorization'] = "Bearer $token" }
        $problem = ''
        # A transport failure is tried again, as curl --retry does; an HTTP status is an answer.
        foreach ($try in 1..3) {
            try {
                Invoke-WebRequest -Uri $url -OutFile $path -UseBasicParsing -Headers $headers | Out-Null
                return 200
            } catch {
                $response = $_.Exception.Response
                if ($response) { return [int]$response.StatusCode }
                $problem = $_.Exception.Message
                Start-Sleep -Seconds $try
            }
        }
        Fail "could not download ${url}: $problem Check the network connection and any proxy settings, then run the installer again."
    }

    function Run-Yavs($exe) {
        $old = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        try {
            $lines = @(& $exe --version 2>&1 | ForEach-Object { "$_" })
            return @{ Code = $LASTEXITCODE; Lines = $lines }
        } catch {
            return @{ Code = -1; Lines = @($_.Exception.Message) }
        } finally {
            $ErrorActionPreference = $old
        }
    }

    # Platform: Windows x86_64 only.
    if ($PSVersionTable.PSEdition -eq 'Core' -and -not $IsWindows) {
        Fail "this is not Windows. On Linux run install.sh (curl -fsSL https://raw.githubusercontent.com/$repo/main/install.sh | sh); macOS has no yavs archive: build it from source, see `"From source`" at $guide"
    }
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    if ($arch -ne 'AMD64') {
        Fail "yavs has a Windows archive for x86_64 only, and this is $arch. Build it from source: see `"From source`" at $guide"
    }
    $target = 'x86_64-pc-windows-msvc'

    # Where to.
    $dir = $env:YAVS_INSTALL_DIR
    if (-not $dir) {
        if (-not $env:LOCALAPPDATA) { Fail 'LOCALAPPDATA is not set, so there is no default install directory. Set YAVS_INSTALL_DIR to a directory of yours.' }
        $dir = Join-Path $env:LOCALAPPDATA 'Programs\yavs'
    }
    $dir = [IO.Path]::GetFullPath($dir).TrimEnd('\')
    $exe = Join-Path $dir 'yavs.exe'

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ('yavs-install-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        # Which version.
        $version = $env:YAVS_VERSION
        if (-not $version) {
            $status = Get-File "https://api.github.com/repos/$repo/releases?per_page=100" "$tmp\releases.json" $env:GITHUB_TOKEN
            if ($status -eq 403 -or $status -eq 429) {
                Fail "GitHub's rate limit stopped the lookup of the newest yavs release (HTTP $status). Wait a while and run again, set GITHUB_TOKEN to a token for a higher limit, or name the version: `$env:YAVS_VERSION='0.14.0' needs no lookup (versions are listed at $releases)."
            }
            if ($status -ne 200) {
                Fail "could not look up the newest yavs release: api.github.com answered HTTP $status (403 or 429 is its rate limit). Run again later, or name the version: `$env:YAVS_VERSION='0.14.0' needs no lookup (versions are listed at $releases)."
            }
            # Stable means a plain vX.Y.Z tag. All yavs releases carry GitHub's pre-release mark while yavs is early,
            # so /releases/latest (which skips them) cannot be used. The vunit-yavs-v* and vscode-v* tags are other products.
            # ponytail: reads the newest 100 releases, enough while a page holds at least one stable release.
            $tags = @((Get-Content -Raw -LiteralPath "$tmp\releases.json" | ConvertFrom-Json) | ForEach-Object { $_.tag_name } | Where-Object { $_ -match '^v\d+\.\d+\.\d+$' })
            if ($tags.Count -eq 0) { Fail "found no yavs release at $releases. Name the version with `$env:YAVS_VERSION='0.14.0'." }
            $version = ($tags | ForEach-Object { $_.Substring(1) } | Sort-Object { [version]$_ } | Select-Object -Last 1)
        } else {
            $version = $version.TrimStart('v')
            if ($version -notmatch '^\d+\.\d+\.\d+$') {
                Fail "'$env:YAVS_VERSION' is not a version. Write an exact version such as 0.14.0, or leave YAVS_VERSION unset for the newest."
            }
        }

        # Which archive. IsProcessorFeaturePresent(40) is PF_AVX2_INSTRUCTIONS_AVAILABLE: the x86-64-v3 archive needs
        # AVX2 and also BMI1, BMI2, FMA, F16C, LZCNT and MOVBE, which every Intel and AMD CPU with AVX2 has; the new yavs
        # checks them all itself before it is installed, and prints what is missing.
        Say 'yavs installer: Windows x86_64'
        $compat = $false
        if ([version]$version -lt [version]'0.14.0') {
            if (Flag 'YAVS_COMPAT') { Fail "YAVS_COMPAT needs yavs 0.14.0 or newer: yavs $version has one x86_64 archive and no -compat one. Use YAVS_VERSION=0.14.0 or newer, or leave YAVS_COMPAT unset." }
            Say "yavs $version has one x86_64 archive, so no CPU check is made."
        } elseif (Flag 'YAVS_COMPAT') {
            $compat = $true
            Say 'YAVS_COMPAT: using the -compat archive, which runs on any x86-64 CPU.'
        } else {
            $avx2 = $null
            if (Load-Native) { $avx2 = [YavsInstall.Native]::IsProcessorFeaturePresent(40) }
            if ($null -eq $avx2) {
                $compat = $true
                Say 'cannot ask Windows whether this CPU has AVX2: using the -compat archive, which runs on any x86-64 CPU.'
            } elseif (-not $avx2) {
                $compat = $true
                Say 'Windows says this CPU has no AVX2: using the -compat archive, which runs on any x86-64 CPU.'
            } else {
                Say 'Windows says this CPU has AVX2 (and so BMI1, BMI2, FMA, F16C, LZCNT and MOVBE): using the x86-64-v3 archive.'
            }
        }
        $suffix = ''
        if ($compat) { $suffix = '-compat' }
        $variant = if ($compat) { '-compat' } else { 'x86-64-v3' }
        $name = "yavs-$version-$target$suffix"
        $file = "$name.zip"

        # What is installed now.
        $current = $null
        $curVariant = $null
        if (Test-Path -LiteralPath $exe) {
            $r = Run-Yavs $exe
            if ($r.Code -eq 0 -and $r.Lines.Count -gt 0 -and $r.Lines[0] -match '^yavs (\d+\.\d+\.\d+)') {
                $current = $Matches[1]
                $text = $r.Lines -join "`n"
                if ($text -match 'x86-64-v3') { $curVariant = 'x86-64-v3' } elseif ($text -match 'x86-64 baseline') { $curVariant = '-compat' }
            }
        }
        if (-not $current) {
            if (Test-Path -LiteralPath $exe) { Say "replacing $exe, which does not report a version, with yavs $version" }
            else { Say "installing yavs $version into $dir" }
        } elseif ($current -eq $version -and (-not $curVariant -or $curVariant -eq $variant)) {
            if (-not (Flag 'YAVS_FORCE')) {
                Say "already at $version ($exe). Nothing to do; YAVS_FORCE=1 installs it again."
                Add-ToPath
                return
            }
            Say "reinstalling yavs $version (YAVS_FORCE)"
        } elseif ($current -eq $version) {
            Say "yavs $current ($curVariant archive) -> $version ($variant archive): same version, other archive"
        } elseif ([version]$version -lt [version]$current) {
            Say "yavs $current -> $version (a downgrade)"
        } else {
            Say "yavs $current -> $version"
        }

        # Download and verify.
        $base = "$releases/download/v$version"
        $status = Get-File "$base/SHA256SUMS" "$tmp\SHA256SUMS" $null
        if ($status -eq 404) { Fail "there is no yavs release v$version (HTTP 404 for $base/SHA256SUMS). Choose a version listed at $releases, or leave YAVS_VERSION unset for the newest." }
        if ($status -ne 200) { Fail "could not download $base/SHA256SUMS: HTTP $status. Run again later; if it keeps failing, check $releases and your access to github.com." }
        $expected = $null
        foreach ($line in Get-Content -LiteralPath "$tmp\SHA256SUMS") {
            $parts = $line.Trim() -split '\s+'
            if ($parts.Count -eq 2 -and $parts[1] -eq $file) { $expected = $parts[0] }
        }
        if (-not $expected) { Fail "SHA256SUMS of yavs v$version does not list $file, so it cannot be verified. Choose another version listed at $releases." }
        Say "downloading $file"
        $status = Get-File "$base/$file" "$tmp\$file" $null
        if ($status -eq 404) { Fail "yavs v$version has no archive $file (HTTP 404). Choose a version at $releases that has one for this system." }
        if ($status -ne 200) { Fail "could not download $base/${file}: HTTP $status. Run again later; if it keeps failing, check $releases and your access to github.com." }
        $got = (Get-FileHash -Algorithm SHA256 -LiteralPath "$tmp\$file").Hash.ToLowerInvariant()
        if ($got -ne $expected.ToLowerInvariant()) {
            Fail "$file does not match its line in SHA256SUMS (expected $expected, got $got): the download is damaged or was changed. Nothing was installed. Run the installer again; if it keeps failing, report it at https://github.com/$repo/issues."
        }
        Say "checksum ok (SHA-256 from the release's SHA256SUMS)"

        # Unpack, try the new binary where it is, then put it in place.
        Expand-Archive -LiteralPath "$tmp\$file" -DestinationPath "$tmp\x" -Force
        $src = "$tmp\x\$name"
        if (-not (Test-Path -LiteralPath "$src\yavs.exe")) { Fail "$file does not hold $name\yavs.exe. Report it at https://github.com/$repo/issues." }
        try {
            New-Item -ItemType Directory -Force -Path $dir | Out-Null
            # A yavs.old.exe left by an earlier update, whose yavs was running then.
            Remove-Item -LiteralPath "$dir\yavs.old.exe" -Force -ErrorAction SilentlyContinue
            Copy-Item -LiteralPath "$src\yavs.exe" -Destination "$dir\yavs.new.exe" -Force
        } catch {
            Fail "could not write to ${dir}: $($_.Exception.Message). Check its permissions and free space, or choose another directory with YAVS_INSTALL_DIR."
        }
        $r = Run-Yavs "$dir\yavs.new.exe"
        $said = ''
        if ($r.Lines.Count -gt 0) { $said = $r.Lines[0] }
        if ($r.Code -ne 0) {
            Remove-Item -LiteralPath "$dir\yavs.new.exe" -Force -ErrorAction SilentlyContinue
            Fail "the downloaded yavs does not run here: $($r.Lines -join ' '). If it names a missing CPU feature, run again with YAVS_COMPAT=1. Requirements: $guide. Your installed yavs, if any, is untouched."
        }
        if ($said -ne "yavs $version" -and -not $said.StartsWith("yavs $version ")) {
            Remove-Item -LiteralPath "$dir\yavs.new.exe" -Force -ErrorAction SilentlyContinue
            Fail "asked for yavs $version, but the archive's yavs --version says: $said. Nothing was installed. Report it at https://github.com/$repo/issues."
        }
        # README and licences first, the binary last. Windows will not overwrite or delete a running yavs.exe but lets
        # it be renamed: the old file moves to yavs.old.exe (a running simulation goes on with it), the new one takes its
        # name, and the old one is deleted, or at the next update if it is still running.
        try {
            Get-ChildItem -LiteralPath $src -File | Where-Object { $_.Name -like 'README*' -or $_.Name -like 'LICENSE*' } |
                ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination "$dir\$($_.Name)" -Force }
            $hadOld = Test-Path -LiteralPath $exe
            if ($hadOld) { Move-Item -LiteralPath $exe -Destination "$dir\yavs.old.exe" -Force }
            try {
                Move-Item -LiteralPath "$dir\yavs.new.exe" -Destination $exe -Force
            } catch {
                if ($hadOld) { Move-Item -LiteralPath "$dir\yavs.old.exe" -Destination $exe -Force }
                throw
            }
        } catch {
            Remove-Item -LiteralPath "$dir\yavs.new.exe" -Force -ErrorAction SilentlyContinue
            Fail "could not replace ${exe}: $($_.Exception.Message). Your installed yavs, if any, still works. Check the permissions of $dir."
        }
        Say "installed yavs ${version}: $exe"
        Say "README and licences: $dir"
        if (Test-Path -LiteralPath "$dir\yavs.old.exe") {
            try { Remove-Item -LiteralPath "$dir\yavs.old.exe" -Force }
            catch { Say "the previous yavs is still running, so its file stays as $dir\yavs.old.exe until the next update; the running yavs is not affected." }
        }
        Add-ToPath
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Load-Native: the two Windows calls the installer makes (CPU feature, environment change broadcast).
function Load-Native {
    try {
        if (-not ('YavsInstall.Native' -as [type])) {
            Add-Type -Namespace YavsInstall -Name Native -MemberDefinition @'
[DllImport("kernel32.dll")] public static extern bool IsProcessorFeaturePresent(uint feature);
[DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint msg, UIntPtr wParam, string lParam, uint flags, uint timeout, out UIntPtr result);
'@
        }
        return $true
    } catch {
        return $false
    }
}

# Test-Interactive: a person can be asked. False for a service session, redirected input and -NonInteractive.
function Test-Interactive {
    if (-not [Environment]::UserInteractive) { return $false }
    if ([Console]::IsInputRedirected) { return $false }
    foreach ($a in [Environment]::GetCommandLineArgs()) { if ($a -like '-noni*') { return $false } }
    return $true
}

# Add-ToPath: if $dir is not on the user PATH, offer to add it (this user only, never the machine's). Nothing is
# asked or changed without a person at the console or with YAVS_NO_MODIFY_PATH set; the way to add it is printed.
# The registry value is read unexpanded and written back with its type, so %VAR% entries stay as they are.
function Add-ToPath {
    $same = { param($a, $b) $a.TrimEnd('\') -ieq $b.TrimEnd('\') }
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    try {
        $value = $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
        if ($key.GetValueNames() -contains 'Path') { $kind = $key.GetValueKind('Path') }
        $present = @($value -split ';' | Where-Object { $_ -and (& $same ([Environment]::ExpandEnvironmentVariables($_)) $dir) }).Count -gt 0
        if ($present) { return }
        Say ''
        Say "$dir is not on your user PATH."
        $how = "To add it yourself, put it in the Path variable of your user (run rundll32 sysdm.cpl,EditEnvironmentVariables), then open a new terminal."
        if (Flag 'YAVS_NO_MODIFY_PATH') { Say "YAVS_NO_MODIFY_PATH is set, so it is not changed. $how"; return }
        if (-not (Test-Interactive)) { Say "No terminal to ask, so it is not changed. $how"; return }
        $answer = Read-Host "Add $dir to your user PATH? [y/N]"
        if ($answer -notmatch '^(y|yes)$') { Say "Not changed. $how"; return }
        $new = $dir
        if ($value) { $new = $value.TrimEnd(';') + ';' + $dir }
        $key.SetValue('Path', $new, $kind)
        # Tell running programs (Explorer, so new terminals) that the environment changed.
        if (Load-Native) {
            $res = [UIntPtr]::Zero
            $null = [YavsInstall.Native]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$res)
        }
        if (-not (@($env:Path -split ';' | Where-Object { $_ -and (& $same $_ $dir) }).Count -gt 0)) { $env:Path = $dir + ';' + $env:Path }
        Say "Added $dir to your user PATH, and to this session's. New terminals pick it up."
    } finally {
        $key.Close()
    }
}

function Install-YavsMain {
    try {
        Install-Yavs
    } catch {
        Write-Host "error: $($_.Exception.Message)" -ForegroundColor Red
        # Run as a file, exit with status 1. Through iex there is no file: exit would close the user's window.
        if ($PSCommandPath) { exit 1 }
        $global:LASTEXITCODE = 1
    }
}

Install-YavsMain
