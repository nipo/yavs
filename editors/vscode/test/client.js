// A VS Code style session against `yavs run --dap=stdio`, driven by the DAP test client
// (@vscode/debugadapter-testsupport). Usage:
//   node test/client.js [path/to/yavs]
// It analyzes ../sample/tb.vhd into a scratch library and runs the steps VS Code takes.
'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawn, spawnSync } = require('child_process');
const { DebugClient } = require('@vscode/debugadapter-testsupport');

const yavs = path.resolve(process.argv[2] || 'yavs');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'yavs-vscode-'));
const src = path.join(dir, 'tb.vhd');
fs.copyFileSync(path.join(__dirname, '..', 'sample', 'tb.vhd'), src);
const work = `work:${path.join(dir, 'work')}`;
const a = spawnSync(yavs, ['analyze', `--work=${work}`, src], { encoding: 'utf8' });
assert.equal(a.status, 0, a.stderr);

// The extension's command line: `yavs run --work=... --dap=stdio TOP`. The client spawns
// RUNTIME EXECUTABLE; with a shell the second word carries all arguments.
const q = (s) => JSON.stringify(s);
const dc = new DebugClient(q(yavs), `run ${q('--work=' + work)} --dap=stdio tb`, 'yavs', {
  shell: true,
  cwd: dir,
});
dc.defaultTimeout = 10000;

