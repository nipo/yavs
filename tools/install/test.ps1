# Tests install.ps1 against the real releases (it downloads yavs 0.13.0 and 0.14.0 and needs network):
#   powershell -File tools\install\test.ps1 [PATH-TO-install.ps1]      (also works with pwsh)
# Windows x86_64. It changes the user PATH in the registry and puts it back at the end: run it on a CI runner
# or a machine you do not mind that on. Exit status 1 if any check fails.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$scriptPath = if ($args.Count -gt 0) { $args[0] } else { Join-Path $here '..\..\install.ps1' }
$scriptPath = (Resolve-Path -LiteralPath $scriptPath).Path
$text = Get-Content -Raw -LiteralPath $scriptPath
# The script as `irm | iex` runs it, except that whether a person is at the console is the test's to say.
$patched = $text.Replace('function Test-Interactive {', 'function Test-InteractiveReal {')
if ($patched -eq $text) { throw 'install.ps1 has no Test-Interactive function' }
$work = Join-Path ([IO.Path]::GetTempPath()) ('yavs-install-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$script:fails = 0
$script:cur = ''
$script:out = ''
$script:interactive = $false
$script:answer = 'n'
$script:corrupt = $false

function Ok($m) { Write-Host "ok: $($script:cur): $m" }
function Bad($m) { Write-Host "FAIL: $($script:cur): $m"; $script:fails++; ($script:out -split "`n") | ForEach-Object { Write-Host "    | $_" } }
function Has($s) { if ($script:out.Contains($s)) { Ok "output has '$s'" } else { Bad "output lacks '$s'" } }
function Lacks($s) { if (-not $script:out.Contains($s)) { Ok "output lacks '$s'" } else { Bad "output has '$s'" } }
function Cond($name, $value) { if ($value) { Ok $name } else { Bad $name } }
function Rcis($n) { if ($global:LASTEXITCODE -eq $n) { Ok "exit status $n" } else { Bad "exit status $($global:LASTEXITCODE), wanted $n" } }

# What the script's Test-Interactive and Read-Host ask for are answered here.
function Test-Interactive { return $script:interactive }
function Read-Host { param($Prompt) Write-Host "${Prompt}$($script:answer)"; return $script:answer }
# Invoke-WebRequest, with the SHA256SUMS corrupted on request.
function Invoke-WebRequest {
    [CmdletBinding()] param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing, [hashtable]$Headers)
    Microsoft.PowerShell.Utility\Invoke-WebRequest @PSBoundParameters
    if ($script:corrupt -and $Uri -like '*/SHA256SUMS') {
        (Get-Content -LiteralPath $OutFile) -replace '^..', 'xx' | Set-Content -LiteralPath $OutFile
    }
}

# Install DIR [name=value ...]: install.ps1 through iex, the way a user runs it.
function Install($dir, $options, $interactive, $answer) {
    foreach ($n in 'YAVS_VERSION', 'YAVS_COMPAT', 'YAVS_FORCE', 'YAVS_NO_MODIFY_PATH') { [Environment]::SetEnvironmentVariable($n, $null) }
    $env:YAVS_INSTALL_DIR = $dir
    foreach ($k in $options.Keys) { [Environment]::SetEnvironmentVariable($k, $options[$k]) }
    $script:interactive = $interactive
    $script:answer = $answer
    $global:LASTEXITCODE = 0
    $script:out = (& { Invoke-Expression $patched } *>&1 | Out-String -Width 4096)
}
function First($exe) { $o = & $exe --version 2>&1 | Select-Object -First 1; return "$o" }
function UserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try {
        $has = $key.GetValueNames() -contains 'Path'
        $v = $key.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $k = $null; if ($has) { $k = $key.GetValueKind('Path') }
        return @{ Value = $v; Kind = $k }
    } finally { $key.Close() }
}
function SetUserPath($p) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    try {
        if ($null -eq $p.Value) { $key.DeleteValue('Path', $false) } else { $key.SetValue('Path', $p.Value, $p.Kind) }
    } finally { $key.Close() }
}

