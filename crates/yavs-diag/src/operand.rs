//! The operand of a condition as text that stays the same across formatting: what a named coverage
//! exclusion and a stored positional one compare.

/// `text` without comments, blanks that separate nothing and outer parentheses, in lower case except
/// inside string and character literals and extended identifiers: `(Count>3)` and `count > 3 -- x` are
/// one operand.
pub fn key(text: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    let mut it = text.char_indices().peekable();
    let word = |c: char| c.is_alphanumeric() || c == '_';
    while let Some((i, c)) = it.next() {
        match c {
            '"' | '\\' => {
                if gap && out.chars().next_back().is_some_and(word) {
                    out.push(' ');
                }
                gap = false;
                out.push(c);
                for (_, x) in it.by_ref() {
                    out.push(x);
                    if x == c {
                        break;
                    }
                }
            }
            '\'' if char_literal(text, i) => {
                gap = false;
                out.push(c);
                for _ in 0..2 {
                    out.extend(it.next().map(|x| x.1));
                }
            }
            '-' if text[i..].starts_with("--") => {
                for (_, x) in it.by_ref() {
                    if x == '\n' {
                        break;
                    }
                }
                gap = true;
            }
            c if c.is_whitespace() => gap = true,
            c => {
                if gap && word(c) && out.chars().next_back().is_some_and(word) {
                    out.push(' ');
                }
                gap = false;
                out.extend(c.to_lowercase());
            }
        }
    }
    let mut s = out.as_str();
    while let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        if !balanced(inner) {
            break;
        }
        s = inner;
    }
    s.to_string()
}

/// Whether the `'` at byte `i` of `text` opens a character literal, not an attribute or a qualified
/// expression (those follow a name).
pub fn char_literal(text: &str, i: usize) -> bool {
    let name_before = text[..i]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == ')' || c == '\\');
    let mut after = text[i + 1..].chars();
    !name_before && after.next().is_some() && after.next() == Some('\'')
}

/// Whether the parentheses of `text` never close more than they opened and end level.
fn balanced(text: &str) -> bool {
    let mut depth = 0i32;
    let mut lit = None;
    for (i, c) in text.char_indices() {
        match (lit, c) {
            (Some(q), _) if q == c => lit = None,
            (Some(_), _) => {}
            (None, '"' | '\\') => lit = Some(c),
            (None, '\'') if char_literal(text, i) => {}
            (None, '(') => depth += 1,
            (None, ')') => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

#[cfg(test)]
mod tests {
    use super::key;

    #[test]
    fn formatting_does_not_count() {
        for (a, b) in [
            ("count > 3", "COUNT>3"),
            ("count  >\n 3 -- limit\n", "(count > 3)"),
            ("((a)) ", "a"),
            ("f(a, b)", "F( a,b )"),
            ("x = '1'", "X='1'"),
            ("a and b", "a  AND\tb"),
            ("t'(a)", "T'(A)"),
        ] {
            assert_eq!(key(a), key(b), "{a}");
        }
    }

    #[test]
    fn what_counts() {
        for (a, b) in [
            ("a and b", "b and a"),
            ("x = 'a'", "x = 'A'"),
            ("s = \"Ab\"", "s = \"aB\""),
            ("\\Ab\\", "\\aB\\"),
            ("a b", "ab"),
            ("(a) and (b)", "a) and (b"),
        ] {
            assert_ne!(key(a), key(b), "{a}");
        }
        assert_eq!(key("(a) and (b)"), "(a)and(b)");
    }
}
