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
fn printer(r: std::io::PipeReader, c: &Client, ev: &Sender<Ev>) {
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
fn editor() -> Option<Box<rustyline::DefaultEditor>> {
    let tty = std::io::stdin().is_terminal();
    tty.then(rustyline::DefaultEditor::new)
        .and_then(Result::ok)
        .map(Box::new)
}

/// The next line from the terminal, with editing and history, or plainly from a pipe; `None` at
/// the end of input.
fn next_line(editor: &mut Option<Box<rustyline::DefaultEditor>>) -> Option<String> {
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
    mut editor: Option<Box<rustyline::DefaultEditor>>,
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
    let editor = editor().filter(|_| interactive);
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
        std::thread::spawn(move || printer(cli_r, &c, &tx))
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
