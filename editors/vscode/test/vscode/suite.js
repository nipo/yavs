// Inside VS Code: set a breakpoint in tb.vhd, start "Debug tb" from launch.json, and check
// that the session stops there with a frame VS Code can open, steps one delta with the yavs
// command, stops on a signal value, then runs to the end.
'use strict';

const assert = require('assert');
const cp = require('child_process');
const net = require('net');
const vscode = require('vscode');

function waitFor(what, ms, register) {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error(`timeout waiting for ${what}`)), ms);
    register((v) => {
      clearTimeout(t);
      resolve(v);
    });
  });
}

async function run() {
  const folder = vscode.workspace.workspaceFolders[0];
  const tb = vscode.Uri.joinPath(folder.uri, 'tb.vhd');
  const doc = await vscode.workspace.openTextDocument(tb);
  assert.equal(doc.languageId, 'vhdl');
  // Line 20, `i := twice(k);` (0-based 19), with a condition VS Code sends, and one on the
  // function body that never holds.
  vscode.debug.addBreakpoints([
    new vscode.SourceBreakpoint(new vscode.Location(tb, new vscode.Position(19, 0)), true, 'k = 1'),
    new vscode.SourceBreakpoint(new vscode.Location(tb, new vscode.Position(11, 0)), true, 'x > 9'),
  ]);

  const stops = [];
  const outputs = [];
  let onStop;
  vscode.debug.registerDebugAdapterTrackerFactory('yavs', {
    createDebugAdapterTracker: () => ({
      onDidSendMessage: (m) => {
        if (m.type === 'event' && m.event === 'output') outputs.push(m.body.output);
        if (m.type === 'event' && m.event === 'stopped') {
          stops.push(m.body);
          if (onStop) onStop(m.body);
        }
      },
    }),
  });
  const stopped = waitFor('stopped', 60000, (r) => (onStop = r));
  const started = await vscode.debug.startDebugging(folder, 'Debug tb');
  assert.ok(started, 'startDebugging failed');
  const s = await stopped;
  assert.equal(s.reason, 'breakpoint');

  const session = vscode.debug.activeDebugSession;
  const st = await session.customRequest('stackTrace', { threadId: s.threadId });
  const top = st.stackFrames[0];
  assert.equal(top.line, 20);
  assert.equal(vscode.Uri.file(top.source.path).toString(), tb.toString());
  // VS Code shows the stop: the active stack item is that frame.
  await new Promise((r) => setTimeout(r, 500));
  const item = vscode.debug.activeStackItem;
  assert.ok(item && item.frameId === top.id, `active stack item ${JSON.stringify(item)}`);

  // The Variables view's scopes, and a composite expanded the way VS Code pages it.
  const scopeNames = (await session.customRequest('scopes', { frameId: top.id })).scopes.map((x) => x.name);
  assert.deepEqual(scopeNames, ['Locals', 'Signals', 'Generics', 'Hierarchy']);
  const vv = await session.customRequest('evaluate', { expression: 'pairs', frameId: top.id });
  const el = await session.customRequest('variables', {
    variablesReference: vv.variablesReference, filter: 'indexed', start: 0, count: vv.indexedVariables,
  });
  assert.deepEqual(el.variables.map((x) => x.value), ['"01"', '"10"']);
  const one = await session.customRequest('evaluate', { expression: 'v', frameId: top.id });
  assert.deepEqual([one.result, one.variablesReference], ['"0101"', 0]);
  // A Watch of nothing, and `help`, which VS Code shows bold from ANSI as it asks for them.
  await assert.rejects(
    session.customRequest('evaluate', { expression: 'tb.cn', frameId: top.id, context: 'watch' }),
    /`tb\.cnt`/,
  );
  await session.customRequest('evaluate', { expression: 'help', context: 'repl' });
  await new Promise((r) => setTimeout(r, 500));
  assert.ok(outputs.some((o) => /\x1b\[1mexamine\x1b\[0m NAME/.test(o)), outputs.join(''));

  // "Set Value" on a local, then the extension's force command (as its context menu entry
  // on a signal calls it, with the answers given).
  const sc = await session.customRequest('scopes', { frameId: top.id });
  const set = await session.customRequest('setVariable', {
    variablesReference: sc.scopes[0].variablesReference, name: 'i', value: '5',
  });
  assert.equal(set.value, '5');
  await vscode.commands.executeCommand('yavs.force', { signal: 'v', value: '1111' });

  // The command palette's "yavs: Step to the next delta" ends the cycle stopped in.
  const stepped = waitFor('stopped after the delta step', 60000, (r) => (onStop = r));
  await vscode.commands.executeCommand('yavs.stepDelta');
  const d = await stepped;
  assert.equal(d.reason, 'step');
  assert.match(d.description, /^step at 0 fs, delta 0$/);
  // The force is made in the next cycle and holds; release it again.
  const step2 = waitFor('stopped after the second delta step', 60000, (r) => (onStop = r));
  await vscode.commands.executeCommand('yavs.stepDelta');
  await step2;
  const ev = await session.customRequest('evaluate', { expression: ':tb:v' });
  assert.equal(ev.result, '"1111"');
  await vscode.commands.executeCommand('yavs.release', { signal: ':tb:v' });

  // A signal value breakpoint (what "yavs: Break when a signal equals..." sends), without the
  // line breakpoint, which the process would hit first.
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  await new Promise((r) => setTimeout(r, 500));
  const hit = waitFor('stopped on the signal', 60000, (r) => (onStop = r));
  await session.customRequest('yavs/setSignalBreakpoints', {
    breakpoints: [{ signal: ':tb:cnt', value: '6' }],
  });
  await session.customRequest('continue', { threadId: 1 });
  const h = await hit;
  assert.equal(h.reason, 'data breakpoint');
  assert.equal(h.text, 'tb.cnt = 6');
  await session.customRequest('yavs/setSignalBreakpoints', { breakpoints: [] });

  const ended = waitFor('end of session', 60000, (r) => vscode.debug.onDidTerminateDebugSession(r));
  await session.customRequest('continue', { threadId: 1 });
  await ended;
  assert.equal(stops.length, 4);

  await launchThatCannotStart(folder);
  await attachWithNoConfiguration(folder, tb, () => waitFor('stopped when attached', 60000, (r) => (onStop = r)));
  await surferGoesToTheDeclaration(folder, tb, () => waitFor('stopped under Surfer', 60000, (r) => (onStop = r)));
  const stoppedAgain = (what) => waitFor(what, 60000, (r) => (onStop = r));
  await sharedVariablesAndThreads(folder, stoppedAgain);
  await reportBeforeALoop(folder);
  await restartWhenAttached(folder, tb, stoppedAgain);
}

const MORE = {
  type: 'yavs',
  request: 'launch',
  name: 'Debug more_tb',
  top: 'more_tb',
  work: 'work:${workspaceFolder}/.yavs/work',
  sources: ['${workspaceFolder}/more.vhd'],
};

/** #938, #959, #942: what the views ask of a shared variable, and the Threads and Call Stack names. */
async function sharedVariablesAndThreads(folder, stoppedAgain) {
  const more = vscode.Uri.joinPath(folder.uri, 'more.vhd');
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  vscode.debug.addBreakpoints([
    new vscode.SourceBreakpoint(new vscode.Location(more, new vscode.Position(31, 0))),
  ]);
  const stopped = stoppedAgain('stopped in more_tb');
  assert.ok(await vscode.debug.startDebugging(folder, MORE), 'startDebugging failed');
  const s = await stopped;
  const session = vscode.debug.activeDebugSession;
  const threads = await session.customRequest('threads', {});
  assert.deepEqual(threads.threads.map((t) => t.name), [
    'more_tb.clk <= ... (more.vhd:21)',
    'more_tb.process (more.vhd:23)',
  ]);
  const st = await session.customRequest('stackTrace', { threadId: s.threadId });
  const top = st.stackFrames[0];
  assert.equal(top.name, 'more_tb.process (more.vhd:23)');
  const scopes = await session.customRequest('scopes', { frameId: top.id });
  const shared = scopes.scopes.find((x) => x.name === 'Shared Variables');
  assert.ok(shared, JSON.stringify(scopes));
  const vars = await session.customRequest('variables', { variablesReference: shared.variablesReference });
  assert.equal(vars.variables[0].value, 'protected');
  const state = await session.customRequest('variables', { variablesReference: vars.variables[0].variablesReference });
  assert.equal(state.variables[0].value, '3');
  for (const context of ['watch', 'hover']) {
    const v = await session.customRequest('evaluate', { expression: 'tally', frameId: top.id, context });
    assert.equal(v.result, 'protected');
  }
  const d = await session.customRequest('evaluate', { expression: 'describe tally', frameId: top.id, context: 'repl' });
  assert.match(d.result, /protected object/);
  const found = await session.customRequest('yavs/find', { kind: 'variables', pattern: 'tally' });
  assert.equal(found.items[0].path, 'more_tb.tally');
  const ended = waitFor('end of more_tb', 60000, (r) => vscode.debug.onDidTerminateDebugSession(r));
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  await new Promise((r) => setTimeout(r, 500));
  await session.customRequest('continue', { threadId: s.threadId });
  await ended;
}

/** #939: the report reaches the Debug Console while the process still runs. */
async function reportBeforeALoop(folder) {
  let onOutput;
  const output = waitFor('the report', 60000, (r) => (onOutput = r));
  const reg = vscode.debug.registerDebugAdapterTrackerFactory('yavs', {
    createDebugAdapterTracker: () => ({
      onDidSendMessage: (m) => {
        if (m.type === 'event' && m.event === 'output' && /before the loop/.test(m.body.output)) onOutput(m.body);
      },
    }),
  });
  const started = await vscode.debug.startDebugging(folder, { ...MORE, generics: { SPIN: 'true' } });
  assert.ok(started, 'startDebugging failed');
  await output;
  reg.dispose();
  const ended = waitFor('end of the loop', 60000, (r) => vscode.debug.onDidTerminateDebugSession(r));
  await vscode.debug.stopDebugging(vscode.debug.activeDebugSession);
  await ended;
}

/** A yavs run with `args` that waits for a client; its port. */
async function waitingRun(folder, args) {
  const run = cp.spawn('yavs', ['run', '--work=work:.yavs/work', ...args, '--dap=0', 'tb'], {
    cwd: folder.uri.fsPath,
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  const port = await new Promise((resolve, reject) => {
    run.on('error', reject);
    run.stderr.on('data', (d) => {
      const m = /listening on 127\.0\.0\.1:(\d+)/.exec(d.toString());
      if (m) resolve(Number(m[1]));
    });
  });
  return { run, port, exited: new Promise((resolve) => run.on('exit', resolve)) };
}

/** #963: the debug toolbar's Restart on an attached session runs again from time 0 in the same
 * run, and when the run cannot, says why and keeps the session. */
async function restartWhenAttached(folder, tb, stoppedAgain) {
  vscode.debug.addBreakpoints([
    new vscode.SourceBreakpoint(new vscode.Location(tb, new vscode.Position(19, 0))),
  ]);
  const { run, port, exited } = await waitingRun(folder, []);
  const attach = { type: 'yavs', request: 'attach', name: 'yavs: Attach to the run', port };
  let stopped = stoppedAgain('stopped when attached');
  assert.ok(await vscode.debug.startDebugging(folder, attach), 'attach failed');
  const first = await stopped;
  const session = vscode.debug.activeDebugSession;
  let ended = false;
  const gone = vscode.debug.onDidTerminateDebugSession(() => (ended = true));
  // To the second hit of the line: a time later than the first.
  stopped = stoppedAgain('the second hit');
  await session.customRequest('continue', { threadId: first.threadId });
  const later = await stopped;
  assert.match(later.description, /^breakpoint at 10 ns/);
  stopped = stoppedAgain('stopped after the restart');
  await vscode.commands.executeCommand('workbench.action.debug.restart');
  const again = await stopped;
  assert.match(again.description, /^breakpoint at 0 fs/);
  assert.equal(ended, false);
  assert.equal(vscode.debug.activeDebugSession.id, session.id);
  assert.equal(run.exitCode, null);
  gone.dispose();
  const done = waitFor('end of the attached run', 60000, (r) => vscode.debug.onDidTerminateDebugSession(r));
  await vscode.debug.stopDebugging(session);
  await done;
  await exited;
  await restartRefused(folder, tb, stoppedAgain);
}

/** With `--load`, the run cannot restart: VS Code gets the reason as a notice and the session stays. */
async function restartRefused(folder, tb, stoppedAgain) {
  const { port, exited } = await waitingRun(folder, ['--load=libm.so.6']);
  let onNotice;
  const notice = waitFor('the notice', 60000, (r) => (onNotice = r));
  const reg = vscode.debug.registerDebugAdapterTrackerFactory('yavs', {
    createDebugAdapterTracker: () => ({
      onDidSendMessage: (m) => {
        if (m.type === 'event' && m.event === 'output' && m.body.category === 'important') onNotice(m.body);
      },
    }),
  });
  const stopped = stoppedAgain('stopped when attached');
  const attach = { type: 'yavs', request: 'attach', name: 'yavs: Attach to the run', port };
  assert.ok(await vscode.debug.startDebugging(folder, attach), 'attach failed');
  const s = await stopped;
  const session = vscode.debug.activeDebugSession;
  let ended = false;
  const gone = vscode.debug.onDidTerminateDebugSession(() => (ended = true));
  await vscode.commands.executeCommand('workbench.action.debug.restart');
  assert.match((await notice).output, /start the run again/);
  reg.dispose();
  const st = await session.customRequest('stackTrace', { threadId: s.threadId });
  assert.equal(st.stackFrames[0].line, 20);
  assert.equal(ended, false);
  gone.dispose();
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  const done = waitFor('end of the refused run', 60000, (r) => vscode.debug.onDidTerminateDebugSession(r));
  await vscode.debug.stopDebugging(session);
  await done;
  await exited;
}

/** A wrong `top`: the session does not start, and the adapter's answer holds the diagnostic that VS
 * Code shows (#934). */
async function launchThatCannotStart(folder) {
  const seen = [];
  const reg = vscode.debug.registerDebugAdapterTrackerFactory('yavs', {
    createDebugAdapterTracker: () => ({
      onDidSendMessage: (m) => {
        if (m.type === 'response' && m.command === 'launch') seen.push(m);
      },
    }),
  });
  // VS Code puts the failure in a dialog, which it refuses to open in a test run.
  const started = await vscode.debug
    .startDebugging(folder, {
      type: 'yavs',
      request: 'launch',
      name: 'Bad top',
      top: 'nosuch',
      work: 'work:' + folder.uri.fsPath + '/.yavs/work',
    })
    .catch(() => false);
  reg.dispose();
  assert.equal(started, false);
  assert.equal(seen.length, 1);
  assert.equal(seen[0].success, false);
  assert.match(seen[0].message, /error\[E0104\]/);
}

/** `surfer` in a launch configuration: a fake WCP endpoint asks for a declaration, and the file
 * opens at its line. */
async function surferGoesToTheDeclaration(folder, tb, stoppedAgain) {
  let send;
  const added = [];
  const server = net.createServer((sock) => {
    let buf = Buffer.alloc(0);
    send = (m) => sock.write(`${JSON.stringify(m)}\0`);
    sock.on('data', (d) => {
      buf = Buffer.concat([buf, d]);
      for (let end; (end = buf.indexOf(0)) >= 0; buf = buf.subarray(end + 1)) {
        const m = JSON.parse(buf.subarray(0, end).toString());
        if (m.type === 'greeting') {
          send({ type: 'greeting', version: '0', commands: [] });
        } else if (m.command === 'add_items' || m.command === 'add_markers') {
          if (m.command === 'add_items') added.push(m.items);
          send({ type: 'response', command: m.command, ids: [] });
        } else {
          send({ type: 'response', command: 'ack' });
        }
        if (m.command === 'load' || m.command === 'reload') {
          send({ type: 'event', event: 'waveforms_loaded', source: 'x' });
        }
      }
    });
    sock.on('error', () => {});
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const stopped = stoppedAgain();
  const started = await vscode.debug.startDebugging(folder, {
    type: 'yavs',
    request: 'launch',
    name: 'Debug tb in Surfer',
    top: 'tb',
    work: 'work:${workspaceFolder}/.yavs/work',
    stopOnEntry: true,
    surfer: `127.0.0.1:${server.address().port}`,
  });
  assert.ok(started, 'startDebugging failed');
  await stopped;
  const session = vscode.debug.activeDebugSession;
  // After VS Code has shown the stop itself.
  await new Promise((r) => setTimeout(r, 1000));
  send({ type: 'event', event: 'goto_declaration', variable: 'tb.cnt' });
  const t0 = Date.now();
  while (!(vscode.window.activeTextEditor?.document.uri.toString() === tb.toString() &&
           vscode.window.activeTextEditor.selection.active.line === 5)) {
    assert.ok(Date.now() - t0 < 60000, 'the declaration did not open');
    await new Promise((r) => setTimeout(r, 100));
  }
  // "Add to Wave" from a view, and from the word under the cursor.
  await vscode.commands.executeCommand('yavs.addToWave', { variable: { name: 'cnt', evaluateName: 'tb.cnt' } });
  const editor = await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(tb));
  const at = new vscode.Position(6, 10); // `signal v : ...`
  editor.selection = new vscode.Selection(at, at);
  await vscode.commands.executeCommand('yavs.addToWave');
  // "Show Drivers" and the quick pick of "Find Signals or Variables...".
  await vscode.commands.executeCommand('yavs.showDrivers', { variable: { name: 'cnt', evaluateName: 'tb.cnt' } });
  const shown = await session.customRequest('evaluate', { expression: 'drivers cnt', context: 'repl' });
  assert.match(shown.result, /tb\.cnt/);
  const { showInputBox, showQuickPick } = vscode.window;
  vscode.window.showInputBox = async () => '*cn*';
  vscode.window.showQuickPick = async (items) => items;
  // The file reaches even a stop at time 0: "Add to Wave" has gone to the viewer by now.
  assert.ok(added.flat().includes('tb.cnt'), JSON.stringify(added));
  assert.ok(added.flat().includes('tb.v'), JSON.stringify(added));
  added.length = 0;
  await vscode.commands.executeCommand('yavs.findSignals');
  vscode.window.showInputBox = showInputBox;
  vscode.window.showQuickPick = showQuickPick;
  const ended = waitFor('end of the Surfer session', 60000, (r) => vscode.debug.onDidTerminateDebugSession(r));
  await session.customRequest('continue', { threadId: 1 });
  await ended;
  // What "Find Signals" picked. The viewer may be told after the session has ended.
  const t1 = Date.now();
  while (!added.flat().includes('tb.cnt') && Date.now() - t1 < 10000) {
    await new Promise((r) => setTimeout(r, 50));
  }
  assert.ok(added.flat().includes('tb.cnt'), JSON.stringify(added));
  server.close();
}

/** The VUnit flow: a run waits on port 4711, and "yavs: Attach" needs no launch.json entry. A
 * look at the port first must not take the run's one session. */
async function attachWithNoConfiguration(folder, tb, stoppedAgain) {
  vscode.debug.addBreakpoints([
    new vscode.SourceBreakpoint(new vscode.Location(tb, new vscode.Position(19, 0))),
  ]);
  const run = cp.spawn('yavs', ['run', '--work=work:.yavs/work', '--dap=4711', 'tb'], {
    cwd: folder.uri.fsPath,
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  const exited = new Promise((resolve) => run.on('exit', resolve));
  await new Promise((resolve, reject) => {
    run.on('error', reject);
    run.stderr.on('data', (d) => d.toString().includes('waiting for a debugger') && resolve());
  });
  const stopped = stoppedAgain();
  await vscode.commands.executeCommand('yavs.attach');
  const s = await stopped;
  assert.equal(s.reason, 'breakpoint');
  const session = vscode.debug.activeDebugSession;
  assert.equal(session.configuration.request, 'attach');
  assert.equal(session.configuration.port, 4711);
  const st = await session.customRequest('stackTrace', { threadId: s.threadId });
  assert.equal(st.stackFrames[0].line, 20);
  assert.equal(vscode.Uri.file(st.stackFrames[0].source.path).toString(), tb.toString());
  // Offered without a launch.json entry; F5 with nothing configured resolves to it.
  const ended = waitFor('end of the attached session', 60000, (r) => vscode.debug.onDidTerminateDebugSession(r));
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  await new Promise((r) => setTimeout(r, 500));
  await session.customRequest('continue', { threadId: s.threadId });
  await ended;
  await exited;
}

module.exports = { run };
