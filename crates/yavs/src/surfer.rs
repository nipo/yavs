//! `run --surfer[=ADDR]`: shows the `--wave` file in the Surfer waveform viewer while the run
//! writes it, through Surfer's Waveform Control Protocol (WCP, #876; `docs/cli-contract.md` 5).
//! JSON messages ended by a NUL byte over TCP; the client greets first.

use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use yavs_diag::Diagnostic;
use yavs_wave::{HierItem, Hierarchy, ScopeKind};

/// How long Surfer may take to start, connect and load the first file.
const WAIT: Duration = Duration::from_secs(30);

/// How long a running Surfer may take to answer before it counts as stalled, and how many of
/// these a load may take. `YAVS_SURFER_WAIT` (ms) shortens it for the tests.
const ANSWER_WAIT: Duration = Duration::from_secs(5);
const LOAD_WAITS: u32 = 6;

/// A window that is not drawn stalls Surfer: `docs/decisions.md` "Surfer and a hidden window".
const WHY_STALLED: &str = "Surfer handles commands only while it draws its window, and a covered or\n  \
                           minimized window may not be drawn: bring it to the front or keep it\n  \
                           beside this one";

fn answer_wait() -> Result<Duration, String> {
    let Ok(v) = std::env::var("YAVS_SURFER_WAIT") else {
        return Ok(ANSWER_WAIT);
    };
    match v.parse() {
        Ok(ms) if ms > 0 => Ok(Duration::from_millis(ms)),
        _ => Err(format!("`{v}` is not a number of milliseconds above zero")),
    }
}

fn stalled_text(wait: Duration) -> String {
    format!(
        "Surfer has not answered for {} s, so the view is behind. The simulation goes on.\n  \
         {WHY_STALLED}, and the view catches up by itself.",
        wait.as_secs_f64()
    )
}

fn not_added_text(items: &[String], wait: Duration) -> String {
    format!(
        "Surfer did not answer within {} s, so `{}` is not in its view yet: it is added as soon \
         as Surfer answers.\n  {WHY_STALLED}.",
        wait.as_secs_f64(),
        items.join("`, `")
    )
}

fn no_list_text(wait: Duration) -> String {
    format!(
        "Surfer did not answer within {} s, so what its view shows is not known.\n  \
         {WHY_STALLED}, and try again.",
        wait.as_secs_f64()
    )
}

fn gone_text(cause: &str) -> String {
    format!(
        "The connection to Surfer is gone ({cause}), so the view no longer follows this run.\n  \
         Surfer was closed or crashed. The waveform file is still written; run again with\n  \
         `--surfer` to watch it."
    )
}

/// A read timeout is a stall and a refusal concerns one command; anything else ends the connection.
fn is_gone(e: &io::Error) -> bool {
    !matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::Other)
}

/// Says it now: in the session's console if there is one, else on stderr.
fn tell(notice: &OnceLock<yavs_dap::NoticeFn>, text: &str) {
    match notice.get() {
        Some(f) => f(text),
        None => {
            crate::stdout::flush();
            eprintln!("{text}");
        }
    }
}

/// At most this many signals are added on their own (`wave add` adds what it is told).
const MAX_SHOWN: usize = 200;

/// What the run tells the viewer thread.
pub enum Msg {
    /// A debugger stop: the file was just synced.
    Stop(yavs_dap::ViewStop),
    /// A console's `wave add`: the items, whether each is a scope in the file, and where the
    /// answer goes.
    Add(Vec<(String, bool)>, Sender<Result<usize, String>>),
    /// A console's `wave cursor`, in fs.
    Cursor(u64),
    /// What the view shows, for `wave save`.
    Items(Sender<Result<Vec<String>, String>>),
    /// The run starts over and the file with it: its size with no values yet.
    Restart(Option<u64>),
}

/// A connected Surfer, driven by its own thread so the simulation never waits for it.
pub struct Surfer {
    tx: Mutex<Option<Sender<Msg>>>,
    thread: Mutex<Option<JoinHandle<io::Result<()>>>>,
    /// Where Surfer's "go to declaration" goes (`yavs_dap::Viewer::on_goto`).
    goto: Arc<OnceLock<yavs_dap::GotoFn>>,
    /// Where stalls and the loss of the connection are told (`yavs_dap::Viewer::on_notice`).
    notice: Arc<OnceLock<yavs_dap::NoticeFn>>,
    /// Why the connection ended, once it has.
    gone: Arc<OnceLock<String>>,
    wait: Duration,
    file: PathBuf,
    /// What the file holds (`recorded`).
    recorded: HashMap<String, (String, bool)>,
}

/// The names of `recorded` that are variables, in a fixed order.
fn variables(recorded: &HashMap<String, (String, bool)>) -> Vec<String> {
    let mut v: Vec<String> = recorded
        .iter()
        .filter(|(_, x)| !x.1)
        .map(|(k, _)| k.clone())
        .collect();
    v.sort_unstable();
    v
}