async function main() {
  await dc.start();
  // VS Code listens for `initialized` from the start: it follows the initialize response.
  const initialized = dc.waitForEvent('initialized');
  // What VS Code sends.
  const init = await dc.initializeRequest({
    clientID: 'vscode',
    clientName: 'Visual Studio Code',
    adapterID: 'yavs',
    pathFormat: 'path',
    linesStartAt1: true,
    columnsStartAt1: true,
    supportsVariableType: true,
    supportsVariablePaging: true,
    supportsRunInTerminalRequest: true,
    locale: 'en',
  });
  assert.equal(init.body.supportsConfigurationDoneRequest, true);
  assert.equal(init.body.supportsEvaluateForHovers, true);
  assert.equal(init.body.supportsDataBreakpoints, true);

  // Launch, a breakpoint on `i := twice(k);`, configurationDone, stopped.
  await Promise.all([
    initialized.then(async () => {
      const bp = await dc.setBreakpointsRequest({
        source: { name: 'tb.vhd', path: src },
        lines: [20],
        breakpoints: [{ line: 20 }],
        sourceModified: false,
      });
      assert.equal(bp.body.breakpoints[0].verified, true);
      assert.equal(bp.body.breakpoints[0].line, 20);
      await dc.setExceptionBreakpointsRequest({ filters: [] });
      await dc.configurationDoneRequest();
    }),
    dc.launchRequest({ type: 'yavs', request: 'launch', name: 'Debug tb', top: 'tb', work }),
    dc.assertStoppedLocation('breakpoint', { path: src, line: 20 }),
  ]);

  const threads = await dc.threadsRequest();
  assert.deepEqual(threads.body.threads.map((t) => t.name), ['tb.p', 'tb.q']);

  // The console's tools in the Debug Console, and the list behind the quick pick.
  const drivers = await dc.evaluateRequest({ expression: 'drivers cnt', context: 'repl' });
  assert.match(drivers.body.result, /process tb\.\w+/);
  const found = await dc.customRequest('yavs/find', { kind: 'signals', pattern: '*cn*' });
  assert.equal(found.body.items[0].path, 'tb.cnt');
  const caps = await dc.setExceptionBreakpointsRequest({ filters: ['error'] });
  assert.ok(caps.success);
  await dc.setExceptionBreakpointsRequest({ filters: [] });

  // Live waves of `cnt` from here on: the hierarchy, then batches of time steps.
  const waves = [];
  dc.on('yavs/waves', (e) => waves.push(e.body));
  const sub = await dc.customRequest('yavs/waveSubscribe', { include: ['tb.cnt'], flushMs: 0 });
  assert.deepEqual(sub.body.scopes[0].children.map((c) => [c.name, c.id, c.type.kind]),
    [['cnt', 0, 'integer']]);

  // VS Code asks for the first frame, then the rest.
  let st = await dc.stackTraceRequest({ threadId: 1, startFrame: 0, levels: 1 });
  const top = st.body.stackFrames[0];
  assert.ok(fs.existsSync(top.source.path), `VS Code cannot open ${top.source.path}`);
  assert.ok(path.isAbsolute(top.source.path));

  const scopes = await dc.scopesRequest({ frameId: top.id });
  assert.deepEqual(scopes.body.scopes.map((s) => s.name),
    ['Locals', 'Signals', 'Generics', 'Hierarchy']);
  const vars = async (i) =>
    Object.fromEntries(
      (await dc.variablesRequest({ variablesReference: scopes.body.scopes[i].variablesReference }))
        .body.variables.map((v) => [v.name, v.value]));
  assert.deepEqual(await vars(0), { i: '0', k: '1' });
  assert.deepEqual(await vars(1), { cnt: '0', v: '"0101"', pairs: '("01", "10")' });
  assert.deepEqual(await vars(2), { N: '3', NAME: '"dap"' });
  const hover = await dc.evaluateRequest({ expression: 'v', frameId: top.id, context: 'hover' });
  assert.equal(hover.body.result, '"0101"');

  // A vector is its one line; one bit through an expression.
  assert.equal(hover.body.variablesReference, 0);
  assert.equal(hover.body.indexedVariables, undefined);
  const bit = await dc.evaluateRequest({ expression: 'v(2)', frameId: top.id, context: 'watch' });
  assert.equal(bit.body.result, "'1'");

  // Other composites expand: VS Code pages an array's elements through `indexedVariables`, and a
  // vector in them is one line.
  const pairs = await dc.evaluateRequest({ expression: 'tb.pairs', frameId: top.id, context: 'watch' });
  assert.equal(pairs.body.indexedVariables, 2);
  const elems = await dc.variablesRequest({
    variablesReference: pairs.body.variablesReference, filter: 'indexed', start: 1, count: 1,
  });
  assert.deepEqual(elems.body.variables.map((v) => [v.name, v.value, v.variablesReference]), [['1', '"10"', 0]]);

  // A Watch of what does not exist is an error with the right name, in the dotted form.
  await assert.rejects(
    dc.evaluateRequest({ expression: 'tb.cn', frameId: top.id, context: 'watch' }),
    /no variable or signal `tb\.cn` here: .*`tb\.cnt`/,
  );
  await assert.rejects(
    dc.evaluateRequest({ expression: ':tb:cn', frameId: top.id, context: 'watch' }),
    /`tb\.cnt`/,
  );
  assert.equal((await dc.evaluateRequest({ expression: ':tb:cnt', frameId: top.id, context: 'watch' })).body.result, '0');

  // The hierarchy: a custom request, and a scope whose regions expand.
  const h = await dc.customRequest('yavs/hierarchy');
  assert.deepEqual([h.body.regions[0].name, h.body.regions[0].processes], ['tb', ['tb.p', 'tb.q']]);
  const roots = (await dc.variablesRequest({
    variablesReference: scopes.body.scopes[3].variablesReference,
  })).body.variables;
  assert.deepEqual(roots.map((v) => [v.name, v.value]), [['tb', 'instance']]);
  const inTb = (await dc.variablesRequest({ variablesReference: roots[0].variablesReference }))
    .body.variables.map((v) => v.name);
  assert.deepEqual(inTb, ['p', 'q', 'cnt', 'v', 'pairs']);

  // "Set Value" in the Variables view: a local is written now, a signal deposited.
  assert.equal(init.body.supportsSetVariable, true);
  const setv = (i, name, value) => dc.setVariableRequest({
    variablesReference: scopes.body.scopes[i].variablesReference, name, value,
  });
  assert.equal((await setv(0, 'i', '16#7#')).body.value, '7');
  assert.equal((await vars(0)).i, '7');
  await assert.rejects(setv(0, 'i', 'x"7"'), /integer/);
  assert.equal((await setv(1, 'v', 'x"F"')).body.value, '"1111"');
  // The extension's "Force a signal..." on `v`: no mode, no quotes; released further down.
  const sigs = (await dc.variablesRequest({
    variablesReference: scopes.body.scopes[1].variablesReference,
  })).body.variables;
  assert.equal(sigs[1].__vscodeVariableMenuContext, 'signal');
  const forced = await dc.customRequest('yavs/force', { signal: 'v', value: '1010' });
  assert.equal(forced.body.signal, 'tb.v');
  await assert.rejects(dc.customRequest('yavs/force', { signal: 'cnt', value: 'abc' }), /`abc` is not a value of `tb\.cnt`, type INTEGER/);
  const part = await dc.customRequest('yavs/deposit', { signal: 'v(0)', value: '1' });
  assert.deepEqual([part.body.signal, part.body.value], ['tb.v(0)', "'1'"]);

  // Step into the function, over its first statement, back out.
  await Promise.all([dc.stepInRequest({ threadId: 1 }), dc.waitForEvent('stopped')]);
  st = await dc.stackTraceRequest({ threadId: 1 });
  // The subprogram header, its variable declaration (initialised on entry), then its body.
  assert.deepEqual(st.body.stackFrames.map((f) => [f.name, f.line]), [['twice', 9], ['tb.p', 20]]);
  const lines = [];
  for (let n = 0; n < 2; n++) {
    await Promise.all([dc.nextRequest({ threadId: 1 }), dc.waitForEvent('stopped')]);
    st = await dc.stackTraceRequest({ threadId: 1 });
    lines.push(st.body.stackFrames[0].line);
  }
  assert.deepEqual(lines, [10, 12]);
  await Promise.all([dc.stepOutRequest({ threadId: 1 }), dc.waitForEvent('stopped')]);
  st = await dc.stackTraceRequest({ threadId: 1 });
  assert.deepEqual(st.body.stackFrames.map((f) => [f.name, f.line]), [['tb.p', 21]]);

  await dc.setBreakpointsRequest({ source: { path: src }, lines: [], breakpoints: [] });
  // "Break on Value Change" on `cnt` in the Variables view: VS Code asks for its data id, then
  // sets every data breakpoint.
  const info = await dc.dataBreakpointInfoRequest({
    variablesReference: scopes.body.scopes[1].variablesReference,
    name: 'cnt',
  });
  assert.equal(info.body.dataId, ':tb:cnt');
  assert.deepEqual(info.body.accessTypes, ['write']);
  const local = await dc.dataBreakpointInfoRequest({
    variablesReference: scopes.body.scopes[0].variablesReference,
    name: 'i',
  });
  assert.equal(local.body.dataId, null);
  const set = await dc.setDataBreakpointsRequest({
    breakpoints: [{ dataId: info.body.dataId, accessType: 'write' }],
  });
  assert.equal(set.body.breakpoints[0].verified, true);
  const stop = async (resume) => (await Promise.all([resume, dc.waitForEvent('stopped')]))[1].body;
  let s = await stop(dc.continueRequest({ threadId: 1 }));
  assert.deepEqual([s.reason, s.description, s.text],
    ['data breakpoint', 'data breakpoint at 0 fs, delta 1', 'tb.cnt = 2']);

  // The force holds while the process toggling `v` runs; release it.
  assert.equal((await dc.evaluateRequest({ expression: 'tb.v' })).body.result, '"1010"');
  await dc.customRequest('yavs/release', { signal: 'tb.v' });

  // A signal breakpoint on a value (the extension's "Break when a signal equals...").
  await dc.setDataBreakpointsRequest({ breakpoints: [] });
  const eq = await dc.customRequest('yavs/setSignalBreakpoints', {
    breakpoints: [{ signal: 'tb.cnt', value: '6' }],
  });
  assert.equal(eq.body.breakpoints[0].verified, true);
  s = await stop(dc.continueRequest({ threadId: 1 }));
  assert.deepEqual([s.reason, s.description, s.text],
    ['data breakpoint', 'data breakpoint at 20 ns, delta 1', 'tb.cnt = 6']);
  await dc.customRequest('yavs/setSignalBreakpoints', { breakpoints: [] });

  // Conditional line breakpoints: `report "done"` stops when cnt = 6, `wait;` never.
  const cb = await dc.setBreakpointsRequest({
    source: { path: src },
    breakpoints: [{ line: 24, condition: 'cnt = 6 and v /= "1111"' }, { line: 25, condition: 'cnt = 7' },
      { line: 22, condition: 'cnt =' }],
  });
  assert.deepEqual(cb.body.breakpoints.map((b) => b.verified), [true, true, false]);
  assert.match(cb.body.breakpoints[2].message, /does not parse/);
  s = await stop(dc.continueRequest({ threadId: 1 }));
  assert.deepEqual([s.reason, s.description], ['breakpoint', 'breakpoint at 30 ns, delta 0']);
  // The console's commands in the Debug Console (context `repl`); other contexts stay evaluations.
  const repl = async (expression) => (await dc.evaluateRequest({ expression, context: 'repl' })).body.result;
  assert.match(await repl('help'), /back to time 0/);
  assert.match(await repl('where'), /^time /);
  assert.match(await repl('break list'), /tb\.vhd|no breakpoints|:/);
  await assert.rejects(dc.evaluateRequest({ expression: 'where', context: 'hover' }), /no variable or signal/);
  const [, back] = await Promise.all([repl('restart'), dc.waitForEvent('stopped')]);
  assert.equal(back.body.reason, 'entry');
  assert.match(await repl('where'), /^time 0 fs, delta 0/);
  const [, ran] = await Promise.all([repl('run 1 ns'), dc.waitForEvent('stopped')]);
  assert.match(ran.body.description, /^step at 1 ns/);
  await dc.setBreakpointsRequest({ source: { path: src }, breakpoints: [] });

  // Run to the end.
  const out = dc.waitForEvent('output');
  const exited = dc.waitForEvent('exited');
  await Promise.all([dc.continueRequest({ threadId: 1 }), dc.waitForEvent('terminated')]);
  assert.match((await out).body.output, /done/);
  assert.equal((await exited).body.exitCode, 0);
  const cnt = waves.flatMap((b) => b.changes.map((c) => decodeWave(sub.body.scopes[0].children[c[1]], c[2])));
  assert.deepEqual(cnt.filter((v, i) => v !== cnt[i - 1]).slice(-3), [2, 4, 6], cnt);
  assert.equal(waves[waves.length - 1].to, 30000000);
  await dc.disconnectRequest({ restart: false });
}

