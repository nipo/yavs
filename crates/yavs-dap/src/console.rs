//! The console: the commands of `yavs run --interactive` and `--do`, and of an `evaluate` with
//! context `repl` (VS Code's Debug Console). One parser, on the run control of the server.
//!
//! A line becomes a `Batch`, served at a stop one command at a time (`work`). A command that
//! resumes the simulation leaves the stop, and the batch goes on at the next stop. A test
//! runner's command (`Hook`) waits for its `done`, and while it waits the runner's `exec` lines
//! are batches of their own. Protocol: `docs/cli-contract.md` "Console hook".

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde_json::{Value as Json, json};
use yavs_interp::Resume;
use yavs_kernel::Time;
use yavs_std::value::LitType;

use crate::{
    At, CONSOLE_BREAKS, Link, Msg, Next, RUN_UNTIL, Run, Shared, SigBp, apply_lines, apply_signals,
    cond, eval_var, rewatch, signal_change, split_part, state_vars, stop_of, thread_proc, tools,
};

/// The environment variable naming a test runner's hook: `<port>:<secret>`.
pub const HOOK_VAR: &str = "YAVS_CONSOLE_HOOK";
/// Code: the hook cannot be used.
pub(crate) const HOOK_ERROR: &str = "E0703";
const HOOK_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) const ENDED: &str = "the simulation has ended: `restart` runs it again, `quit` leaves";

/// The groups of `COMMANDS`, in the order `help` and the user guide show them.
const GROUPS: [&str; 8] = [
    "Run and step",
    "Breakpoints",
    "Inspect",
    "Change values",
    "Waves",
    "Find",
    "Diagnose",
    "Session and scripts",
];

/// The console's commands for the user guide: per group its `(syntax, meaning)` rows.
pub fn commands() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    GROUPS
        .iter()
        .map(|&g| {
            let rows = COMMANDS.iter().filter(|c| c.1 == g && !c.2.is_empty());
            (g, rows.filter_map(|c| c.2.split_once(": ")).collect())
        })
        .collect()
}

/// In `Severity as u8` order.
pub(crate) const SEVERITIES: [&str; 4] = ["note", "warning", "error", "failure"];

const COMMANDS: &[(&str, &str, &str)] = &[
    (
        "run",
        "Run and step",
        "run [TIME | until CONDITION]: to the end or a breakpoint, for TIME, or until a change of a signal in the condition makes it true",
    ),
    (
        "step",
        "Run and step",
        "step [delta | timestep]: to the next statement, entering calls; or to the end of the delta or time step",
    ),
    (
        "next",
        "Run and step",
        "next: to the next statement, over calls",
    ),
    (
        "stop",
        "Run and step",
        "stop: pause a running simulation (Ctrl-C on a terminal)",
    ),
    (
        "examine",
        "Inspect",
        "examine NAME [@ TIME]: the value of a local or a signal, or a field, element or slice of one; with a time, from the wave file (also `print`)",
    ),
    ("print", "Inspect", ""),
    (
        "drivers",
        "Diagnose",
        "drivers NAME: what drives a signal: each driver's value and source line, the resolved value, any force",
    ),
    (
        "describe",
        "Inspect",
        "describe NAME: type, bounds, resolution function, mode and declaration of a signal or variable",
    ),
    (
        "find",
        "Find",
        "find signals PATTERN | find variables PATTERN: search the hierarchy; `*` within a level, `**` across, a pattern without a dot matches the last name",
    ),
    (
        "waits",
        "Diagnose",
        "waits: where each process stands and what it waits for",
    ),
    (
        "trace",
        "Diagnose",
        "trace x NAME: follow an unknown value back through drivers and the signals they read",
    ),
    (
        "activity",
        "Diagnose",
        "activity: the processes that ran most often and the signals that changed most often, so far",
    ),
    (
        "asserts",
        "Diagnose",
        "asserts: assertion failures and reports so far, by severity and source line",
    ),
    (
        "force",
        "Change values",
        "force NAME VALUE [-out]: hold a signal at a value, from the next cycle",
    ),
    (
        "deposit",
        "Change values",
        "deposit NAME VALUE: give a signal a value once, in the next cycle",
    ),
    (
        "release",
        "Change values",
        "release NAME [-out]: end a force",
    ),
    (
        "where",
        "Inspect",
        "where: time, delta, process and source line",
    ),
    (
        "break",
        "Breakpoints",
        "break [FILE:]LINE [if CONDITION] | SIGNAL [= VALUE] [if CONDITION] | assert [note|warning|error|failure] | list | delete N | delete assert | delete all: stop at a line, at a change of a signal, or at a report of this severity and worse; list or delete them",
    ),
    (
        "wave",
        "Waves",
        "wave add NAME|PATTERN... | wave cursor [TIME] | wave save [FILE] | wave load [FILE]: with `--surfer`, show signals or scopes in the viewer; move its cursor (default: now); save or load the signal list",
    ),
    (
        "restart",
        "Session and scripts",
        "restart: back to time 0 with the same design and breakpoints (also `reset`)",
    ),
    ("reset", "Session and scripts", ""),
    (
        "source",
        "Session and scripts",
        "source FILE: run the commands of a file; the first failure ends it",
    ),
    ("help", "Session and scripts", "help: this list"),
    (
        "quit",
        "Session and scripts",
        "quit [STATUS]: leave; the exit status is STATUS, else that of the ended run, else 0",
    ),
];

/// What the client asked for at breakpoints, kept over a restart and resolved again.
#[derive(Default)]
pub(crate) struct Kept {
    /// `break assert`: severities to stop at, one bit each (`SEVERITIES` order).
    asserts: u8,
    /// The client's exception filters, likewise.
    filters: u8,
    lines: BTreeMap<String, Vec<Json>>,
    signals: [Vec<Json>; CONSOLE_BREAKS + 1],
    next_id: u64,
}

impl Kept {
    fn numbered(&mut self, mut list: Vec<Json>) -> Vec<Json> {
        for b in &mut list {
            self.next_id += 1;
            b["yavsId"] = json!(self.next_id);
        }
        list
    }

    pub(crate) fn set_lines(&mut self, key: &str, list: Vec<Json>) {
        let list = self.numbered(list);
        self.lines.insert(key.to_string(), list);
    }

    pub(crate) fn lines(&self, key: &str) -> Vec<Json> {
        self.lines.get(key).cloned().unwrap_or_default()
    }

    pub(crate) fn set_signals(&mut self, set: usize, list: Vec<Json>) {
        self.signals[set] = self.numbered(list);
    }

    pub(crate) fn signals(&self, set: usize) -> Vec<Json> {
        self.signals[set].clone()
    }
}

