//! The console's tools for finding out why a value is what it is: `drivers`, `describe`, `find`,
//! `waits`, `trace x`, `examine NAME @ TIME`, `activity` and `asserts`. All of it reads state at
//! a stop; nothing here runs unless asked for.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use yavs_elab::{ProcTable, ScopeId};
use yavs_interp::ProcState;
use yavs_kernel::{PortLink, ProcId, Range, SignalId, SignalSources, Time, Value};
use yavs_sem::hir::{ObjRoot, StmtKind};
use yavs_std::value::LitType;

use crate::console::{name_word, qualify, thread_of};
use crate::{At, Shared, design::Sig, thread_proc};

/// A table of left-aligned columns, the last one free.
fn table(rows: &[Vec<String>]) -> String {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols)
        .map(|c| {
            (rows.iter())
                .filter_map(|r| r.get(c))
                .map(|x| x.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut o = String::new();
    for r in rows {
        let mut line = String::from("  ");
        for (c, x) in r.iter().enumerate() {
            if c + 1 == r.len() {
                line.push_str(x);
            } else {
                let _ = write!(line, "{x:w$}  ", w = widths[c]);
            }
        }
        o.push_str(line.trim_end());
        o.push('\n');
    }
    o
}

fn row<const N: usize>(x: [&str; N]) -> Vec<String> {
    x.iter().map(|s| s.to_string()).collect()
}

use yavs_diag::dotted;

fn place(file: &str, line: usize) -> String {
    let p = std::path::Path::new(file);
    let short = std::env::current_dir()
        .ok()
        .and_then(|d| p.strip_prefix(d).ok().map(|x| x.display().to_string()));
    format!("{}:{line}", short.as_deref().unwrap_or(file))
}

fn span_place(sh: &Shared, s: yavs_diag::Span) -> String {
    let f = sh.db.sources.get(s.file);
    place(&f.path, f.line_col(s.start).0)
}

/// The signal `name` means at this stop.
fn signal<'s>(sh: &'s Shared, at: &At<'_, '_>, cmd: &str, name: &str) -> Result<&'s Sig, String> {
    if name.is_empty() {
        return Err(format!("`{cmd}` needs a signal name"));
    }
    let proc = thread_proc(sh, thread_of(at));
    sh.design
        .find(proc, &qualify(sh, at, name))
        .map(|k| &sh.design.sigs[k])
        .ok_or_else(|| crate::console::no_signal(sh, thread_of(at), name))
}

fn sources(at: &At<'_, '_>, s: SignalId) -> Option<SignalSources> {
    match at {
        At::Process(st) => st.ctx?.sources(s),
        At::Kernel(sim) => sim.sources(s),
    }
}

fn proc_name(sh: &Shared, p: ProcId) -> String {
    sh.elab
        .procs
        .get(p.0 as usize)
        .map_or_else(|| "a foreign process".to_string(), |x| sh.elab.proc_name(x))
}

/// The statements of process `p` that assign `sig` (or pass it to a procedure that does), and
/// else where it starts.
fn assign_places(sh: &Shared, p: ProcId, sig: &Sig) -> Vec<String> {
    let Some(inst) = sh.elab.procs.get(p.0 as usize) else {
        return Vec::new();
    };
    let db = &sh.db;
    let mut places = BTreeSet::new();
    let mut todo = vec![inst.stmt];
    while let Some(st) = todo.pop() {
        if matches!(
            db.stmts[st].kind,
            StmtKind::SignalAssign { .. }
                | StmtKind::ConcSignalAssign { .. }
                | StmtKind::Force { .. }
                | StmtKind::ProcCall(_)
                | StmtKind::ConcProcCall { .. }
        ) {
            let mut lsps = Vec::new();
            db.collect_drivers(&[st], &mut lsps);
            if lsps.iter().any(|l| root_is(sh, inst.scope, l.root, sig)) {
                places.insert(span_place(sh, db.stmts[st].span));
            }
        }
        todo.extend(db.stmt_children(st));
    }
    if places.is_empty() {
        places.insert(span_place(sh, db.stmts[inst.stmt].span));
    }
    places.into_iter().collect()
}

/// The `sigs` index of what `root` names, seen from scope `from` and outward.
fn root_sig(sh: &Shared, from: ScopeId, root: ObjRoot) -> Option<usize> {
    let ObjRoot::Decl(d) = root else { return None };
    let mut cur = Some(from);
    while let Some(s) = cur {
        if let Some(&o) = sh.elab.sigs.get(&(s, d)) {
            let obj = sh.elab.nets.obj(o);
            let path = format!(
                "{}{}",
                sh.elab.hier.path_name(s, yavs_elab::Case::Lrm),
                obj.name.key
            );
            return sh.design.find(None, &path.to_ascii_lowercase());
        }
        cur = sh.elab.hier.scope(s).parent;
    }
    None
}

fn root_is(sh: &Shared, from: ScopeId, root: ObjRoot, sig: &Sig) -> bool {
    root_sig(sh, from, root).is_some_and(|k| std::ptr::eq(&sh.design.sigs[k], sig))
}

