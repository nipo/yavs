//! The terminal side of `yavs run --interactive` and `--do`: a DAP client of the server in this
//! process, over a pipe. Each line goes in as `evaluate` with context `repl`; all commands are
//! in `yavs_dap`. stdout: reports, textio output and results of commands, as a batch run would
//! print them. stderr: stop notices and errors (docs/cli-contract.md 5).

use std::io::{BufReader, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use serde_json::{Value as Json, json};
use yavs_dap::{read_message, write_message};
use yavs_diag::Diagnostic;

/// What the printing thread tells the driving thread.
enum Ev {
    Prompt { failed: bool },
    Ended,
}

/// Tab at the prompt: the server's `completions`, which is what an editor's console asks too.
struct Tab {
    c: Arc<Client>,
    answers: Mutex<Receiver<Vec<(String, usize)>>>,
}

impl rustyline::completion::Completer for Tab {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        let answers = self.answers.lock().unwrap_or_else(|p| p.into_inner());
        self.c
            .request("completions", json!({"text": line, "column": pos + 1}));
        // The prompt never waits for long: the server answers at the stop the prompt is at.
        let found = answers
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap_or_default();
        let start = found.first().map_or(pos, |(_, n)| pos.saturating_sub(*n));
        Ok((start, found.into_iter().map(|(l, _)| l).collect()))
    }
}

impl rustyline::hint::Hinter for Tab {
    type Hint = String;
}
impl rustyline::highlight::Highlighter for Tab {}
impl rustyline::validate::Validator for Tab {}
impl rustyline::Helper for Tab {}

type Editor = rustyline::Editor<Tab, rustyline::history::DefaultHistory>;

struct Client {
    w: Mutex<std::io::PipeWriter>,
    seq: Mutex<i64>,
    /// A command is under way, so Ctrl-C means pause.
    busy: AtomicBool,
}

impl Client {
    fn request(&self, command: &str, arguments: Json) {
        let mut seq = self.seq.lock().unwrap_or_else(|p| p.into_inner());
        *seq += 1;
        let m = json!({"type": "request", "seq": *seq, "command": command, "arguments": arguments});
        let mut w = self.w.lock().unwrap_or_else(|p| p.into_inner());
        // A closed pipe: the server has gone, and the printing thread sees the end too.
        let _ = write_message(&mut *w, &m);
    }

    fn line(&self, text: &str) {
        self.busy.store(true, Ordering::Relaxed);
        self.request("evaluate", json!({"expression": text, "context": "repl"}));
    }
}

fn print_out(text: &str) {
    let mut o = std::io::stdout().lock();
    // A closed stdout: nothing is left to print to, and the session ends by its commands.
    let _ = o.write_all(text.as_bytes());
    let _ = o.flush();
}

