//! The worked examples of `docs/interactive.rst` are run, not guessed: a comment
//! `.. example: FLAGS` starts one, and the literal block after it holds `yavs>` lines and what
//! they print. `YAVS_BLESS=1` writes the output of a real run into the page. An example with
//! `--surfer` needs the real viewer and a display (`YAVS_TEST_SURFER=1`).

#![cfg(unix)]

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const FILE: &str = "tb.vhd";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// What the commands print, with the prompts put back in, and the run's directory as `/work`.
/// Between commands a word that is no command marks where the answer of one ends.
fn run(dir: &Path, flags: &str, cmds: &[&str]) -> Vec<String> {
    const MARK: &str = "zzmarkzz";
    let log = dir.join("example.log");
    let file = std::fs::File::create(&log).expect("log");
    let mut c = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .current_dir(dir)
        .env("YAVS_STD_CACHE", "off")
        .args(["run", "--work=work:lib", "tb", "--interactive"])
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
    let text = std::fs::read_to_string(&log).expect("log");
    let canon = dir.canonicalize().expect("dir");
    let (a, b) = (
        dir.to_string_lossy().into_owned(),
        canon.to_string_lossy().into_owned(),
    );
    let text = text.replace(&b, "/work").replace(&a, "/work");
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

#[test]
fn the_examples_of_the_interactive_page_are_what_a_run_prints() {
    let page = root().join("docs/interactive.rst");
    let text = std::fs::read_to_string(&page).expect("page");
    let d = common::tempdir("docs-debugger");
    std::fs::copy(root().join("docs/examples/debugger/tb.vhd"), d.join(FILE)).expect("copy");
    let ok = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .current_dir(&d)
        .env("YAVS_STD_CACHE", "off")
        .args(["analyze", "--std=2008", "--work=work:lib", FILE])
        .status()
        .expect("yavs");
    assert!(ok.success());
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let mut examples = 0;
    let bless = std::env::var_os("YAVS_BLESS").is_some();
    while i < lines.len() {
        out.push(lines[i].to_string());
        let Some(flags) = lines[i].strip_prefix(".. example:") else {
            i += 1;
            continue;
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
        let cmds: Vec<&str> = block
            .iter()
            .filter_map(|l| l.strip_prefix("yavs> "))
            .collect();
        assert!(
            !cmds.is_empty(),
            "an example without commands at line {start}"
        );
        examples += 1;
        if flags.contains("--surfer") && std::env::var_os("YAVS_TEST_SURFER").is_none() {
            out.extend(block.iter().map(|l| format!("    {l}")));
            continue;
        }
        let now = run(&d, flags, &cmds);
        if bless {
            out.extend(now.iter().map(|l| format!("    {l}")));
        } else {
            let was: Vec<String> = block.iter().map(|l| l.to_string()).collect();
            assert_eq!(was, now, "the example at line {start} of the page differs");
            out.extend(block.iter().map(|l| format!("    {l}")));
        }
    }
    assert!(examples >= 6, "{examples} examples");
    if bless {
        let mut t = out.join("\n");
        t.push('\n');
        std::fs::write(&page, t).expect("write");
    }
}
