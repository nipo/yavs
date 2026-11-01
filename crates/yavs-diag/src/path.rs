//! How yavs writes a hierarchical path: `tb.g(1).u.p`, in every text of its own (#975,
//! `docs/decisions.md` "Dotted paths"). Colons belong to the values of `'path_name` and
//! `'instance_name`, the LRM's form, which is what the elaborator builds names from.

/// The bytes of `path`, each with whether it can separate levels: not inside an extended
/// identifier (`\\a:b\\`) or a for-generate index (`g(':')`, an enumeration literal). Everything
/// that matters here is ASCII.
fn scan(path: &str) -> impl Iterator<Item = (usize, u8, bool)> {
    let (mut ext, mut depth, mut quoted) = (false, 0, 0);
    path.bytes().enumerate().map(move |(i, c)| {
        if quoted > 0 {
            quoted -= 1;
        } else if ext {
            ext = c != b'\\';
        } else {
            match c {
                b'\\' => ext = true,
                b'(' => depth += 1,
                b')' => depth -= 1,
                b'\'' if depth > 0 => quoted = 2,
                _ => return (i, c, depth == 0),
            }
        }
        (i, c, false)
    })
}

fn swap(path: &str, from: u8, to: u8) -> String {
    let swapped = scan(path).map(|(_, c, sep)| if c == from && sep { to } else { c });
    String::from_utf8_lossy(&swapped.collect::<Vec<u8>>()).into_owned()
}

/// A path as yavs writes it: `:tb:g(1):u:` is `tb.g(1).u`. A dotted path stays as it is, so a
/// path a user typed in either form can go through here.
pub fn dotted(path: &str) -> String {
    swap(path.trim_matches(':'), b':', b'.')
}

/// The LRM's form of a dotted path, for a text the LRM defines: `tb.u` is `:tb:u`.
pub fn colons(path: &str) -> String {
    format!(":{}", swap(path, b'.', b':'))
}

/// The path of the region around a dotted path, or the path itself at the top: `tb.u.p` is
/// `tb.u`.
pub fn parent(path: &str) -> &str {
    let last = scan(path).filter(|&(_, c, sep)| c == b'.' && sep).last();
    last.map_or(path, |(i, ..)| &path[..i])
}

/// How many regions a process path in either form sits below the top: 2 for `tb.u.p`.
pub fn depth(path: &str) -> u32 {
    let path = path.trim_matches(':');
    let is_sep = |c| matches!(c, b':' | b'.');
    // The scan only matters for a name that can hold a separator; every process has a path.
    let n = if path.contains(['\\', '\'']) {
        scan(path).filter(|&(_, c, sep)| sep && is_sep(c)).count()
    } else {
        path.bytes().filter(|&c| is_sep(c)).count()
    };
    n as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms() {
        assert_eq!(dotted(":tb:g(1):u:p"), "tb.g(1).u.p");
        assert_eq!(dotted(":tb:g(1):u:"), "tb.g(1).u");
        assert_eq!(dotted("tb.g(1).u"), "tb.g(1).u");
        assert_eq!(dotted(":tb:r.f"), "tb.r.f");
        assert_eq!(dotted(":tb(a):u@leaf(a):"), "tb(a).u@leaf(a)");
        assert_eq!(dotted(":tb:\\a:b.c\\:s"), "tb.\\a:b.c\\.s");
        assert_eq!(dotted(":"), "");
        assert_eq!(dotted(":tb:g(':'):u:"), "tb.g(':').u");
        assert_eq!(dotted(":tb:g('('):g2('.'):u"), "tb.g('(').g2('.').u");
        assert_eq!(parent("tb.g('.')"), "tb");
        assert_eq!(
            (depth(":tb:g(':'):p"), depth("tb.g(1).\\a.b\\"), depth("tb")),
            (2, 2, 0)
        );
        assert_eq!(colons("tb.g(1).\\a.b\\"), ":tb:g(1):\\a.b\\");
        assert_eq!(parent("tb.u.p"), "tb.u");
        assert_eq!(parent("tb.\\a.b\\"), "tb");
        assert_eq!(parent("tb"), "tb");
    }
}
