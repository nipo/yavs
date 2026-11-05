//! Text a user reads says what is wrong and, for a language rule, cites the LRM. It compares with
//! no other tool and points to nothing inside the repository (user, 2026-11-05).

use std::path::Path;

/// The string literals of `src`, without comments.
fn literals(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let (mut i, mut out) = (0, Vec::new());
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'\'' if b.get(i + 2) == Some(&b'\'') => i += 3,
            b'\'' if b.get(i + 1) == Some(&b'\\') => i += 4,
            b'"' => {
                let start = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                out.push(src[start..i.min(b.len())].to_string());
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).expect("read dir").flatten() {
        let p = e.path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs")
            && !p.to_string_lossy().ends_with("tests.rs")
        {
            out.push(p);
        }
    }
}

#[test]
fn messages_name_no_other_tool_and_no_internal_document() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let bad = [
        "nvc",
        "ghdl",
        "sim1",
        "sim2",
        "sim3",
        "sim4",
        "other simulator",
        "some simulator",
        "most simulator",
        "reference simulator",
        "major simulator",
        "the simulators",
        "ir.md",
        "sem.md",
        "plan.md",
        "decisions.md",
        "lrm-checks",
        "NATIVE.md",
        "STATUS.md",
        "AGENTS.md",
    ];
    let mut found = Vec::new();
    for c in std::fs::read_dir(&crates).expect("crates").flatten() {
        let mut files = Vec::new();
        if c.path().join("src").is_dir() {
            sources(&c.path().join("src"), &mut files);
        }
        for f in files {
            let text = std::fs::read_to_string(&f).expect("source");
            // Test modules are not messages.
            let text = text.split("#[cfg(test)]").next().expect("split");
            for l in literals(text) {
                let low = l.to_lowercase();
                // `include_str!` paths name a file, and are not read by a user.
                if let Some(w) = bad.iter().find(|w| low.contains(&w.to_lowercase()))
                    && !l.starts_with("../")
                {
                    found.push(format!("{}: `{w}` in \"{l}\"", f.display()));
                }
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}