/// Resolve what is kept against a new simulation; what no longer resolves is said.
pub(crate) fn keep_breakpoints(sh: &Shared) {
    apply_asserts(sh);
    let (files, sets): (Vec<String>, Vec<usize>) = {
        let k = kept(sh);
        let sets = (0..k.signals.len()).filter(|&s| !k.signals[s].is_empty());
        (k.lines.keys().cloned().collect(), sets.collect())
    };
    let lost = files
        .iter()
        .flat_map(|f| apply_lines(sh, f))
        .chain(sets.into_iter().flat_map(|s| apply_signals(sh, s)));
    for b in lost.filter(|b| b["verified"] != true) {
        let m = b["message"].as_str().unwrap_or_default();
        sh.output("stderr", &format!("a kept breakpoint is lost: {m}\n"));
    }
}

fn kept<'a>(sh: &'a Shared) -> MutexGuard<'a, Kept> {
    sh.link.kept.lock().unwrap_or_else(|p| p.into_inner())
}

enum Origin {
    /// An `evaluate` request, until it is answered.
    Client(Option<Json>),
    /// A runner's `exec` or `reload`, answered when the batch ends.
    Hook(i64),
}

struct Batch {
    origin: Origin,
    lines: VecDeque<String>,
    text: String,
    error: Option<String>,
    /// The id of the runner's command under way.
    awaiting: Option<i64>,
}

#[derive(Default)]
pub(crate) struct State {
    batches: Vec<Batch>,
    /// `quit`'s status.
    exit: Option<u8>,
    failed: bool,
    told: bool,
}

fn state<'a>(link: &'a Link) -> MutexGuard<'a, State> {
    link.con.lock().unwrap_or_else(|p| p.into_inner())
}

/// What a command leaves for the batch.
enum Exec {
    Text(String),
    Next(Next),
    Script(Vec<String>),
    /// A runner's command and its arguments.
    Call(String, String),
}

/// The commands of a line (`;` separates, `#` at the start of a command and `--` end the line,
/// except a `--` after a space and before a letter in a command, which is a flag; not inside
/// strings and character literals).
fn split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'"' => quoted = !quoted,
            b'\'' if !quoted && b.get(i + 2) == Some(&b'\'') => {
                cur.push_str(&line[i..i + 3]);
                i += 3;
                continue;
            }
            b';' if !quoted => {
                out.push(std::mem::take(&mut cur));
                i += 1;
                continue;
            }
            b'#' if !quoted && cur.trim().is_empty() => break,
            // Not in a flag of a runner's command (`vunit_load --stop-time=30ns`, #933).
            b'-' if !quoted
                && b.get(i + 1) == Some(&b'-')
                && (cur.trim().is_empty()
                    || !b.get(i + 2).is_some_and(u8::is_ascii_alphabetic)
                    || !cur.ends_with(char::is_whitespace)) =>
            {
                break;
            }
            _ => {}
        }
        // Whole characters: everything the matches above look at is ASCII.
        let n = line[i..].chars().next().map_or(1, char::len_utf8);
        cur.push_str(&line[i..i + n]);
        i += n;
    }
    out.push(cur);
    out.iter()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect()
}

/// Whether a request is a console line: anything in a console session, in another one what
/// starts with a command (a plain name stays an evaluation).
pub(crate) fn is_command(sh: &Shared, req: &Json) -> bool {
    if req["arguments"]["context"] != "repl" {
        return false;
    }
    if sh.link.console.load(Ordering::Relaxed) {
        return true;
    }
    let text = req["arguments"]["expression"].as_str().unwrap_or_default();
    let line = split(text);
    let word = line
        .first()
        .and_then(|c| c.split_whitespace().next())
        .unwrap_or_default();
    COMMANDS.iter().any(|(n, ..)| *n == word)
        || sh
            .link
            .hook
            .get()
            .is_some_and(|h| h.commands.iter().any(|(n, _)| n == word))
}

/// Take an `evaluate` as a console line.
pub(crate) fn start(sh: &Shared, req: Json) {
    let text = req["arguments"]["expression"].as_str().unwrap_or_default();
    let lines = split(text).into();
    push(&sh.link, Origin::Client(Some(req)), lines);
}

fn push(link: &Link, origin: Origin, lines: VecDeque<String>) {
    let mut st = state(link);
    st.told = false;
    st.batches.push(Batch {
        origin,
        lines,
        text: String::new(),
        error: None,
        awaiting: None,
    });
}

impl Batch {
    fn say(&mut self, link: &Link, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Origin::Client(None) = self.origin {
            link.output("console", &format!("{text}\n"));
            return;
        }
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(text);
    }

    /// Answer the request now: what comes later arrives as output.
    fn flush(&mut self, link: &Link) {
        if let Origin::Client(req) = &mut self.origin
            && let Some(req) = req.take()
        {
            let text = std::mem::take(&mut self.text);
            link.respond(&req, json!({"result": text, "variablesReference": 0}));
        }
    }

    fn finish(mut self, link: &Link, st: &mut State) {
        match self.origin {
            Origin::Client(Some(ref req)) => match &self.error {
                Some(m) => {
                    // What the commands before it said is not lost.
                    if !self.text.is_empty() {
                        link.output("console", &format!("{}\n", self.text));
                    }
                    link.fail(req, m);
                }
                None => link.respond(req, json!({"result": self.text, "variablesReference": 0})),
            },
            Origin::Client(None) => {
                if let Some(m) = &self.error {
                    link.output("stderr", &format!("error: {m}\n"));
                }
            }
            Origin::Hook(id) => {
                let text = std::mem::take(&mut self.text);
                if let Some(h) = link.hook.get() {
                    h.send(&json!({
                        "result": id,
                        "ok": self.error.is_none(),
                        "text": text,
                        "error": self.error,
                    }));
                }
                return;
            }
        }
        st.failed |= self.error.is_some();
    }
}

/// Serve the batches at this stop until one resumes the simulation or all wait; then, on a
/// console, tell the client it may send the next line.
pub(crate) fn work(sh: &Shared, at: &At<'_, '_>) -> Option<Next> {
    let link = &*sh.link;
    loop {
        let mut st = state(link);
        let Some(b) = st.batches.last_mut() else {
            if !st.told && link.console.load(Ordering::Relaxed) {
                st.told = true;
                let failed = std::mem::take(&mut st.failed);
                drop(st);
                link.event("yavs/prompt", json!({"failed": failed}));
            }
            return None;
        };
        if b.awaiting.is_some() {
            return None;
        }
        let Some(line) = b.lines.pop_front() else {
            let b = st.batches.pop().expect("the last batch is there");
            b.finish(link, &mut st);
            continue;
        };
        drop(st);
        let r = exec(sh, at, &line);
        let mut st = state(link);
        let b = st.batches.last_mut().expect("a command keeps its batch");
        match r {
            Ok(Exec::Text(t)) => b.say(link, &t),
            Ok(Exec::Script(mut lines)) => {
                lines.extend(std::mem::take(&mut b.lines));
                b.lines = lines.into();
            }
            Ok(Exec::Call(name, args)) => {
                // What the commands before it said goes first: a runner's `print` comes straight
                // from its connection and would overtake it (#947).
                b.flush(link);
                match link.hook.get().map(|h| h.call(&name, &args)) {
                    Some(Ok(id)) => b.awaiting = Some(id),
                    Some(Err(m)) => {
                        b.error = Some(m);
                        b.lines.clear();
                    }
                    None => {
                        b.error =
                            Some("this is a bug in yavs: a runner's command with no runner".into());
                        b.lines.clear();
                    }
                }
            }
            Ok(Exec::Next(n)) => {
                b.flush(link);
                st.told = false;
                return Some(n);
            }
            Err(m) => {
                b.error = Some(m);
                b.lines.clear();
            }
        }
    }
}

