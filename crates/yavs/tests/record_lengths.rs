//! A composite value whose array lengths differ from its target's is `E3009` at the statement that
//! assigns it, on every path that takes a record or an array of composites and in every tier
//! (LRM 10.6.2.1, 6.5.7.1, #1005): `@@` in a case marks the expected position.

mod common;
use common::tempdir;

use std::process::Command;

const PKG: &str = r#"package p is
  type ru is record x : bit_vector; end record;
  subtype ru3 is ru(x(1 to 3));
  subtype ru4 is ru(x(1 to 4));
  type rn is record r : ru; end record;
  type r2 is record a : bit_vector; b : bit_vector; end record;
  type ri is record v : integer_vector; end record;
  type rs is record v : string; end record;
  type aru is array (natural range <>) of ru;
  type aa is array (natural range <>) of bit_vector;
  type raa is record e : aa; end record;
  type rar is record e : aru; end record;
  type pair is record p : ru; q : ru; end record;
  type acc is access ru;
  function f4 return ru;
  function four return bit_vector;
end;
package body p is
  function f4 return ru is begin return (x => "1111"); end;
  function four return bit_vector is begin return "1111"; end;
end;
"#;

const V: &str = "variable s : ru3; variable o : ru4;\n";

/// (name, declarations of the architecture, of the process, statements).
const CASES: &[(&str, &str, &str, &str)] = &[
    ("var_whole", "", V, "@@s := o;"),
    ("var_short", "", V, "@@o := s;"),
    ("var_fn", "", V, "@@s := f4;"),
    ("var_agg", "", V, "@@s := (x => four);"),
    ("var_cond", "", V, "@@s := o when s.x(1) = '0' else o;"),
    (
        "nested",
        "",
        "variable a : rn(r(x(1 to 3))); variable b : rn(r(x(1 to 4)));\n",
        "@@a := b;",
    ),
    (
        "nested_field",
        "",
        "variable a : rn(r(x(1 to 3))); variable b : ru4;\n",
        "@@a.r := b;",
    ),
    (
        "second_field",
        "",
        "variable a : r2(a(1 to 2), b(1 to 3)); variable b : r2(a(1 to 2), b(1 to 4));\n",
        "@@a := b;",
    ),
    (
        "same_size",
        "",
        "variable a : r2(a(1 to 3), b(1 to 2)); variable b : r2(a(1 to 2), b(1 to 3));\n",
        "@@a := b;",
    ),
    (
        "field_int",
        "",
        "variable a : ri(v(1 to 3)); variable b : ri(v(1 to 4));\n",
        "@@a := b;",
    ),
    (
        "field_string",
        "",
        "variable a : rs(v(1 to 3)); variable b : rs(v(1 to 4));\n",
        "@@a := b;",
    ),
    (
        "arr_rec",
        "",
        "variable a : aru(0 to 1)(x(1 to 3)); variable b : aru(0 to 1)(x(1 to 4));\n",
        "@@a := b;",
    ),
    (
        "arr_rec_elem",
        "",
        "variable a : aru(0 to 1)(x(1 to 3)); variable b : ru4;\n",
        "@@a(0) := b;",
    ),
    (
        "arr_rec_slice",
        "",
        "variable a : aru(0 to 1)(x(1 to 3)); variable b : aru(0 to 1)(x(1 to 4));\n",
        "@@a(0 to 0) := b(0 to 0);",
    ),
    (
        "arr_arr",
        "",
        "variable a : aa(0 to 1)(1 to 3); variable b : aa(0 to 1)(1 to 4);\n",
        "@@a := b;",
    ),
    (
        "rec_arr",
        "",
        "variable a : raa(e(0 to 1)(1 to 3)); variable b : raa(e(0 to 1)(1 to 4));\n",
        "@@a := b;",
    ),
    (
        "rec_arr_rec",
        "",
        "variable a : rar(e(0 to 1)(x(1 to 3))); variable b : rar(e(0 to 1)(x(1 to 4)));\n",
        "@@a := b;",
    ),
    (
        "agg_target",
        "",
        "variable a, b : ru3; variable v : pair(p(x(1 to 4)), q(x(1 to 4)));\n",
        "@@(a, b) := v;",
    ),
    (
        "out_unconstrained",
        "",
        "variable s : ru3;\nprocedure pa (p : out ru) is begin @@p := (x => four); end;\n",
        "pa(s);",
    ),
    (
        "inout_unconstrained",
        "",
        "variable s : ru3;\nprocedure pa (p : inout ru) is variable l : ru4; begin @@p := l; end;\n",
        "pa(s);",
    ),
    (
        "out_field_actual",
        "",
        "variable a : rn(r(x(1 to 3)));\nprocedure pa (p : out ru) is variable l : ru4; begin @@p := l; end;\n",
        "pa(a.r);",
    ),
    (
        "out_constrained",
        "",
        "variable s : ru3;\nprocedure pa (p : out ru4) is begin null; end;\n",
        "@@pa(s);",
    ),
    (
        "inout_constrained",
        "",
        "variable s : ru3;\nprocedure pa (p : inout ru4) is begin null; end;\n",
        "@@pa(s);",
    ),
    (
        "in_constrained",
        "",
        "variable o : ru4;\nprocedure pa (p : in ru3) is begin null; end;\n",
        "@@pa(o);",
    ),
    (
        "in_arr_rec",
        "",
        "variable o : aru(0 to 0)(x(1 to 4));\nprocedure pa (p : in aru(0 to 0)(x(1 to 3))) is begin null; end;\n",
        "@@pa(o);",
    ),
    (
        "default_param",
        "",
        "constant d : ru := f4;\nprocedure pa (p : ru3 := d) is begin null; end;\n",
        "@@pa;",
    ),
    (
        "var_init",
        "",
        "variable o : ru4;\nprocedure pa is variable @@s : ru3 := o; begin null; end;\n",
        "pa;",
    ),
    (
        "arr_init",
        "",
        "variable o : aru(0 to 1)(x(1 to 4));\nprocedure pa is variable @@s : aru(0 to 1)(x(1 to 3)) := o; begin null; end;\n",
        "pa;",
    ),
    (
        "const_init",
        "",
        "variable o : ru4;\nprocedure pa is constant @@s : ru3 := o; begin null; end;\n",
        "pa;",
    ),
    ("qualified", "", V, "@@s := ru3'(o);"),
    (
        "access_all",
        "",
        "variable a : acc; variable o : ru4;\n",
        "a := new ru3; @@a.all := o;",
    ),
    (
        "access_new",
        "",
        "variable a : acc; variable o : ru4;\n",
        "@@a := new ru3'(o);",
    ),
    (
        "sig_whole",
        "signal t : ru3;\n",
        "variable o : ru4;\n",
        "@@t <= o; wait for 1 ns;",
    ),
    (
        "sig_arr_arr",
        "signal t : aa(0 to 1)(1 to 3);\n",
        "variable o : aa(0 to 1)(1 to 4);\n",
        "@@t <= o; wait for 1 ns;",
    ),
    (
        "sig_param",
        "signal t : ru3;\n",
        "variable o : ru4;\nprocedure pa (signal q : out ru) is begin @@q <= o; end;\n",
        "pa(t); wait for 1 ns;",
    ),
    (
        "sig_param_constrained",
        "signal t : ru4;\n",
        "procedure pa (signal q : out ru3) is begin null; end;\n",
        "@@pa(t); wait for 1 ns;",
    ),
    (
        "force_sig",
        "signal t : ru3;\n",
        "variable o : ru4;\n",
        "@@t <= force o; wait for 1 ns;",
    ),
];

