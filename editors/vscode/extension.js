// Starts `yavs run --dap=stdio` for a "yavs" debug configuration, or attaches to a port.
'use strict';

const vscode = require('vscode');
const net = require('net');
const { spawnSync } = require('child_process');

/** The `yavs` arguments common to analyze and run. */
function libArgs(c) {
  const a = [];
  if (c.std) a.push(`--std=${c.std}`);
  a.push(`--work=${c.work || 'work:.yavs/work'}`);
  for (const m of c.libraries || []) a.push(`--map=${m}`);
  return a;
}

/** `yavs run` arguments of a launch configuration. */
function runArgs(c) {
  const a = ['run', ...libArgs(c)];
  for (const [k, v] of Object.entries(c.generics || {})) a.push(`-g${k}=${v}`);
  if (c.surfer) a.push(c.surfer === true ? '--surfer' : `--surfer=${c.surfer}`);
  a.push(...(c.args || []), '--dap=stdio', c.top);
  return a;
}

const ATTACH_PORT = 4711;
const ATTACH = { type: 'yavs', request: 'attach', name: 'yavs: Attach', port: ATTACH_PORT };

/** Whether something listens on the port. The server takes a connection closed at once for no
 * session. */
function listening(port) {
  return new Promise((resolve) => {
    const s = net.connect({ port, host: '127.0.0.1' });
    const done = (v) => {
      s.destroy();
      resolve(v);
    };
    s.once('connect', () => done(true));
    s.once('error', () => done(false));
    s.setTimeout(1000, () => done(false));
  });
}

/** "yavs: Attach" with no launch.json: offered in the Run and Debug view and taken by F5 when
 * nothing is configured. It asks for a port only if nothing listens on the default. */
class Configurations {
  async resolveDebugConfiguration(_folder, c) {
    if (!c.type && !c.request && !c.name) c = { ...ATTACH };
    if (c.request !== 'attach') return c;
    const port = c.port || ATTACH_PORT;
    if (await listening(port)) return { ...c, port };
    const answer = await vscode.window.showInputBox({
      prompt: `Nothing listens on 127.0.0.1:${port}. Start a test with: python run.py lib.tb.test --yavs-debug. Port to attach to:`,
      value: String(port),
      validateInput: (v) => (/^[1-9][0-9]*$/.test(v) && Number(v) < 65536 ? null : 'A port number'),
    });
    return answer ? { ...c, port: Number(answer) } : undefined;
  }
}

class Factory {
  createDebugAdapterDescriptor(session) {
    const c = session.configuration;
    if (c.port) return new vscode.DebugAdapterServer(c.port, '127.0.0.1');
    const exe = c.yavsPath || 'yavs';
    const cwd = c.cwd || (vscode.workspace.workspaceFolders || [])[0]?.uri.fsPath;
    if ((c.sources || []).length > 0) {
      const r = spawnSync(exe, ['analyze', ...libArgs(c), ...c.sources], { cwd, encoding: 'utf8' });
      if (r.error || r.status !== 0) {
        const msg = r.error ? r.error.message : (r.stderr || r.stdout);
        vscode.window.showErrorMessage(`yavs analyze failed: ${msg}`);
        return undefined;
      }
    }
    return new vscode.DebugAdapterExecutable(exe, runArgs(c), { cwd });
  }
}

/** What the adapter does not get to say itself: it ended before it answered anything (a command
 * line it refuses, a crash). A diagnostic the server could send is the failure of `launch`. */
class ExitTracker {
  createDebugAdapterTracker(session) {
    const attached = !!session.configuration.port;
    let answered = false;
    return {
      onDidSendMessage: (m) => {
        if (m.type === 'response') answered = true;
        // The menus of "Add to Wave": this session shows its waves in a viewer.
        if (m.type === 'response' && m.command === 'initialize') {
          vscode.commands.executeCommand('setContext', 'yavs.viewer', m.body?.yavsViewer === true);
        }
      },
      onExit: (code, signal) => {
        if (attached || answered || (code === 0 && !signal)) return;
        const c = session.configuration;
        const how = signal ? `was ended by ${signal}` : `exited with status ${code}`;
        vscode.window.showErrorMessage(
          `yavs ${how} before the session started. In a terminal: ${c.yavsPath || 'yavs'} ${runArgs(c).join(' ')}`,
        );
      },
    };
  }
}

/** A command sending custom request `command` to the yavs session, asking for a time first
 * when `prompt` is given. */
function custom(command, prompt) {
  return async () => {
    const s = vscode.debug.activeDebugSession;
    if (!s || s.type !== 'yavs') return;
    const args = {};
    if (prompt) {
      args.time = await vscode.window.showInputBox({ prompt, placeHolder: '10 ns' });
      if (!args.time) return;
    }
    try {
      await s.customRequest(command, args);
    } catch (e) {
      vscode.window.showErrorMessage(`yavs: ${e.message}`);
    }
  };
}

/** Signal value breakpoints per session id: the server replaces the whole set per request. */
const valueBps = new Map();

/** Ask for a signal and a value, add the breakpoint, and send the session's set. */
async function breakOnValue() {
  const s = vscode.debug.activeDebugSession;
  if (!s || s.type !== 'yavs') return;
  const signal = await vscode.window.showInputBox({
    prompt: 'Signal (a name visible from the selected frame, or a path such as :tb:u1:s)',
  });
  if (!signal) return;
  const value = await vscode.window.showInputBox({ prompt: `Stop when ${signal} becomes`, placeHolder: "'1'" });
  if (!value) return;
  const bps = [...(valueBps.get(s.id) || []), { signal, value }];
  await sendValueBps(s, bps);
}

