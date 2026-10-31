//! `yavs run --interactive` and `--do`: the console commands over `--do` and piped standard input,
//! restart, Ctrl-C, and the console hook of a test runner (an in-test runner over TCP).

mod common;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use common::tempdir;
use serde_json::{Value, json};

fn yavs() -> Command {
    Command::new(env!("CARGO_BIN_EXE_yavs"))
}

fn setup(name: &str) -> common::Tmp {
    let d = tempdir(&format!("console-{name}"));
    let cases = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/dap");
    std::fs::copy(cases.join("tb.vhd"), d.join("tb.vhd")).expect("copy");
    analyze(&d, "tb.vhd");
    d
}

fn analyze(d: &Path, file: &str) {
    let ok = yavs()
        .current_dir(d)
        .args(["analyze", "--work=work:lib", file])
        .status()
        .expect("yavs")
        .success();
    assert!(ok, "analyze failed");
}

fn run(d: &Path, top: &str, args: &[&str]) -> Output {
    yavs()
        .current_dir(d)
        .args(["run", "--work=work:lib", top])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("yavs")
}

fn script(d: &Path, top: &str, cmds: &str) -> Output {
    run(d, top, &[&format!("--do={cmds}")])
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// `--do` to the end prints and exits as a plain run.
#[test]
fn a_script_to_the_end_equals_a_batch_run() {
    let d = setup("batch");
    let plain = run(&d, "tb", &[]);
    let console = script(&d, "tb", "run; quit");
    assert_eq!(stdout(&console), stdout(&plain));
    assert_eq!(console.status.code(), plain.status.code());
    // Without `quit`, the end of the script ends the session.
    let console = script(&d, "tb", "run");
    assert_eq!(stdout(&console), stdout(&plain));
    assert_eq!(console.status.code(), Some(0));
}

#[test]
fn run_examine_where() {
    let d = setup("run");
    let o = script(
        &d,
        "tb",
        "examine cnt; where; run 10 ns; examine cnt; examine v(1 downto 0); print v(0); where",
    );
    assert_eq!(
        stdout(&o),
        "0\ntime 0 fs, delta 0\nbetween cycles\n4\n\"10\"\n'0'\ntime 10 ns, delta 2\nbetween cycles\n"
    );
    assert!(stderr(&o).contains("at 10 ns, delta 2"), "{}", stderr(&o));
    assert_eq!(o.status.code(), Some(0));
}

#[test]
fn run_until_a_condition() {
    let d = setup("until");
    let o = script(&d, "tb", "run until cnt = 4; where; examine cnt");
    assert_eq!(stdout(&o), "time 10 ns, delta 1\nbetween cycles\n4\n");
    let o = script(&d, "tb", "run until N = 3");
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("names no signal"), "{}", stderr(&o));
}

#[test]
fn step_and_next() {
    let d = setup("step");
    let o = script(
        &d,
        "tb",
        "break tb.vhd:20; run; where; step; where; next; where; step delta; step timestep; where",
    );
    let out = stdout(&o);
    let at: Vec<&str> = out
        .lines()
        .filter_map(|l| {
            l.rsplit(['/', '\\'])
                .next()
                .filter(|t| t.starts_with("tb.vhd:"))
        })
        .collect();
    // The set line and the stop, into the function, then its next line; nothing after the steps.
    assert_eq!(
        at,
        ["tb.vhd:20", "tb.vhd:20", "tb.vhd:9", "tb.vhd:10"],
        "{out}"
    );
    assert!(out.ends_with("between cycles\n"), "{out}");
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
}

#[test]
fn force_deposit_release() {
    let d = setup("force");
    let o = script(
        &d,
        "tb",
        "force cnt 5; run 1 ns; examine cnt; release cnt; deposit v \"1111\"; run 1 ns; examine v",
    );
    assert_eq!(
        stdout(&o),
        // The deposit, then `q` inverts it when the release changes `cnt`.
        ":tb:cnt = 5\n5\n:tb:cnt\n:tb:v = \"1111\"\n\"0000\"\n"
    );
}

