//! `yavs run --threads=auto` (`docs/parallel.md`): the output of one thread whatever it decides,
//! one thread without a trial for a small design, the memory of the last run, and the token
//! server read as for a number above 1.

mod common;

use std::path::Path;
use std::process::{Command, Output, Stdio};

/// 64 registers on one clock (`big`), and a design of two processes (`small`).
const DESIGN: &str = r#"
entity big is generic (cycles : natural := 300; runner_cfg : string := ""); end;
architecture a of big is
  signal clk : bit := '0';
  type arr is array (0 to 63) of natural;
  signal r : arr := (others => 0);
begin
  clk <= not clk after 5 ns when now < cycles * 10 ns;
  g : for i in 0 to 63 generate
    process (clk) begin
      if clk'event and clk = '1' then
        r(i) <= (r(i) * 7 + i + 1) mod 65521;
      end if;
    end process;
  end generate;
  chk : process
    variable s : natural := 0;
  begin
    wait for 10 ns * cycles + 1 ns;
    for i in 0 to 63 loop
      s := (s * 31 + r(i)) mod 1000003;
    end loop;
    report "sum " & integer'image(s);
    wait;
  end process;
end;

entity small is end;
architecture a of small is
  signal n : natural := 0;
begin
  n <= n + 1 after 1 ns when n < 100;
  process (n) begin
    if n = 100 then report "done"; end if;
  end process;
end;
"#;

fn yavs() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yavs"));
    c.args(["--color", "never"]).env_remove("YAVS_TOKENS");
    c
}

fn analyze(dir: &Path) -> String {
    std::fs::write(dir.join("d.vhd"), DESIGN).expect("write");
    let lib = format!("--work=work:{}", dir.join("lib").display());
    let o = yavs()
        .args(["analyze", "--std=2008", &lib])
        .arg(dir.join("d.vhd"))
        .output()
        .expect("analyze");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    lib
}

fn run(lib: &str, args: &[&str], env: &[(&str, &str)]) -> Output {
    let o = yavs()
        .args(["run", lib, "--tier=interp"])
        .args(args)
        .envs(env.iter().copied())
        .env("YAVS_COUNTERS", "1")
        .stdin(Stdio::null())
        .output()
        .expect("run");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    o
}

fn counter(o: &Output, name: &str) -> u64 {
    let err = String::from_utf8_lossy(&o.stderr);
    let key = format!("counters: {name}=");
    err.lines()
        .find_map(|l| l.strip_prefix(&key))
        .unwrap_or_else(|| panic!("no {name} in\n{err}"))
        .parse()
        .expect("number")
}

fn note(o: &Output) -> String {
    let err = String::from_utf8_lossy(&o.stderr);
    err.lines()
        .find(|l| l.starts_with("note: --threads=auto"))
        .unwrap_or_else(|| panic!("no note in\n{err}"))
        .to_string()
}

#[test]
fn auto_gives_the_output_of_one_thread() {
    let t = common::tempdir("threads_auto");
    let lib = analyze(&t);
    let one = run(&lib, &["--threads=1", "big"], &[]);
    assert!(String::from_utf8_lossy(&one.stdout).contains("sum "));
    let auto = run(&lib, &["--threads=auto", "--no-cache", "big"], &[]);
    assert_eq!(auto.stdout, one.stdout);
    // Turned off and on at every check, every batch on the pool while on.
    let flip = [("YAVS_POOL_ALL", "1"), ("YAVS_THREADS_FLIP", "1")];
    let o = run(&lib, &["--threads=auto", "--no-cache", "big"], &flip);
    assert_eq!(o.stdout, one.stdout);
    if std::thread::available_parallelism().is_ok_and(|n| n.get() > 1) {
        assert!(counter(&o, "threads_windows") > 10);
        assert!(counter(&o, "pooled_batches") > 10);
    }
}

