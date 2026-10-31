//! `yavs run --load`: VHPIDIRECT foreign subprograms calling a C library compiled at test
//! time with the compiler `cc` picks for the build target (#95).

mod common;
use common::tempdir;

use std::path::Path;
use std::process::Command;

const C: &str = r#"
#include <stdint.h>
#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif
EXPORT int32_t add1(int32_t x) { return x + 1; }
EXPORT int8_t twice(int8_t x) { return (int8_t)(x * 2); }
EXPORT double half(double x) { return x / 2; }
EXPORT uint8_t is_odd(int32_t x) { return (uint8_t)(x & 1); }
EXPORT void set42(int32_t *x) { *x = 42; }
EXPORT int64_t sum(const int32_t *p, int64_t n) {
    int64_t s = 0;
    for (int64_t i = 0; i < n; i++) s += p[i];
    return s;
}
/* std_logic codes: '0' is 2, '1' is 3. */
EXPORT void invert(uint8_t *p, int64_t n) {
    for (int64_t i = 0; i < n; i++) p[i] = p[i] == 3 ? 2 : 3;
}
EXPORT uint8_t bad_bool(void) { return 7; }
"#;

const VHDL: &str = r#"
library ieee; use ieee.std_logic_1164.all;
package c is
  function add1(x : integer) return integer;
  attribute foreign of add1 : function is "VHPIDIRECT add1";
  function twice(x : integer range 0 to 50) return integer;
  attribute foreign of twice : function is "VHPIDIRECT libc_test twice";
  function half(x : real) return real;
  attribute foreign of half : function is "VHPIDIRECT half";
  function is_odd(x : integer) return boolean;
  attribute foreign of is_odd : function is "VHPIDIRECT is_odd";
  procedure set42(x : out integer);
  attribute foreign of set42 : procedure is "VHPIDIRECT set42";
  function sum(v : integer_vector) return integer;
  attribute foreign of sum : function is "VHPIDIRECT sum";
  procedure invert(v : inout std_logic_vector);
  attribute foreign of invert : procedure is "VHPIDIRECT invert";
  function bad_bool return boolean;
  attribute foreign of bad_bool : function is "VHPIDIRECT bad_bool";
  function missing return integer;
  attribute foreign of missing : function is "VHPIDIRECT no_such_symbol";
end;
package body c is
  -- Placeholder bodies, as other simulators require; the foreign attribute wins.
  function add1(x : integer) return integer is begin return 0; end;
  function twice(x : integer range 0 to 50) return integer is begin return 0; end;
  function half(x : real) return real is begin return 0.0; end;
  function is_odd(x : integer) return boolean is begin return false; end;
  procedure set42(x : out integer) is begin x := 0; end;
  function sum(v : integer_vector) return integer is begin return 0; end;
  procedure invert(v : inout std_logic_vector) is begin end;
end;

library ieee; use ieee.std_logic_1164.all; use work.c.all;
entity tb is end;
architecture a of tb is begin
  process
    variable x : integer;
    variable v : std_logic_vector(3 downto 0) := "1100";
  begin
    assert add1(41) = 42;
    assert twice(21) = 42;
    assert half(5.0) = 2.5;
    assert is_odd(3) and not is_odd(4);
    set42(x);
    assert x = 42;
    assert sum((1, 2, 3, 4)) = 10;
    invert(v);
    assert v = "0011";
    report "foreign ok";
    wait;
  end process;
end;

use work.c.all;
entity bad is end;
architecture a of bad is begin
  process begin report boolean'image(bad_bool); wait; end process;
end;

use work.c.all;
entity lazy is end;
architecture a of lazy is begin
  process
    variable n : integer := 0;
  begin
    if n > 0 then report integer'image(missing); end if;
    report "lazy ok";
    wait;
  end process;
end;

use work.c.all;
entity miss is end;
architecture a of miss is begin
  process begin report integer'image(missing); wait; end process;
end;
"#;

/// Builds `src` into a shared library at `out`.
fn shared_lib(src: &Path, out: &Path) {
    let tool = cc::Build::new()
        .target(env!("YAVS_TARGET"))
        .host(env!("YAVS_HOST"))
        .opt_level(0)
        .debug(false)
        .cargo_metadata(false)
        .cargo_warnings(false)
        .get_compiler();
    let mut cmd = tool.to_command();
    cmd.current_dir(out.parent().expect("dir"));
    if tool.is_like_msvc() {
        cmd.args(["/nologo", "/W3", "/LD"])
            .arg(src)
            .arg(format!("/Fe{}", out.display()));
    } else {
        cmd.args(["-Wall", "-Werror", "-shared"])
            .arg(src)
            .arg("-o")
            .arg(out);
        if !cfg!(windows) {
            cmd.arg("-fPIC");
        }
    }
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("cannot run {cmd:?}: {e}"));
    assert!(status.success(), "{cmd:?} failed: {status}");
}

