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

const READER: &str = r#"
use std.textio.all;
entity rd is end;
architecture a of rd is begin
  process begin
    assert endfile(input) report "standard input is not at its end" severity failure;
    report "stdin is empty";
    wait;
  end process;
end;
"#;

/// With the flag the pipe belongs to the watcher alone (#940): the design, a plugin or an embedded
/// interpreter that looks at standard input finds the null device, instead of blocking on a pipe
/// the parent keeps open (on Windows that hung an embedded Python's start-up).
#[test]
fn the_design_sees_the_null_device_not_the_pipe() {
    let tmp = common::tempdir("stdin_null");
    let src = tmp.join("rd.vhd");
    std::fs::write(&src, READER).expect("write");
    let lib = format!("--work=work:{}", tmp.join("lib").display());
    let st = yavs(&["analyze", "--std=2008", &lib, &src.display().to_string()])
        .status()
        .expect("analyze");
    assert!(st.success());

    let mut child = yavs(&["run", &lib, "--exit-on-stdin-close", "rd"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let _pipe = child.stdin.take(); // open until the run is over
    let t = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().expect("wait") {
            break s;
        }
        if t.elapsed() > Duration::from_secs(30) {
            child.kill().expect("kill");
            panic!("the run blocks on standard input although the parent holds the pipe");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let out = child.wait_with_output().expect("output");
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(status.success(), "{text}");
    assert!(text.contains("stdin is empty"), "{text}");
}