#[test]
fn breakpoints_on_lines_and_signals() {
    let d = setup("break");
    let o = script(
        &d,
        "tb",
        "break tb.vhd:24; break cnt = 4; break list; run; where; break delete 2; run; where; break delete all; break list",
    );
    let out = stdout(&o);
    assert!(out.contains("breakpoint 1: "), "{out}");
    assert!(out.contains("breakpoint 2: :tb:cnt"), "{out}");
    let times: Vec<&str> = out.lines().filter(|l| l.starts_with("time ")).collect();
    assert_eq!(
        times,
        ["time 10 ns, delta 1", "time 30 ns, delta 0"],
        "{out}"
    );
    assert!(out.ends_with("no breakpoints\n"), "{out}");
    // A breakpoint with a condition; a file that is not in the design; a bad number.
    let o = script(&d, "tb", "break tb.vhd:22 if k = 2; run; examine k");
    assert!(stdout(&o).ends_with("2\n"), "{}", stdout(&o));
    let o = script(&d, "tb", "break nope.vhd:1");
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("`nope.vhd` is not a source file"),
        "{}",
        stderr(&o)
    );
    let o = script(&d, "tb", "break delete 9");
    assert!(stderr(&o).contains("no breakpoint 9"), "{}", stderr(&o));
}

/// Back at time 0 the design runs again with the same output; breakpoints stay.
#[test]
fn restart_keeps_breakpoints_and_repeats_the_run() {
    let d = setup("restart");
    let plain = stdout(&run(&d, "tb", &[]));
    let o = script(
        &d,
        "tb",
        "run; restart; where; examine cnt; run; reset; run",
    );
    let want = format!("{plain}time 0 fs, delta 0\nbetween cycles\n0\n{plain}{plain}");
    assert_eq!(stdout(&o), want);
    // From a stop inside a process: what the process would still do is not seen.
    let o = script(
        &d,
        "tb",
        "break tb.vhd:24; run; restart; run; where; restart; break list",
    );
    let out = stdout(&o);
    assert_eq!(
        out.matches("tb.vhd:24").count(),
        3,
        "set, where and list: {out}"
    );
    assert!(!out.contains("done"), "{out}");
}

/// A process that never suspends, stopped at a breakpoint and restarted.
#[test]
fn restart_out_of_a_process_that_never_suspends() {
    let d = setup("spinloop");
    std::fs::write(
        d.join("l.vhd"),
        "entity l is end;\narchitecture a of l is\nbegin\n  process\n    variable i : natural := 0;\n  begin\n    loop\n      i := (i + 1) mod 7;\n    end loop;\n  end process;\nend;\n",
    )
    .expect("write");
    analyze(&d, "l.vhd");
    let o = script(&d, "l", "break l.vhd:8; run; restart; run; examine i; quit");
    assert!(stdout(&o).ends_with("\n0\n"), "{}", stdout(&o));
    assert_eq!(o.status.code(), Some(0));
}