// A `yavs/waves` value: base64 of the bytes, read by the signal's `encoding` in the hierarchy.
function decodeWave(sig, b64) {
  const b = Buffer.from(b64, 'base64');
  switch (sig.encoding.layout) {
    case 'i64le': return Number(b.readBigInt64LE(0));
    case 'f64le': return b.readDoubleLE(0);
    case 'u32le': return sig.type.literals[b.readUInt32LE(0)];
    case 'u8-per-element': return Array.from(b, (p) => sig.type.element.literals[p]).join('');
    default: throw new Error(`unknown layout ${sig.encoding.layout}`);
  }
}

// A launch that cannot start: VS Code shows the failure of the launch request (#934).
async function failedLaunch() {
  const bad = new DebugClient(q(yavs), `run ${q('--work=' + work)} --dap=stdio nosuch`, 'yavs', {
    shell: true,
    cwd: dir,
  });
  bad.defaultTimeout = 10000;
  await bad.start();
  await bad.initializeRequest({ clientID: 'vscode', adapterID: 'yavs', linesStartAt1: true, columnsStartAt1: true });
  await assert.rejects(bad.launch({}), (e) => {
    assert.match(e.message, /error\[E0104\]: .*nosuch/);
    return true;
  });
  await bad.stop();
}

const more = path.join(dir, 'more.vhd');