/// Whether a runner's command is under way (the simulation then stays at its end for it).
pub(crate) fn awaiting(sh: &Shared) -> bool {
    state(&sh.link).batches.iter().any(|b| b.awaiting.is_some())
}

/// The status `quit` asked for.
pub(crate) fn exit_status(sh: &Shared) -> Option<u8> {
    state(&sh.link).exit
}

/// Say what stopped the run, on a console; the signal conditions of `run until` are over.
pub(crate) fn on_stop(sh: &Shared, at: &At<'_, '_>, stopped: &Json) {
    let until = {
        let mut all = sh.sig_bps.lock().unwrap_or_else(|p| p.into_inner());
        !std::mem::take(&mut all[RUN_UNTIL]).is_empty()
    };
    if until {
        rewatch(sh);
    }
    if !sh.link.console.load(Ordering::Relaxed) || stopped["reason"] == "entry" {
        return;
    }
    let mut text = stopped["description"]
        .as_str()
        .unwrap_or("stopped")
        .to_string();
    if let At::Process(s) = at
        && let Some(l) = sh.design.loc(s.stmt)
    {
        text += &format!(", {}:{}", l.path, l.line);
    }
    if let Some(t) = stopped["text"].as_str() {
        text += &format!(": {t}");
    }
    sh.output("important", &format!("{text}\n"));
}

/// The first word of `line` and the rest.
pub(crate) fn word(line: &str) -> (&str, &str) {
    let line = line.trim();
    match line.find(char::is_whitespace) {
        Some(i) => (&line[..i], line[i..].trim_start()),
        None => (line, ""),
    }
}

/// A name that can hold spaces inside parentheses (`s(3 downto 0)`), and the rest.
pub(crate) fn name_word(text: &str) -> (&str, &str) {
    let text = text.trim();
    let mut depth = 0;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c.is_whitespace() && depth == 0 => return (&text[..i], text[i..].trim_start()),
            _ => {}
        }
    }
    (text, "")
}

/// `name` as the signal functions read it: between cycles no process sees it, so a simple name
/// is taken from the top level.
pub(crate) fn qualify(sh: &Shared, at: &At<'_, '_>, name: &str) -> String {
    if at.process().is_some() || split_part(sh, None, name).is_some() {
        return name.to_string();
    }
    let Some(&top) = sh.design.roots.first() else {
        return name.to_string();
    };
    let full = format!("{}{}", sh.design.regions[top].path, name);
    match split_part(sh, None, &full) {
        Some(_) => full,
        None => name.to_string(),
    }
}

pub(crate) fn thread_of(at: &At<'_, '_>) -> i64 {
    at.process().map_or(0, |p| p as i64 + 1)
}

fn examine(sh: &Shared, at: &At<'_, '_>, name: &str) -> Result<String, String> {
    if name.is_empty() {
        return Err("`examine` needs a name".into());
    }
    let frame = match at {
        At::Process(s) => s.frames.depth().saturating_sub(1),
        At::Kernel(_) => 0,
    };
    let j = eval_var(sh, at, thread_of(at), frame, name)?;
    Ok(j["result"].as_str().unwrap_or("?").to_string())
}

/// "A `noun` with a similar name exists": the shared helper, over any list of names.
pub(crate) fn similar(word: &str, noun: &'static str, names: &[String]) -> Option<String> {
    let keys: Vec<(String, &str)> = names
        .iter()
        .map(|n| (n.to_ascii_lowercase(), n.as_str()))
        .collect();
    let key = word.to_ascii_lowercase();
    yavs_sem::suggest::suggest(&key, noun, keys.iter().map(|(k, n)| (k.as_str(), *n)))
}

/// The help for a word that names no command.
pub(crate) fn did_you_mean(sh: &Shared, word: &str) -> Option<String> {
    let commands = COMMANDS.iter().map(|(n, ..)| n.to_string());
    let hook = sh.link.hook.get().into_iter().flat_map(|h| &h.commands);
    let all: Vec<String> = commands.chain(hook.map(|(n, _)| n.clone())).collect();
    similar(word, "command", &all)
}

/// The help for a word that names nothing at this stop.
pub(crate) fn name_help(
    sh: &Shared,
    at: &At<'_, '_>,
    tid: i64,
    frame: usize,
    word: &str,
) -> Option<String> {
    similar(word, "signal or variable", &names_here(sh, at, tid, frame))
}

/// The evaluator's error, with the help for the name it could not find.
pub(crate) fn explain(sh: &Shared, at: &At<'_, '_>, tid: i64, frame: usize, e: String) -> String {
    let word = e
        .strip_prefix("no local or signal `")
        .and_then(|r| r.split('`').next());
    match word.and_then(|w| name_help(sh, at, tid, frame, w)) {
        Some(h) => format!("{e}: {h}"),
        None => e,
    }
}

/// "No such signal", with the help from the signals in scope.
pub(crate) fn no_signal(sh: &Shared, tid: i64, name: &str) -> String {
    let in_scope = thread_proc(sh, tid).map_or_else(
        || sh.design.sigs.iter().map(|s| s.name.clone()).collect(),
        |p| {
            let sigs = p.signals.iter();
            sigs.map(|&k| sh.design.sigs[k].name.clone())
                .collect::<Vec<_>>()
        },
    );
    match similar(name, "signal", &in_scope) {
        Some(h) => format!("no signal `{name}` here: {h}"),
        None => format!("no signal `{name}` here"),
    }
}

/// A subcommand that is none of `subs`.
fn bad_sub(cmd: &str, sub: &str, subs: &[&str]) -> String {
    let list: Vec<String> = subs.iter().map(|s| s.to_string()).collect();
    let names = subs.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>();
    let mut m = format!("`{cmd} {sub}`: expected {}", names.join(" or "));
    if let Some(h) = similar(sub, "subcommand", &list) {
        m += &format!(": {h}");
    }
    m
}

