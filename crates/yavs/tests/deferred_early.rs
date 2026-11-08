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

/// Complete sources: the same early call from a package nested in the package body, from a
/// generic package instantiated as a library unit and from a package nested in the body of a
/// generic package. `@@` is as above; the entity is `t_NAME`.
const BAD_UNITS: &[(&str, &str)] = &[
    (
        "nested_package",
        "package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body p is
  function mk(n : integer) return vec_t is
    variable r : vec_t(1 to n) := (others => 7);
  begin return r; end;
  package n is function h return integer; end package;
  package body n is
    function h return integer is begin return @@U'length; end;
  end package body;
  constant X1 : integer := n.h;
  constant U : vec_t := mk(4);
  function x return integer is begin return X1; end;
end package body;
use work.p.all;
entity t_nested_package is end entity;
architecture a of t_nested_package is
begin
  process begin report \"RES \" & integer'image(x); wait; end process;
end architecture;
",
    ),
    (
        "generic_instance",
        "package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body gp is
  function mk(m : integer) return vec_t is
    variable r : vec_t(1 to m) := (others => 7);
  begin return r; end;
  function h return integer is begin return @@U'length; end;
  constant X1 : integer := h;
  constant U : vec_t := mk(n);
  function x return integer is begin return X1; end;
end package body;
package ip is new work.gp generic map (n => 4);
entity t_generic_instance is end entity;
architecture a of t_generic_instance is
begin
  process begin report \"RES \" & integer'image(work.ip.x); wait; end process;
end architecture;
",
    ),
    (
        "generic_nested",
        "package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body gp is
  function mk(m : integer) return vec_t is
    variable r : vec_t(1 to m) := (others => 7);
  begin return r; end;
  package k is function h return integer; end package;
  package body k is function h return integer is begin return @@U'length; end; end package body;
  constant X1 : integer := k.h;
  constant U : vec_t := mk(n);
  function x return integer is begin return X1; end;
end package body;
package ip is new work.gp generic map (n => 4);
entity t_generic_nested is end entity;
architecture a of t_generic_nested is
begin
  process begin report \"RES \" & integer'image(work.ip.x); wait; end process;
end architecture;
",
    ),
    (
        "instance_in_package_body",
        "package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body gp is
  function mk(m : integer) return vec_t is
    variable r : vec_t(1 to m) := (others => 7);
  begin return r; end;
  function h return integer is begin return @@U'length; end;
  constant X1 : integer := h;
  constant U : vec_t := mk(n);
  function x return integer is begin return X1; end;
end package body;
package q is function y return integer; end package;
package body q is
  package ip is new work.gp generic map (n => 4);
  function y return integer is begin return ip.x; end;
end package body;
entity t_instance_in_package_body is end entity;
architecture a of t_instance_in_package_body is
begin
  process begin report \"RES \" & integer'image(work.q.y); wait; end process;
end architecture;
",
    ),
];

const GENERIC: &str = "package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body gp is
  function mk(m : integer) return vec_t is
    variable r : vec_t(1 to m) := (others => 7);
  begin return r; end;
  constant U : vec_t := mk(n);
  function x return integer is begin return U'length; end;
end package body;
";

/// The same generic package with the early call (`@@` marks the read), `X1` above `U`.
const GENERIC_EARLY: &str = "package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body gp is
  function mk(m : integer) return vec_t is
    variable r : vec_t(1 to m) := (others => 7);
  begin return r; end;
  function h return integer is begin return @@U'length; end;
  constant X1 : integer := h;
  constant U : vec_t := mk(n);
  function x return integer is begin return X1; end;
end package body;
";

/// (name, architecture): where the instance is declared. `W` is the early generic package,
/// `V` the same one with the call below the full declaration (`GENERIC` gives `U'length`).
const REGIONS: &[(&str, &str)] = &[
    (
        "architecture",
        "  package ip is new work.W generic map (n => 4);\nbegin\n  process begin report \"RES \" & integer'image(EXPR); wait; end process;",
    ),
    (
        "block",
        "begin\n  b : block\n    package ip is new work.W generic map (n => 4);\n  begin\n    process begin report \"RES \" & integer'image(EXPR); wait; end process;\n  end block;",
    ),
    (
        "generate",
        "begin\n  g : for i in 4 to 4 generate\n    package ip is new work.W generic map (n => i);\n  begin\n    process begin report \"RES \" & integer'image(EXPR); wait; end process;\n  end generate;",
    ),
    (
        "process",
        "begin\n  process\n    package ip is new work.W generic map (n => 4);\n  begin report \"RES \" & integer'image(EXPR); wait; end process;",
    ),
    (
        "function",
        "  function f return integer is\n    package ip is new work.W generic map (n => 4);\n  begin return EXPR; end function;\nbegin\n  process begin report \"RES \" & integer'image(f); wait; end process;",
    ),
    (
        "nested",
        "  package outer is new work.GO generic map (n => 4);\nbegin\n  process begin report \"RES \" & integer'image(outer.y); wait; end process;",
    ),
    (
        "nested_in_process",
        "begin\n  process\n    package outer is new work.GO generic map (n => 4);\n  begin report \"RES \" & integer'image(outer.y); wait; end process;",
    ),
];

/// A package that instantiates `W` in its body and reads the result: the instance inside an
/// instance.
const NESTED_PKG: &str = "package go is
  generic (n : integer);
  function y return integer;
