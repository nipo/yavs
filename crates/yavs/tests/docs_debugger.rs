//! The worked examples of the console pages (`PAGES`) are run, not guessed: a comment
//! `.. example: [top=NAME] FLAGS` starts one, and the literal block after it holds `yavs>` lines
//! and what they print. `.. example-shell:` starts a block of `$ COMMAND` lines (`yavs` is the
//! binary under test) and what each prints. `YAVS_BLESS=1` writes the output of a real run into
//! the page. An example with `--surfer` needs the real viewer and a display
//! (`YAVS_TEST_SURFER=1`). The design files are those of `docs/examples/debugger`, analyzed into
//! the library `lib`, with the run's directory shown as `/work`.

#![cfg(unix)]

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The pages with examples.
const PAGES: [&str; 2] = ["docs/interactive.rst", "docs/console-reference.rst"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The text of a run in `dir`, with the run's directory as `/work`.
fn plain(dir: &Path, text: &str) -> String {
    let canon = dir.canonicalize().expect("dir");
    let (a, b) = (
        dir.to_string_lossy().into_owned(),
        canon.to_string_lossy().into_owned(),
    );
    text.replace(&b, "/work").replace(&a, "/work")
}

/// What the commands print, with the prompts put back in. Between commands a word that is no
/// command marks where the answer of one ends.
fn run(dir: &Path, top: &str, flags: &str, cmds: &[&str]) -> Vec<String> {
    const MARK: &str = "zzmarkzz";
    let log = dir.join("example.log");
    let file = std::fs::File::create(&log).expect("log");
    let mut c = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .current_dir(dir)
        .env("YAVS_STD_CACHE", "off")
        .args(["run", "--work=work:lib", top, "--interactive"])
        .args(flags.split_whitespace())
        .stdin(Stdio::piped())
        .stdout(file.try_clone().expect("clone"))
        .stderr(file)
        .spawn()
        .expect("yavs");
    let mut input = String::new();
    for k in cmds {
        input += &format!("{k}\n{MARK}\n");
    }
    input += "quit\n";
    c.stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write");
    c.wait().expect("wait");
    let text = plain(dir, &std::fs::read_to_string(&log).expect("log"));
    let mut out = Vec::new();
    let mut next = cmds.iter();
    out.push(format!("yavs> {}", next.next().expect("a command")));
    for l in text.lines() {
        if l.is_empty() {
            continue;
        }
        if l.contains(MARK) {
            if let Some(k) = next.next() {
                out.push(format!("yavs> {k}"));
            }
            continue;
        }
        out.push(l.to_string());
    }
    out
}

/// What shell commands print (both output streams), each after its `$ ` line.
fn shell(dir: &Path, cmds: &[&str]) -> Vec<String> {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let link = bin.join("yavs");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_yavs"), &link).expect("link");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut out = Vec::new();
    for k in cmds {
        out.push(format!("$ {k}"));
        let log = dir.join("example.log");
        let file = std::fs::File::create(&log).expect("log");
        let status = Command::new("sh")
            .current_dir(dir)
            .env("PATH", &path)
            .env("YAVS_STD_CACHE", "off")
            .args(["-c", k])
            .stdin(Stdio::null())
            .stdout(file.try_clone().expect("clone"))
            .stderr(file)
            .status()
            .expect("sh");
        assert!(status.code().is_some(), "`{k}` was killed");
        let text = plain(dir, &std::fs::read_to_string(&log).expect("log"));
        out.extend(text.lines().map(str::to_string));
    }
    out
}

/// A directory with the example designs, analyzed.
fn setup() -> common::Tmp {
    let d = common::tempdir("docs-debugger");
    let src = root().join("docs/examples/debugger");
    for e in std::fs::read_dir(&src).expect("examples").flatten() {
        let name = e.file_name();
        std::fs::copy(e.path(), d.join(&name)).expect("copy");
        if name.to_string_lossy().ends_with(".vhd") {
            let ok = Command::new(env!("CARGO_BIN_EXE_yavs"))
                .current_dir(&d)
                .env("YAVS_STD_CACHE", "off")
                .args(["analyze", "--std=2008", "--work=work:lib"])
                .arg(&name)
                .status()
                .expect("yavs");
            assert!(ok.success(), "{name:?} does not analyze");
        }
    }
    d
}

/// The page with every example of it run; the number of them. With `bless` the page is written.
fn check_page(rel: &str, dir: &Path, bless: bool) -> usize {
    let page = root().join(rel);
    let text = std::fs::read_to_string(&page).expect("page");
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let mut examples = 0;
    while i < lines.len() {
        out.push(lines[i].to_string());
        let (shell_block, flags) = match (
            lines[i].strip_prefix(".. example-shell:"),
            lines[i].strip_prefix(".. example:"),
        ) {
            (Some(f), _) => (true, f),
            (_, Some(f)) => (false, f),
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;
        // Up to the block: then its indented lines.
        while !lines[i].starts_with(".. code-block::") {
            out.push(lines[i].to_string());
            i += 1;
        }
        out.push(lines[i].to_string());
        out.push(String::new());
        i += 2;
        let start = i;
        while i < lines.len() && (lines[i].starts_with("    ") || lines[i].is_empty()) {
            i += 1;
        }
        while lines[i - 1].is_empty() {
            i -= 1;
        }
        let block: Vec<&str> = lines[start..i]
            .iter()
            .map(|l| l.strip_prefix("    ").unwrap_or(l))
            .collect();
        let prompt = if shell_block { "$ " } else { "yavs> " };
        let cmds: Vec<&str> = block
            .iter()
            .filter_map(|l| l.strip_prefix(prompt))
            .collect();
        assert!(
            !cmds.is_empty(),
            "{rel}: an example without commands at line {start}"
        );
        examples += 1;
        if flags.contains("--surfer") && std::env::var_os("YAVS_TEST_SURFER").is_none() {
            out.extend(block.iter().map(|l| format!("    {l}")));
            continue;
        }
        let now = if shell_block {
            shell(dir, &cmds)
        } else {
            let mut top = "tb";
            let mut rest = Vec::new();
            for f in flags.split_whitespace() {
                match f.strip_prefix("top=") {
                    Some(t) => top = t,
                    None => rest.push(f),
                }
            }
            run(dir, top, &rest.join(" "), &cmds)
        };
        if bless {
            out.extend(now.iter().map(|l| format!("    {l}")));
        } else {
            let was: Vec<String> = block.iter().map(|l| l.to_string()).collect();
            assert_eq!(
                was, now,
                "{rel}: the example at line {start} of the page differs"
            );
            out.extend(block.iter().map(|l| format!("    {l}")));
        }
    }
    if bless {
        let mut t = out.join("\n");
        t.push('\n');
        std::fs::write(&page, t).expect("write");
    }
    examples
}

#[test]
fn the_examples_of_the_console_pages_are_what_a_run_prints() {
    let d = setup();
    let bless = std::env::var_os("YAVS_BLESS").is_some();
    for rel in PAGES {
        let n = check_page(rel, &d, bless);
        assert!(n >= 6, "{rel}: {n} examples");
    }
}
