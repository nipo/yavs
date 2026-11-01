//! How a pattern in a configuration or exclusion file names source files: the one matching of
//! `yavs lint`'s `exclude` and waivers and of coverage exclusion files. A pattern is relative to
//! the directory of the file holding it, may name a directory, and uses `*` and `?` (which stop
//! at `/`) and `**`.

use std::path::{Path, PathBuf};

/// A pattern as written, with `\` as `/` and no leading `./`.
pub fn normalize(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches("./").to_owned()
}

/// `*` and `?` stop at `/`, `**` does not.
pub fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern {
        [] => text.is_empty(),
        [b'*', b'*', rest @ ..] => {
            let rest = rest.strip_prefix(b"/").unwrap_or(rest);
            (0..=text.len()).any(|i| glob(rest, &text[i..]))
        }
        [b'*', rest @ ..] => (0..=text.len())
            .take_while(|&i| i == 0 || text[i - 1] != b'/')
            .any(|i| glob(rest, &text[i..])),
        [b'?', rest @ ..] => text.first().is_some_and(|&c| c != b'/') && glob(rest, &text[1..]),
        [c, rest @ ..] => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

/// `path` with `.` and `..` taken out, without asking the file system.
pub fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// The absolute path of the file `path` names: symlinks and `..` resolved as far as the path
/// exists, the rest cleaned of `.` and `..`, and no `\\?\` prefix on Windows. Two spellings of one
/// file give one path, which `std::path::absolute` does not (it leaves both as they are).
pub fn resolve(path: &Path) -> PathBuf {
    let a = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let c: Vec<_> = a.components().collect();
    let resolved = (1..=c.len()).rev().find_map(|n| {
        let head = c[..n].iter().collect::<PathBuf>().canonicalize().ok()?;
        Some(c[n..].iter().fold(head, |mut r, x| {
            match x {
                std::path::Component::ParentDir => drop(r.pop()),
                std::path::Component::CurDir => {}
                x => r.push(x),
            }
            r
        }))
    });
    let r = resolved.unwrap_or(a);
    let Some(s) = r.to_str() else { return r };
    match s.strip_prefix(r"\\?\UNC\") {
        Some(unc) => PathBuf::from(format!(r"\\{unc}")),
        None => PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(s)),
    }
}

/// [`resolve`], remembered: a pattern is asked about every file and finding.
fn resolve_memo(path: &Path) -> PathBuf {
    thread_local!(static MEMO: std::cell::RefCell<std::collections::HashMap<PathBuf, PathBuf>> = Default::default());
    MEMO.with_borrow_mut(|m| {
        m.entry(path.to_path_buf())
            .or_insert_with(|| resolve(path))
            .clone()
    })
}

/// A file as a pattern sees it.
pub struct Place {
    /// Relative to the directory of the file holding the pattern, when the file is under it.
    relative: Option<String>,
    pub absolute: String,
    /// The same with symlinks resolved, when that differs: a pattern may name either.
    resolved: Option<Box<Place>>,
    /// The directory of the file holding the pattern.
    base: PathBuf,
}

impl Place {
    pub fn new(file: &Path, base: &Path) -> Place {
        let mut p = Place::lexical(file, base);
        let (file, base) = (resolve(file), resolve(base));
        let r = Place::lexical(&file, &base);
        if r.absolute != p.absolute || r.relative != p.relative {
            p.resolved = Some(Box::new(r));
        }
        p
    }

    fn lexical(file: &Path, base: &Path) -> Place {
        let (file, base) = (lexical(file), lexical(base));
        Place {
            relative: file
                .strip_prefix(&base)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/")),
            absolute: file.to_string_lossy().replace('\\', "/"),
            resolved: None,
            base,
        }
    }

    /// The absolute path as spelled, then resolved when that differs.
    pub fn absolutes(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.absolute.as_str())
            .chain(self.resolved.iter().map(|r| r.absolute.as_str()))
    }

    /// Whether the pattern names the file or a directory above it. A pattern is relative to the
    /// base; one that starts with `/` or `**` also matches the absolute path, for files outside.
    pub fn covered_by(&self, pattern: &str) -> bool {
        self.covered_here(pattern)
            || self
                .resolved
                .as_ref()
                .is_some_and(|r| r.covered_here(pattern))
            || self.literal(pattern)
    }

    /// A pattern without wildcards names a path that may itself pass a symlink: it covers the
    /// file when both resolve to one path, or to a directory above it.
    // ponytail: a wildcard pattern is not resolved, so `link/*.vhd` does not match a file recorded
    // as `src/c.vhd`; resolve the literal head of a pattern if that is ever asked for.
    fn literal(&self, pattern: &str) -> bool {
        // A pattern reaches only below its base: `..` does not leave it.
        let mut depth = 0_i32;
        for c in Path::new(pattern).components() {
            depth += match c {
                std::path::Component::ParentDir => -1,
                std::path::Component::Normal(_) => 1,
                _ => 0,
            };
            if depth < 0 {
                return false;
            }
        }
        if pattern.contains(['*', '?']) {
            return false;
        }
        let target = resolve_memo(&self.base.join(pattern.trim_end_matches('/')));
        let target = target.to_string_lossy().replace('\\', "/");
        let me = self
            .resolved
            .as_ref()
            .map_or(&self.absolute, |r| &r.absolute);
        me.strip_prefix(&target)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    }

    fn covered_here(&self, pattern: &str) -> bool {
        let pattern = pattern.trim_end_matches('/');
        let covers = |path: &str| {
            let mut upto = path;
            loop {
                if glob(pattern.as_bytes(), upto.as_bytes()) {
                    return true;
                }
                match upto.rfind('/') {
                    Some(i) if i > 0 => upto = &upto[..i],
                    _ => return false,
                }
            }
        };
        self.relative.as_deref().is_some_and(covers)
            || ((pattern.starts_with('/')
                || pattern.starts_with("**")
                || pattern.as_bytes().get(1) == Some(&b':'))
                && covers(&self.absolute))
    }
}

/// A file a `files` list may be about: as reported, and as a pattern sees it.
pub struct Found {
    reported: String,
    place: Place,
}

impl Found {
    pub fn new(file: &str, base: &Path) -> Found {
        let path = Path::new(file);
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        Found {
            reported: normalize(file),
            place: Place::new(&absolute, base),
        }
    }

    /// The pattern names the file as [`Place::covered_by`] says, or matches the path as reported,
    /// or is its last components (a bare file name under any directory).
    pub fn matches(&self, pattern: &str) -> bool {
        self.place.covered_by(pattern)
            || glob(pattern.as_bytes(), self.reported.as_bytes())
            || Path::new(&self.reported).ends_with(pattern)
    }

    /// What a pattern most likely means for this file: its path relative to the base, else as
    /// reported.
    pub fn shown(&self) -> &str {
        self.place.relative.as_deref().unwrap_or(&self.reported)
    }
}