impl Surfer {
    /// Connects to the Surfer at `addr`, or starts `surfer` from PATH when `None`, loads `file`
    /// and adds `names`; then reloads every `every` while the file grows. An error is E0130.
    pub fn start(
        addr: Option<&str>,
        file: &Path,
        names: Vec<String>,
        recorded: HashMap<String, (String, bool)>,
        every: Option<Duration>,
        layout: Option<PathBuf>,
    ) -> Result<Arc<Self>, Box<Diagnostic>> {
        let fail = |m: &str, note: String, help: &str| {
            Box::new(
                Diagnostic::error("E0130", m.to_string())
                    .with_note(note)
                    .with_help(help.to_string()),
            )
        };
        let wait = answer_wait().map_err(|e| {
            fail(
                "`YAVS_SURFER_WAIT` cannot be used",
                e,
                "give it in milliseconds, such as `2000`, or unset it",
            )
        })?;
        let listen_help = "start Surfer 0.6.0 or later, run its command `wcp_server_start` (or set \
                           `[wcp] autostart = true` in its config) and pass its address, by default \
                           `--surfer=127.0.0.1:54321`";
        let stream = match addr {
            Some(a) => connect(a).map_err(|e| {
                fail(
                    &format!("cannot connect to Surfer at `{a}`"),
                    e.to_string(),
                    listen_help,
                )
            })?,
            None => spawn().map_err(|e| {
                fail(
                    "cannot start Surfer",
                    e.to_string(),
                    "install Surfer 0.6.0 or later so that `surfer` is on PATH, or start it \
                     yourself and pass `--surfer=HOST:PORT`",
                )
            })?,
        };
        let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
        let goto: Arc<OnceLock<yavs_dap::GotoFn>> = Arc::default();
        let mut w = Wcp::open(stream, goto.clone())
            .and_then(|mut w| {
                w.command("load", json!({"source": file}))?;
                w.wait_loaded(1)?;
                Ok(w)
            })
            .map_err(|e| {
                fail(
                    "Surfer did not take the waveform file",
                    e.to_string(),
                    "use Surfer 0.6.0 or later and keep its window in view: it handles commands \
                     only while it draws; its own log says more",
                )
            })?;
        let (tx, rx) = channel();
        // Before the run: the thread comes to it when the run may have written.
        let empty = std::fs::metadata(&file).map(|m| m.len()).ok();
        let path = file.clone();
        let known = variables(&recorded);
        w.wait = wait;
        let notice: Arc<OnceLock<yavs_dap::NoticeFn>> = Arc::default();
        let gone: Arc<OnceLock<String>> = Arc::default();
        let (told, cause) = (notice.clone(), gone.clone());
        let thread = std::thread::spawn(move || {
            let r = drive(
                &mut w,
                &rx,
                &path,
                every,
                names,
                empty,
                &known,
                layout.as_deref(),
                &told,
            );
            if let Some(e) = r.as_ref().err().filter(|e| is_gone(e)) {
                let text = gone_text(&e.to_string());
                tell(&told, &text);
                let _ = cause.set(text);
            }
            r
        });
        Ok(Arc::new(Self {
            tx: Mutex::new(Some(tx)),
            thread: Mutex::new(Some(thread)),
            goto,
            notice,
            gone,
            wait,
            file,
            recorded,
        }))
    }

    fn send(&self, m: Msg) {
        if let Some(tx) = &*self.tx.lock().unwrap_or_else(|p| p.into_inner()) {
            // The thread has ended: `gone` says why.
            let _ = tx.send(m);
        }
    }

    fn size(&self) -> Option<u64> {
        std::fs::metadata(&self.file).map(|m| m.len()).ok()
    }

    fn gone_text(&self) -> String {
        self.gone
            .get()
            .cloned()
            .unwrap_or_else(|| gone_text("the viewer thread ended"))
    }

    /// The run starts over: the file is new.
    pub fn restart(&self) {
        self.send(Msg::Restart(self.size()));
    }

    /// After the file is finished: reload it and show the whole run. A failure at any time of
    /// the run is warning W0131; the run's result never depends on the viewer.
    pub fn end(&self) -> Option<Diagnostic> {
        self.tx.lock().unwrap_or_else(|p| p.into_inner()).take();
        let t = self
            .thread
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()?;
        let err = match t.join() {
            Ok(Ok(())) => return None,
            Ok(Err(e)) => e,
            Err(_) => io::Error::other("the viewer thread panicked"),
        };
        if !is_gone(&err) {
            return Some(
                Diagnostic::warning("W0131", "Surfer did not answer at the end of the run")
                    .with_note(err.to_string())
                    .with_note(
                        "the simulation was not affected; Surfer is asked to read the finished \
                         file and does so when it draws again, but the signal list of this \
                         session was not saved",
                    )
                    .with_help(WHY_STALLED),
            );
        }
        Some(
            Diagnostic::warning("W0131", "lost the connection to Surfer")
                .with_note(err.to_string())
                .with_note(
                    "the simulation was not affected; Surfer shows the run up to the failure",
                )
                .with_help("Surfer was closed or crashed: run again with `--surfer` to watch"),
        )
    }
}

impl yavs_dap::Viewer for Surfer {
    fn stop(&self, s: yavs_dap::ViewStop) {
        self.send(Msg::Stop(s));
    }