async function sendValueBps(s, bps) {
  try {
    const r = await s.customRequest('yavs/setSignalBreakpoints', {
      frameId: vscode.debug.activeStackItem?.frameId,
      breakpoints: bps,
    });
    const bad = r.breakpoints.findIndex((b) => !b.verified);
    if (bad >= 0) {
      vscode.window.showErrorMessage(`yavs: ${r.breakpoints[bad].message}`);
      bps.splice(bad, 1);
    }
    valueBps.set(s.id, bps);
  } catch (e) {
    vscode.window.showErrorMessage(`yavs: ${e.message}`);
  }
}

/** "Add to Wave": the variable of the Variables or Watch view's context menu, or the selection or
 * the word at the cursor in the editor. The server resolves it in the selected frame, as the
 * console's `wave add` does. */
async function addToWave(ctx) {
  const s = vscode.debug.activeDebugSession;
  if (!s || s.type !== 'yavs') return;
  let name = ctx?.variable?.evaluateName || ctx?.variable?.name || ctx?.expression;
  if (!name) {
    const e = vscode.window.activeTextEditor;
    if (e) {
      const word = e.selection.isEmpty
        ? e.document.getWordRangeAtPosition(e.selection.active, /[A-Za-z][A-Za-z0-9_.]*/)
        : e.selection;
      name = word && e.document.getText(word).trim();
    }
  }
  if (!name) {
    vscode.window.showErrorMessage('yavs: select a signal or variable to add to the wave');
    return;
  }
  try {
    const r = await s.customRequest('evaluate', {
      expression: `wave add ${name}`,
      context: 'repl',
      frameId: vscode.debug.activeStackItem?.frameId,
    });
    vscode.window.setStatusBarMessage(`yavs: ${name}: ${r.result}`, 5000);
  } catch (e) {
    vscode.window.showErrorMessage(`yavs: ${e.message}`);
  }
}

/** Force (asking for the mode), deposit or release a signal: the variable of the Variables
 * view's context menu, the `signal` (and `value`, `mode`) of a programmatic call, or asked
 * for. */
function signalOp(command, withValue) {
  return async (ctx) => {
    const s = vscode.debug.activeDebugSession;
    if (!s || s.type !== 'yavs') return;
    const signal = ctx?.signal || ctx?.variable?.name || (await vscode.window.showInputBox({
      prompt: 'Signal (a name visible from the selected frame, or a path such as :tb:u1:s)',
    }));
    if (!signal) return;
    const args = { signal, frameId: vscode.debug.activeStackItem?.frameId };
    if (command !== 'yavs/deposit') {
      args.mode = ctx?.mode || (await vscode.window.showQuickPick(['in', 'out'], {
        placeHolder: 'in: the effective value (what readers see), out: the driving value',
      }));
      if (!args.mode) return;
    }
    if (withValue) {
      args.value = ctx?.value || (await vscode.window.showInputBox({
        prompt: `Value for ${signal}`,
        placeHolder: `'1', 42, x"FF", (others => '0'), 10 ns`,
      }));
      if (!args.value) return;
    }
    try {
      await s.customRequest(command, args);
    } catch (e) {
      vscode.window.showErrorMessage(`yavs: ${e.message}`);
    }
  };
}

function activate(context) {
  const reg = vscode.commands.registerCommand;
  context.subscriptions.push(
    vscode.debug.registerDebugConfigurationProvider(
      'yavs',
      { provideDebugConfigurations: () => [ATTACH] },
      vscode.DebugConfigurationProviderTriggerKind.Dynamic,
    ),
    vscode.debug.registerDebugConfigurationProvider('yavs', new Configurations()),
    // Surfer's "go to declaration" (`--surfer`).
    vscode.debug.onDidReceiveDebugSessionCustomEvent(async (e) => {
      if (e.session.type !== 'yavs' || e.event !== 'yavs/showSource') return;
      const doc = await vscode.workspace.openTextDocument(e.body.path);
      const at = new vscode.Position(e.body.line - 1, 0);
      await vscode.window.showTextDocument(doc, { selection: new vscode.Range(at, at) });
    }),
    vscode.commands.registerCommand('yavs.attach', () =>
      vscode.debug.startDebugging(vscode.workspace.workspaceFolders?.[0], ATTACH),
    ),
    vscode.debug.registerDebugAdapterDescriptorFactory('yavs', new Factory()),
    vscode.debug.registerDebugAdapterTrackerFactory('yavs', new ExitTracker()),
    reg('yavs.runFor', custom('yavs/runFor', 'Run for how long (from now)?')),
    reg('yavs.runUntil', custom('yavs/runUntil', 'Run until which simulation time?')),
    reg('yavs.stepDelta', custom('yavs/stepDelta')),
    reg('yavs.stepTimestep', custom('yavs/stepTimestep')),
    reg('yavs.breakOnValue', breakOnValue),
    reg('yavs.addToWave', addToWave),
    reg('yavs.force', signalOp('yavs/force', true)),
    reg('yavs.deposit', signalOp('yavs/deposit', true)),
    reg('yavs.release', signalOp('yavs/release', false)),
    reg('yavs.clearValueBreaks', () => {
      const s = vscode.debug.activeDebugSession;
      if (s && s.type === 'yavs') return sendValueBps(s, []);
    }),
    vscode.debug.onDidTerminateDebugSession((s) => {
      valueBps.delete(s.id);
      if (s.type === 'yavs') vscode.commands.executeCommand('setContext', 'yavs.viewer', false);
    }));
}

function deactivate() {}

module.exports = { activate, deactivate, runArgs };