/// The names `examine` and the like see at this stop: the frame's locals, the variables of its
/// protected object, the process frame's locals and the signals in scope.
pub(crate) fn names_here(sh: &Shared, at: &At<'_, '_>, tid: i64, frame: usize) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(s) = stop_of(at, tid) {
        for i in [frame, 0] {
            out.extend(s.frames.locals(i).iter().map(|l| l.name.to_string()));
            out.extend(state_vars(sh, s, at, i).into_iter().map(|(n, ..)| n));
        }
    }
    match thread_proc(sh, tid) {
        Some(p) => out.extend(p.signals.iter().map(|&k| sh.design.sigs[k].name.clone())),
        None => {
            let top = sh.design.roots.first();
            let sigs = top.into_iter().flat_map(|&r| &sh.design.regions[r].sigs);
            out.extend(sigs.map(|&k| sh.design.sigs[k].name.clone()));
        }
    }
    out.retain(|n| n != "self");
    out.sort();
    out.dedup();
    out
}

/// The DAP `completions` request (also the terminal's Tab): what can follow in the last command of
/// `text` up to `column`. A target's `length` is how much of the text it replaces.
pub(crate) fn completions(sh: &Shared, at: &At<'_, '_>, req: &Json) {
    let a = &req["arguments"];
    let text = a["text"].as_str().unwrap_or_default();
    let column = a["column"].as_u64().unwrap_or(1).saturating_sub(1) as usize;
    let before = text.get(..column).unwrap_or(text);
    let (tid, frame) = match a["frameId"].as_i64() {
        Some(f) => (f >> 16, (f & 0xffff) as usize),
        None => (
            thread_of(at),
            match at {
                At::Process(s) => s.frames.depth().saturating_sub(1),
                At::Kernel(_) => 0,
            },
        ),
    };
    let line = before.rsplit(';').next().unwrap_or_default().trim_start();
    let word_start = line.rfind(char::is_whitespace).map_or(0, |i| i + 1);
    let (head, word) = line.split_at(word_start);
    let words: Vec<&str> = head.split_whitespace().collect();
    let names = || names_here(sh, at, tid, frame);
    let mut cands: Vec<String> = match words[..] {
        [] => {
            let hook = sh.link.hook.get().into_iter().flat_map(|h| &h.commands);
            COMMANDS
                .iter()
                .map(|(n, ..)| n.to_string())
                .chain(hook.map(|(n, _)| n.clone()))
                .chain(names())
                .collect()
        }
        ["examine" | "print"] | ["force" | "deposit" | "release"] | ["run", "until"] => names(),
        ["wave", "add", ..] => names(),
        ["wave"] => ["add", "cursor", "save", "load"].map(String::from).to_vec(),
        ["drivers" | "describe"] | ["trace", "x"] => names(),
        ["trace"] => vec!["x".into()],
        ["find"] => vec!["signals".into(), "variables".into()],
        ["break", "assert"] => SEVERITIES.map(String::from).to_vec(),
        ["step"] => vec!["delta".into(), "timestep".into()],
        ["run"] => vec!["until".into()],
        ["break"] => {
            let files = sh.design.files().into_iter().map(|f| format!("{f}:"));
            let subs = ["list", "delete", "assert"].into_iter().map(String::from);
            subs.chain(files).chain(names()).collect()
        }
        ["break", "delete"] => vec!["all".into(), "assert".into()],
        ["break", ..] if words.contains(&"if") => names(),
        ["break", _] => vec!["if".into()],
        _ => Vec::new(),
    };
    // Inside a name: a field of a record, or through an access value.
    if let Some((prefix, _)) = word.rsplit_once('.') {
        let typed = cond::parse(prefix).ok().and_then(|c| {
            let top = (tid == 0).then(|| sh.design.roots.first()).flatten();
            let names = crate::Names {
                sh,
                at,
                tid,
                frame: Some(frame),
                region: top.map(|&r| sh.design.regions[r].path.as_str()),
            };
            c.evaluate(&names).ok()
        });
        cands = match typed {
            Some((LitType::Record(fs), _)) => fs
                .into_iter()
                .map(|(n, _)| format!("{prefix}.{n}"))
                .collect(),
            Some((t @ LitType::Handle { .. }, x)) => match crate::deref(sh, at, &t, &x) {
                Some((LitType::Record(fs), _)) => fs
                    .into_iter()
                    .map(|(n, _)| format!("{prefix}.{n}"))
                    .collect(),
                _ => vec![format!("{prefix}.all")],
            },
            _ => Vec::new(),
        };
        // Or a path through the hierarchy.
        if cands.is_empty() {
            let lower = word.to_ascii_lowercase();
            cands.extend(
                sh.design
                    .sigs
                    .iter()
                    .map(|s| dotted(&s.path))
                    .filter(|p| p.starts_with(&lower)),
            );
        }
    }
    let lower = word.to_ascii_lowercase();
    let mut labels: Vec<String> = cands
        .into_iter()
        .filter(|c| c.to_ascii_lowercase().starts_with(&lower) && !c.is_empty())
        .collect();
    labels.sort();
    labels.dedup();
    let length = word.len();
    let targets: Vec<Json> = labels
        .iter()
        .map(|l| json!({"label": l, "length": length}))
        .collect();
    sh.respond(req, json!({"targets": targets}));
}

fn change(sh: &Shared, at: &At<'_, '_>, cmd: &str, rest: &str) -> Result<String, String> {
    let (name, mut rest) = name_word(rest);
    if name.is_empty() {
        return Err(format!("`{cmd}` needs a signal"));
    }
    let mode = if let Some(r) = rest.strip_suffix("-out") {
        rest = r.trim_end();
        "out"
    } else {
        "in"
    };
    if cmd != "release" && rest.is_empty() {
        return Err(format!("`{cmd}` needs a value"));
    }
    let name = qualify(sh, at, name);
    let args = json!({"signal": name, "value": rest, "mode": mode});
    let r = signal_change(sh, at, &format!("yavs/{cmd}"), &args)?;
    Ok(match r["value"].as_str() {
        Some(v) => format!("{} = {v}", r["signal"].as_str().unwrap_or_default()),
        None => r["signal"].as_str().unwrap_or_default().to_string(),
    })
}

fn where_text(sh: &Shared, at: &At<'_, '_>) -> String {
    match at {
        At::Process(s) => {
            let mut t = match (s.now, s.delta) {
                (Some(n), Some(d)) => format!("time {n}, delta {d}\n"),
                _ => "before time 0\n".to_string(),
            };
            let procs = sh.session().processes();
            t += procs.get(s.process).map_or("?", |p| &p.name);
            if let Some(l) = sh.design.loc(s.stmt) {
                t += &format!(", {}:{}", l.path, l.line);
            }
            t
        }
        At::Kernel(sim) => format!("time {}, delta {}\nbetween cycles", sim.now(), sim.delta()),
    }
}

/// A condition's text after ` if `, and what comes before.
fn split_if(text: &str) -> (&str, Option<&str>) {
    match text.split_once(" if ") {
        Some((a, c)) => (a.trim(), Some(c.trim())),
        None => (text.trim(), None),
    }
}

fn add_field(b: &mut Json, name: &str, v: Option<&str>) {
    if let Some(v) = v {
        b[name] = json!(v);
    }
}