/// The signals process `p` reads, as far as its statements name them.
fn reads(sh: &Shared, p: ProcId) -> Vec<usize> {
    let Some(inst) = sh.elab.procs.get(p.0 as usize) else {
        return Vec::new();
    };
    let mut lsps = Vec::new();
    sh.db.reads_deep(&[inst.stmt], &mut lsps);
    let mut out: Vec<usize> = (lsps.iter())
        .filter_map(|l| root_sig(sh, inst.scope, l.root))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The elements `r` of a vector signal as VHDL indices, else nothing.
fn index_text(sig: &Sig, leaf_len: u32, r: Range) -> String {
    if r.len >= leaf_len {
        return String::new();
    }
    if let LitType::Array {
        left, ascending, ..
    } = &sig.lit
    {
        // Kernel offset 0 is the rightmost element.
        let n = i64::from(leaf_len);
        let right = if *ascending {
            left + n - 1
        } else {
            left - n + 1
        };
        let at = |k: i64| if *ascending { right - k } else { right + k };
        let (lo, hi) = (i64::from(r.offset), i64::from(r.offset + r.len - 1));
        return if lo == hi {
            format!("({})", at(lo))
        } else if *ascending {
            format!("({} to {})", at(hi), at(lo))
        } else {
            format!("({} downto {})", at(hi), at(lo))
        };
    }
    format!("(elements {} to {})", r.offset, r.offset + r.len - 1)
}

/// `r` relative to the part it lies in.
fn rel(r: Range, part: Range) -> Range {
    let start = r.offset.max(part.offset);
    let end = (r.offset + r.len).min(part.offset + part.len);
    Range::new(start - part.offset, end.saturating_sub(start))
}

/// The value of a source or a forced part as VHDL.
fn image(sig: &Sig, leaf: usize, whole: bool, v: &Value) -> String {
    if whole && sig.leaf_count() == 1 {
        let read = |_: yavs_kernel::SigRef| Some(v.clone());
        return sig.show(&read);
    }
    sig.leaf_image(leaf, v)
}

/// A port association through which the value of `s` comes or goes.
fn port_of(links: &[PortLink], s: SignalId, slot: u32) -> Option<SignalId> {
    links.iter().find_map(|l| {
        let at = l.actual.iter().position(|a| a.0 == s)?;
        (l.slots.get(at) == Some(&slot)).then(|| l.formal.first().map(|f| f.0))?
    })
}

/// The actual an `in` or `inout` port takes its value from.
fn actual_of(links: &[PortLink], s: SignalId) -> Option<SignalId> {
    links.iter().find_map(|l| {
        l.formal
            .iter()
            .any(|f| f.0 == s)
            .then(|| l.actual.first().map(|a| a.0))?
    })
}

fn sig_named(sh: &Shared, s: SignalId) -> Option<&Sig> {
    sh.design.sigs.iter().find(|x| x.leaves().any(|l| l == s))
}

fn header(sig: &Sig, at: &At<'_, '_>) -> String {
    let decl = sig.decl.as_ref().map_or_else(String::new, |(f, l)| {
        format!(", declared at {}", place(f, *l as usize))
    });
    format!(
        "{} : {} = {}{decl}\n",
        dotted(&sig.path),
        sig.declared,
        sig.show(&|r| at.read(r))
    )
}

/// `drivers NAME`.
pub(crate) fn drivers(sh: &Shared, at: &At<'_, '_>, name: &str) -> Result<String, String> {
    let sig = signal(sh, at, "drivers", name)?;
    let mut o = header(sig, at);
    let links = sh.ports.get().map_or(&[][..], Vec::as_slice);
    let mut rows = vec![row(["driver", "value", "assigned at"])];
    let mut forced = Vec::new();
    let mut driving = None;
    let mut resolved = false;
    for (leaf, (s, part)) in sig.leaf_parts().enumerate() {
        let Some(info) = sources(at, s) else { continue };
        resolved |= info.resolved;
        for src in info.sources.iter().filter(|x| x.range.overlaps(part)) {
            let idx = index_text(sig, part.len, rel(src.range, part));
            let whole = src.range.len >= part.len;
            let value = image(sig, leaf, whole, &src.value);
            let off = if src.disconnected {
                " (disconnected)"
            } else {
                ""
            };
            match src.owner {
                Some(p) => rows.push(vec![
                    format!("process {}{idx}", dotted(&proc_name(sh, p))),
                    format!("{value}{off}"),
                    assign_places(sh, p, sig).join(", "),
                ]),
                None => {
                    let from = port_of(links, s, src.slot).and_then(|f| sig_named(sh, f));
                    rows.push(vec![
                        format!(
                            "port {}{idx}",
                            from.map_or_else(|| "?".to_string(), |f| dotted(&f.path))
                        ),
                        format!("{value}{off}"),
                        from.and_then(|f| f.decl.as_ref())
                            .map_or_else(String::new, |(f, l)| place(f, *l as usize)),
                    ]);
                }
            }
        }
        // A signal with no driver of its own has no driving value to show.
        let driven = info.sources.iter().any(|x| x.range.overlaps(part));
        if driven && info.driving != info.effective && info.forced.is_empty() {
            driving = Some(image(sig, leaf, true, &info.driving));
        }
        for (r, v, kind) in (info.forced.iter().map(|(r, v)| (r, v, "force")))
            .chain(info.forced_out.iter().map(|(r, v)| (r, v, "force out")))
        {
            if r.overlaps(part) {
                let idx = index_text(sig, part.len, rel(*r, part));
                let value = image(sig, leaf, r.len >= part.len, v);
                forced.push(format!("  {kind}{idx}: {value}\n"));
            }
        }
    }
    if rows.len() > 1 {
        o.push_str(&table(&rows));
    } else {
        let s = sig.leaves().next();
        match s
            .and_then(|s| actual_of(links, s))
            .and_then(|a| sig_named(sh, a))
        {
            Some(a) => {
                let _ = writeln!(
                    o,
                    "  no driver of its own: this port takes its value from {}",
                    dotted(&a.path)
                );
            }
            None => {
                o.push_str("  no driver: it keeps its initial value unless forced or deposited\n")
            }
        }
    }
    if let Some(res) = &sig.resolution {
        let _ = writeln!(o, "  resolved by {res}");
    } else if resolved {
        o.push_str("  resolved\n");
    }
    if let Some(d) = driving {
        let _ = writeln!(o, "  driving value {d}");
    }
    o.extend(forced);
    if let Some(t) = sh.deposited(&sig.path) {
        let _ = writeln!(
            o,
            "  a deposit at {t} set it; a later assignment replaces that value"
        );
    }
    Ok(o.trim_end().to_string())
}

fn bounds(t: &LitType) -> String {
    match t {
        LitType::Array {
            left,
            ascending,
            len,
            elem,
        } => {
            let n = *len as i64;
            let right = if *ascending {
                left + n - 1
            } else {
                left - n + 1
            };
            let dir = if *ascending { "to" } else { "downto" };
            let inner = bounds(elem);
            let more = if matches!(**elem, LitType::Array { .. }) {
                format!(", elements {inner}")
            } else {
                String::new()
            };
            format!("{left} {dir} {right} ({len} elements){more}")
        }
        LitType::Int(b) => format!("{} to {}", b.low, b.high),
        LitType::Record(fs) => {
            let names: Vec<&str> = fs.iter().map(|f| f.0.as_str()).collect();
            format!("fields {}", names.join(", "))
        }
        LitType::Enum(e) => match e.lits.len() {
            0 => "none".to_string(),
            n => {
                let image =
                    |k: usize| yavs_std::value::literal_image(&yavs_std::value::Lit::Enum(k), t);
                format!("{} to {} ({n} literals)", image(0), image(n - 1))
            }
        },
        _ => "none".to_string(),
    }
}

/// The name of shared variable `k` in the wave file; `name` is what the user called it.
pub(crate) fn recorded_shared(sh: &Shared, k: usize, name: &str) -> Result<String, String> {
    let v = &sh.design.shared[k];
    if sh
        .built
        .get()
        .is_some_and(|b| b.objects.contains_key(&(v.region, v.decl)))
    {
        return Err(format!(
            "`{name}` is a protected object, whose state is not recorded in waves: `examine` shows \
             it at a stop"
        ));
    }
    Ok(dotted(&v.path))
}

/// The rows of `describe` for shared variable `k`.
fn describe_shared(sh: &Shared, at: &At<'_, '_>, k: usize) -> String {
    let v = &sh.design.shared[k];
    let shown = crate::shared_value(sh, at, k);
    let protected = matches!(
        &shown,
        Some((
            LitType::Handle {
                protected: true,
                ..
            },
            _
        ))
    );
    let what = if protected {
        "protected object"
    } else {
        "shared variable"
    };
    let mut rows = vec![vec!["kind".to_string(), what.to_string()]];
    rows.extend(declared_rows(sh, v.decl));
    match shown {
        Some((t, x)) if protected => {
            if let Some((state, _)) = crate::deref(sh, at, &t, &x) {
                rows.push(vec!["variables".to_string(), bounds(&state)]);
            }
            rows.push(vec!["value".to_string(), "protected".to_string()]);
        }
        Some((t, x)) => {
            rows.push(vec!["bounds".to_string(), bounds(&t)]);
            rows.push(vec![
                "value".to_string(),
                yavs_std::value::literal_image(&x, &t),
            ]);
        }
        None => rows.push(vec![
            "value".to_string(),
            "no object: nothing uses its region".to_string(),
        ]),
    }
    format!("{}\n{}", dotted(&v.path), table(&rows))
        .trim_end()
        .to_string()
}

/// `describe NAME`.
pub(crate) fn describe(sh: &Shared, at: &At<'_, '_>, name: &str) -> Result<String, String> {
    if name.is_empty() {
        return Err("`describe` needs a name".to_string());
    }
    let proc = thread_proc(sh, thread_of(at));
    let Some(k) = sh.design.find(proc, &qualify(sh, at, name)) else {
        return describe_variable(sh, at, name);
    };
    let s = &sh.design.sigs[k];
    let mut rows = vec![
        vec!["kind".to_string(), s.kind.clone()],
        vec!["type".to_string(), s.declared.clone()],
        vec!["bounds".to_string(), bounds(&s.lit)],
        vec![
            "resolution".to_string(),
            s.resolution
                .clone()
                .unwrap_or_else(|| "none (one driver at most)".to_string()),
        ],
    ];
    if let Some((f, l)) = &s.decl {
        rows.push(vec!["declared".to_string(), place(f, *l as usize)]);
    }
    rows.push(vec!["value".to_string(), s.show(&|r| at.read(r))]);
    Ok(format!("{}\n{}", dotted(&s.path), table(&rows))
        .trim_end()
        .to_string())
}

/// The variable `name` of the process at dotted path `proc`, if it has one: its declaration.
fn process_variable(sh: &Shared, proc: &str, name: &str) -> Option<yavs_sem::DeclId> {
    let inst = sh
        .elab
        .procs
        .iter()
        .find(|p| dotted(&sh.elab.proc_name(p)) == proc)?;
    let StmtKind::Process { scope, .. } = &sh.db.stmts[inst.stmt].kind else {
        return None;
    };
    sh.db.scopes[*scope].order.iter().copied().find(|&d| {
        sh.db.decls[d].name.to_string().eq_ignore_ascii_case(name)
            && matches!(&sh.db.decls[d].kind, yavs_sem::decl::DeclKind::Object(_))
    })
}

/// The rows of `describe` a declaration gives.
fn declared_rows(sh: &Shared, d: yavs_sem::DeclId) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    if let Some(t) = crate::design::declared_type(&sh.db, d) {
        rows.push(vec!["type".to_string(), t]);
    }
    rows.push(vec![
        "declared".to_string(),
        span_place(sh, sh.db.decls[d].name_span()),
    ]);
    rows
}