$origPath = UserPath
$origSession = $env:Path
try {
    $p = Join-Path $work 'p'
    $exe = "$p\yavs.exe"

    $script:cur = 'fresh install of 0.14.0'
    $before = UserPath
    Install $p @{ YAVS_VERSION = '0.14.0' } $false 'n'
    Rcis 0; Has 'installing yavs 0.14.0'; Has 'checksum ok'; Has 'using the x86-64-v3 archive'; Has 'is not on your user PATH'; Has 'No terminal to ask'
    Cond 'yavs runs and is 0.14.0' ((First $exe) -eq 'yavs 0.14.0 (compiled tier: cranelift)')
    Cond 'README.txt and a licence are there' ((Test-Path "$p\README.txt") -and (Test-Path "$p\LICENSE-MIT"))
    Cond 'no staging file is left' (-not (Test-Path "$p\yavs.new.exe"))
    Cond 'the user PATH was not changed' ((UserPath).Value -eq $before.Value)
    $created = (Get-Item $exe).CreationTimeUtc.Ticks

    $script:cur = 'update no-op'
    Install $p @{ YAVS_VERSION = '0.14.0' } $false 'n'
    Rcis 0; Has 'already at 0.14.0'; Lacks 'downloading'
    Cond 'the binary was not replaced' ((Get-Item $exe).CreationTimeUtc.Ticks -eq $created)

    $script:cur = 'YAVS_FORCE'
    Install $p @{ YAVS_VERSION = '0.14.0'; YAVS_FORCE = '1' } $false 'n'
    Rcis 0; Has 'reinstalling yavs 0.14.0'; Has 'checksum ok'

    $script:cur = 'replacing a running yavs'
    Remove-Item -LiteralPath $exe -Force
    Copy-Item -LiteralPath "$env:SystemRoot\System32\ping.exe" -Destination $exe
    $proc = Start-Process -FilePath $exe -ArgumentList '-n', '40', '127.0.0.1' -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds 1
    Install $p @{ YAVS_VERSION = '0.14.0' } $false 'n'
    Rcis 0; Has 'does not report a version'; Has 'installed yavs 0.14.0'; Has 'still running'
    Cond 'the running process was not disturbed' (-not $proc.HasExited)
    Cond 'the new yavs is 0.14.0' ((First $exe) -eq 'yavs 0.14.0 (compiled tier: cranelift)')
    Cond 'the old file is kept as yavs.old.exe' (Test-Path "$p\yavs.old.exe")
    $proc.Kill(); $proc.WaitForExit()
    Install $p @{ YAVS_VERSION = '0.14.0'; YAVS_FORCE = '1' } $false 'n'
    Cond 'the next update deletes yavs.old.exe' (-not (Test-Path "$p\yavs.old.exe"))

    $script:cur = 'downgrade to 0.13.0'
    Install $p @{ YAVS_VERSION = '0.13.0' } $false 'n'
    Rcis 0; Has 'yavs 0.14.0 -> 0.13.0 (a downgrade)'; Has 'has one x86_64 archive'
    Cond 'yavs is 0.13.0' ((First $exe) -eq 'yavs 0.13.0 (compiled tier: cranelift)')

    $script:cur = 'YAVS_COMPAT on 0.13.0'
    Install $p @{ YAVS_VERSION = '0.13.0'; YAVS_COMPAT = '1' } $false 'n'
    Rcis 1; Has 'YAVS_COMPAT needs yavs 0.14.0 or newer'
    Cond 'the installed yavs still runs' ((First $exe) -eq 'yavs 0.13.0 (compiled tier: cranelift)')

    $script:cur = 'newest version'
    Install $p @{} $false 'n'
    Rcis 0; Has 'yavs 0.13.0 -> '
    $newest = [version]((First $exe) -replace '^yavs (\d+\.\d+\.\d+).*', '$1')
    Cond "yavs is now 0.14.0 or newer ($newest)" ($newest -ge [version]'0.14.0')

    $script:cur = 'YAVS_COMPAT'
    Install $p @{ YAVS_VERSION = '0.14.0' } $false 'n'
    Install $p @{ YAVS_VERSION = '0.14.0'; YAVS_COMPAT = '1' } $false 'n'
    Rcis 0; Has 'using the -compat archive'; Has 'x86-64-v3 archive) -> 0.14.0 (-compat archive)'
    Cond 'yavs is the baseline build' (((& $exe --version) -join "`n").Contains('Build: x86-64 baseline'))
    $script:cur = 'back to the plain archive'
    Install $p @{ YAVS_VERSION = '0.14.0' } $false 'n'
    Has '-compat archive) -> 0.14.0 (x86-64-v3 archive)'
    Cond 'yavs is the v3 build' (((& $exe --version) -join "`n").Contains('Build: x86-64-v3'))

    $script:cur = 'bad checksum'
    $script:corrupt = $true
    $bad = Join-Path $work 'pbad'
    Install $bad @{ YAVS_VERSION = '0.14.0' } $false 'n'
    $script:corrupt = $false
    Rcis 1; Has 'does not match its line in SHA256SUMS'; Has 'Nothing was installed'
    Cond 'nothing was installed' (-not (Test-Path "$bad\yavs.exe"))

    $script:cur = 'unknown version'
    Install (Join-Path $work 'pnone') @{ YAVS_VERSION = '9.9.9' } $false 'n'
    Rcis 1; Has 'there is no yavs release v9.9.9'

    # The PATH question, on the user PATH in the registry (put back at the end).
    $entry = '%USERPROFILE%\yavs-test-keep'
    SetUserPath @{ Value = "$entry;C:\yavs-test-other"; Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString }
    $start = UserPath
    $script:cur = 'PATH question: no terminal'
    Install $p @{ YAVS_VERSION = '0.14.0' } $false 'y'
    Has 'is not on your user PATH'; Has 'No terminal to ask'; Lacks '[y/N]'
    Cond 'the user PATH is unchanged' ((UserPath).Value -eq $start.Value)
    $script:cur = 'PATH question: YAVS_NO_MODIFY_PATH'
    Install $p @{ YAVS_VERSION = '0.14.0'; YAVS_NO_MODIFY_PATH = '1' } $true 'y'
    Has 'YAVS_NO_MODIFY_PATH is set'; Lacks '[y/N]'
    Cond 'the user PATH is unchanged' ((UserPath).Value -eq $start.Value)
    $script:cur = 'PATH question: no'
    Install $p @{ YAVS_VERSION = '0.14.0' } $true 'n'
    Has "Add $p to your user PATH? [y/N]"; Has 'Not changed'
    Cond 'the user PATH is unchanged' ((UserPath).Value -eq $start.Value)
    $script:cur = 'PATH question: yes'
    Install $p @{ YAVS_VERSION = '0.14.0' } $true 'y'
    Has "Add $p to your user PATH? [y/N]"; Has 'to your user PATH, and to this session'
    $now = UserPath
    Cond 'the directory is appended to the old value' ($now.Value -eq "$($start.Value);$p")
    Cond '%USERPROFILE% is still unexpanded' ($now.Value.StartsWith($entry))
    Cond 'the value kind is kept' ($now.Kind -eq $start.Kind)
    Cond 'this session has it too' (($env:Path -split ';') -contains $p)
    $script:cur = 'PATH question: second run'
    Install $p @{ YAVS_VERSION = '0.14.0' } $true 'y'
    Lacks '[y/N]'; Lacks 'not on your user PATH'
    Cond 'the directory is there once' ((($now.Value -split ';') | Where-Object { $_ -eq $p }).Count -eq 1 -and (UserPath).Value -eq $now.Value)
    SetUserPath $origPath

    # The real check for a person at the console, in a child process started without one.
    $script:cur = 'child process with -NonInteractive'
    $self = (Get-Process -Id $PID).Path
    $env:YAVS_INSTALL_DIR = Join-Path $work 'pchild'
    $env:YAVS_VERSION = '0.14.0'
    $env:YAVS_NO_MODIFY_PATH = $null
    $script:out = (& $self -NoProfile -NonInteractive -File $scriptPath 2>&1 | Out-String -Width 4096)
    $global:LASTEXITCODE = $LASTEXITCODE
    Rcis 0; Has 'checksum ok'; Has 'No terminal to ask'; Lacks '[y/N]'
    Cond 'the user PATH is unchanged' ((UserPath).Value -eq $origPath.Value)
    $env:YAVS_VERSION = '9.9.9'
    $script:out = (& $self -NoProfile -NonInteractive -File $scriptPath 2>&1 | Out-String -Width 4096)
    $global:LASTEXITCODE = $LASTEXITCODE
    Rcis 1; Has 'there is no yavs release v9.9.9'
} finally {
    SetUserPath $origPath
    $env:Path = $origSession
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Host ''
if ($script:fails -eq 0) { Write-Host 'install.ps1: all checks passed'; exit 0 } else { Write-Host "install.ps1: $($script:fails) checks FAILED"; exit 1 }
