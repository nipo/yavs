//! `YAVS_CLIFT_LIST` (`yavs-clift` `blockcov.rs`): a coverage run (`write`) stores the blocks that
//! never ran, a later run (`on`) lowers them cheaply; a function whose listed blocks then run too
//! much (`YAVS_CLIFT_AGAIN` stepped instructions per IR instruction) is made again in full and leaves the list, so
//! the run after links its full code. The output is the interpreter's throughout. The coverage
//! run stops before `late` takes its first branch, so the list is wrong for it on purpose; the run
//! goes on long after the trip, which wakes the compile thread.
#![cfg(feature = "clift")]

mod common;
use common::tempdir;

use std::path::Path;
use std::process::{Command, Output};

const SRC: &str = "entity cl is end;
architecture a of cl is
  signal clk : bit := '0';
  signal cnt : integer := 0;
  function late(x : integer) return integer is
    variable s : integer := 0;
  begin
    for i in 0 to 63 loop
      if x > 600 then
        s := s + (i * x) mod 1000;
      elsif x < 0 then
        s := s - 1;
      else
        s := s + i;
      end if;
    end loop;
    return s;
  end function;
begin
  clk <= not clk after 5 ns when now < 100 us else clk;
  process (clk)
    variable acc : integer := 0;
  begin
    if clk = '1' then
      acc := (acc + late(cnt)) mod 1000003;
      cnt <= cnt + 1;
      if cnt = 9999 then
        report \"acc \" & integer'image(acc);
      end if;
    end if;
  end process;
end;
";

fn yavs(dir: &Path, list: &str, args: &[&str]) -> Output {
    yavs_with(dir, list, args, &[])
}

fn yavs_with(dir: &Path, list: &str, args: &[&str], env: &[(&str, &str)]) -> Output {
    let o = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(["--color", "never", "run", "--work=work:work", "--threads=1"])
        .args(args)
        .arg("cl")
        .env("YAVS_COUNTERS", "1")
        .env("YAVS_CLIFT_LIST", list)
        .env("YAVS_CLIFT_AGAIN", "1")
        .env("YAVS_AUTO_COUNT", "1")
        .env("YAVS_AUTO_GATE", "0")
        .env("YAVS_AUTO_SYNC", "1")
        .env_remove("YAVS_AUTO_POLICY")
        .envs(env.iter().copied())
        .current_dir(dir)
        .output()
        .expect("run yavs");
    assert!(o.status.success(), "{o:?}");
    o
}

fn counter(o: &Output, name: &str) -> u64 {
    String::from_utf8_lossy(&o.stderr)
        .lines()
        .find_map(|l| l.strip_prefix(&format!("counters: {name}=")))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("no counter {name}"))
}

#[test]
fn a_wrong_list_is_made_again_in_full_and_leaves_the_list() {
    let d = tempdir("clift_list");
    std::fs::write(d.join("t.vhd"), SRC).expect("write");
    let a = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(["analyze", "--work=work:work", "t.vhd"])
        .current_dir(&d)
        .output()
        .expect("analyze");
    assert!(a.status.success(), "{a:?}");
    let want = yavs(&d, "off", &["--tier=interp"]).stdout;
    assert!(String::from_utf8_lossy(&want).contains("acc "));
    // The coverage run ends before `late` takes its first branch.
    let w = yavs(&d, "write", &["--tier=auto", "--stop-time=4us"]);
    assert_eq!(counter(&w, "clift_listed"), 0);
    let on = yavs(&d, "on", &["--tier=auto"]);
    assert_eq!(on.stdout, want);
    assert!(counter(&on, "clift_listed") > 0, "{on:?}");
    assert!(counter(&on, "clift_again") > 0, "{on:?}");
    // `late` left the list: its full code is linked, nothing trips.
    let next = yavs(&d, "on", &["--tier=auto"]);
    assert_eq!(next.stdout, want);
    assert_eq!(counter(&next, "clift_tripped"), 0, "{next:?}");
    // Every block listed: whatever trips is made again, the output stays.
    let all = yavs(&d, "all", &["--tier=auto", "--threads=4"]);
    assert_eq!(all.stdout, want);
    assert!(counter(&all, "clift_again") > 0, "{all:?}");
    // Native-frame variants that leave at every listed block.
    let leave = yavs_with(
        &d,
        "all",
        &["--tier=auto", "--threads=4"],
        &[("YAVS_CLIFT_LIST_NATIVE", "cheap")],
    );
    assert_eq!(leave.stdout, want);
}