fn describe_variable(sh: &Shared, at: &At<'_, '_>, name: &str) -> Result<String, String> {
    // `tb.counter.total`, as `find variables` lists it: known wherever the session stands.
    let dotted_name = dotted(name);
    if let Some((proc, var)) = dotted_name.trim_matches('.').rsplit_once('.')
        && let Some(d) = process_variable(sh, proc, var)
    {
        let mut rows = vec![vec![
            "kind".to_string(),
            format!("variable of process {proc}"),
        ]];
        rows.extend(declared_rows(sh, d));
        let head = format!("{proc}.{var}");
        return Ok(format!("{head}\n{}", table(&rows)).trim_end().to_string());
    }
    let shared = || match shared_part(sh, at, name) {
        Some((k, "")) => Ok(describe_shared(sh, at, k)),
        _ => Err(crate::console::no_signal(sh, thread_of(at), name)),
    };
    let At::Process(st) = at else {
        return shared();
    };
    let proc = sh
        .session()
        .processes()
        .get(st.process)
        .map(|p| dotted(&p.name));
    for i in (0..st.frames.depth()).rev() {
        let locals = st.frames.locals(i);
        let Some(l) = locals.iter().find(|l| l.name.eq_ignore_ascii_case(name)) else {
            continue;
        };
        let (t, v, _) = crate::local(sh, st, i, l.name)
            .ok_or_else(|| format!("`{name}` cannot be read here"))?;
        let mut rows = vec![vec![
            "kind".to_string(),
            format!("variable of {}", st.frames.func_name(i).unwrap_or("?")),
        ]];
        // A process's own variable has its declaration; a subprogram's locals do not say.
        match proc
            .as_deref()
            .filter(|_| i == 0)
            .and_then(|p| process_variable(sh, p, l.name))
        {
            Some(d) => rows.extend(declared_rows(sh, d)),
            None => rows.push(vec![
                "declared".to_string(),
                "not known for a local of a subprogram".to_string(),
            ]),
        }
        rows.push(vec!["bounds".to_string(), bounds(&t)]);
        rows.push(vec![
            "value".to_string(),
            yavs_std::value::literal_image(&v, &t),
        ]);
        return Ok(format!("{}\n{}", l.name, table(&rows))
            .trim_end()
            .to_string());
    }
    shared()
}