/// Print what the server says, until the session is over.
fn printer(
    r: std::io::PipeReader,
    c: &Client,
    ev: &Sender<Ev>,
    tab: &Sender<Vec<(String, usize)>>,
) {
    let mut r = BufReader::new(r);
    while let Ok(Some(m)) = read_message(&mut r) {
        match m["type"].as_str() {
            Some("response") => match m["command"].as_str() {
                Some("evaluate") if m["success"] == true => {
                    let t = m["body"]["result"].as_str().unwrap_or_default();
                    if !t.is_empty() {
                        print_out(&format!("{t}\n"));
                    }
                }
                Some("evaluate") => {
                    eprintln!("error: {}", m["message"].as_str().unwrap_or_default());
                }
                Some("completions") => {
                    let targets = m["body"]["targets"].as_array();
                    let found = targets.into_iter().flatten().filter_map(|t| {
                        let label = t["label"].as_str()?.to_string();
                        Some((label, t["length"].as_u64()? as usize))
                    });
                    let _ = tab.send(found.collect());
                }
                Some("disconnect") => return,
                _ => {}
            },
            Some("event") => match m["event"].as_str() {
                Some("output") => {
                    let t = m["body"]["output"].as_str().unwrap_or_default();
                    match m["body"]["category"].as_str() {
                        Some("stderr" | "important") => eprint!("{t}"),
                        _ => print_out(t),
                    }
                }
                Some("yavs/prompt") => {
                    c.busy.store(false, Ordering::Relaxed);
                    let failed = m["body"]["failed"] == true;
                    let _ = ev.send(Ev::Prompt { failed });
                }
                Some("terminated") => {
                    let _ = ev.send(Ev::Ended);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// Wait for the prompt event; `None` when the session ended.
fn wait(ev: &Receiver<Ev>) -> Option<bool> {
    match ev.recv() {
        Ok(Ev::Prompt { failed }) => Some(failed),
        Ok(Ev::Ended) | Err(_) => None,
    }
}

/// The line editor, when standard input is a terminal.
fn editor(tab: Tab) -> Option<Box<Editor>> {
    if !std::io::stdin().is_terminal() {
        return None;
    }
    let config = rustyline::Config::builder()
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut e = Editor::with_config(config).ok()?;
    e.set_helper(Some(tab));
    Some(Box::new(e))
}

/// The next line from the terminal, with editing and history, or plainly from a pipe; `None` at
/// the end of input.
fn next_line(editor: &mut Option<Box<Editor>>) -> Option<String> {
    let Some(e) = editor else {
        let mut l = String::new();
        return match std::io::stdin().read_line(&mut l) {
            Ok(n) if n > 0 => Some(l),
            _ => None,
        };
    };
    loop {
        match e.readline("yavs> ") {
            Ok(l) => {
                let _ = e.add_history_entry(l.as_str());
                return Some(l);
            }
            // Ctrl-C at the prompt drops the line.
            Err(rustyline::error::ReadlineError::Interrupted) => {}
            Err(_) => return None,
        }
    }
}

/// Send the script, then the terminal's lines, then `quit`.
fn driver(
    c: &Client,
    ev: &Receiver<Ev>,
    script: &[String],
    interactive: bool,
    mut editor: Option<Box<Editor>>,
) {
    c.request("initialize", json!({}));
    c.request("launch", json!({"yavsConsole": true}));
    c.request("configurationDone", json!({}));
    let finish = |line: &str| {
        c.line(line);
        while wait(ev).is_some() {}
    };
    if wait(ev).is_none() {
        return c.request("disconnect", json!({}));
    }
    for line in script {
        c.line(line);
        match wait(ev) {
            None => return c.request("disconnect", json!({})),
            // A script without a terminal ends at its first failure.
            Some(true) if !interactive => {
                finish("quit 1");
                return c.request("disconnect", json!({}));
            }
            Some(_) => {}
        }
    }
    if interactive {
        while let Some(line) = next_line(&mut editor) {
            if line.trim().is_empty() {
                continue;
            }
            c.line(&line);
            if wait(ev).is_none() {
                return c.request("disconnect", json!({}));
            }
        }
    }
    finish("quit");
    c.request("disconnect", json!({}));
}

/// Serve the session with this process's terminal or script as the client.
pub(crate) fn serve(
    loaded: yavs_dap::Loaded,
    script: &[String],
    interactive: bool,
    options: yavs_dap::Options,
    attach: impl yavs_dap::Attach,
    report: yavs_dap::Report,
) -> Result<u8, Box<Diagnostic>> {
    let pipes = std::io::pipe().and_then(|a| Ok((a, std::io::pipe()?)));
    let ((srv_r, cli_w), (cli_r, srv_w)) = pipes.map_err(|err| {
        Box::new(
            Diagnostic::error(yavs_dap::IO_ERROR, "the console's pipes cannot be opened")
                .with_note(err.to_string()),
        )
    })?;
    let c = Arc::new(Client {
        w: Mutex::new(cli_w),
        seq: Mutex::new(0),
        busy: AtomicBool::new(false),
    });
    // Before the Ctrl-C handler: the editor takes SIGINT when it is made (and restores it only when
    // dropped), and a handler installed after it is the one that runs, then calls the editor's. At
    // the prompt the terminal is raw, so Ctrl-C is a key, not a signal (#932).
    // ponytail: each Ctrl-C of a run also leaves a byte in the editor's own pipe, read at its next
    // interrupted read: a window resize at the prompt after a pause can drop the typed line once.
    // Restoring the handler around each line would need an unsafe `sigaction`.
    let (tab_tx, tab_rx) = channel();
    let tab = Tab {
        c: c.clone(),
        answers: Mutex::new(tab_rx),
    };
    let editor = if interactive { editor(tab) } else { None };
    if interactive {
        let sig = c.clone();
        let plain = !std::io::stdin().is_terminal();
        let pause = move || {
            if sig.busy.load(Ordering::Relaxed) {
                sig.request("pause", json!({}));
            } else if plain {
                // Nothing to pause, and no line editor that would take it.
                std::process::exit(130);
            }
        };
        yavs_wave::interrupt::every_ctrl_c(pause).map_err(|err| {
            Box::new(
                Diagnostic::error("E0803", "cannot install the Ctrl-C handler")
                    .with_note(err.to_string()),
            )
        })?;
    }
    let (tx, rx) = channel();
    let out = {
        let c = c.clone();
        std::thread::spawn(move || printer(cli_r, &c, &tx, &tab_tx))
    };
    let script = script.to_vec();
    std::thread::spawn(move || driver(&c, &rx, &script, interactive, editor));
    let code = yavs_dap::serve_on(
        loaded,
        Box::new(srv_r),
        Box::new(srv_w),
        None,
        options,
        attach,
        report,
    )?;
    // All of the session's text is out.
    let _ = out.join();
    Ok(code)
}