fn break_cmd(sh: &Shared, at: &At<'_, '_>, rest: &str) -> Result<String, String> {
    let (first, tail) = word(rest);
    match first {
        "" => return Err("`break` needs a place: see `help`".into()),
        "list" => return Ok(break_list(sh)),
        "delete" if tail == "assert" => {
            kept(sh).asserts = 0;
            apply_asserts(sh);
            return Ok("no stops at assertions".into());
        }
        "delete" => return break_delete(sh, tail),
        "assert" => return break_assert(sh, tail),
        _ => {}
    }
    let (head, cond_text) = split_if(rest);
    let line = match head.rsplit_once(':') {
        Some((f, l)) if !f.is_empty() && l.parse::<u32>().is_ok() => {
            Some((Some(f), l.parse::<u32>().unwrap_or(0)))
        }
        _ => head.parse::<u32>().ok().map(|l| (None, l)),
    };
    if let Some((file, line)) = line {
        return break_line(sh, at, file, line, cond_text);
    }
    let (signal, value) = match head.split_once('=') {
        Some((s, v)) => (s.trim(), Some(v.trim())),
        None => (head, None),
    };
    let mut b = json!({"signal": qualify(sh, at, signal)});
    add_field(&mut b, "value", value);
    add_field(&mut b, "condition", cond_text);
    let id = {
        let mut k = kept(sh);
        k.next_id += 1;
        b["yavsId"] = json!(k.next_id);
        k.signals[CONSOLE_BREAKS].push(b);
        k.next_id
    };
    let out = apply_signals(sh, CONSOLE_BREAKS);
    let r = out.last().cloned().unwrap_or(Json::Null);
    if r["verified"] != true {
        kept(sh).signals[CONSOLE_BREAKS].pop();
        apply_signals(sh, CONSOLE_BREAKS);
        return Err(r["message"].as_str().unwrap_or_default().to_string());
    }
    Ok(format!(
        "breakpoint {id}: {}",
        r["description"].as_str().unwrap_or_default()
    ))
}

fn break_line(
    sh: &Shared,
    at: &At<'_, '_>,
    file: Option<&str>,
    line: u32,
    cond_text: Option<&str>,
) -> Result<String, String> {
    let key = match file {
        Some(f) => sh.design.source_file(f)?,
        None => match at {
            At::Process(s) => sh
                .design
                .loc(s.stmt)
                .map(|l| l.path.clone())
                .ok_or("this stop has no source file: write FILE:LINE")?,
            At::Kernel(_) => return Err("no source file here: write FILE:LINE".into()),
        },
    };
    let mut b = json!({"line": line});
    add_field(&mut b, "condition", cond_text);
    let id = {
        let mut k = kept(sh);
        k.next_id += 1;
        b["yavsId"] = json!(k.next_id);
        k.lines.entry(key.clone()).or_default().push(b);
        k.next_id
    };
    let out = apply_lines(sh, &key);
    let r = out.last().cloned().unwrap_or(Json::Null);
    if r["verified"] != true {
        if let Some(l) = kept(sh).lines.get_mut(&key) {
            l.pop();
        }
        apply_lines(sh, &key);
        return Err(r["message"].as_str().unwrap_or_default().to_string());
    }
    Ok(format!(
        "breakpoint {id}: {}:{}",
        sh.design.shown_file(&key),
        r["line"]
    ))
}

/// `break assert [SEVERITY]`: stop at reports of this severity and worse.
fn break_assert(sh: &Shared, what: &str) -> Result<String, String> {
    let at = if what.is_empty() { "error" } else { what };
    let Some(k) = SEVERITIES.iter().position(|s| *s == at) else {
        return Err(bad_sub("break assert", at, &SEVERITIES));
    };
    kept(sh).asserts = 0xf << k & 0xf;
    apply_asserts(sh);
    Ok(format!(
        "stop at assertions and reports of severity {}",
        SEVERITIES[k..].join(", ")
    ))
}

/// What the console and the client's exception filters ask for, told to the session.
pub(crate) fn apply_asserts(sh: &Shared) {
    let k = kept(sh);
    sh.session().set_assert_break(k.asserts | k.filters);
}

/// The client's exception filters (`setExceptionBreakpoints`).
pub(crate) fn set_filters(sh: &Shared, filters: &[Json]) {
    let mut mask = 0;
    for f in filters.iter().filter_map(Json::as_str) {
        if let Some(k) = SEVERITIES.iter().position(|s| *s == f) {
            mask |= 1 << k;
        }
    }
    kept(sh).filters = mask;
    apply_asserts(sh);
}

fn break_list(sh: &Shared) -> String {
    let k = kept(sh);
    let mut out = Vec::new();
    if k.asserts != 0 {
        let on: Vec<&str> = (0..4)
            .filter(|b| k.asserts >> b & 1 == 1)
            .map(|b| SEVERITIES[b])
            .collect();
        out.push(format!("assert: severity {}", on.join(", ")));
    }
    for (f, list) in &k.lines {
        for b in list {
            let mut t = format!("{} {}:{}", b["yavsId"], sh.design.shown_file(f), b["line"]);
            if let Some(c) = b["condition"].as_str() {
                t += &format!(" if {c}");
            }
            out.push(t);
        }
    }
    for (set, list) in k.signals.iter().enumerate() {
        for b in list {
            let name = &b[if set == 0 { "dataId" } else { "signal" }];
            let mut t = format!("{} {}", b["yavsId"], name.as_str().unwrap_or_default());
            if let Some(v) = b["value"].as_str() {
                t += &format!(" = {v}");
            }
            if let Some(c) = b["condition"].as_str() {
                t += &format!(" if {c}");
            }
            out.push(t);
        }
    }
    if out.is_empty() {
        return "no breakpoints".into();
    }
    out.join("\n")
}

fn break_delete(sh: &Shared, what: &str) -> Result<String, String> {
    let all = what == "all";
    let id = what.parse::<u64>().ok();
    if !all && id.is_none() {
        return Err(
            bad_sub("break delete", what, &["all", "assert"]) + ", or a breakpoint's number"
        );
    }
    if all {
        kept(sh).asserts = 0;
        apply_asserts(sh);
    }
    let gone = |b: &Json| all || b["yavsId"].as_u64() == id;
    let (files, sets, n) = {
        let mut k = kept(sh);
        let mut n = 0;
        let mut files = Vec::new();
        for (f, list) in &mut k.lines {
            let before = list.len();
            list.retain(|b| !gone(b));
            if list.len() != before {
                n += before - list.len();
                files.push(f.clone());
            }
        }
        let mut sets = Vec::new();
        for (set, list) in k.signals.iter_mut().enumerate() {
            let before = list.len();
            list.retain(|b| !gone(b));
            if list.len() != before {
                n += before - list.len();
                sets.push(set);
            }
        }
        (files, sets, n)
    };
    if n == 0 && !all {
        return Err(format!("no breakpoint {what}: see `break list`"));
    }
    for f in &files {
        apply_lines(sh, f);
    }
    for s in sets {
        apply_signals(sh, s);
    }
    Ok(format!("{n} deleted"))
}