/// The shared variable `name` means at this stop (between cycles, seen from the top level), and
/// the part of it after its name.
pub(crate) fn shared_part<'n>(
    sh: &Shared,
    at: &At<'_, '_>,
    name: &'n str,
) -> Option<(usize, &'n str)> {
    let seen = thread_proc(sh, thread_of(at));
    let top = (at.process().is_none())
        .then(|| sh.design.roots.first())
        .flatten()
        .map(|&r| sh.design.regions[r].path.as_str());
    let means = |n: &str| {
        let in_top = top.and_then(|t| sh.design.named(None, &format!("{t}{n}")));
        in_top.or_else(|| sh.design.named(seen, n))
    };
    let ends = (name.char_indices())
        .filter(|&(_, c)| c == '.' || c == '(')
        .map(|(i, _)| i)
        .chain([name.len()]);
    let ends: Vec<usize> = ends.collect();
    ends.into_iter()
        .rev()
        .find_map(|i| match means(&name[..i])? {
            crate::design::Named::Shared(k) => Some((k, &name[i..])),
            crate::design::Named::Signal(_) => None,
        })
}

/// `examine tb.counter.total`, a name of `find variables`: in the process stopped in, its value;
/// elsewhere, the wave file at this time, or why it cannot be read. `None`: not such a name.
pub(crate) fn examine_path(
    sh: &Shared,
    at: &At<'_, '_>,
    name: &str,
) -> Option<Result<String, String>> {
    let (proc, var) = name.trim_matches('.').rsplit_once('.')?;
    process_variable(sh, proc, var)?;
    if let At::Process(st) = at
        && sh
            .session()
            .processes()
            .get(st.process)
            .is_some_and(|p| dotted(&p.name) == proc)
    {
        let (t, v, _) = crate::local(sh, st, 0, var)?;
        return Some(Ok(yavs_std::value::literal_image(&v, &t)));
    }
    if sh.wave_path().is_none() {
        return Some(Err(format!(
            "`{name}` is a variable of process {proc}: stop in that process to read it, or run \
             with `--wave=FILE.fst` and read it from there"
        )));
    }
    let now = at.now().map_or(0, Time::as_fs);
    Some(examine_at(sh, at, &format!("{name} @ {now} fs")))
}

