//! One session over several files gives the per-file result of `yavs analyze`: a file that
//! failed is not recorded, so its units, even the ones that analyzed, are not visible to later
//! files, and a file that needs them fails (silently, 8.2) instead of passing.

use yavs_sem::{Db, LangRev, Options, default_stdlib};

#[test]
fn failed_file_hides_its_units() {
    let tmp = tempfile::tempdir().expect("scratch dir");
    let dir = tmp.path();
    let file = |name: &str, text: &str| {
        let p = dir.join(name);
        std::fs::write(&p, text).expect("write");
        p
    };
    // `good` analyzes, `bad` in the same file does not.
    let a = file(
        "a.vhd",
        "package good is\nend package;\npackage bad is\n  constant c : integer := undeclared;\nend package;\n",
    );
    let b = file(
        "b.vhd",
        "use work.good.all;\npackage user is\nend package;\n",
    );
    let c = file("c.vhd", "use work.bad.all;\nentity e is\nend entity;\n");
    let mut db = Db::new(Options::new(LangRev::Vhdl2008, default_stdlib()));
    assert!(db.analyze_file("work", &a, LangRev::Vhdl2008).is_err());
    let errors = db.error_count();
    assert!(db.analyze_file("work", &b, LangRev::Vhdl2008).is_err());
    assert!(db.analyze_file("work", &c, LangRev::Vhdl2008).is_err());
    let msgs: Vec<String> = db
        .diagnostics()
        .iter()
        .map(|d| format!("{} {}", d.code, d.message))
        .collect();
    assert_eq!(db.error_count(), errors, "cascades stay silent: {msgs:?}");
}

/// A package analyzed again from another file in one session is a new package (13.5), so a type of
/// the old and of the new one have one printed name: the message says what tells them apart.
#[test]
fn types_with_one_name_say_what_differs() {
    let tmp = tempfile::tempdir().expect("scratch dir");
    let dir = tmp.path();
    let file = |name: &str, text: &str| {
        let p = dir.join(name);
        std::fs::write(&p, text).expect("write");
        p
    };
    let p = "package p is\n  type t is (a, b);\nend package;\n";
    let first = file("p1.vhd", p);
    let holder = file(
        "holder.vhd",
        "use work.p.all;\npackage holder is\n  signal s : t;\nend package;\n",
    );
    let second = file("p2.vhd", p);
    let user = file(
        "user.vhd",
        "use work.p.all;\nentity e is\nend entity;\narchitecture a of e is\n  signal x : t;\nbegin\n  x <= work.holder.s;\nend architecture;\n",
    );
    let mut db = Db::new(Options::new(LangRev::Vhdl2008, default_stdlib()));
    for f in [&first, &holder, &second] {
        assert!(db.analyze_file("work", f, LangRev::Vhdl2008).is_ok());
    }
    assert!(db.analyze_file("work", &user, LangRev::Vhdl2008).is_err());
    let d = db.diagnostics().last().expect("a diagnostic");
    assert_eq!(d.code, "E1300", "{}", d.message);
    assert!(
        d.notes.iter().any(|n| n.contains("two different types")),
        "{:?}",
        d.notes
    );
}