/// `Output` of `cmd`, killed and failed when it does not end: a hang must fail the test, not CI.
fn bounded(mut cmd: Command, input: &str) -> Output {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("yavs");
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(input.as_bytes())
        .expect("write");
    let t0 = std::time::Instant::now();
    while child.try_wait().expect("wait").is_none() {
        if t0.elapsed() > std::time::Duration::from_secs(120) {
            let _ = child.kill();
            panic!("still running: {:?}", child.wait_with_output());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    child.wait_with_output().expect("output")
}

/// A stop in a loop inside a subprogram, and in a loop with no statement at all: the session
/// can be left and restarted (#918).
#[test]
fn leaving_an_endless_loop_in_a_subprogram() {
    let d = setup("spinsub");
    std::fs::write(
        d.join("l.vhd"),
        "entity l is end;\narchitecture a of l is\n  procedure spin is\n    variable i : natural := 0;\n  begin\n    loop\n      i := (i + 1) mod 7;\n    end loop;\n  end procedure;\nbegin\n  process begin\n    spin;\n    wait;\n  end process;\nend;\n",
    )
    .expect("write");
    std::fs::write(
        d.join("e.vhd"),
        "entity e is end;\narchitecture a of e is\n  procedure spin is\n  begin\n    loop\n    end loop;\n  end procedure;\nbegin\n  process begin\n    report \"before\";\n    spin;\n    wait;\n  end process;\nend;\n",
    )
    .expect("write");
    analyze(&d, "l.vhd");
    analyze(&d, "e.vhd");
    let session = |top: &str, args: &[&str], input: &str| {
        let mut c = yavs();
        c.current_dir(&*d).args(["run", "--work=work:lib", top]);
        c.args(args);
        bounded(c, input)
    };
    let stop = "--do=break l.vhd:7; run";
    for (input, status) in [("quit\n", 0), ("quit 3\n", 3), ("", 0)] {
        let o = session("l", &[stop, "--interactive"], input);
        assert_eq!(o.status.code(), Some(status), "{input:?}: {}", stderr(&o));
    }
    let o = session(
        "l",
        &[stop, "--interactive"],
        "restart\nwhere\nrun\nexamine i\nquit\n",
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("time 0 fs, delta 0"), "{}", stdout(&o));
    // No statement in the loop: nothing to stop at, but the session still ends.
    let mut c = yavs();
    c.current_dir(&*d)
        .args(["run", "--work=work:lib", "e", "--dap=stdio"]);
    let mut child = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("yavs");
    let mut w = child.stdin.take().expect("piped");
    let mut r = BufReader::new(child.stdout.take().expect("piped"));
    let send = |w: &mut std::process::ChildStdin, seq: i64, cmd: &str| {
        let m = json!({"type": "request", "seq": seq, "command": cmd, "arguments": {}}).to_string();
        write!(w, "Content-Length: {}\r\n\r\n{m}", m.len()).expect("write");
    };
    let read = |r: &mut BufReader<std::process::ChildStdout>| -> Value {
        let mut n = 0;
        loop {
            let mut l = String::new();
            assert!(r.read_line(&mut l).expect("read") > 0, "the session ended");
            if let Some(v) = l.trim().strip_prefix("Content-Length: ") {
                n = v.parse().expect("a length");
            } else if l.trim().is_empty() {
                break;
            }
        }
        let mut b = vec![0; n];
        std::io::Read::read_exact(r, &mut b).expect("read");
        serde_json::from_slice(&b).expect("json")
    };
    for (seq, cmd) in [(1, "initialize"), (2, "launch"), (3, "configurationDone")] {
        send(&mut w, seq, cmd);
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    send(&mut w, 4, "disconnect");
    while read(&mut r)["command"] != "disconnect" {}
    let t0 = std::time::Instant::now();
    while child.try_wait().expect("wait").is_none() {
        assert!(
            t0.elapsed() < std::time::Duration::from_secs(60),
            "still running"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

const PARTS: &str = "library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity t is end;
architecture a of t is
  type rec is record x : integer; y : integer; end record;
  type vec is array (0 to 3) of integer;
  type ptr_t is access rec;
  type node_t;
  type node_ptr is access node_t;
  type node_t is record v : integer; next_n : node_ptr; end record;
  type counter_t is protected
    procedure incr;
  end protected;
  type counter_t is protected body
    variable n : integer := 0;
    variable text : string(1 to 2) := \"ab\";
    procedure incr is
    begin
      n := n + 1;
    end procedure;
  end protected body;
  shared variable sc : counter_t;
  signal cnt : unsigned(7 downto 0) := to_unsigned(3, 8);
  signal sg : signed(3 downto 0) := to_signed(-2, 4);
  signal slv : std_logic_vector(3 downto 0) := \"0101\";
  signal pt : rec := (5, 6);
begin
  process
    variable p : rec := (1, 2);
    variable v : vec := (1, 2, 3, 4);
    variable s : string(1 to 5) := \"hello\";
    variable q : ptr_t := new rec'(x => 7, y => 8);
    variable nul : ptr_t;
    variable lst : node_ptr := new node_t'(1, new node_t'(2, null));
  begin
    sc.incr;
    wait for 1 ns;
    report \"stop here\";
    wait;
  end process;
end;
";

fn parts(name: &str) -> common::Tmp {
    let d = setup(name);
    std::fs::write(d.join("t.vhd"), PARTS).expect("write");
    let ok = yavs()
        .current_dir(&*d)
        .args(["analyze", "--std=2008", "--work=work:lib", "t.vhd"])
        .status()
        .expect("yavs")
        .success();
    assert!(ok, "analyze failed");
    d
}

/// Every line of `exprs` examined at the `report` statement, one answer per line (#930, #936).
#[track_caller]
fn check_examine(d: &Path, cases: &[(&str, &str)]) {
    let cmds: Vec<String> = cases.iter().map(|(e, _)| format!("examine {e}")).collect();
    let o = script(d, "t", &format!("break t.vhd:39; run; {}", cmds.join("; ")));
    let got = stdout(&o);
    let lines: Vec<&str> = got.lines().skip(1).collect();
    let want: Vec<&str> = cases.iter().map(|(_, w)| *w).collect();
    assert_eq!(lines, want, "{}", stderr(&o));
}

#[test]
fn examine_parts_of_variables_and_access_values() {
    let d = parts("parts");
    check_examine(
        &d,
        &[
            ("p", "(x => 1, y => 2)"),
            ("p.x", "1"),
            ("v(2)", "3"),
            ("s(2 to 3)", "\"el\""),
            ("q", "(x => 7, y => 8)"),
            ("q.all", "(x => 7, y => 8)"),
            ("q.all.x", "7"),
            ("q.y", "8"),
            ("nul", "null"),
            ("lst", "(v => 1, next_n => access)"),
            ("lst.next_n.v", "2"),
            ("lst.next_n.next_n", "null"),
            ("cnt(1 downto 0)", "\"11\""),
            ("pt.y", "6"),
        ],
    );
}

#[test]
fn examine_computes_as_a_condition_does() {
    let d = parts("computes");
    check_examine(
        &d,
        &[
            ("cnt = 3", "TRUE"),
            ("3 = cnt", "TRUE"),
            ("cnt > 3", "FALSE"),
            ("sg = -2", "TRUE"),
            ("sg < 0", "TRUE"),
            ("to_integer(cnt) + 1", "4"),
            ("to_integer(sg)", "-2"),
            ("v(1) + 1", "3"),
            ("nul = null", "TRUE"),
            ("q /= null", "TRUE"),
        ],
    );
}

#[test]
fn examine_errors_say_what_is_wrong() {
    let d = parts("experr");
    for (e, want) in [
        ("nosuch", "no variable or signal `nosuch` here"),
        ("p.nosuch", "no field `nosuch`"),
        ("v(9)", "index 9 is outside the array"),
        ("nul.x", "the access value is null"),
        ("p.all", "not an access value"),
        ("slv = 3", "`std_ulogic_vector`, which has no numbers"),
        ("cnt + 1", "to_integer(name)"),
    ] {
        let o = script(&d, "t", &format!("break t.vhd:39; run; examine {e}"));
        assert!(stderr(&o).contains(want), "{e}: {}", stderr(&o));
        assert_eq!(o.status.code(), Some(1), "{e}");
    }
}

/// The variables of a protected body, in the frame of its method (#936).
#[test]
fn variables_of_a_protected_body() {
    let d = parts("prot");
    let o = script(
        &d,
        "t",
        "break t.vhd:20; run; examine n; examine text; examine text(1); examine n + 1",
    );
    assert_eq!(
        stdout(&o).lines().skip(1).collect::<Vec<_>>(),
        ["0", "\"ab\"", "'a'", "1"],
        "{}",
        stderr(&o)
    );
}

/// Every name that misses says what is close (rule: one helper for all of them).
#[test]
fn every_name_suggests() {
    let d = parts("suggest");
    for (cmds, want) in [
        ("step dlta", "`delta`"),
        ("wave adn", "needs a viewer"),
        ("break delete al", "`all`"),
        ("break t.vh:1", "similar name exists: `t.vhd`"),
        ("run 1 ns; examine cn", "similar name exists: `cnt`"),
        ("run 1 ns; print cn", "similar name exists: `cnt`"),
        ("force slb '1'", "similar name exists: `slv`"),
        ("deposit cn 1", "`cnt`"),
        ("release cn", "`cnt`"),
        ("break cn", "`cnt`"),
        ("run 1 ns; examine pt.xx", "`x`"),
        ("run 1 ns; examine cn = 3", "`cnt`"),
    ] {
        let o = script(&d, "t", cmds);
        assert!(stderr(&o).contains(want), "{cmds}: {}", stderr(&o));
    }
}

#[test]
fn help_is_wrapped() {
    let d = setup("helpwrap");
    let o = script(&d, "tb", "help");
    let out = stdout(&o);
    assert!(out.contains("examine NAME"), "{out}");
    assert!(out.lines().all(|l| l.len() <= 80), "{out}");
}

/// A typo in a command names the command; a condition on a number works and its error comes when
/// it is set (#935).
#[test]
fn typos_and_conditions_are_helped() {
    let d = parts("typos");
    let o = script(&d, "t", "hepl");
    assert!(
        stderr(&o).contains("similar name exists: `help`"),
        "{}",
        stderr(&o)
    );
    let o = script(&d, "t", "break t.vhd:39 if slv = 3");
    assert!(
        stderr(&o).contains("which has no numbers"),
        "{}",
        stderr(&o)
    );
    assert_eq!(o.status.code(), Some(1));
    let o = script(&d, "t", "break t.vhd:39 if cnt = 4; run; where");
    assert!(stderr(&o).contains("ended"), "{}", stderr(&o));
    let o = script(&d, "t", "break t.vhd:39 if cnt = 3; run; where");
    assert!(stdout(&o).contains("t.vhd:39"), "{}", stdout(&o));
    let o = script(&d, "t", "break t.vhd:39 if to_integer(cnt) = 3; run; where");
    assert!(stdout(&o).contains("t.vhd:39"), "{}", stdout(&o));
}

#[test]
fn restart_recreates_the_wave_file() {
    let d = setup("wave");
    let o = run(
        &d,
        "tb",
        &["--wave=w.fst", "--do=run; restart; run 15 ns; quit"],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let f = wellen::simple::read(d.join("w.fst")).expect("readable");
    let t = f.time_table().last().copied().unwrap_or(0);
    // A whole run goes on to 30 ns: the file is the second run's.
    assert!(t < 20_000_000, "{t}");
}

/// `wave` without `--surfer` is an error that says what to do (found testing the console by hand).
#[test]
fn wave_commands_need_a_viewer() {
    let d = setup("wave");
    for cmd in ["wave add cnt", "wave cursor"] {
        let o = script(&d, "tb", cmd);
        assert_eq!(o.status.code(), Some(1), "{cmd}");
        assert!(
            stderr(&o).contains("`wave` needs a viewer: run with `--surfer`"),
            "{cmd}: {}",
            stderr(&o)
        );
    }
}

#[test]
fn source_files() {
    let d = setup("source");
    std::fs::write(
        d.join("s.do"),
        "# a comment\nrun 10 ns -- to here\nexamine cnt; examine v\n\nbogus\nexamine cnt\n",
    )
    .expect("write");
    let o = script(&d, "tb", "source s.do; examine cnt");
    assert_eq!(stdout(&o), "4\n\"1010\"\n", "the failure ends the script");
    assert!(
        stderr(&o).contains("unknown command or signal `bogus`"),
        "{}",
        stderr(&o)
    );
    assert_eq!(o.status.code(), Some(1));
    let o = script(&d, "tb", "source nope.do");
    assert!(
        stderr(&o).contains("cannot read `nope.do`"),
        "{}",
        stderr(&o)
    );
}

#[test]
fn quit_statuses() {
    let d = setup("quit");
    assert_eq!(script(&d, "tb", "quit 7").status.code(), Some(7));
    assert_eq!(script(&d, "tb", "run 1 ns; quit").status.code(), Some(0));
    assert_eq!(script(&d, "tb", "quit x").status.code(), Some(1));
    std::fs::write(
        d.join("f.vhd"),
        "entity f is end;\narchitecture a of f is\nbegin\n  process begin\n    report \"bad\" severity failure;\n    wait;\n  end process;\nend;\n",
    )
    .expect("write");
    analyze(&d, "f.vhd");
    let o = script(&d, "f", "run; quit");
    assert_eq!(o.status.code(), run(&d, "f", &[]).status.code());
    assert_eq!(o.status.code(), Some(1));
    // The end stays serviceable until `quit`.
    assert_eq!(script(&d, "f", "run; step").status.code(), Some(1));
    assert_eq!(
        script(&d, "f", "run; run; where; quit 0").status.code(),
        Some(0)
    );
}

#[test]
fn piped_standard_input() {
    let d = setup("piped");
    let mut c = yavs()
        .current_dir(&d)
        .args(["run", "--work=work:lib", "tb", "--interactive"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("yavs");
    c.stdin
        .take()
        .expect("stdin")
        .write_all(b"run 10 ns\nbogus\nexamine cnt\n\nhelp\n")
        .expect("write");
    let o = c.wait_with_output().expect("wait");
    let out = stdout(&o);
    // No prompt on a pipe; a failure does not end a session with a user; end of input is `quit`.
    assert!(out.starts_with("4\n"), "{out}");
    assert!(!out.contains("yavs>"), "{out}");
    assert!(out.contains("restart: back to time 0"), "{out}");
    assert!(stderr(&o).contains("unknown command or signal `bogus`"));
    assert_eq!(o.status.code(), Some(0));
}

#[test]
fn flags_conflict_and_debug_rules() {
    let d = setup("flags");
    let o = run(&d, "tb", &["--interactive", "--dap=stdio"]);
    assert_eq!(o.status.code(), Some(2));
    let o = run(&d, "tb", &["--do=run", "--dap=stdio"]);
    assert_eq!(o.status.code(), Some(2));
    let o = run(&d, "tb", &["--do=run", "--threads=4"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stderr(&o).contains("W0109"), "{}", stderr(&o));
    assert!(stderr(&o).contains("--interactive"), "{}", stderr(&o));
    let o = run(&d, "tb", &["--do=run", "--profile"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("E0125"), "{}", stderr(&o));
}

/// A subprogram with unconstrained parameters under the debugger (#863).
#[test]
fn string_parameters_under_the_debugger() {
    let d = setup("lean");
    std::fs::write(
        d.join("m.vhd"),
        r#"package p is
  impure function has(d, key : string) return boolean;
  impure function a(cfg : string) return boolean;
end package;
package body p is
  impure function has(d, key : string) return boolean is
  begin
    return d'length > key'length;
  end;
  impure function a(cfg : string) return boolean is
  begin
    if has(cfg, "ab") then
      return cfg = "abc";
    else
      return false;
    end if;
  end;
end package body;
use work.p.all;
entity m is end;
architecture x of m is
begin
  process begin
    report boolean'image(a("abc"));
    wait;
  end process;
end;
"#,
    )
    .expect("write");
    analyze(&d, "m.vhd");
    let o = script(&d, "m", "run");
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("true"), "{}", stdout(&o));
}

#[cfg(unix)]
#[test]
fn ctrl_c_pauses_and_the_console_goes_on() {
    use std::time::{Duration, Instant};
    let d = setup("ctrlc");
    let mut c = yavs()
        .current_dir(&d)
        .args(["run", "--work=work:lib", "spin", "--interactive"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("yavs");
    let mut input = c.stdin.take().expect("stdin");
    input.write_all(b"run\n").expect("write");
    input.flush().expect("flush");
    let mut err = BufReader::new(c.stderr.take().expect("stderr"));
    // Twice: a first SIGINT may arrive before the command, a pause must come of every other one.
    let t0 = Instant::now();
    let pid = c.id().to_string();
    let killer = std::thread::spawn(move || {
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(700));
            let _ = Command::new("kill").args(["-INT", &pid]).status();
        }
    });
    let mut line = String::new();
    err.read_line(&mut line).expect("read");
    assert!(line.starts_with("pause at "), "{line}");
    assert!(t0.elapsed() < Duration::from_secs(60));
    input.write_all(b"where\nquit 5\n").expect("write");
    drop(input);
    killer.join().expect("killer");
    let o = c.wait_with_output().expect("wait");
    assert!(stdout(&o).contains("time "), "{}", stdout(&o));
    assert_eq!(o.status.code(), Some(5));
}

/// What the in-test runner does when it gets a call.
struct Runner {
    r: BufReader<TcpStream>,
    w: TcpStream,
}

impl Runner {
    fn send(&mut self, m: Value) {
        writeln!(self.w, "{m}").expect("send");
    }

    fn recv(&mut self) -> Option<Value> {
        let mut l = String::new();
        match self.r.read_line(&mut l) {
            Ok(n) if n > 0 => serde_json::from_str(&l).ok(),
            _ => None,
        }
    }

    /// `exec` or `reload`: the answer.
    fn ask(&mut self, m: Value) -> Value {
        self.send(m);
        self.recv().expect("an answer")
    }
}

/// Serve the hook: `commands` to register; `on_call` handles each call and gives its `done`.
fn hook_server(
    secret: &'static str,
    commands: Value,
    mut on_call: impl FnMut(&mut Runner, &str, &str) -> Value + Send + 'static,
) -> (String, std::thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let value = format!("{}:{secret}", l.local_addr().expect("addr").port());
    let t = std::thread::spawn(move || {
        let (s, _) = l.accept().expect("accept");
        let mut c = Runner {
            r: BufReader::new(s.try_clone().expect("clone")),
            w: s,
        };
        let hello = c.recv().expect("hello");
        assert_eq!(hello["secret"], secret);
        c.send(json!({"commands": commands}));
        while let Some(m) = c.recv() {
            let name = m["name"].as_str().unwrap_or_default().to_string();
            let args = m["args"].as_str().unwrap_or_default().to_string();
            let mut done = on_call(&mut c, &name, &args);
            done["done"] = m["call"].clone();
            c.send(done);
        }
    });
    (value, t)
}

fn with_hook(d: &Path, top: &str, value: &str, args: &[&str]) -> Output {
    yavs()
        .current_dir(d)
        .env(yavs_hook_var(), value)
        .args(["run", "--work=work:lib", top])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("yavs")
}

fn yavs_hook_var() -> &'static str {
    "YAVS_CONSOLE_HOOK"
}

#[test]
fn hook_commands_run_in_the_runner() {
    let d = setup("hook");
    let (value, t) = hook_server(
        "s3cret",
        json!([
            {"name": "greet", "help": "say hello"},
            {"name": "hidden", "help": ""},
            {"name": "boom", "help": "fail"},
            {"name": "drive", "help": "run a bit"},
        ]),
        |c, name, args| match name {
            "greet" => {
                c.send(json!({"print": format!("hello {args}\n")}));
                json!({"result": "false"})
            }
            "boom" => json!({"result": "", "error": "it broke"}),
            "drive" => {
                let a = c.ask(json!({"exec": 1, "line": "run 10 ns"}));
                assert_eq!(a["ok"], true, "{a}");
                let a = c.ask(json!({"exec": 2, "line": "examine cnt"}));
                assert_eq!(a["text"], "4", "{a}");
                let a = c.ask(json!({"exec": 3, "line": "bogus"}));
                assert_eq!(a["ok"], false, "{a}");
                assert!(a["error"].as_str().unwrap_or("").contains("bogus"), "{a}");
                // To the end: the answer comes at the end, and the session is there.
                let a = c.ask(json!({"exec": 4, "line": "run"}));
                assert_eq!(a["ok"], true, "{a}");
                let a = c.ask(json!({"exec": 5, "line": "restart; examine cnt"}));
                assert_eq!(a["text"], "0", "{a}");
                json!({"result": "true"})
            }
            _ => json!({"result": ""}),
        },
    );
    let o = with_hook(
        &d,
        "tb",
        &value,
        &["--do=help; greet you there; where; drive; hidden; where; boom; where"],
    );
    let out = stdout(&o);
    assert!(out.contains("greet: say hello\n"), "{out}");
    assert!(!out.contains("hidden:"), "{out}");
    assert!(out.contains("hello you there\nfalse\n"), "{out}");
    // In the order of the commands, whatever the timing (#947).
    assert!(
        out.find("greet: say hello").unwrap_or(usize::MAX)
            < out.find("hello you there").unwrap_or(0),
        "{out}"
    );
    assert!(out.contains("** Note: 30000000fs+0: done"), "{out}");
    assert!(out.contains("\ntrue\n"), "{out}");
    assert!(!out.ends_with("between cycles\nbetween cycles\n"), "{out}");
    assert!(stderr(&o).contains("error: it broke"), "{}", stderr(&o));
    assert_eq!(o.status.code(), Some(1));
    t.join().expect("the runner");
}

#[test]
fn hook_reload_sees_new_code_and_keeps_breakpoints() {
    let d = setup("reload");
    std::fs::write(
        d.join("r.vhd"),
        "entity r is\n  generic (g : integer := 1);\nend;\narchitecture a of r is\nbegin\n  process begin\n    report \"one\" & integer'image(g);\n    wait;\n  end process;\nend;\n",
    )
    .expect("write");
    analyze(&d, "r.vhd");
    let dir = d.to_path_buf();
    let (value, t) = hook_server(
        "x",
        json!([{"name": "again", "help": "reload"}]),
        move |c, _, args| {
            match args {
                "edit" => {
                    let new = "entity r is\n  generic (g : integer := 1);\nend;\narchitecture a of r is\nbegin\n  process begin\n    report \"two\" & integer'image(g);\n    wait;\n  end process;\nend;\n";
                    std::fs::write(dir.join("r.vhd"), new).expect("write");
                    analyze(&dir, "r.vhd");
                    let a = c.ask(json!({"reload": 1, "args": []}));
                    assert_eq!(a["ok"], true, "{a}");
                }
                "flags" => {
                    let a = c.ask(json!({"reload": 2, "args": ["-gg=5"]}));
                    assert_eq!(a["ok"], true, "{a}");
                }
                _ => {
                    // A design that does not elaborate: the old one stays loaded.
                    let bad = c.ask(json!({"reload": 3, "args": ["--no-such-flag"]}));
                    assert_eq!(bad["ok"], false, "{bad}");
                    let w = c.ask(json!({"exec": 4, "line": "where"}));
                    assert!(w["text"].as_str().unwrap_or("").contains("delta"), "{w}");
                }
            }
            json!({"result": "false"})
        },
    );
    let o = with_hook(
        &d,
        "r",
        &value,
        &["--do=break r.vhd:7; again bad; again edit; where; run; run; again flags; run; run"],
    );
    let out = stdout(&o);
    assert_eq!(out.matches("two1").count(), 1, "{out}\n{}", stderr(&o));
    assert_eq!(out.matches("two5").count(), 1, "{out}");
    assert!(!out.contains("one"), "{out}");
    let err = stderr(&o);
    assert_eq!(err.matches("breakpoint at").count(), 2, "{err}");
    assert_eq!(o.status.code(), Some(0), "{err}");
    t.join().expect("the runner");
}

#[test]
fn hook_errors_are_e0703_at_the_start() {
    let d = setup("hookerr");
    let bad = |value: &str| with_hook(&d, "tb", value, &["--do=quit"]);
    for v in ["nonsense", "0:x", "1:"] {
        let o = bad(v);
        assert_eq!(o.status.code(), Some(2), "{v}");
        assert!(stderr(&o).contains("E0703"), "{}", stderr(&o));
    }
    // Nobody listens on this port any more.
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    let o = bad(&format!("{port}:x"));
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("E0703"), "{}", stderr(&o));
    assert!(!stderr(&o).contains(":x"), "the secret is not printed");
    // A runner that would take the name of a yavs command.
    let (v, t) = hook_server("x", json!([{"name": "run", "help": ""}]), |_, _, _| {
        json!({})
    });
    let o = with_hook(&d, "tb", &v, &["--do=quit"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("a yavs command"), "{}", stderr(&o));
    t.join().expect("the runner");
}

#[test]
fn hook_going_away_fails_what_waits() {
    let d = setup("hookgone");
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let value = format!("{}:x", l.local_addr().expect("addr").port());
    let t = std::thread::spawn(move || {
        let (s, _) = l.accept().expect("accept");
        let mut c = Runner {
            r: BufReader::new(s.try_clone().expect("clone")),
            w: s,
        };
        c.recv();
        c.send(json!({"commands": [{"name": "die", "help": "d"}]}));
        c.recv();
    });
    let o = with_hook(&d, "tb", &value, &["--do=die"]);
    t.join().expect("the runner");
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("closed its connection"),
        "{}",
        stderr(&o)
    );
}