/// Whether `pattern` selects dotted path `p`: with a `.` over the whole path, else its last name.
pub(crate) fn path_matches(pattern: &str, p: &str) -> bool {
    let pat = dotted(pattern);
    let pat = pat.trim_matches('.');
    if pat.contains('.') {
        yavs_wave::glob_match(pat, p) || yavs_wave::glob_match(&format!("**.{pat}"), p)
    } else {
        yavs_wave::glob_match(pat, p.rsplit('.').next().unwrap_or(p))
    }
}

/// The signals `pattern` selects, by `sigs` index.
pub(crate) fn matching_signals(sh: &Shared, pattern: &str) -> Vec<usize> {
    (0..sh.design.sigs.len())
        .filter(|&k| path_matches(pattern, &dotted(&sh.design.sigs[k].path)))
        .collect()
}

/// `(dotted path, what it is)` of the variables of every process and the shared variables.
fn all_variables(sh: &Shared) -> Vec<(String, String)> {
    let t: ProcTable = sh.elab.proc_table();
    let mut out = Vec::new();
    for (k, p) in sh.elab.procs.iter().enumerate() {
        let name = sh.elab.proc_name(p);
        let func = t.func(ProcId(k as u32)).and_then(|f| sh.prog.func(f));
        for l in func.iter().flat_map(|f| &f.locals) {
            // Not the compiler's own slots.
            if !l.name.starts_with('_') && l.name != "self" {
                let what = format!("variable of {}", dotted(&name));
                out.push((format!("{}.{}", dotted(&name), l.name), what));
            }
        }
        // Loop parameters may share a name.
        out.dedup();
    }
    for v in &sh.design.shared {
        let is_object = sh
            .built
            .get()
            .is_some_and(|b| b.objects.contains_key(&(v.region, v.decl)));
        let what = if is_object {
            "protected object"
        } else {
            "shared variable"
        };
        out.push((dotted(&v.path), what.to_string()));
    }
    out
}

/// `find signals PATTERN` and `find variables PATTERN`.
pub(crate) fn find(sh: &Shared, at: &At<'_, '_>, rest: &str) -> Result<String, String> {
    let (what, pattern) = crate::console::word(rest);
    if what.is_empty() {
        return Err(crate::console::bad_sub(
            "find",
            what,
            &["signals", "variables"],
        ));
    }
    if pattern.is_empty() && matches!(what, "signals" | "signal" | "variables" | "variable") {
        let eg = if what.starts_with("signal") {
            "*clk*"
        } else {
            "count"
        };
        return Err(format!("`find {what}` needs a pattern: `find {what} {eg}`"));
    }
    let none = |noun: &'static str, cands: Vec<String>| {
        let keys: Vec<(String, &str)> = (cands.iter())
            .map(|c| {
                let last = c.rsplit('.').next().unwrap_or(c);
                (last.to_ascii_lowercase(), c.as_str())
            })
            .collect();
        let key = pattern.trim_matches('*').to_ascii_lowercase();
        let help =
            yavs_sem::suggest::suggest(&key, noun, keys.iter().map(|(k, c)| (k.as_str(), *c)));
        let help = help.map_or(String::new(), |h| format!(": {h}"));
        format!("no {noun} matches `{pattern}`{help}")
    };
    match what {
        "signals" | "signal" => {
            let found = matching_signals(sh, pattern);
            if found.is_empty() {
                return Err(none(
                    "signal",
                    sh.design.sigs.iter().map(|s| dotted(&s.path)).collect(),
                ));
            }
            let mut rows = vec![row(["signal", "kind", "type", "value"])];
            for k in &found {
                let s = &sh.design.sigs[*k];
                rows.push(vec![
                    dotted(&s.path),
                    s.kind.clone(),
                    s.declared.clone(),
                    s.show(&|r| at.read(r)),
                ]);
            }
            Ok(table(&rows).trim_end().to_string())
        }
        "variables" | "variable" => {
            let all = all_variables(sh);
            let found: Vec<_> = (all.iter())
                .filter(|(p, _)| path_matches(pattern, p))
                .collect();
            if found.is_empty() {
                return Err(none("variable", all.iter().map(|v| v.0.clone()).collect()));
            }
            let mut rows = vec![row(["variable", "kind"])];
            for (p, what) in found {
                rows.push(vec![p.clone(), what.clone()]);
            }
            Ok(table(&rows).trim_end().to_string())
        }
        _ => Err(crate::console::bad_sub(
            "find",
            what,
            &["signals", "variables"],
        )),
    }
}

