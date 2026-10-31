//! `yavs run --exit-on-stdin-close`: a run that waits for a debugger ends when the pipe from its
//! parent closes, as it does when the parent is killed without a word (#940).

mod common;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DESIGN: &str = "entity idle is end; architecture a of idle is begin end;\n";

fn yavs(args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yavs"));
    c.args(["--color", "never"]).args(args);
    c
}

#[test]
fn run_waiting_for_a_debugger_ends_with_its_stdin() {
    let tmp = common::tempdir("stdin_close");
    let src = tmp.join("idle.vhd");
    std::fs::write(&src, DESIGN).expect("write");
    let lib = format!("--work=work:{}", tmp.join("lib").display());
    let st = yavs(&["analyze", &lib, &src.display().to_string()])
        .status()
        .expect("analyze");
    assert!(st.success());

    let mut child = yavs(&["run", &lib, "--dap=0", "--exit-on-stdin-close", "idle"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut err = BufReader::new(child.stderr.take().expect("stderr"));
    // It is up and waiting for a debugger.
    let mut line = String::new();
    while !line.contains("waiting for a debugger") {
        line.clear();
        assert!(
            err.read_line(&mut line).expect("read") > 0,
            "yavs ended early"
        );
    }
    assert!(child.try_wait().expect("wait").is_none());
    // The parent's end closes the pipe.
    drop(child.stdin.take());
    let t = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().expect("wait") {
            break s;
        }
        if t.elapsed() > Duration::from_secs(30) {
            child.kill().expect("kill");
            panic!("still waiting for a debugger 30 s after its stdin closed");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(130));
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut err, &mut rest).expect("read");
    assert!(rest.contains("standard input closed"), "{rest}");
}

#[test]
fn not_with_interactive() {
    let o = yavs(&["run", "--interactive", "--exit-on-stdin-close", "idle"])
        .output()
        .expect("run");
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("cannot be used with"));
}