#[test]
fn auto_keeps_one_thread_for_a_small_design() {
    let t = common::tempdir("threads_auto_small");
    let lib = analyze(&t);
    let o = run(
        &lib,
        &["--threads=auto", "--verbose", "small"],
        &[("YAVS_POOL_ALL", "1")],
    );
    assert!(String::from_utf8_lossy(&o.stdout).contains("done"));
    assert_eq!(counter(&o, "pooled_batches"), 0);
    if std::thread::available_parallelism().is_ok_and(|n| n.get() > 1) {
        assert!(
            note(&o).contains("one thread: too few processes"),
            "{}",
            note(&o)
        );
    }
}

#[test]
fn auto_remembers_the_last_run() {
    if std::thread::available_parallelism().is_ok_and(|n| n.get() == 1) {
        return;
    }
    let t = common::tempdir("threads_auto_memory");
    let lib = analyze(&t);
    let flip = [("YAVS_POOL_ALL", "1"), ("YAVS_THREADS_FLIP", "1")];
    let args = ["--threads=auto", "--verbose", "big"];
    let first = run(&lib, &args, &flip);
    assert!(!note(&first).contains("last run"), "{}", note(&first));
    // A flipping run never keeps the threads: the next run of the same command line knows.
    let second = run(&lib, &args, &flip);
    assert!(
        note(&second).contains("(the last run did not gain)"),
        "{}",
        note(&second)
    );
    assert_eq!(second.stdout, first.stdout);
    let uncached = run(
        &lib,
        &["--threads=auto", "--verbose", "--no-cache", "big"],
        &flip,
    );
    assert!(!note(&uncached).contains("last run"), "{}", note(&uncached));
}

#[test]
fn auto_remembers_a_test_whatever_its_seed() {
    if std::thread::available_parallelism().is_ok_and(|n| n.get() == 1) {
        return;
    }
    let t = common::tempdir("threads_auto_runner_cfg");
    let lib = analyze(&t);
    let flip = [("YAVS_POOL_ALL", "1"), ("YAVS_THREADS_FLIP", "1")];
    let cfg = |test: &str, seed: &str| {
        format!("-grunner_cfg=enabled_test_cases : {test},output path : out/{seed}/,seed : {seed}")
    };
    let first = run(
        &lib,
        &["--threads=auto", "--verbose", "big", &cfg("a", "1111")],
        &flip,
    );
    assert!(!note(&first).contains("last run"), "{}", note(&first));
    // As a test runner runs it again: a response file, a new seed and output path.
    let rsp = t.join("args.rsp");
    let lines = ["--threads=auto", "--verbose", "big", &cfg("a", "2222")];
    std::fs::write(&rsp, lines.join("\n") + "\n").expect("write");
    let second = run(&lib, &[&format!("@{}", rsp.display())], &flip);
    assert!(
        note(&second).contains("(the last run did not gain)"),
        "{}",
        note(&second)
    );
    let other = run(
        &lib,
        &["--threads=auto", "--verbose", "big", &cfg("b", "1111")],
        &flip,
    );
    assert!(!note(&other).contains("last run"), "{}", note(&other));
}

#[test]
fn auto_reads_the_token_server() {
    let t = common::tempdir("threads_auto_tokens");
    let lib = analyze(&t);
    let server = format!("{}:0123456789abcdef", common::NO_LISTENER);
    let o = run(
        &lib,
        &["--threads=auto", "small"],
        &[("YAVS_TOKENS", &server)],
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("W0118"));
}

#[test]
fn auto_is_ignored_by_a_console_session() {
    let t = common::tempdir("threads_auto_console");
    let lib = analyze(&t);
    let o = yavs()
        .args(["run", &lib, "--threads=auto", "--do=run", "small"])
        .stdin(Stdio::null())
        .output()
        .expect("run");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("W0109"), "{err}");
}

#[test]
fn a_wrong_thread_count_names_auto() {
    let o = yavs()
        .args(["run", "--work=work:x", "--threads=many", "top"])
        .output()
        .expect("run");
    assert_eq!(o.status.code(), Some(2));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("or `auto`"), "{err}");
}