/** A session on `more_tb`, with `generics` and started by `start`. */
async function moreClient(generics, run) {
  fs.copyFileSync(path.join(__dirname, '..', 'sample', 'more.vhd'), more);
  const r = spawnSync(yavs, ['analyze', `--work=${work}`, more], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  const c = new DebugClient(q(yavs), `run ${q('--work=' + work)} ${generics} --dap=stdio more_tb`, 'yavs', {
    shell: true,
    cwd: dir,
  });
  c.defaultTimeout = 10000;
  await c.start();
  try {
    await c.initializeRequest({ clientID: 'vscode', adapterID: 'yavs', linesStartAt1: true, columnsStartAt1: true });
    await run(c);
  } finally {
    c.stopAdapter();
  }
}

/** #938, #959, #942: a shared variable in the views and the names of unlabelled processes. */
async function sharedAndThreads() {
  await moreClient('', async (c) => {
    await c.launchRequest({});
    await c.setBreakpointsRequest({ source: { path: more }, breakpoints: [{ line: 32 }] });
    const stopped = c.waitForEvent('stopped');
    await c.configurationDoneRequest();
    const s = await stopped;
    const names = (await c.threadsRequest()).body.threads.map((t) => t.name);
    assert.deepEqual(names, ['more_tb.clk <= ... (more.vhd:21)', 'more_tb.process (more.vhd:23)']);
    const st = await c.stackTraceRequest({ threadId: s.body.threadId });
    const top = st.body.stackFrames[0];
    assert.equal(top.name, 'more_tb.process (more.vhd:23)');
    const scopes = (await c.scopesRequest({ frameId: top.id })).body.scopes;
    const shared = scopes.find((x) => x.name === 'Shared Variables');
    assert.ok(shared, JSON.stringify(scopes));
    const list = (await c.variablesRequest({ variablesReference: shared.variablesReference })).body.variables;
    assert.equal(list[0].name, 'tally');
    assert.equal(list[0].value, 'protected');
    const state = await c.variablesRequest({ variablesReference: list[0].variablesReference });
    assert.equal(state.body.variables[0].name, 'total');
    assert.equal(state.body.variables[0].value, '3');
    for (const context of ['watch', 'hover']) {
      const v = await c.evaluateRequest({ expression: 'tally', frameId: top.id, context });
      assert.equal(v.body.result, 'protected');
    }
    const part = await c.evaluateRequest({ expression: 'tally.total', frameId: top.id, context: 'watch' });
    assert.equal(part.body.result, '3');
    const repl = (expression) => c.evaluateRequest({ expression, frameId: top.id, context: 'repl' });
    assert.match((await repl('describe tally')).body.result, /protected object/);
    assert.match((await repl('find variables tally')).body.result, /more_tb\.tally +protected object/);
    await c.disconnectRequest({});
  });
}

/** #939: the report comes while the process is still running, and a pause still stops it. */
async function liveReport() {
  await moreClient('-gSPIN=true', async (c) => {
    await c.launchRequest({});
    const out = c.waitForEvent('output');
    await c.configurationDoneRequest();
    assert.match((await out).body.output, /before the loop/);
    const stopped = c.waitForEvent('stopped');
    await c.pauseRequest({ threadId: 1 });
    await stopped;
    await c.disconnectRequest({});
  });
}

/** #963: an attached client's restart: the run starts again and stops at the breakpoint. */
async function attachedRestart() {
  const r = spawnSync(yavs, ['analyze', `--work=${work}`, src], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  const run = spawn(yavs, ['run', `--work=${work}`, '--dap=0', 'tb'], { cwd: dir, stdio: ['ignore', 'ignore', 'pipe'] });
  const exited = new Promise((resolve) => run.on('exit', resolve));
  const port = await new Promise((resolve) => {
    run.stderr.on('data', (d) => {
      const m = /listening on 127\.0\.0\.1:(\d+)/.exec(d.toString());
      if (m) resolve(Number(m[1]));
    });
  });
  const c = new DebugClient('unused', 'unused', 'yavs');
  c.defaultTimeout = 10000;
  await c.start(port);
  const init = await c.initializeRequest({ clientID: 'vscode', adapterID: 'yavs', linesStartAt1: true, columnsStartAt1: true });
  assert.equal(init.body.supportsRestartRequest, true);
  await c.attachRequest({});
  await c.setBreakpointsRequest({ source: { path: src }, breakpoints: [{ line: 22 }] });
  let stopped = c.waitForEvent('stopped');
  await c.configurationDoneRequest();
  await stopped;
  const before = await c.evaluateRequest({ expression: ':tb:cnt' });
  stopped = c.waitForEvent('stopped');
  await c.restartRequest({ arguments: {} });
  const s = await stopped;
  assert.equal(s.body.reason, 'breakpoint');
  assert.match(s.body.description, /^breakpoint at 0 fs/);
  assert.equal(before.body.result, '0');
  await c.disconnectRequest({});
  c.stopAdapter();
  await exited;
}

// Exit 1 unless the session passed (an unsettled promise must not end as success).
process.exitCode = 1;
main()
  .then(failedLaunch)
  .then(sharedAndThreads)
  .then(liveReport)
  .then(attachedRestart)
  .then(() => {
    fs.rmSync(dir, { recursive: true, force: true });
    console.log('ok: VS Code style session passed');
    process.exitCode = 0;
  })
  .catch((e) => console.error(e))
  .finally(() => dc.stopAdapter());