/// `yavs/find`: what `pattern` selects, for a client's list.
pub(crate) fn find_list(sh: &Shared, kind: &str, pattern: &str) -> Vec<serde_json::Value> {
    let design = &sh.design;
    if kind == "variables" {
        return all_variables(sh)
            .into_iter()
            .filter(|(p, _)| path_matches(pattern, p))
            .map(|(p, what)| serde_json::json!({"path": p, "detail": what}))
            .collect();
    }
    matching_signals(sh, pattern)
        .into_iter()
        .map(|k| {
            let s = &design.sigs[k];
            serde_json::json!({"path": dotted(&s.path), "detail": format!("{}, {}", s.kind, s.declared)})
        })
        .collect()
}

/// `waits`: where each process stands and what it waits for.
pub(crate) fn waits(sh: &Shared, at: &At<'_, '_>) -> String {
    let mut rows = vec![row(["process", "state", "waits for", "at"])];
    let here = at.process();
    for (i, p) in sh.session().processes().iter().enumerate() {
        let on: Vec<String> = (p.sensitivity.iter())
            .filter_map(|r| sig_named(sh, r.signal))
            .map(|s| dotted(&s.path))
            .collect();
        let mut what = on.join(", ");
        if let Some(t) = p.timeout {
            if what.is_empty() {
                what = format!("{t}");
            } else {
                what = format!("{what}, or {t}");
            }
        }
        let state = match p.state {
            _ if here == Some(i) => "stopped here",
            ProcState::NotStarted => "not started",
            ProcState::Running => "running",
            ProcState::Waiting if what.is_empty() => {
                what = "nothing: `wait;`".to_string();
                "ended"
            }
            ProcState::Waiting => "waiting",
            ProcState::Terminated => "ended",
        };
        let loc = (p.wait)
            .and_then(|s| sh.design.loc(s))
            .map_or_else(String::new, |l| place(&l.path, l.line as usize));
        rows.push(vec![dotted(&p.name), state.to_string(), what, loc]);
    }
    table(&rows).trim_end().to_string()
}

/// `examine NAME @ TIME`.
pub(crate) fn examine_at(sh: &Shared, at: &At<'_, '_>, text: &str) -> Result<String, String> {
    let (name, time) = text
        .split_once('@')
        .map(|(a, b)| (a.trim(), b.trim()))
        .ok_or("expected `examine NAME @ TIME`")?;
    let fs = parse_time(time)?;
    if let Some(now) = at.now()
        && fs > now.as_fs()
    {
        return Err(format!(
            "{time} is still to come: the simulation is at {now}"
        ));
    }
    if let Some((k, _)) = shared_part(sh, at, name) {
        recorded_shared(sh, k, name)?;
    }
    let path = sh.wave_path().ok_or_else(|| {
        format!(
            "`examine {name} @ {time}` reads the wave file, and this run records none: run with \
             `--wave=FILE.fst`"
        )
    })?;
    sh.sync_wave(at);
    let mut h = yavs_wave::history::History::open(path)?;
    let wanted = wave_name(sh, at, name);
    match h.value_at(&wanted, fs) {
        Some(Some(v)) => Ok(v),
        Some(None) => Err(format!("`{name}` has no recorded value at {time} yet")),
        None => {
            // A composite is recorded element by element.
            let below: Vec<String> = h
                .names()
                .filter(|n| {
                    n.starts_with(&format!("{wanted}.")) || n.starts_with(&format!("{wanted}("))
                })
                .map(str::to_string)
                .collect();
            if below.is_empty() {
                let names: Vec<String> = h.names().map(str::to_string).collect();
                let help = yavs_sem::suggest::suggest(
                    &wanted,
                    "recorded name",
                    names.iter().map(|n| (n.as_str(), n.as_str())),
                );
                let help = help.unwrap_or_else(|| {
                    "check `--wave-include`, `--wave-exclude` and `--wave-max-array`".to_string()
                });
                return Err(format!("`{name}` is not recorded in `{path}`: {help}"));
            }
            let mut rows = Vec::new();
            for n in below.iter().take(32) {
                let v = h
                    .value_at(n, fs)
                    .flatten()
                    .unwrap_or_else(|| "(no value yet)".into());
                rows.push(vec![n.clone(), v]);
            }
            if below.len() > 32 {
                rows.push(vec![format!("and {} more", below.len() - 32)]);
            }
            Ok(table(&rows).trim_end().to_string())
        }
    }
}

fn parse_time(text: &str) -> Result<u64, String> {
    crate::console::parse_time(text).map(Time::as_fs)
}

/// The name of a signal or a variable in the wave file, seen from this stop.
fn wave_name(sh: &Shared, at: &At<'_, '_>, name: &str) -> String {
    let proc = thread_proc(sh, thread_of(at));
    if let Some(k) = sh.design.find(proc, &qualify(sh, at, name)) {
        return dotted(&sh.design.sigs[k].path);
    }
    // A variable of the process stopped in.
    if let At::Process(st) = at {
        let locals = st.frames.locals(0);
        if locals.iter().any(|l| l.name.eq_ignore_ascii_case(name))
            && let Some(p) = sh.session().processes().get(st.process)
        {
            return format!("{}.{}", dotted(&p.name), name.to_ascii_lowercase());
        }
    }
    if let Some((k, part)) = shared_part(sh, at, name) {
        return format!(
            "{}{}",
            dotted(&sh.design.shared[k].path),
            part.to_ascii_lowercase()
        );
    }
    dotted(name).trim_matches('.').to_ascii_lowercase()
}