    fn add(&self, items: Vec<String>) -> Result<usize, yavs_dap::AddError> {
        let mut found = Vec::with_capacity(items.len());
        for item in &items {
            found.push(self.recorded.get(&item.to_ascii_lowercase()).cloned().ok_or_else(|| {
                let mut m = format!(
                    "`{item}` is not in the waveform file: only signals, process variables and \
                     shared variables are, without what `--wave-include`, `--wave-exclude` or \
                     `--wave-max-array` leave out"
                );
                let keys = self.recorded.keys().map(|k| (k.as_str(), k.as_str()));
                if let Some(h) = yavs_sem::suggest::suggest(item, "signal or variable", keys) {
                    m += &format!(": {h}");
                }
                yavs_dap::AddError::NotRecorded(m)
            })?);
        }
        let (tx, rx) = channel();
        self.send(Msg::Add(found, tx));
        // Short: the debugger waits for this. What is too late is added when Surfer answers.
        let r = rx.recv_timeout(self.wait + Duration::from_secs(1));
        match r {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => Err(not_added_text(&items, self.wait)),
            Err(RecvTimeoutError::Disconnected) => Err(self.gone_text()),
        }
        .map_err(yavs_dap::AddError::Viewer)
    }

    fn items(&self) -> Result<Vec<String>, String> {
        let (tx, rx) = channel();
        self.send(Msg::Items(tx));
        let r = rx.recv_timeout(self.wait + self.wait + Duration::from_secs(1));
        match r {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => Err(no_list_text(self.wait)),
            Err(RecvTimeoutError::Disconnected) => Err(self.gone_text()),
        }
    }

    fn cursor(&self, fs: u64) {
        self.send(Msg::Cursor(fs));
    }

    fn on_goto(&self, f: yavs_dap::GotoFn) {
        let _ = self.goto.set(f);
    }

    fn on_notice(&self, f: yavs_dap::NoticeFn) {
        let _ = self.notice.set(f);
    }
}

/// The signals to show on their own: those in the top scope, with the records and arrays
/// directly in it (scopes in the file) one level down, or every recorded one when `all` (the
/// `--wave-include` globs chose them); at most [`MAX_SHOWN`] and how many were left out. Names as
/// Surfer reads them from the file.
pub fn names(h: &Hierarchy, all: bool) -> (Vec<String>, usize) {
    let (mut path, mut out) = (Vec::new(), Vec::new());
    // Below the top scope, whether each open scope is a composite of the object.
    let mut composite = Vec::new();
    for it in h.items() {
        match it {
            HierItem::Scope { name, kind, .. } => {
                path.push(name);
                composite.push(matches!(kind, ScopeKind::Record | ScopeKind::Array));
            }
            HierItem::Up => {
                path.pop();
                composite.pop();
            }
            HierItem::Signal { name, .. }
                if all
                    || path.len() == 1
                    || (path.len() == 2 && composite.last() == Some(&true)) =>
            {
                // Vector bounds in the name (`data[7:0]`) are not part of the viewer's name.
                let name = match name.rfind('[') {
                    Some(i) if name.ends_with(']') => &name[..i],
                    _ => name,
                };
                // The viewer's file reader takes an array's name off its elements: `m(0)` in
                // scope `m` is `tb.m.(0)` there.
                let name = match (composite.last(), path.last()) {
                    (Some(true), Some(p)) => name.strip_prefix(p).unwrap_or(name),
                    _ => name,
                };
                out.push(format!("{}.{name}", path.join(".")));
            }
            HierItem::Signal { .. } => {}
        }
    }
    let over = out.len().saturating_sub(MAX_SHOWN);
    out.truncate(MAX_SHOWN);
    (out, over)
}

/// Everything in the file by lower-case name: the name Surfer reads it under, and whether it is a
/// scope. An element of an array has two names: `tb.m(0)` as VHDL writes it, `tb.m.(0)` as the
/// viewer's file reader does (it takes the array's name off its elements).
pub fn recorded(h: &Hierarchy) -> HashMap<String, (String, bool)> {
    let mut out = HashMap::new();
    // Per open scope: its name, the path Surfer knows, the path as VHDL writes it.
    let mut open: Vec<(&str, String, String, bool)> = Vec::new();
    let mut add = |exact: String, vhdl: String, scope: bool| {
        out.insert(vhdl.to_ascii_lowercase(), (exact.clone(), scope));
        out.insert(exact.to_ascii_lowercase(), (exact, scope));
    };
    for it in h.items() {
        match it {
            HierItem::Scope { name, kind, .. } => {
                let (exact, vhdl) = match open.last() {
                    None => (name.to_string(), name.to_string()),
                    Some((p, e, v, array)) => {
                        let tail = name.strip_prefix(*p).filter(|_| *array);
                        (
                            format!("{e}.{name}"),
                            tail.map_or_else(|| format!("{v}.{name}"), |t| format!("{v}{t}")),
                        )
                    }
                };
                add(exact.clone(), vhdl.clone(), true);
                let array = matches!(kind, ScopeKind::Array);
                open.push((name, exact, vhdl, array));
            }
            HierItem::Up => {
                open.pop();
            }
            HierItem::Signal { name, .. } => {
                let Some((p, e, v, array)) = open.last() else {
                    continue;
                };
                let name = match name.rfind('[') {
                    Some(i) if name.ends_with(']') => &name[..i],
                    _ => name,
                };
                let tail = name.strip_prefix(*p).filter(|_| *array);
                match tail {
                    Some(t) => add(format!("{e}.{t}"), format!("{v}{t}"), false),
                    None => add(format!("{e}.{name}"), format!("{v}.{name}"), false),
                }
            }
        }
    }
    out
}

