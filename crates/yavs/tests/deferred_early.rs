//! The bounds of a deferred constant whose subtype is not fully constrained come from its full
//! declaration's value. A call made while elaborating a declaration of the package body above
//! that full declaration, to a subprogram declared above it that reads the bounds, is erroneous
//! (LRM 14.4.2): `E3009` at the read, in every tier. The same subprogram called once the full
//! declaration is elaborated reads the bounds as before (#1076). `@@` marks the line of the
//! error (the statement that reads the bounds).

mod common;
use common::tempdir;

use std::process::{Command, Output};

/// (name, declarations of the package body after `mk`): the call is made by `X1`, above `U`.
const BAD: &[(&str, &str)] = &[
    (
        "folded",
        "function h return integer is begin return @@U'length; end;
constant X1 : integer := h;
constant U : vec_t := (1, 2, 3, 4);",
    ),
    (
        "computed",
        "function h return integer is begin return @@U'length; end;
constant X1 : integer := h;
constant U : vec_t := mk(4);",
    ),
    (
        "range",
        "function h return integer is variable s : integer := 0;
begin for i in @@U'range loop s := s + 1; end loop; return s; end;
constant X1 : integer := h;
constant U : vec_t := mk(4);",
    ),
    (
        "parameter",
        "procedure q (constant d : in bit_vector(U'range)) is begin null; end;
function h return integer is begin @@q(\"0000\"); return 1; end;
constant X1 : integer := h;
constant U : vec_t := mk(4);",
    ),
    (
        "nested_call",
        "function h return integer is begin return @@U'left; end;
function g return integer is begin return h + 1; end;
constant X1 : integer := g;
constant U : vec_t := mk(4);",
    ),
];

/// (name, declarations, result): the same subprograms, called once `U` is elaborated.
const OK: &[(&str, &str, i64)] = &[
    (
        "above_called_late",
        "function h return integer is begin return U'length + U'left; end;
constant U : vec_t := mk(4);
constant X1 : integer := h;",
        5,
    ),
    (
        "folded_called_late",
        "function h return integer is begin return U'length + U'left; end;
constant U : vec_t := (5, 6, 7, 8);
constant X1 : integer := h;",
        4,
    ),
    (
        "below",
        "constant U : vec_t := mk(4);
function h return integer is begin return U'length + U'left; end;
constant X1 : integer := h;",
        5,
    ),
    (
        "range_loop",
        "function h return integer is variable s : integer := 0;
begin for i in U'range loop s := s + i; end loop; return s + U'length + U'left - 1; end;
constant U : vec_t := mk(4);
constant X1 : integer := h;",
        14,
    ),
];

fn source(name: &str, body: &str) -> String {
    format!(
        "package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body p is
  function mk(n : integer) return vec_t is
    variable r : vec_t(1 to n) := (others => 7);
  begin return r; end;
{body}
  function x return integer is begin return X1; end;
end package body;
use work.p.all;
entity t_{name} is end entity;
architecture a of t_{name} is
begin
  process begin
    report \"RES \" & integer'image(x);
    wait;
  end process;
end architecture;
"
    )
}

fn tiers() -> Vec<&'static str> {
    let mut tiers = vec!["--tier=interp"];
    if cfg!(feature = "clift") {
        tiers.push("--tier=clift");
    }
    if cfg!(feature = "llvm") {
        tiers.push("--tier=llvm");
    }
    tiers
}

fn yavs(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(["--color", "never"])
        .args(args)
        .output()
        .expect("run")
}

/// Analyse `src` as `name/case.vhd` under `tmp` and return the `--work` argument.
fn analyse(tmp: &std::path::Path, name: &str, src: &str) -> String {
    let dir = tmp.join(name);
    std::fs::create_dir_all(&dir).expect("dir");
    let file = dir.join("case.vhd");
    std::fs::write(&file, src).expect("write");
    let work = format!("--work=work:{}/lib", dir.display());
    let o = yavs(&["analyze", "--std=2008", &work, file.to_str().expect("path")]);
    assert!(
        o.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    work
}

#[test]
fn a_bounds_read_before_the_full_declaration_is_an_error_in_every_tier() {
    let tmp = tempdir("deferred_early");
    for (name, body) in BAD {
        let src = source(name, body);
        let at = src.find("@@").expect("marker");
        let line = src[..at].matches('\n').count() + 1;
        let work = analyse(&tmp, name, &src.replacen("@@", "", 1));
        for tier in tiers() {
            let o = yavs(&["run", &work, tier, "--threads=1", &format!("t_{name}")]);
            let e = String::from_utf8_lossy(&o.stderr);
            assert_eq!(o.status.code(), Some(2), "{name} {tier}: {e}");
            for want in [
                "error[E3009]: the bounds of deferred constant `U` are read",
                "its full declaration has not been elaborated yet",
                "move the declaration that makes the call below the full declaration of `U`",
                &format!("case.vhd:{line}:"),
                "called from here",
            ] {
                assert!(e.contains(want), "{name} {tier}: {want}: {e}");
            }
        }
    }
}

#[test]
fn the_same_read_after_the_full_declaration_gives_the_bounds() {
    let tmp = tempdir("deferred_early_ok");
    for (name, body, result) in OK {
        let work = analyse(&tmp, name, &source(name, body));
        for tier in tiers() {
            let o = yavs(&["run", &work, tier, "--threads=1", &format!("t_{name}")]);
            let all = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            assert!(o.status.success(), "{name} {tier}: {all}");
            assert!(
                all.contains(&format!("RES {result}")),
                "{name} {tier}: want {result}: {all}"
            );
        }
    }
}