fn yavs(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(["--color", "never"])
        .args(args)
        .output()
        .expect("run");
    (
        o.status.code().expect("exit code"),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
    )
}

#[test]
fn load_and_call_c() {
    let tmp = tempdir("foreign");
    let c = tmp.join("c_test.c");
    std::fs::write(&c, C).expect("write");
    let so = tmp.join(if cfg!(windows) {
        "c_test.dll"
    } else {
        "libc_test.so"
    });
    shared_lib(&c, &so);
    let src = tmp.join("tb.vhd");
    std::fs::write(&src, VHDL).expect("write");
    let lib = format!("--work=work:{}", tmp.join("lib").display());
    let load = format!("--load={}", so.display());
    let (code, err, _) = yavs(&["analyze", &lib, &src.display().to_string()]);
    assert_eq!(code, 0, "{err}");

    let (code, err, out) = yavs(&["run", &lib, &load, "tb"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("foreign ok"), "{out}");

    // The state of a loaded library cannot be reset: the console says so (#904).
    let (code, err, _) = yavs(&["run", &lib, &load, "tb", "--do=restart"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("restart cannot reset the state"), "{err}");

    // A result outside its VHDL type is a runtime error, not a wrong value.
    let (code, err, _) = yavs(&["run", &lib, &load, "bad"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("error[E3"), "{err}");

    // Warn early, fail at the call: an unbound symbol on a branch never taken is a warning
    // naming what was searched, and the run completes.
    let (code, err, out) = yavs(&["run", &lib, &load, "lazy"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("lazy ok"), "{out}");
    assert!(
        err.contains("warning[W0108]: symbol `no_such_symbol` of foreign subprogram `missing`"),
        "{err}"
    );
    assert!(err.contains("searched") && err.contains("c_test"), "{err}");

    // `-Werror` turns the warning into an error before the simulation.
    let (code, err, out) = yavs(&["run", &lib, &load, "-Werror", "lazy"]);
    assert_eq!(code, 2, "{err}");
    assert!(!out.contains("lazy ok"), "{out}");

    // Calling it is E0107, a runtime error at the call with the same note.
    let (code, err, _) = yavs(&["run", &lib, &load, "miss"]);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("error[E0107]: symbol `no_such_symbol` of foreign subprogram `missing`"),
        "{err}"
    );
    assert!(err.contains("searched") && err.contains("c_test"), "{err}");

    // Without `--load` the C functions are not found either: a warning each, an error at the
    // first call.
    let (code, err, _) = yavs(&["run", &lib, "tb"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("warning[W0108]"), "{err}");
    assert!(err.contains("error[E0107]"), "{err}");

    let missing = format!("--load={}", tmp.join("nope.so").display());
    let (code, err, _) = yavs(&["run", &lib, &missing, "tb"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("error[E0106]: cannot load library"), "{err}");
    let _ = std::fs::remove_dir_all(&tmp);
}

const FAT_C: &str = r#"
#include <stdint.h>
#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif
struct bounds { int32_t left, right; uint8_t dir; uint32_t len; };
struct fat { void *base; struct bounds *b; };
/* Characters summed, and every bound of the string checked against the caller's. */
EXPORT int32_t str_sum(struct fat *s, int32_t left, int32_t right, uint8_t downto) {
    int32_t sum = 0;
    if (s->b->left != left || s->b->right != right || s->b->dir != downto) return -1;
    for (uint32_t i = 0; i < s->b->len; i++) sum += ((const uint8_t *)s->base)[i];
    return sum;
}
/* A constrained array is its data pointer. */
EXPORT int32_t first(const uint8_t *p) { return p[0]; }
/* In `integer range`, all 32 bits; `inout` a pointer to them. */
EXPORT int32_t negate(int32_t x) { return -x; }
EXPORT void bump(int32_t *x, uint8_t *b, double *r) { *x += 1; *b = !*b; *r = *r * 2; }
/* Elements as C sees them; a `length` library could not get the bounds. */
EXPORT void invert(struct fat *v) {
    uint8_t *p = v->base;
    for (uint32_t i = 0; i < v->b->len; i++) p[i] = p[i] == 3 ? 2 : 3;
}
/* An element of an integer range type is 32 bits, not the smallest that holds it. */
EXPORT int32_t sum3(struct fat *v) {
    const int32_t *p = v->base;
    return p[0] + p[1] + p[2];
}
/* Index types decide each dimension's struct: boolean bytes first, then natural. */
struct bbounds { uint8_t left, right, dir; uint32_t len; };
struct mixed { struct bbounds a; struct bounds b; };
EXPORT int32_t mix(struct fat *m) {
    const struct mixed *b = (const struct mixed *)m->b;
    const uint8_t *p = m->base;
    return b->a.left * 1000 + b->a.len * 100 + b->b.right * 10 + p[1];
}
/* A 64 bit index: left, right, dir at 16, len at 20. */
struct lbounds { int64_t left, right; uint8_t dir; uint32_t len; };
struct lfat { void *base; struct lbounds *b; };
EXPORT int64_t wide(struct lfat *v) { return v->b->left + v->b->len + v->b->dir; }
/* Natural alignment; integer fields in 32 bits whatever their subtype. */
struct rec { uint8_t a; int32_t b; double c; };
EXPORT void rec_bump(struct rec *r) { r->a = !r->a; r->b = -r->b; r->c += 1.5; }
/* More than 256 literals: 32 bits. */
EXPORT uint32_t enum_next(uint32_t x) { return x + 1; }
EXPORT int64_t to_ns(int64_t t) { return t / 1000000; }
EXPORT void take_access(void *p) { (void)p; }
"#;

const FAT_VHDL: &str = r#"
library ieee; use ieee.std_logic_1164.all;
package f is
  subtype s8 is integer range -100 to 50;
  type big is range 0 to 2**40;
  type bigv is array (big range <>) of bit;
  type mixa is array (boolean range <>, natural range <>) of bit;
  type tiny is range 0 to 3;
  type tiny_v is array (natural range <>) of tiny;
  type rec is record a : boolean; b : s8; c : real; end record;
  type e300 is (FAT_LITS);
  function str_sum(s : string; left, right : integer; down : boolean) return integer;
  attribute foreign of str_sum : function is "VHPIDIRECT str_sum";
  function first(v : std_logic_vector(3 downto 0)) return integer;
  attribute foreign of first : function is "VHPIDIRECT first";
  function negate(x : s8) return s8;
  attribute foreign of negate : function is "VHPIDIRECT negate";
  procedure bump(x : inout s8; b : inout boolean; r : inout real);
  attribute foreign of bump : procedure is "VHPIDIRECT bump";
  procedure invert(v : inout std_logic_vector);
  attribute foreign of invert : procedure is "VHPIDIRECT invert";
  function sum3(v : tiny_v) return integer;
  attribute foreign of sum3 : function is "VHPIDIRECT sum3";
  function mix(m : mixa) return integer;
  attribute foreign of mix : function is "VHPIDIRECT mix";
  function wide(v : bigv) return big;
  attribute foreign of wide : function is "VHPIDIRECT wide";
  procedure rec_bump(r : inout rec);
  attribute foreign of rec_bump : procedure is "VHPIDIRECT rec_bump";
  function enum_next(x : e300) return e300;
  attribute foreign of enum_next : function is "VHPIDIRECT enum_next";
  function to_ns(t : time) return integer;
  attribute foreign of to_ns : function is "VHPIDIRECT to_ns";
  -- In the library loaded in the default layout.
  function sum(v : integer_vector) return integer;
  attribute foreign of sum : function is "VHPIDIRECT sum";
  type int_ptr is access integer;
  procedure take_access(variable p : int_ptr);
  attribute foreign of take_access : procedure is "VHPIDIRECT take_access";
end;

use work.f.all;
library ieee; use ieee.std_logic_1164.all;
entity fat_tb is
  generic (name : string := "abc");
end;
architecture a of fat_tb is
  -- During elaboration, as a memory model opens its image: a string generic.
  signal id : integer := str_sum(name, 1, 3, false);
  signal clk : bit;
begin
  clk <= not clk after 1 ns when now < 40 ns;
  process (clk)
    variable x : s8 := -7;
    variable b : boolean := false;
    variable r : real := 1.25;
    variable v : std_logic_vector(0 to 2) := "101";
    variable w : rec := (true, -9, 0.5);
    variable d : string(7 downto 4) := "wxyz";
    variable done : boolean := false;
  begin
    if rising_edge(clk) and not done then
      done := true;
      assert id = 294 report integer'image(id) severity failure;
      assert str_sum("hello", 1, 5, false) = 532 report "check 2" severity failure;
      assert str_sum(d, 7, 4, true) = 482 report "check 3" severity failure;
      assert str_sum("", 1, 0, false) = 0 report "check 4" severity failure;
      assert first("1010") = 3 report "check 5" severity failure;
      assert negate(-1) = 1 and negate(5) = -5 report "check 6" severity failure;
      bump(x, b, r);
      assert x = -6 and b and r = 2.5 report "check 7" severity failure;
      invert(v);
      assert v = "010" report "check 8" severity failure;
      assert sum3((1, 2, 3)) = 6 report "check 9" severity failure;
      assert mix((false => "101", true => "010")) = 220 report "check 10" severity failure;
      assert wide((big'(9) downto big'(5) => '1')) = 10 report "check 11" severity failure;
      rec_bump(w);
      assert w.a = false and w.b = 9 and w.c = 2.0 report "check 12" severity failure;
      assert enum_next(l298) = l299 report "check 13" severity failure;
      assert to_ns(3 ns) = 3 report "check 14" severity failure;
      assert sum((1, 2, 3, 4)) = 10 report "check 15" severity failure;
      report "fat ok";
    end if;
  end process;
end;

use work.f.all;
entity fat_access is end;
architecture a of fat_access is begin
  process
    variable p : int_ptr;
  begin
    take_access(p);
    report "access passed";
    wait;
  end process;
end;
"#;

#[test]
fn fat_pointer_layout() {
    let tmp = tempdir("foreign_fat");
    let lib_of = |name: &str, src: &str| {
        let c = tmp.join(format!("{name}.c"));
        std::fs::write(&c, src).expect("write");
        let so = tmp.join(if cfg!(windows) {
            format!("{name}.dll")
        } else {
            format!("lib{name}.so")
        });
        shared_lib(&c, &so);
        so
    };
    let fat = lib_of("fat_test", FAT_C);
    let len = lib_of("len_test", C);
    let src = tmp.join("tb.vhd");
    let lits: Vec<String> = (0..300).map(|i| format!("l{i}")).collect();
    std::fs::write(&src, FAT_VHDL.replace("FAT_LITS", &lits.join(", "))).expect("write");
    let lib = format!("--work=work:{}", tmp.join("lib").display());
    let (code, err, _) = yavs(&["analyze", "--std=2008", &lib, &src.display().to_string()]);
    assert_eq!(code, 0, "{err}");
    let load_fat = format!("--load={}:args=fat-pointer", fat.display());
    let load_len = format!("--load={}:args=length", len.display());

    // Each library in its own layout, in one run, on every tier this build has; a compiled
    // tier compiles the calling process at once.
    let tiers: &[&[&str]] = &[
        &["--tier=interp"],
        &["--tier=interp", "--threads=4"],
        #[cfg(feature = "clift")]
        &["--tier=clift"],
        #[cfg(feature = "clift")]
        &["--tier=clift-auto", "--tier-heat=1,0,0"],
        #[cfg(feature = "llvm")]
        &["--tier=llvm"],
    ];
    for tier in tiers {
        let mut args = vec!["run", "--std=2008", &lib, &load_fat, &load_len];
        args.extend_from_slice(tier);
        args.push("fat_tb");
        let (code, err, out) = yavs(&args);
        assert_eq!(code, 0, "{tier:?}: {err}\n{out}");
        assert!(out.contains("fat ok"), "{tier:?}: {out}");
    }

    // What the layout does not pass is an error before time 0, not a crash at the call.
    let (code, err, _) = yavs(&[
        "run",
        "--std=2008",
        &lib,
        &load_fat,
        &load_len,
        "fat_access",
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("error[E0107]") && err.contains("the `fat-pointer` layout does not support"),
        "{err}"
    );

    // The same subprogram in the default layout passes it.
    let default = format!("--load={}", fat.display());
    let (code, err, out) = yavs(&["run", "--std=2008", &lib, &default, "fat_access"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("access passed"), "{out}");

    let bad = format!("--load={}:args=sideways", fat.display());
    let (code, err, _) = yavs(&["run", "--std=2008", &lib, &bad, "fat_tb"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("error[E0136]: unknown argument layout `sideways`"),
        "{err}"
    );

    // A near miss names the layout meant; the list of layouts is there either way.
    let near = format!("--load={}:args=fat-pointr", fat.display());
    let (code, err, _) = yavs(&["run", "--std=2008", &lib, &near, "fat_tb"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("a layout with a similar name exists: `fat-pointer`"),
        "{err}"
    );
    let (_, err, _) = yavs(&["run", "--std=2008", &lib, &bad, "fat_tb"]);
    assert!(!err.contains("similar name"), "{err}");
    assert!(err.contains("the layouts are"), "{err}");

    // Without the library, a call during elaboration names the symbol.
    let (code, err, _) = yavs(&["run", "--std=2008", &lib, "fat_tb"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("error[E2107]") && err.contains("symbol `str_sum`"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