/// `run until`: stop at the first change of a signal the condition names after which it holds.
fn run_until(sh: &Shared, at: &At<'_, '_>, text: &str) -> Result<Exec, String> {
    let first = cond::parse(text)?;
    let proc = thread_proc(sh, thread_of(at));
    let sigs: Vec<usize> = first
        .names()
        .iter()
        .map(|n| qualify(sh, at, n))
        .filter_map(|n| split_part(sh, proc, &n).map(|(k, _)| k))
        .collect();
    if sigs.is_empty() {
        return Err(format!("`{text}` names no signal to watch"));
    }
    let tid = thread_of(at);
    let mut bps = Vec::new();
    for sig in sigs {
        bps.push(SigBp {
            sig,
            value: None,
            cond: Some(cond::parse(text)?),
            hit: yavs_interp::HitCond::Always,
            hits: 0,
            tid,
        });
    }
    sh.sig_bps.lock().unwrap_or_else(|p| p.into_inner())[RUN_UNTIL] = bps;
    rewatch(sh);
    Ok(Exec::Next(Next::Run(Run::ToEnd)))
}

fn run(sh: &Shared, at: &At<'_, '_>, rest: &str) -> Result<Exec, String> {
    if sh.ended.load(Ordering::Relaxed) {
        // As a simulator's `run` past the end: nothing to do, and no error.
        return Ok(Exec::Text(ENDED.to_string()));
    }
    if rest.is_empty() {
        return Ok(Exec::Next(Next::Run(Run::ToEnd)));
    }
    if let ("until", cond_text) = word(rest) {
        return run_until(sh, at, cond_text);
    }
    let t = yavs_std::value::time_literal(rest)
        .map_err(|e| e.to_string())
        .and_then(|fs| Time::from_vhdl(fs).map_err(|e| e.to_string()))
        .map_err(|e| format!("`{rest}` is not a time: {e}"))?;
    let now = at.now().ok_or("no simulation time at this stop")?;
    let to = now.checked_add(t).map_err(|e| e.to_string())?;
    Ok(Exec::Next(Next::Run(Run::Until(to))))
}

fn step(sh: &Shared, n: Next) -> Result<Exec, String> {
    if sh.ended.load(Ordering::Relaxed) {
        return Err(ENDED.into());
    }
    Ok(Exec::Next(n))
}

fn restart(sh: &Shared) -> Result<Exec, String> {
    if let Some(why) = &sh.link.no_restart {
        return Err(why.clone());
    }
    *sh.restart.lock().unwrap_or_else(|p| p.into_inner()) = Some(None);
    Ok(Exec::Next(Next::Restart))
}

/// `line` in lines of at most `width` columns (0: as it is), the later ones indented.
fn wrapped(line: &str, width: usize) -> String {
    const INDENT: &str = "    ";
    if width == 0 {
        return line.to_string();
    }
    let mut out = String::new();
    let mut col = 0;
    for word in line.split(' ') {
        if col > 0 && col + 1 + word.len() > width {
            out += "\n";
            out += INDENT;
            col = INDENT.len();
        } else if col > 0 {
            out.push(' ');
            col += 1;
        }
        out += word;
        col += word.len();
    }
    out
}

fn help(sh: &Shared) -> String {
    let width = sh.link.columns.load(Ordering::Relaxed);
    let mut lines = Vec::new();
    for g in GROUPS {
        lines.push(format!("{g}:"));
        let rows = COMMANDS.iter().filter(|c| c.1 == g && !c.2.is_empty());
        lines.extend(rows.map(|c| wrapped(c.2, width)));
    }
    if let Some(h) = sh.link.hook.get() {
        let shown = h.commands.iter().filter(|(_, help)| !help.is_empty());
        lines.extend(shown.map(|(n, help)| wrapped(&format!("{n}: {help}"), width)));
    }
    lines.join("\n")
}

fn exec(sh: &Shared, at: &At<'_, '_>, line: &str) -> Result<Exec, String> {
    let (cmd, rest) = word(line);
    match cmd {
        "run" => run(sh, at, rest),
        "step" => match rest {
            "" => step(sh, Next::Resume(Resume::StepIn)),
            "delta" => step(sh, Next::Run(Run::Delta)),
            "timestep" => step(sh, Next::Run(Run::Timestep)),
            _ => Err(bad_sub("step", rest, &["delta", "timestep"])),
        },
        "next" => step(sh, Next::Resume(Resume::StepOver)),
        "stop" => Ok(Exec::Text("the simulation is stopped".into())),
        "examine" | "print" if rest.contains('@') => {
            tools::examine_at(sh, at, rest).map(Exec::Text)
        }
        "examine" | "print" => tools::examine_path(sh, at, rest)
            .unwrap_or_else(|| examine(sh, at, rest))
            .map(Exec::Text),
        "drivers" => tools::drivers(sh, at, rest).map(Exec::Text),
        "describe" => tools::describe(sh, at, rest).map(Exec::Text),
        "find" => tools::find(sh, at, rest).map(Exec::Text),
        "waits" => Ok(Exec::Text(tools::waits(sh, at))),
        "trace" => tools::trace_x(sh, at, rest).map(Exec::Text),
        "activity" => Ok(Exec::Text(tools::activity(sh))),
        "asserts" => Ok(Exec::Text(tools::asserts(sh))),
        "force" | "deposit" | "release" => change(sh, at, cmd, rest).map(Exec::Text),
        "where" => Ok(Exec::Text(where_text(sh, at))),
        "break" => break_cmd(sh, at, rest).map(Exec::Text),
        "restart" | "reset" => restart(sh),
        "wave" => wave(sh, at, rest).map(Exec::Text),
        "source" => {
            let text =
                std::fs::read_to_string(rest).map_err(|e| format!("cannot read `{rest}`: {e}"))?;
            Ok(Exec::Script(text.lines().flat_map(split).collect()))
        }
        "help" => Ok(Exec::Text(help(sh))),
        "quit" => quit(sh, rest),
        _ => {
            if let Some(h) = sh.link.hook.get()
                && h.commands.iter().any(|(n, _)| n == cmd)
            {
                return Ok(Exec::Call(cmd.to_string(), rest.to_string()));
            }
            if cmd.starts_with("vunit_") && sh.link.hook.get().is_none() {
                return Err(format!(
                    "`{cmd}` is only available through the test runner interface: start the test \
                     with `python run.py lib.tb.test --yavs-interactive` (vunit-yavs), not with \
                     `yavs run --interactive`"
                ));
            }
            examine(sh, at, line).map(Exec::Text).map_err(|_| {
                let help = did_you_mean(sh, cmd).unwrap_or_else(|| "see `help`".to_string());
                format!("unknown command or signal `{cmd}`: {help}")
            })
        }
    }
}