fn connect(addr: &str) -> io::Result<TcpStream> {
    let mut last = io::Error::new(ErrorKind::InvalidInput, "the address names no host");
    for a in addr.to_socket_addrs()? {
        match TcpStream::connect_timeout(&a, Duration::from_secs(5)) {
            Ok(s) => return Ok(s),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Starts `surfer --wcp-initiate PORT`, which connects back to a port yavs listens on.
fn spawn() -> io::Result<TcpStream> {
    let l = TcpListener::bind(("127.0.0.1", 0))?;
    let port = l.local_addr()?.port();
    let mut child = Command::new("surfer")
        .args(["--wcp-initiate", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => io::Error::new(e.kind(), "`surfer` is not on PATH"),
            _ => io::Error::new(e.kind(), format!("`surfer`: {e}")),
        })?;
    l.set_nonblocking(true)?;
    let t0 = Instant::now();
    loop {
        match l.accept() {
            Ok((s, _)) => {
                s.set_nonblocking(false)?;
                return Ok(s);
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        if let Some(st) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "`surfer` ended ({st}) before it connected"
            )));
        }
        if t0.elapsed() > WAIT {
            return Err(io::Error::new(
                ErrorKind::TimedOut,
                "`surfer` did not connect within 30 s",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The recorded name that the name Surfer displays (`cnt [3:0]`, `…u1.clk`) stands for, if one.
fn shown_name(display: &str, known: &[String]) -> Option<String> {
    let name = display.split(" [").next()?.trim_start_matches('…');
    let key = name.to_ascii_lowercase();
    let tail = format!(".{}", key.trim_start_matches('.'));
    let mut hits = known.iter().filter(|k| **k == key || k.ends_with(&tail));
    let first = hits.next()?;
    hits.next().is_none().then(|| first.clone())
}

/// One WCP connection: messages read by a thread of their own, `waveforms_loaded` counted, and
/// the displayed items by name.
struct Wcp {
    w: TcpStream,
    rx: Receiver<Json>,
    loaded: u64,
    /// How long `command` waits.
    wait: Duration,
    /// The commands whose answer stopped being waited for, oldest first, with the names of what
    /// an `add_items` added. Surfer answers each command once and in order, so the next answer
    /// is the first of these, however late, and never the answer to a later command.
    late: VecDeque<Option<Vec<String>>>,
    /// When the oldest of `late` was sent.
    late_since: Instant,
    shown: HashMap<String, Json>,
    /// The items coloured for the last stop.
    coloured: Vec<Json>,
    goto: Arc<OnceLock<yavs_dap::GotoFn>>,
}

/// The colour of the stopped process's signals, and of the others after it.
const STOPPED_COLOUR: &str = "Yellow";
// ponytail: WCP cannot give an item its default colour back; this is the default theme's.
const PLAIN_COLOUR: &str = "Green";

impl Wcp {
    fn open(w: TcpStream, goto: Arc<OnceLock<yavs_dap::GotoFn>>) -> io::Result<Self> {
        w.set_write_timeout(Some(WAIT))?;
        let r = w.try_clone()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(r);
            loop {
                let mut b = Vec::new();
                match r.read_until(0, &mut b) {
                    Ok(_) if b.pop() == Some(0) => {}
                    _ => break,
                }
                let Ok(m) = serde_json::from_slice(&b) else {
                    break;
                };
                if tx.send(m).is_err() {
                    break;
                }
            }
        });
        let mut s = Self {
            w,
            rx,
            loaded: 0,
            wait: WAIT,
            late: VecDeque::new(),
            late_since: Instant::now(),
            shown: HashMap::new(),
            coloured: Vec::new(),
            goto,
        };
        s.send(json!({
            "type": "greeting",
            "version": "0",
            "commands": ["waveforms_loaded", "goto_declaration"],
        }))?;
        let g = s.next(Instant::now() + WAIT)?;
        if g["type"] != "greeting" {
            return Err(io::Error::other(format!(
                "Surfer answered the greeting with {g}"
            )));
        }
        Ok(s)
    }

    fn send(&mut self, m: Json) -> io::Result<()> {
        let mut b = m.to_string().into_bytes();
        b.push(0);
        // A write that stops halfway leaves half a frame: nothing can follow it.
        self.w.write_all(&b).map_err(|e| {
            let what = format!("cannot write to Surfer: {e}");
            io::Error::new(ErrorKind::ConnectionAborted, what)
        })
    }

    /// The next message, by `until`.
    fn next(&mut self, until: Instant) -> io::Result<Json> {
        let left = until.saturating_duration_since(Instant::now());
        let m = self.rx.recv_timeout(left).map_err(|e| match e {
            RecvTimeoutError::Timeout => io::Error::new(
                ErrorKind::TimedOut,
                format!("Surfer did not answer within {} s", self.wait.as_secs_f64()),
            ),
            RecvTimeoutError::Disconnected => closed(),
        })?;
        self.on_event(&m);
        Ok(m)
    }

    /// An answer came: true if it was a late one, which is now dealt with.
    fn take_late(&mut self, r: &Json) -> bool {
        let Some(names) = self.late.pop_front() else {
            return false;
        };
        self.late_since = Instant::now();
        let ids = r["ids"].as_array();
        if let (Some(names), Some(ids)) = (names, ids)
            && names.len() == ids.len()
        {
            self.shown
                .extend(names.into_iter().zip(ids.iter().cloned()));
        }
        true
    }

    fn on_event(&mut self, m: &Json) {
        if m["type"] != "event" {
            return;
        }
        if m["event"] == "waveforms_loaded" {
            self.loaded += 1;
        }
        if m["event"] == "goto_declaration"
            && let (Some(f), Some(v)) = (self.goto.get(), m["variable"].as_str())
        {
            f(v);
        }
    }

    /// Add items to the view, remembering their ids; how many were added. A scope in `add_items`
    /// is also looked for as a variable, and Surfer opens its log window on that failure.
    fn add(&mut self, items: &[(String, bool)]) -> io::Result<usize> {
        let (scopes, vars): (Vec<_>, Vec<_>) = items.iter().partition(|(_, scope)| *scope);
        let vars: Vec<&String> = vars.iter().map(|(n, _)| n).collect();
        let mut n = 0;
        if !vars.is_empty() {
            let lower = vars.iter().map(|n| n.to_ascii_lowercase()).collect();
            let r = self.command_for("add_items", json!({"items": vars}), Some(lower))?;
            let ids = r["ids"].as_array().cloned().unwrap_or_default();
            n += ids.len();
            if ids.len() == vars.len() {
                let names = vars.iter().map(|n| n.to_ascii_lowercase());
                self.shown.extend(names.zip(ids));
            }
        }
        for (name, _) in scopes {
            let r = self.command("add_scope", json!({"scope": name, "recursive": false}))?;
            n += r["ids"].as_array().map_or(0, Vec::len);
        }
        Ok(n)
    }

    /// The stopped process's signals in colour, the last stop's back.
    fn colour(&mut self, signals: &[String]) -> io::Result<()> {
        for id in std::mem::take(&mut self.coloured) {
            self.colour_item(id, PLAIN_COLOUR)?;
        }
        // A composite is shown as its elements.
        let ids: Vec<Json> = self
            .shown
            .iter()
            .filter(|(k, _)| {
                signals.iter().any(|n| {
                    k.strip_prefix(n.as_str())
                        .is_some_and(|r| r.is_empty() || r.starts_with('.'))
                })
            })
            .map(|(_, id)| id.clone())
            .collect();
        for id in ids {
            self.colour_item(id.clone(), STOPPED_COLOUR)?;
            self.coloured.push(id);
        }
        Ok(())
    }

    /// An item the user removed in Surfer is refused: there is nothing to colour.
    fn colour_item(&mut self, id: Json, colour: &str) -> io::Result<()> {
        match self.command("set_item_color", json!({"id": id, "color": colour})) {
            Err(e) if e.kind() == ErrorKind::Other => Ok(()),
            r => r.map(drop),
        }
    }

    /// The signal list of the view, in order, as dotted names. Surfer gives only the name it
    /// displays: what yavs added it knows by id, the rest it finds by that name in `known`, and
    /// what stays ambiguous is left out.
    fn items(&mut self, known: &[String]) -> io::Result<Vec<String>> {
        let ids = self.command("get_item_list", json!({}))?;
        let ids = ids["ids"].clone();
        let info = self.command("get_item_info", json!({"ids": ids.clone()}))?;
        let list = info["results"].as_array().cloned().unwrap_or_default();
        let ids = ids.as_array().cloned().unwrap_or_default();
        let by_id = |id: &Json| {
            self.shown
                .iter()
                .find(|(_, v)| *v == id)
                .map(|(k, _)| k.clone())
        };
        Ok(list
            .iter()
            .zip(&ids)
            .filter(|(i, _)| i["type"] == "Variable")
            .filter_map(|(i, id)| by_id(id).or_else(|| shown_name(i["name"].as_str()?, known)))
            .collect())
    }

    /// Sends a command and waits for its response, at most `wait`.
    fn command(&mut self, cmd: &str, m: Json) -> io::Result<Json> {
        self.command_for(cmd, m, None)
    }

    fn command_for(
        &mut self,
        cmd: &str,
        mut m: Json,
        names: Option<Vec<String>>,
    ) -> io::Result<Json> {
        m["type"] = json!("command");
        m["command"] = json!(cmd);
        self.send(m)?;
        let sent = Instant::now();
        let until = sent + self.wait;
        loop {
            let r = match self.next(until) {
                Ok(r) => r,
                Err(e) => {
                    if e.kind() == ErrorKind::TimedOut {
                        if self.late.is_empty() {
                            self.late_since = sent;
                        }
                        self.late.push_back(names);
                    }
                    return Err(e);
                }
            };
            if !matches!(r["type"].as_str(), Some("response" | "error")) || self.take_late(&r) {
                continue;
            }
            match r["type"].as_str() {
                Some("response") => return Ok(r),
                _ => {
                    return Err(io::Error::other(format!(
                        "Surfer refused `{cmd}`: {}",
                        r["message"].as_str().unwrap_or_default()
                    )));
                }
            }
        }
    }

    /// Sends a command and does not wait: its answer is a late one.
    fn post(&mut self, cmd: &str) -> io::Result<()> {
        self.send(json!({"type": "command", "command": cmd}))?;
        if self.late.is_empty() {
            self.late_since = Instant::now();
        }
        self.late.push_back(None);
        Ok(())
    }

    /// Waits until the first load is complete, when the connection opens.
    fn wait_loaded(&mut self, n: u64) -> io::Result<()> {
        let until = Instant::now() + self.wait;
        while self.loaded < n {
            self.next(until)?;
        }
        Ok(())
    }

    /// Takes in what arrived, without waiting. An end of the connection shows here, also when
    /// nothing is being asked of Surfer.
    fn poll(&mut self) -> io::Result<()> {
        loop {
            match self.rx.try_recv() {
                Ok(m) => {
                    self.on_event(&m);
                    if matches!(m["type"].as_str(), Some("response" | "error")) {
                        self.take_late(&m);
                    }
                }
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => return Err(closed()),
            }
        }
    }
}

fn closed() -> io::Error {
    io::Error::new(ErrorKind::ConnectionAborted, "Surfer closed the connection")
}

/// A `wave add`: its items and where the answer goes.
type PendingAdd = (Vec<(String, bool)>, Sender<Result<usize, String>>);

/// What the viewer thread keeps, and what Surfer is still to be told. What is due waits while
/// Surfer owes an answer or a load is under way, and later news replaces earlier: a stalled
/// Surfer reads no more of the socket once its queue of 100 is full, so it is not sent more.
struct View<'a> {
    file: &'a PathBuf,
    /// The file's size when it held no values yet: Surfer panics on reading the signals of a file
    /// with an empty time table, so nothing reloads or adds to the view before it has grown.
    empty: Option<u64>,
    seen: Option<u64>,
    /// `reload`s and the `load`, against `Wcp::loaded`.
    asked: u64,
    /// When the load under way was asked for, and whether its slowness was checked (`watch`).
    load_since: Instant,
    load_probed: bool,
    stalled: bool,
    reload_due: bool,
    /// Breakpoint times to mark, the signals to colour, where the cursor goes.
    marks_due: VecDeque<u64>,
    colour_due: Option<Vec<String>>,
    cursor_due: Option<u64>,
    /// Markers of an earlier run to remove.
    remove_due: Vec<Json>,
    /// What to add once the file has values and Surfer answers.
    waiting: Vec<(String, bool)>,
    /// `wave add`s whose caller is waiting, done when a load under way is complete.
    adds: Vec<PendingAdd>,
    /// The markers of breakpoints, and how many there were.
    markers: Vec<Json>,
    marks: u64,
    notice: &'a OnceLock<yavs_dap::NoticeFn>,
}

impl View<'_> {
    fn len(&self) -> Option<u64> {
        std::fs::metadata(self.file).map(|m| m.len()).ok()
    }

    fn has_values(&self) -> bool {
        self.len() != self.empty
    }

    fn stop(&mut self, s: yavs_dap::ViewStop) {
        if self.has_values() {
            self.reload_due = true;
            if let (true, Some(t)) = (s.breakpoint, s.now_fs) {
                self.marks_due.push_back(t);
            }
            self.colour_due = Some(s.signals);
        }
        if s.now_fs.is_some() {
            self.cursor_due = s.now_fs;
        }
    }

    fn due(&self) -> bool {
        self.reload_due
            || !self.marks_due.is_empty()
            || self.colour_due.is_some()
            || self.cursor_due.is_some()
            || !self.remove_due.is_empty()
            || !self.adds.is_empty()
            || (!self.waiting.is_empty() && self.has_values())
    }

    /// How long the thread may sleep; it has no other way to learn that Surfer woke up.
    fn tick(&self, w: &Wcp, every: Option<Duration>) -> Duration {
        if !w.late.is_empty() {
            Duration::from_millis(250)
        } else if self.due() || w.loaded < self.asked {
            Duration::from_millis(20)
        } else {
            every.unwrap_or(Duration::from_secs(1))
        }
    }

    /// Notices a stall and its end when they happen, and a load that ended without its event:
    /// Surfer sends `waveforms_loaded` only for a load that succeeds, and a failed one must not
    /// hold the view up for good.
    fn watch(&mut self, w: &mut Wcp) -> io::Result<()> {
        if !self.stalled && !w.late.is_empty() && w.late_since.elapsed() >= w.wait {
            self.stalled = true;
            tell(self.notice, &stalled_text(w.wait));
        }
        if self.stalled && w.late.is_empty() {
            self.stalled = false;
            tell(self.notice, "Surfer answers again: the view catches up.");
        }
        if w.loaded < self.asked && self.load_since.elapsed() >= w.wait * LOAD_WAITS {
            if !self.load_probed {
                // Is it the load, or is Surfer not drawing?
                self.load_probed = true;
                self.load_since = Instant::now();
                w.post("get_item_list")?;
            } else if w.late.is_empty() {
                tell(
                    self.notice,
                    &format!(
                        "Surfer answers but did not finish reading `{}`, so the view may be \
                         behind.\n  Its log says why. The next change of the file tries again.",
                        self.file.display()
                    ),
                );
                self.asked = w.loaded;
            }
        }
        Ok(())
    }

    /// Tells Surfer what is due, as far as it can be now.
    fn catch_up(&mut self, w: &mut Wcp) -> io::Result<()> {
        if !w.late.is_empty() {
            for (items, answer) in std::mem::take(&mut self.adds) {
                let names: Vec<String> = items.iter().map(|i| i.0.clone()).collect();
                self.waiting.extend(items);
                let _ = answer.send(Err(not_added_text(&names, w.wait)));
                // A new command asks Surfer to draw: it may be able to again by now.
                w.post("get_item_list")?;
            }
            return Ok(());
        }
        // Two loads at once make Surfer read the body twice, and what is added or marked must
        // find the new time table.
        if w.loaded < self.asked {
            return Ok(());
        }
        if self.reload_due {
            self.reload_due = false;
            self.seen = self.len();
            self.asked += 1;
            self.load_since = Instant::now();
            self.load_probed = false;
            return w.command("reload", json!({})).map(drop);
        }
        if !self.waiting.is_empty() && self.has_values() {
            w.add(&std::mem::take(&mut self.waiting))?;
        }
        while let Some(t) = self.marks_due.pop_front() {
            self.marks += 1;
            let name = format!("bp{}", self.marks);
            let marker = json!({"time": t, "name": name, "move_focus": false});
            let r = w.command("add_markers", json!({"markers": [marker]}))?;
            self.markers
                .extend(r["ids"].as_array().into_iter().flatten().cloned());
        }
        if let Some(signals) = self.colour_due.take() {
            w.colour(&signals)?;
        }
        if let Some(t) = self.cursor_due.take() {
            w.command("set_cursor", json!({"timestamp": t}))?;
            w.command("set_viewport_to", json!({"timestamp": t}))?;
        }
        let ids = std::mem::take(&mut self.remove_due);
        if !ids.is_empty() {
            w.command("remove_items", json!({"ids": ids}))?;
        }
        while !self.adds.is_empty() {
            let (items, answer) = self.adds.remove(0);
            let names: Vec<String> = items.iter().map(|i| i.0.clone()).collect();
            let r = w.add(&items);
            let _ = answer.send(r.as_ref().map(|n| *n).map_err(|e| match e.kind() {
                ErrorKind::TimedOut => not_added_text(&names, w.wait),
                ErrorKind::Other => e.to_string(),
                _ => gone_text(&e.to_string()),
            }));
            r?;
        }
        Ok(())
    }

    /// One command's trouble is told or left to `watch`; only a lost connection ends the thread.
    fn settle(&self, r: io::Result<()>) -> io::Result<()> {
        match r {
            Err(e) if e.kind() == ErrorKind::TimedOut => Ok(()),
            Err(e) if e.kind() == ErrorKind::Other => {
                tell(self.notice, &format!("{e}. The view goes on following."));
                Ok(())
            }
            r => r,
        }
    }

    /// The run is over: the whole of it in view. Waits for a Surfer that answers, not for one
    /// that stalled: it is asked to read the file and sees to it when it draws.
    fn finish(&mut self, w: &mut Wcp, known: &[String], layout: Option<&Path>) -> io::Result<()> {
        self.reload_due = self.has_values();
        let until = Instant::now() + w.wait * LOAD_WAITS;
        let mut probed = Instant::now();
        loop {
            w.poll()?;
            // A load shows no progress: is it slow, or is Surfer not drawing?
            if w.loaded < self.asked && w.late.is_empty() && probed.elapsed() >= w.wait {
                probed = Instant::now();
                w.post("get_item_list")?;
            }
            if !w.late.is_empty() && w.late_since.elapsed() >= w.wait {
                if self.reload_due {
                    w.post("reload")?;
                }
                // What still waits for an answer is told so by `catch_up`.
                self.catch_up(w)?;
                let wait = w.wait.as_secs_f64();
                let m = format!("Surfer did not answer within {wait} s");
                return Err(io::Error::new(ErrorKind::TimedOut, m));
            }
            if w.late.is_empty() {
                let r = self.catch_up(w);
                self.settle(r)?;
            }
            if w.late.is_empty() && w.loaded >= self.asked && !self.due() {
                break;
            }
            if Instant::now() > until {
                let m = format!(
                    "Surfer did not finish reading `{}` within {} s",
                    self.file.display(),
                    (w.wait * LOAD_WAITS).as_secs_f64()
                );
                return Err(io::Error::new(ErrorKind::TimedOut, m));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // The list of this session, for the next, unless it shows nothing.
        if let Some(f) = layout
            && let Ok(items) = w.items(known)
            && !items.is_empty()
        {
            let _ = yavs_dap::save_layout(f, &items);
        }
        if w.late.is_empty() {
            let r = w
                .command("zoom_to_fit", json!({"viewport_idx": 0}))
                .map(drop);
            if r.as_ref().is_err_and(|e| e.kind() == ErrorKind::TimedOut) {
                return r;
            }
            self.settle(r)?;
        }
        Ok(())
    }
}

/// The viewer thread: a reload when the file grew (at most one under way), the cursor, markers
/// and colours at debugger stops, and when the run is over (the sender dropped) the whole run in
/// view. It waits for Surfer no longer than `Wcp::wait` at a time, and ends only when the
/// connection does.
#[allow(clippy::too_many_arguments)]
fn drive(
    w: &mut Wcp,
    rx: &Receiver<Msg>,
    file: &PathBuf,
    every: Option<Duration>,
    names: Vec<String>,
    empty: Option<u64>,
    known: &[String],
    layout: Option<&Path>,
    notice: &OnceLock<yavs_dap::NoticeFn>,
) -> io::Result<()> {
    let mut v = View {
        file,
        empty,
        seen: empty,
        asked: 1,
        load_since: Instant::now(),
        load_probed: false,
        stalled: false,
        reload_due: false,
        marks_due: VecDeque::new(),
        colour_due: None,
        cursor_due: None,
        remove_due: Vec::new(),
        waiting: names.into_iter().map(|n| (n, false)).collect(),
        adds: Vec::new(),
        markers: Vec::new(),
        marks: 0,
        notice,
    };
    loop {
        let m = rx.recv_timeout(v.tick(w, every));
        w.poll()?;
        v.watch(w)?;
        let r = match m {
            Err(RecvTimeoutError::Timeout) => {
                // ponytail: a reload may meet a block being written; the next one shows it.
                if every.is_some() && v.len() != v.seen && v.has_values() {
                    v.reload_due = true;
                }
                Ok(())
            }
            Ok(Msg::Stop(s)) => {
                v.stop(s);
                Ok(())
            }
            Ok(Msg::Add(items, answer)) => {
                if v.has_values() {
                    v.adds.push((items, answer));
                } else {
                    // No values yet: they are shown when there are.
                    let n = items.len();
                    v.waiting.extend(items);
                    let _ = answer.send(Ok(n));
                }
                Ok(())
            }
            Ok(Msg::Items(answer)) => {
                let r = if w.late.is_empty() {
                    w.items(known)
                } else {
                    Err(io::Error::new(ErrorKind::TimedOut, ""))
                };
                let _ = answer.send(match &r {
                    Ok(l) => Ok(l.clone()),
                    Err(e) if e.kind() == ErrorKind::TimedOut => Err(no_list_text(w.wait)),
                    Err(e) if e.kind() == ErrorKind::Other => Err(e.to_string()),
                    Err(e) => Err(gone_text(&e.to_string())),
                });
                r.map(drop)
            }
            Ok(Msg::Cursor(t)) => {
                v.cursor_due = Some(t);
                Ok(())
            }
            Ok(Msg::Restart(empty)) => {
                // The new file is empty: what is shown stays, and reloads when it has values. The
                // breakpoints' markers were of the old run.
                v.empty = empty;
                v.seen = empty;
                let ids = std::mem::take(&mut v.markers);
                v.remove_due.extend(ids);
                Ok(())
            }
            Err(RecvTimeoutError::Disconnected) => return v.finish(w, known, layout),
        };
        v.settle(r)?;
        let r = v.catch_up(w);
        v.settle(r)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_answers_go_to_their_own_commands_in_order() {
        let l = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let w = TcpStream::connect(l.local_addr().expect("addr")).expect("connect");
        let peer = l.accept().expect("accept").0;
        let (tx, rx) = channel();
        let mut c = Wcp {
            w,
            rx,
            loaded: 0,
            wait: Duration::from_secs(5),
            late: VecDeque::from([Some(vec!["tb.a".to_string()]), None]),
            late_since: Instant::now(),
            shown: HashMap::new(),
            coloured: Vec::new(),
            goto: Arc::default(),
        };
        // The answers of two earlier commands, then the one to this command.
        tx.send(json!({"type": "response", "command": "add_items", "ids": [7]}))
            .expect("send");
        tx.send(json!({"type": "error", "error": "reload", "message": "late refusal"}))
            .expect("send");
        tx.send(json!({"type": "response", "command": "get_item_list", "ids": [1, 2]}))
            .expect("send");
        let r = c.command("get_item_list", json!({})).expect("the answer");
        assert_eq!(r["ids"], json!([1, 2]));
        assert!(c.late.is_empty());
        assert_eq!(c.shown["tb.a"], json!(7));
        drop(peer);
    }

    #[test]
    fn a_displayed_name_is_found_when_it_means_one_signal() {
        let known = ["xt.clk", "xt.u1.clk", "xt.cnt"].map(String::from);
        assert_eq!(shown_name("cnt [3:0]", &known).as_deref(), Some("xt.cnt"));
        assert_eq!(shown_name("…u1.clk", &known).as_deref(), Some("xt.u1.clk"));
        assert_eq!(shown_name("xt.clk", &known).as_deref(), Some("xt.clk"));
        assert_eq!(shown_name("clk", &known), None);
        assert_eq!(shown_name("nope", &known), None);
    }
}