end package;
package body go is
  package inner is new work.W generic map (n => n);
  function y return integer is begin return inner.U'length * 100 + inner.U'right * 10 + inner.U(inner.U'left); end function;
end package body;
";

fn region_source(name: &str, early: bool, arch: &str) -> String {
    let pkg = if early { GENERIC_EARLY } else { GENERIC };
    let nested = NESTED_PKG.replace("W", "gp");
    let expr = "ip.U'length * 100 + ip.U'right * 10 + ip.U(ip.U'left)";
    let arch = arch
        .replace("W", "gp")
        .replace("GO", "go")
        .replace("EXPR", expr);
    format!(
        "{pkg}{nested}entity t_{name} is end entity;\narchitecture a of t_{name} is\n{arch}\nend architecture;\n"
    )
}

#[test]
fn a_bounds_read_before_the_full_declaration_is_an_error_in_every_tier() {
    let tmp = tempdir("deferred_early");
    let cases = BAD
        .iter()
        .map(|(name, body)| (*name, source(name, body)))
        .chain(BAD_UNITS.iter().map(|(name, src)| (*name, src.to_string())))
        .chain(
            REGIONS
                .iter()
                .map(|(name, arch)| (*name, region_source(name, true, arch))),
        );
    for (name, src) in cases {
        let at = src.find("@@").expect("marker");
        let line = src[..at].matches('\n').count() + 1;
        let work = analyse(&tmp, name, &src.replacen("@@", "", 1));
        for tier in tiers() {
            let o = yavs(&["run", &work, tier, "--threads=1", &format!("t_{name}")]);
            let e = String::from_utf8_lossy(&o.stderr);
            // Exit 1: the instance is elaborated by a process or a call, while the simulation
            // runs; 2: while the design is elaborated.
            let code = if matches!(name, "process" | "function" | "nested_in_process") {
                1
            } else {
                2
            };
            assert_eq!(o.status.code(), Some(code), "{name} {tier}: {e}");
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

/// The instance declared in an architecture, block, generate, process or function (also inside
/// another instance) gives the bounds and elements of the full declaration (#1079).
#[test]
fn a_package_instance_in_a_region_reads_the_bounds_of_its_deferred_constant() {
    let tmp = tempdir("deferred_early_regions");
    for (name, arch) in REGIONS {
        let name = &format!("{name}_ok");
        let src = region_source(name, false, arch);
        let work = analyse(&tmp, name, &src);
        for tier in tiers() {
            let o = yavs(&["run", &work, tier, "--threads=1", &format!("t_{name}")]);
            let all = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            assert!(o.status.success(), "{name} {tier}: {all}");
            assert!(all.contains("RES 447"), "{name} {tier}: {all}");
        }
    }
}

/// The bounds are needed while the design is elaborated: a signal subtype, a generate range, a
/// constant initialised by a function and a constant subtype (#1079).
#[test]
fn a_package_instance_in_an_architecture_gives_its_bounds_at_elaboration() {
    let tmp = tempdir("deferred_early_elab");
    let src = format!(
        "{GENERIC}entity t_elab is end entity;
architecture a of t_elab is
  package ip is new work.gp generic map (n => 4);
  signal s : bit_vector(1 to ip.U'length);
  signal t : bit_vector(ip.U'range);
  function f return integer is begin return ip.U'length * 10 + ip.U'right; end;
  constant K : integer := f;
  constant V : bit_vector(ip.U'range) := (others => '1');
  constant W : integer := ip.U'length + ip.U(1);
begin
  g : for i in ip.U'range generate
    process begin report \"GEN \" & integer'image(i); wait; end process;
  end generate;
  process begin
    report \"RES \" & integer'image(s'length) & \" \" & integer'image(t'right) & \" \" & integer'image(K) & \" \" & integer'image(V'length) & \" \" & integer'image(W);
    wait;
  end process;
end architecture;
"
    );
    let work = analyse(&tmp, "elab", &src);
    for tier in tiers() {
        let o = yavs(&["run", &work, tier, "--threads=1", "t_elab"]);
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(o.status.success(), "{tier}: {all}");
        assert!(all.contains("RES 4 4 44 4 11"), "{tier}: {all}");
        assert!(
            all.contains("GEN 1") && all.contains("GEN 4"),
            "{tier}: {all}"
        );
    }
}

#[test]
fn a_generic_package_instance_and_a_nested_package_read_the_bounds_after_the_full_declaration() {
    let tmp = tempdir("deferred_early_ok_units");
    for (name, src, result) in [
        (
            "generic_after",
            format!(
                "{GENERIC}package ip is new work.gp generic map (n => 4);\nentity t_generic_after is end entity;\narchitecture a of t_generic_after is\nbegin\n  process begin report \"RES \" & integer'image(work.ip.x); wait; end process;\nend architecture;\n"
            ),
            4,
        ),
        (
            "nested_after",
            "package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body p is
  function mk(n : integer) return vec_t is
    variable r : vec_t(1 to n) := (others => 7);
  begin return r; end;
  package n is function h return integer; end package;
  package body n is
    function h return integer is begin return U'length + U'left; end;
  end package body;
  constant U : vec_t := mk(4);
  constant X1 : integer := n.h;
  function x return integer is begin return X1; end;
end package body;
use work.p.all;
entity t_nested_after is end entity;
architecture a of t_nested_after is
begin
  process begin report \"RES \" & integer'image(x); wait; end process;
end architecture;
"
            .to_string(),
            5,
        ),
    ] {
        let work = analyse(&tmp, name, &src);
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
                "{name} {tier}: {all}"
            );
        }
    }
}