/// `trace x NAME`.
pub(crate) fn trace_x(sh: &Shared, at: &At<'_, '_>, rest: &str) -> Result<String, String> {
    let (kind, name) = crate::console::word(rest);
    if kind != "x" {
        return Err(crate::console::bad_sub("trace", kind, &["x"]));
    }
    let (name, _) = name_word(name);
    let sig = signal(sh, at, "trace x", name)?;
    let k = sh.design.sigs.iter().position(|s| std::ptr::eq(s, sig));
    let text = sig.show(&|r| at.read(r));
    if !yavs_wave::history::unknown(&text) {
        return Ok(format!(
            "{} = {text}: all known, nothing to trace",
            dotted(&sig.path)
        ));
    }
    let mut hist = sh.wave_path().and_then(|p| {
        sh.sync_wave(at);
        yavs_wave::history::History::open(p).ok()
    });
    let now = at.now().map_or(u64::MAX, Time::as_fs);
    let mut t = Trace {
        sh,
        at,
        hist: &mut hist,
        now,
        seen: BTreeSet::new(),
        out: String::new(),
        origins: Vec::new(),
        cut: false,
    };
    t.follow(k.ok_or("the signal is gone")?, 0);
    let mut o = t.out;
    if !t.origins.is_empty() {
        o.push_str("Where it starts:\n");
        for x in &t.origins {
            let _ = writeln!(o, "  {x}");
        }
    }
    if t.cut {
        o.push_str("The trace stops at 40 signals.\n");
    }
    o.push_str(
        "It follows drivers and the signals their processes name. It cannot see variables,\n\
         values from files, subprogram parameters, or which signal read made this value.",
    );
    Ok(o)
}

/// A driver holds U, X, W or `-` (`Z` is a value a driver means to give).
fn driver_unknown(v: &Value) -> bool {
    let t = v.to_string();
    matches!(v, Value::Logic(_)) && t.chars().any(|c| matches!(c, 'U' | 'X' | 'W' | '-'))
}

/// The elements of `i` where two drivers hold opposite strong values.
fn conflicts(sh: &Shared, sig: &Sig, i: &SignalSources) -> Vec<String> {
    // Element to (process or port, its value there), per source.
    let mut per: std::collections::BTreeMap<u32, Vec<(String, char)>> = Default::default();
    for src in &i.sources {
        let Value::Logic(_) = src.value else { continue };
        let text = src.value.to_string();
        let chars: Vec<char> = text.trim_matches('"').chars().collect();
        let who = src.owner.map_or_else(
            || "a port".to_string(),
            |p| format!("process {}", dotted(&proc_name(sh, p))),
        );
        for (k, c) in chars.iter().rev().enumerate() {
            per.entry(src.range.offset + k as u32)
                .or_default()
                .push((who.clone(), *c));
        }
    }
    let len = match &i.effective {
        Value::Logic(l) => l.len() as u32,
        _ => 0,
    };
    let mut out = Vec::new();
    for (e, list) in per {
        let strong = |x: char| list.iter().find(|y| y.1 == x);
        if let (Some(a), Some(b)) = (strong('0'), strong('1')) {
            let idx = index_text(sig, len, Range::new(e, 1));
            out.push(format!(
                "element {idx}: {} drives '0' and {} drives '1', which resolve to 'X'",
                a.0, b.0
            ));
        }
    }
    out
}

struct Trace<'a, 'b, 'c> {
    sh: &'a Shared,
    at: &'a At<'b, 'c>,
    hist: &'a mut Option<yavs_wave::history::History>,
    now: u64,
    seen: BTreeSet<usize>,
    out: String,
    origins: Vec<String>,
    cut: bool,
}