/// The console's `wave` commands.
fn wave(sh: &Shared, at: &At<'_, '_>, rest: &str) -> Result<String, String> {
    let Some(v) = sh.link.viewer.get() else {
        return Err("`wave` needs a viewer: run with `--surfer`".into());
    };
    let (sub, mut args) = word(rest);
    match sub {
        "add" => {
            let mut items = Vec::new();
            while !args.is_empty() {
                let (name, more) = name_word(args);
                if name.contains('*') || name.contains('?') {
                    let found = tools::matching_signals(sh, name);
                    if found.is_empty() {
                        return tools::find(sh, at, &format!("signals {name}"));
                    }
                    items.extend(found.into_iter().map(|k| dotted(&sh.design.sigs[k].path)));
                } else {
                    items.push(viewer_name(sh, at, name)?);
                }
                args = more;
            }
            if items.is_empty() {
                return Err("`wave add` needs a signal or scope".into());
            }
            Ok(format!("{} added", v.add(items)?))
        }
        "cursor" => {
            let t = if args.is_empty() {
                at.now().ok_or("no simulation time at this stop")?
            } else {
                yavs_std::value::time_literal(args)
                    .map_err(|e| e.to_string())
                    .and_then(|fs| Time::from_vhdl(fs).map_err(|e| e.to_string()))
                    .map_err(|e| format!("`{args}` is not a time: {e}"))?
            };
            v.cursor(t.as_fs());
            Ok(String::new())
        }
        "save" => {
            let file = layout_file(sh, args)?;
            let n = crate::layout::save(&file, &v.items()?)?;
            Ok(format!("{n} signals saved to `{}`", file.display()))
        }
        "load" => {
            let file = layout_file(sh, args)?;
            let (items, _) = crate::layout::load(&file, &|_| true)?;
            // One by one: a list from another run may name what this one does not record.
            let (mut n, mut gone) = (0, Vec::new());
            for i in items {
                match v.add(vec![i.clone()]) {
                    Ok(k) => n += k,
                    Err(_) => gone.push(i),
                }
            }
            let mut o = format!("{n} added from `{}`", file.display());
            for m in gone {
                o += &format!("\nnot in this design: {m}");
            }
            Ok(o)
        }
        _ => Err(bad_sub("wave", sub, &["add", "cursor", "save", "load"])),
    }
}

/// The file of `wave save` and `wave load`: the one named, else this testbench's.
fn layout_file(sh: &Shared, name: &str) -> Result<std::path::PathBuf, String> {
    if !name.is_empty() {
        return Ok(name.into());
    }
    sh.link
        .layout
        .clone()
        .ok_or_else(|| "this run has no work library to keep a layout in: name a file".to_string())
}

/// What the console means by `name`, as a dotted path: a signal or a part of one, a variable of
/// the stopped process, a scope, or a path. The viewer checks it against what the file holds.
fn viewer_name(sh: &Shared, at: &At<'_, '_>, name: &str) -> Result<String, String> {
    let tid = thread_of(at);
    let proc = thread_proc(sh, tid);
    if let At::Process(s) = at {
        let end = name.find(['.', '(']).unwrap_or(name.len());
        let (head, part) = name.split_at(end);
        let named = |i: usize| {
            s.frames
                .locals(i)
                .iter()
                .any(|l| l.name.eq_ignore_ascii_case(head))
        };
        let top = s.frames.depth().saturating_sub(1);
        if top > 0
            && (named(top)
                || state_vars(sh, s, at, top)
                    .iter()
                    .any(|(n, ..)| n.eq_ignore_ascii_case(head)))
        {
            return Err(format!(
                "`{head}` is a local of a subprogram, which exists only during a call: only signals, \
                 process variables and shared variables are recorded"
            ));
        }
        if named(0)
            && let Some(p) = crate::proc_name(sh, tid)
        {
            return Ok(format!("{}.{head}{part}", dotted(&p)));
        }
    }
    if let Some((k, part)) = split_part(sh, proc, &qualify(sh, at, name)) {
        return Ok(format!("{}{part}", dotted(&sh.design.sigs[k].path)));
    }
    if !name.contains(['.', ':'])
        && let Some(&t) = sh.design.roots.first()
    {
        return Ok(format!("{}.{name}", dotted(&sh.design.regions[t].path)));
    }
    Ok(name.trim_matches(':').replace(':', "."))
}

/// A design path (`:top:u1:s`) as the viewer writes it.
pub(crate) fn dotted(path: &str) -> String {
    path.trim_matches(':').replace(':', ".")
}

/// Tell the viewer of a stop: it reloads, moves its cursor, marks a breakpoint and colours the
/// stopped process's signals.
pub(crate) fn tell_viewer(sh: &Shared, at: &At<'_, '_>, stopped: &Json) {
    let Some(v) = sh.link.viewer.get() else {
        return;
    };
    let reason = stopped["reason"].as_str().unwrap_or_default();
    let proc = thread_proc(sh, thread_of(at));
    v.stop(crate::ViewStop {
        now_fs: at.now().map(|t| t.as_fs()),
        breakpoint: matches!(reason, "breakpoint" | "data breakpoint"),
        signals: proc
            .into_iter()
            .flat_map(|p| &p.signals)
            .map(|&k| dotted(&sh.design.sigs[k].path))
            .collect(),
    });
}

/// Take the viewer of the session, once. Its "go to declaration" prints the place and asks an
/// editor to open it (`yavs/showSource`).
pub(crate) fn set_viewer(link: &std::sync::Arc<Link>, v: std::sync::Arc<dyn crate::Viewer>) {
    if link.viewer.set(v.clone()).is_err() {
        return;
    }
    let link = std::sync::Arc::downgrade(link);
    v.on_goto(Box::new(move |name| {
        let Some(link) = link.upgrade() else {
            return;
        };
        let sh = link
            .current
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let Some(sh) = sh else {
            return;
        };
        // A field or an element in the viewer is declared with its signal.
        let decl = std::iter::successors(Some(name), |n| n.rsplit_once('.').map(|(p, _)| p))
            .find_map(|n| sh.design.find(None, n))
            .and_then(|k| sh.design.sigs[k].decl.clone());
        if let Some((path, line)) = decl {
            link.output("console", &format!("{name}: {path}:{line}\n"));
            link.event("yavs/showSource", json!({"path": path, "line": line}));
        }
    }));
}

fn quit(sh: &Shared, rest: &str) -> Result<Exec, String> {
    let status = match rest {
        "" => (!sh.ended.load(Ordering::Relaxed)).then_some(0),
        s => Some(
            s.parse::<u8>()
                .map_err(|_| format!("`quit {s}`: expected a status 0..255"))?,
        ),
    };
    state(&sh.link).exit = status;
    sh.terminating.store(true, Ordering::Relaxed);
    sh.discard.store(true, Ordering::Relaxed);
    if let Some(i) = sh.interrupt.get() {
        i.stop();
    }
    Ok(Exec::Next(Next::Quit))
}