#[test]
fn every_path_checks_the_lengths() {
    let mut tiers = vec!["--tier=interp"];
    if cfg!(feature = "clift") {
        tiers.push("--tier=clift");
    }
    if cfg!(feature = "llvm") {
        tiers.push("--tier=llvm");
    }
    let tmp = tempdir("record_lengths");
    let mut src = PKG.to_string();
    let mut at = Vec::new();
    for (name, arch, decl, stmt) in CASES {
        let body = format!(
            "use work.p.all;\nentity {name} is end;\narchitecture a of {name} is\n{arch}begin process\n{decl}begin\n{stmt}\nwait; end process; end;\n"
        );
        for line in body.split_inclusive('\n') {
            match line.find("@@") {
                Some(col) => {
                    at.push((src.lines().count() + 1, col + 1));
                    src.push_str(&line.replacen("@@", "", 1));
                }
                None => src.push_str(line),
            }
        }
    }
    let file = tmp.join("lengths.vhd");
    std::fs::write(&file, &src).expect("write");
    let work = format!("--work=work:{}/lib", tmp.display());
    let yavs = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_yavs"))
            .args(["--color", "never"])
            .args(args)
            .output()
            .expect("run")
    };
    let o = yavs(&["analyze", "--std=2008", &work, file.to_str().expect("path")]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(at.len(), CASES.len());
    for tier in tiers {
        for ((name, ..), (line, col)) in CASES.iter().zip(&at) {
            let o = yavs(&["run", &work, tier, "--threads=1", name]);
            let e = String::from_utf8_lossy(&o.stderr);
            assert_eq!(o.status.code(), Some(1), "{name} {tier}: {e}");
            assert!(
                e.contains("error[E3009]: value length"),
                "{name} {tier}: {e}"
            );
            assert!(e.contains("does not match"), "{name} {tier}: {e}");
            assert!(
                e.contains(&format!(".vhd:{line}:{col}")),
                "{name} {tier}: at {line}:{col}: {e}"
            );
            assert!(
                e.contains("help: make both sides the same length"),
                "{name} {tier}: {e}"
            );
        }
    }
}