impl Trace<'_, '_, '_> {
    /// Since when `sig` has been unknown: from the wave file, else its last event.
    fn since(&mut self, sig: &Sig, info: &[SignalSources]) -> String {
        if let Some(h) = self.hist.as_mut()
            && let Some(ch) = h.changes(&dotted(&sig.path), self.now)
        {
            let start = ch
                .iter()
                .rev()
                .take_while(|c| yavs_wave::history::unknown(&c.1))
                .last();
            if let Some((t, _)) = start
                && let Some(t) = Time::fs(*t)
            {
                return format!("since {t}");
            }
        }
        match info.iter().filter_map(|i| i.last_event).max() {
            Some(t) => format!("last event at {t}"),
            None => "never changed".to_string(),
        }
    }

    fn follow(&mut self, k: usize, depth: usize) {
        let sh = self.sh;
        let at = self.at;
        let sig = &sh.design.sigs[k];
        if !self.seen.insert(k) {
            let _ = writeln!(
                self.out,
                "{:depth$}{} (already listed: a loop)",
                "",
                dotted(&sig.path),
                depth = depth * 2
            );
            return;
        }
        if self.seen.len() > 40 {
            self.cut = true;
            return;
        }
        let pad = " ".repeat(depth * 2);
        let info: Vec<SignalSources> = sig.leaves().filter_map(|s| sources(at, s)).collect();
        let value = sig.show(&|r| at.read(r));
        let since = self.since(sig, &info);
        let decl = sig.decl.as_ref().map_or_else(String::new, |(f, l)| {
            format!(", declared at {}", place(f, *l as usize))
        });
        let _ = writeln!(
            self.out,
            "{pad}{} = {value}, {since}{decl}",
            dotted(&sig.path)
        );
        let name = dotted(&sig.path);
        if info
            .iter()
            .any(|i| !i.forced.is_empty() || !i.forced_out.is_empty())
        {
            self.origins.push(format!("{name} is forced"));
            return;
        }
        let links = sh.ports.get().map_or(&[][..], Vec::as_slice);
        let mut any = false;
        let mut bad = 0;
        let leaves: Vec<SignalId> = sig.leaves().collect();
        for (s, i) in leaves.iter().zip(&info) {
            let clash = conflicts(sh, sig, i);
            bad += clash.len();
            for c in clash {
                let _ = writeln!(self.out, "{pad}  {c}");
                self.origins.push(format!("{name}: {c}"));
            }
            for src in &i.sources {
                any = true;
                if !driver_unknown(&src.value) {
                    continue;
                }
                bad += 1;
                match src.owner {
                    Some(p) => {
                        let text = sig.leaf_image(0, &src.value);
                        self.through_process(p, sig, depth + 1, &text)
                    }
                    None => {
                        if let Some(f) = port_of(links, *s, src.slot).and_then(|f| {
                            sh.design
                                .sigs
                                .iter()
                                .position(|x| x.leaves().any(|l| l == f))
                        }) {
                            self.follow(f, depth + 1);
                        }
                    }
                }
            }
        }
        if bad > 0 {
            return;
        }
        if any {
            self.origins.push(format!(
                "{name}: its drivers hold known values now, so the unknown value comes from an earlier state or a part not shown"
            ));
        } else if let Some(a) = leaves
            .first()
            .and_then(|s| actual_of(links, *s))
            .and_then(|a| {
                sh.design
                    .sigs
                    .iter()
                    .position(|x| x.leaves().any(|l| l == a))
            })
        {
            self.follow(a, depth + 1);
        } else {
            self.origins.push(format!(
                "{name}: nothing drives it, it keeps its initial value{}",
                sig.decl.as_ref().map_or_else(String::new, |(f, l)| format!(
                    " ({})",
                    place(f, *l as usize)
                ))
            ));
        }
    }

    /// A process drives an unknown value into `sig`.
    fn through_process(&mut self, p: ProcId, sig: &Sig, depth: usize, value: &str) {
        let sh = self.sh;
        let at = self.at;
        let places = assign_places(sh, p, sig).join(", ");
        let name = dotted(&proc_name(sh, p));
        let pad = " ".repeat(depth * 2);
        let _ = writeln!(
            self.out,
            "{pad}driven {value} by process {name}, at {places}"
        );
        let unknown_reads: Vec<usize> = reads(sh, p)
            .into_iter()
            .filter(|&k| yavs_wave::history::unknown(&sh.design.sigs[k].show(&|r| at.read(r))))
            .collect();
        if unknown_reads.is_empty() {
            self.origins.push(format!(
                "process {name} assigns {value} at {places}; none of the signals it names is \
                 unknown now, so look at a literal, a variable or an earlier value"
            ));
            return;
        }
        for k in unknown_reads {
            self.follow(k, depth + 1);
        }
    }
}

/// `asserts`: the summary of `--assert-summary` so far.
pub(crate) fn asserts(sh: &Shared) -> String {
    let rt = sh.built.get().map(|b| &*b.rt);
    yavs_elab::assert_summary_text(rt, &sh.db.sources)
        .trim_end()
        .to_string()
}

/// `--activity=FILE`.
pub(crate) fn activity_file(sh: &Shared, file: &str) -> Result<(), String> {
    let Some(a) = sh.activity.get() else {
        return Ok(());
    };
    let names = yavs_elab::ActivityNames {
        procs: (sh.elab.procs.iter())
            .map(|p| dotted(&sh.elab.proc_name(p)))
            .collect(),
        signals: sh.signal_names(),
    };
    let t = sh.elab.proc_table();
    let j = yavs_elab::activity_json(&sh.elab, &t, &a.runs(), &a.events(), &names, &sh.db.sources);
    std::fs::write(file, format!("{j:#}\n")).map_err(|e| format!("`{file}`: {e}"))
}

/// `activity`: the tables of `--activity` so far.
pub(crate) fn activity(sh: &Shared) -> String {
    let Some(a) = sh.activity.get() else {
        return "no counts yet".to_string();
    };
    let t = sh.elab.proc_table();
    let names = yavs_elab::ActivityNames {
        procs: (sh.elab.procs.iter())
            .map(|p| dotted(&sh.elab.proc_name(p)))
            .collect(),
        signals: sh.signal_names(),
    };
    yavs_elab::activity_text(&sh.elab, &t, &a.runs(), &a.events(), &names, &sh.db.sources)
        .trim_end()
        .to_string()
}
