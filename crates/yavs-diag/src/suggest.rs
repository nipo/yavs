//! "Did you mean" suggestions (sem.md 8.3): the one helper every name lookup in yavs uses,
//! in `yavs-diag` so that every crate (library layer, lint, coverage, the binary) reaches it.

/// Optimal string alignment distance (Damerau-Levenshtein with adjacent transpositions).
pub fn distance(a: &str, b: &str) -> usize {
    strsim::osa_distance(a, b)
}

/// Up to two of `cands` (`(folded key, payload)`) close to `key`, best first: edit distance
/// within `max(1, len/3)`, or `key` an abbreviation of the name (`cnt` for `count`: three letters
/// or more, same first letter, letters in order, name at most twice as long), ranked after
/// the edit matches. A second one comes only when it ranks equal to the first.
pub fn similar<'a, T>(key: &str, cands: impl Iterator<Item = (&'a str, T)>) -> Vec<T> {
    let limit = (key.chars().count() / 3).max(1);
    let mut found: Vec<(usize, &str, T)> = cands
        .filter(|(c, _)| *c != key)
        .filter_map(|(c, t)| {
            let d = distance(key, c);
            let rank = if d <= limit {
                d
            } else if abbreviates(key, c) {
                limit + 1
            } else {
                return None;
            };
            Some((rank, c, t))
        })
        .collect();
    found.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let best = found.first().map(|f| f.0);
    found
        .into_iter()
        .take_while(|f| Some(f.0) == best)
        .take(2)
        .map(|f| f.2)
        .collect()
}

fn abbreviates(key: &str, name: &str) -> bool {
    let (k, n) = (key.chars().count(), name.chars().count());
    if k < 3 || n > 2 * k || key.chars().next() != name.chars().next() {
        return false;
    }
    let mut rest = name.chars();
    key.chars().all(|c| rest.any(|x| x == c))
}

/// The help line for the `cands` = (folded key, spelling) close to `key`, all of kind `noun`.
pub fn suggest<'a>(
    key: &str,
    noun: &'static str,
    cands: impl Iterator<Item = (&'a str, &'a str)>,
) -> Option<String> {
    let found = similar(key, cands);
    let shown: Vec<_> = found.iter().map(|&n| (n.to_string(), noun)).collect();
    (!shown.is_empty()).then(|| help(&shown))
}

/// `; a key with a similar name exists: `rule``, for the end of an error message that is one
/// string (configuration files): the closest of `names` to `key`, compared without case; empty
/// when none is close.
pub fn tail<'a>(key: &str, noun: &'static str, names: impl Iterator<Item = &'a str>) -> String {
    let names: Vec<_> = names.map(|n| (n.to_lowercase(), n)).collect();
    let cands = names.iter().map(|(k, n)| (k.as_str(), *n));
    suggest(&key.to_lowercase(), noun, cands).map_or_else(String::new, |h| format!("; {h}"))
}

/// The help line for `found` = (original spelling, kind): `a signal with a similar name exists:
/// `count``, or both when two names are equally close.
pub fn help(found: &[(String, &str)]) -> String {
    match found {
        [(n, kind)] => {
            // "an unit" is wrong: a leading `u` sounds like `y` in `unit` and `user`.
            let vowel = kind.starts_with(['a', 'e', 'i', 'o'])
                || (kind.starts_with('u') && !kind.starts_with("uni") && !kind.starts_with("use"));
            let a = if vowel { "an" } else { "a" };
            format!("{a} {kind} with a similar name exists: `{n}`")
        }
        _ => {
            let names: Vec<String> = found.iter().map(|(n, k)| format!("`{n}` ({k})")).collect();
            format!("similar names exist: {}", names.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick<'a>(key: &str, c: &[&'a str]) -> Vec<&'a str> {
        similar(key, c.iter().map(|&n| (n, n)))
    }

    #[test]
    fn similar_finds_typos_and_abbreviations() {
        assert_eq!(distance("ab", "ba"), 1);
        let c = ["numeric_std", "std_logic_1164"];
        assert_eq!(pick("numeric_sdt", &c), ["numeric_std"]);
        assert!(pick("xyz", &c).is_empty());
        assert_eq!(pick("cnt", &["count", "other"]), ["count"]);
        // too short, other first letter, or name too long: no abbreviation
        assert!(pick("ct", &["count"]).is_empty());
        assert!(pick("ont", &["count"]).is_empty());
        assert!(pick("cnt", &["counter_value"]).is_empty());
    }

    #[test]
    fn similar_prefers_edits_and_keeps_ties() {
        assert_eq!(pick("clk", &["clock", "clx"]), ["clx"]);
        assert_eq!(pick("data", &["datb", "datc", "dat"]), ["dat", "datb"]);
    }

    #[test]
    fn tail_ends_a_message() {
        let names = ["rule", "exclude"];
        assert_eq!(
            tail("Rulee", "key", names.into_iter()),
            "; a key with a similar name exists: `rule`"
        );
        assert_eq!(tail("zzzz", "key", names.into_iter()), "");
    }

    #[test]
    fn help_wording() {
        assert_eq!(
            help(&[("count".into(), "signal")]),
            "a signal with a similar name exists: `count`"
        );
        assert_eq!(
            help(&[("Idx".into(), "attribute")]),
            "an attribute with a similar name exists: `Idx`"
        );
        assert_eq!(
            help(&[("ns".into(), "time unit")]),
            "a time unit with a similar name exists: `ns`"
        );
        assert_eq!(
            help(&[("ns".into(), "unit")]),
            "a unit with a similar name exists: `ns`"
        );
        assert_eq!(
            help(&[("a".into(), "signal"), ("b".into(), "constant")]),
            "similar names exist: `a` (signal), `b` (constant)"
        );
    }
}