/// A test runner's connection: it serves the commands it registered.
pub(crate) struct Hook {
    w: Mutex<TcpStream>,
    pub(crate) commands: Vec<(String, String)>,
    next: AtomicI64,
    gone: AtomicBool,
}

impl Hook {
    /// Connect to the hook in `value` and read what it registers; the error is a note for E0703.
    pub(crate) fn connect(value: &str) -> Result<(Hook, BufReader<TcpStream>), String> {
        // The secret is never printed.
        let (port, secret) = value
            .split_once(':')
            .and_then(|(p, s)| Some((p.parse::<u16>().ok().filter(|&p| p != 0)?, s)))
            .filter(|(_, s)| !s.is_empty())
            .ok_or_else(|| format!("`{HOOK_VAR}` is not `<port>:<secret>`"))?;
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let fail = |e: std::io::Error| format!("the hook on {addr}: {e}");
        let s = TcpStream::connect_timeout(&addr, HOOK_TIMEOUT).map_err(fail)?;
        let _ = s.set_nodelay(true);
        let mut w = s.try_clone().map_err(fail)?;
        writeln!(w, "{}", json!({"hello": 1, "secret": secret})).map_err(fail)?;
        s.set_read_timeout(Some(HOOK_TIMEOUT)).map_err(fail)?;
        let mut r = BufReader::new(s);
        let mut line = String::new();
        r.read_line(&mut line).map_err(fail)?;
        r.get_ref().set_read_timeout(None).map_err(fail)?;
        let m: Json = serde_json::from_str(&line)
            .map_err(|_| format!("the hook on {addr} answered no command list"))?;
        let list = m["commands"]
            .as_array()
            .ok_or_else(|| format!("the hook on {addr} answered no command list"))?;
        let mut commands = Vec::new();
        for c in list {
            let name = c["name"].as_str().unwrap_or_default().to_string();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(format!("the hook registers a bad command name `{name}`"));
            }
            if COMMANDS.iter().any(|(n, ..)| *n == name) {
                return Err(format!("the hook registers `{name}`, a yavs command"));
            }
            commands.push((name, c["help"].as_str().unwrap_or_default().to_string()));
        }
        let hook = Hook {
            w: Mutex::new(w),
            commands,
            next: AtomicI64::new(1),
            gone: AtomicBool::new(false),
        };
        Ok((hook, r))
    }

    fn send(&self, m: &Json) {
        let mut w = self.w.lock().unwrap_or_else(|p| p.into_inner());
        // A closed connection is seen by the reader thread (`hook_gone`).
        let _ = writeln!(w, "{m}");
    }

    fn call(&self, name: &str, args: &str) -> Result<i64, String> {
        if self.gone.load(Ordering::Relaxed) {
            return Err(format!("`{name}`: the test runner closed its connection"));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.send(&json!({"call": id, "name": name, "args": args}));
        Ok(id)
    }
}

/// The reader thread of the hook: `print` at once (it can come at any time), the rest at a stop.
pub(crate) fn hook_reader(mut r: BufReader<TcpStream>, link: &Link, tx: &Sender<Msg>) {
    let mut line = String::new();
    loop {
        line.clear();
        match r.read_line(&mut line) {
            Ok(n) if n > 0 => {}
            _ => break,
        }
        let Ok(m) = serde_json::from_str::<Json>(&line) else {
            continue;
        };
        match m["print"].as_str() {
            Some(t) => link.output("console", t),
            None => {
                let _ = tx.send(Msg::Hook(m));
            }
        }
    }
    let _ = tx.send(Msg::HookGone);
}

/// A runner's message at a stop.
pub(crate) fn on_hook(sh: &Shared, at: &At<'_, '_>, m: Json) -> Option<Next> {
    let link = &*sh.link;
    let id = m["exec"]
        .as_i64()
        .or_else(|| m["reload"].as_i64())
        .or_else(|| m["done"].as_i64());
    let id = id?;
    if m.get("exec").is_some() {
        let lines = split(m["line"].as_str().unwrap_or_default()).into();
        push(link, Origin::Hook(id), lines);
        return None;
    }
    if m.get("reload").is_some() {
        return reload(sh, at, id, &m);
    }
    let mut st = state(link);
    let b = st
        .batches
        .iter_mut()
        .rev()
        .find(|b| b.awaiting == Some(id))?;
    b.awaiting = None;
    b.say(link, m["result"].as_str().unwrap_or_default());
    if let Some(e) = m["error"].as_str() {
        b.error = Some(e.to_string());
        b.lines.clear();
    }
    None
}

/// `reload`: elaborate first, so a failure leaves everything as it is.
fn reload(sh: &Shared, _at: &At<'_, '_>, id: i64, m: &Json) -> Option<Next> {
    let link = &*sh.link;
    let refuse = |why: &str| {
        if let Some(h) = link.hook.get() {
            h.send(&json!({"result": id, "ok": false, "text": "", "error": why}));
        }
    };
    if let Some(why) = &link.no_restart {
        refuse(why);
        return None;
    }
    let args: Vec<String> = m["args"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a.as_str().map(String::from))
        .collect();
    let mut guard = link.reload.lock().unwrap_or_else(|p| p.into_inner());
    let Some(f) = guard.as_mut() else {
        refuse("this session cannot reload");
        return None;
    };
    // On its own thread: this one is inside a stopped process.
    let r = std::thread::scope(|s| {
        let t = std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn_scoped(s, || f(&args));
        match t {
            Ok(t) => t
                .join()
                .unwrap_or_else(|_| Err("this is a bug in yavs: the reload panicked".into())),
            Err(e) => Err(e.to_string()),
        }
    });
    drop(guard);
    match r {
        Err(why) => {
            refuse(&why);
            None
        }
        Ok(new) => {
            *sh.restart.lock().unwrap_or_else(|p| p.into_inner()) = Some(Some(new));
            push(link, Origin::Hook(id), VecDeque::new());
            state(link).told = false;
            Some(Next::Restart)
        }
    }
}

/// The runner's connection closed: what waits for it fails.
pub(crate) fn hook_gone(sh: &Shared) {
    if let Some(h) = sh.link.hook.get() {
        h.gone.store(true, Ordering::Relaxed);
    }
    for b in &mut state(&sh.link).batches {
        if b.awaiting.take().is_some() {
            b.error = Some("the test runner closed its connection".into());
            b.lines.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::split;

    #[test]
    fn split_keeps_flags_and_ends_at_comments() {
        assert_eq!(split("run 10 ns; where"), ["run 10 ns", "where"]);
        assert_eq!(split("run -- to the end"), ["run"]);
        assert_eq!(split("-- only a comment"), Vec::<String>::new());
        assert_eq!(split("--comment"), Vec::<String>::new());
        assert_eq!(split("run; --comment"), ["run"]);
        assert_eq!(
            split("vunit_load --stop-time=30ns --no-such-flag"),
            ["vunit_load --stop-time=30ns --no-such-flag"]
        );
        assert_eq!(split("examine a--1"), ["examine a"]);
    }
}
