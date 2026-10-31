//! The waveform file at a debugger stop against what the debugger shows, read back with the
//! console's `examine NAME @ TIME`, which reads the file (#965).

mod common;

use std::process::Command;

use common::tempdir;

const TB: &str = r#"
entity tb is end;
architecture a of tb is
  shared variable sv : integer := 0;
  signal tick : bit;
begin
  tick <= not tick after 5 ns;
  chk : process
    variable n : integer := 0;
  begin
    wait for 12 ns;
    n := 7;
    sv := 8;
    n := 9;                  -- line 14: stopped here, `n` and `sv` are new at 12 ns
    wait for 3 ns;
    n := 10;
    wait;
  end process;
  other : process
    variable m : integer := 0;
  begin
    wait for 12 ns;
    m := 5;
    wait;
  end process;
end;
"#;

fn console(d: &std::path::Path, script: &str) -> (bool, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .current_dir(d)
        .args(["run", "--work=work:build/work", "tb", "--wave=w.fst"])
        .arg(format!("--do={script}"))
        .output()
        .expect("yavs");
    (
        o.status.success(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn variables_at_a_stop_are_in_the_file() {
    let d = tempdir("wave-stop");
    std::fs::write(d.join("tb.vhd"), TB).expect("write");
    let ok = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .current_dir(&*d)
        .args(["analyze", "--std=1993", "--work=work:build/work", "tb.vhd"])
        .status()
        .expect("yavs")
        .success();
    assert!(ok, "analyze failed");

    // Inside the process: its own variables and the shared ones, as `print` shows them.
    let (ok, out, err) = console(
        &d,
        "break tb.vhd:14; run; print n; examine n @ 12 ns; examine tb.sv @ 12 ns; quit",
    );
    assert!(ok, "{out}{err}");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.ends_with(&["7", "7", "8"]), "{out}\n{err}");

    // Between cycles, past the last time step: every process's variables are in the file.
    let (ok, out, err) = console(
        &d,
        "run 13 ns; examine tb.chk.n @ 13 ns; examine tb.other.m @ 13 ns; \
         run 5 ns; examine tb.chk.n @ 18 ns; quit",
    );
    assert!(ok, "{out}{err}");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.ends_with(&["9", "5", "10"]), "{out}\n{err}");
}
