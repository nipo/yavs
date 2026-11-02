/* Recorder of the process replay harness (tools/realbench/replay.py, AGENTS.md section 8b): a VHPI plugin that writes every change of
 * the `in` ports of one instance, per delta cycle, to a stimulus file the generated replay testbench reads. Written by Claude.
 *
 *   YAVS_REPLAY_INST  the instance, as VHPI names it (`:tb:u0:core`) or dotted (`tb.u0.core`)
 *   YAVS_REPLAY_OUT   the stimulus file; YAVS_REPLAY_OUT.ports gets the port leaves, generics and the entity's source
 *
 * Stimulus lines: `T <fs> fs` a new time step, `D <delta>` the delta cycle, `<leaf> <value>` a change. Values: l / L std_ulogic and its
 * vectors as characters, e / E positions of other enumerations, i / I integers (vectors space separated). Any other kind stops the run. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "vhpi_user.h"

static FILE *out;
static int nleaf;
static long long last_t = -1;
static long last_d = -1;
static const char LOGIC[] = "UX01ZWLH-";
static char buf[1 << 16];

static void fail(const char *what, const char *name) {
  vhpi_printf("replay recorder: %s: %s\n", what, name ? name : "");
  vhpi_control(vhpiFinish, 2);
  exit(2);
}

/* The leaf's kind letter, from the format yavs reports as its own (vhpiObjTypeVal). */
static char kind_of(vhpiHandleT h) {
  vhpiValueT v = {.format = vhpiObjTypeVal, .bufSize = sizeof buf, .value.ptr = buf};
  if (vhpi_get_value(h, &v) < 0) fail("cannot read", (const char *)vhpi_get_str(vhpiFullNameP, h));
  switch (v.format) {
  case vhpiLogicVal: return 'l';
  case vhpiLogicVecVal: return 'L';
  case vhpiEnumVal: case vhpiSmallEnumVal: case vhpiCharVal: return 'e';
  case vhpiEnumVecVal: case vhpiSmallEnumVecVal: case vhpiStrVal: return 'E';
  case vhpiIntVal: return 'i';
  case vhpiIntVecVal: return 'I';
  default: break;
  }
  fail("a port element of a kind the replay does not drive (real, time, physical or access)", (const char *)vhpi_get_str(vhpiFullNameP, h));
  return 0;
}

/* One value as text: a leaf of kind k. */
static const char *text(vhpiHandleT h, char k) {
  static char t[1 << 18];
  int f = k == 'l' ? vhpiLogicVal : k == 'L' ? vhpiLogicVecVal : k == 'e' ? vhpiEnumVal : k == 'E' ? vhpiEnumVecVal : k == 'i' ? vhpiIntVal : vhpiIntVecVal;
  vhpiValueT v = {.format = f, .bufSize = sizeof buf, .value.ptr = buf};
  if (vhpi_get_value(h, &v) < 0) fail("cannot read", (const char *)vhpi_get_str(vhpiFullNameP, h));
  size_t n = 0;
  switch (k) {
  case 'l': t[n++] = LOGIC[v.value.enumv % 9]; break;
  case 'L': for (int i = 0; i < v.numElems; i++) t[n++] = LOGIC[v.value.enumvs[i] % 9]; break;
  case 'e': n = (size_t)snprintf(t, sizeof t, "%u", (unsigned)v.value.enumv); break;
  case 'i': n = (size_t)snprintf(t, sizeof t, "%d", (int)v.value.intg); break;
  default:
    for (int i = 0; i < v.numElems && n + 16 < sizeof t; i++)
      n += (size_t)snprintf(t + n, sizeof t - n, i ? " %ld" : "%ld", k == 'E' ? (long)v.value.enumvs[i] : (long)v.value.intgs[i]);
  }
  t[n] = 0;
  return t;
}

typedef struct { vhpiHandleT h; char k; } Leaf;
static Leaf *leaves;
static int cap;

static void on_change(const vhpiCbDataT *cb) {
  int i = (int)(long)cb->user_data;
  vhpiTimeT now;
  long delta;
  vhpi_get_time(&now, &delta);
  long long t = ((long long)now.high << 32) | now.low;
  if (t != last_t) fprintf(out, "T %lld fs\n", t), last_t = t, last_d = -1;
  if (delta != last_d) fprintf(out, "D %ld\n", delta), last_d = delta;
  fprintf(out, "%d %s\n", i, text(leaves[i].h, leaves[i].k));
}

static void leaf(FILE *ports, vhpiHandleT h, size_t strip) {
  int composite = vhpi_get(vhpiIsCompositeP, h);
  vhpiHandleT ty = vhpi_handle(vhpiBaseType, vhpi_handle(vhpiType, h));
  int record = ty && vhpi_get(vhpiKindP, ty) == vhpiRecordTypeDeclK;
  int scalar_elems = 0;
  if (composite && !record) {
    vhpiHandleT el = vhpi_handle(vhpiElemType, ty);
    scalar_elems = el && vhpi_get(vhpiIsScalarP, el);
  }
  if (composite && !scalar_elems) {
    vhpiHandleT it = vhpi_iterator(record ? vhpiSelectedNames : vhpiIndexedNames, h), s;
    if (!it) fail("cannot walk the elements of", (const char *)vhpi_get_str(vhpiFullNameP, h));
    while ((s = vhpi_scan(it))) leaf(ports, s, strip);
    return;
  }
  if (nleaf == cap) leaves = realloc(leaves, sizeof *leaves * (size_t)(cap = cap ? 2 * cap : 256));
  char k = kind_of(h);
  leaves[nleaf] = (Leaf){h, k};
  const vhpiCharT *tn = ty ? vhpi_get_str(vhpiNameP, ty) : NULL;
  fprintf(ports, "L %d %c %s %s\n", nleaf, k, (const char *)vhpi_get_str(vhpiFullNameP, h) + strip, tn && *tn ? (const char *)tn : "?");
  vhpiCbDataT cb = {.reason = vhpiCbValueChange, .cb_rtn = on_change, .obj = h, .user_data = (void *)(long)nleaf};
  if (!vhpi_register_cb(&cb, vhpiReturnCb)) fail("cannot watch", (const char *)vhpi_get_str(vhpiFullNameP, h));
  nleaf++;
}

static void generic(FILE *ports, vhpiHandleT g) {
  const char *name = (const char *)vhpi_get_str(vhpiNameP, g);
  vhpiValueT v = {.format = vhpiObjTypeVal, .bufSize = sizeof buf, .value.ptr = buf};
  if (vhpi_get_value(g, &v) < 0) {  /* records and arrays of composites: the generator uses the declared default */
    fprintf(ports, "G %s ?\n", name);
    return;
  }
  switch (v.format) {
  case vhpiIntVal: fprintf(ports, "G %s i %d\n", name, (int)v.value.intg); break;
  case vhpiLongIntVal: fprintf(ports, "G %s i %lld\n", name, (long long)v.value.longintg); break;
  case vhpiEnumVal: case vhpiSmallEnumVal: case vhpiLogicVal: case vhpiCharVal: {
    vhpiValueT e = {.format = vhpiEnumVal};
    vhpi_get_value(g, &e);
    fprintf(ports, "G %s e %u\n", name, (unsigned)e.value.enumv);
    break;
  }
  case vhpiRealVal: fprintf(ports, "G %s r %.17g\n", name, v.value.real); break;
  case vhpiStrVal: fprintf(ports, "G %s s %.*s\n", name, v.numElems, (const char *)buf); break;
  case vhpiLogicVecVal: case vhpiSmallEnumVecVal: case vhpiEnumVecVal: {
    vhpiValueT s = {.format = vhpiBinStrVal, .bufSize = sizeof buf, .value.str = (vhpiCharT *)buf};
    if (vhpi_get_value(g, &s) >= 0) { fprintf(ports, "G %s b %s\n", name, buf); break; }
  } /* fall through */
  default: fprintf(ports, "G %s ?\n", name);
  }
}

static void at_end(const vhpiCbDataT *cb) {
  (void)cb;
  if (out) fclose(out), out = NULL;
}

static void start(const vhpiCbDataT *cb) {
  (void)cb;
  const char *want = getenv("YAVS_REPLAY_INST"), *path = getenv("YAVS_REPLAY_OUT");
  if (!want || !path) fail("set YAVS_REPLAY_INST and YAVS_REPLAY_OUT", "");
  vhpiHandleT inst = vhpi_handle_by_name(want, NULL);
  if (!inst) fail("no such instance", want);
  char pp[4096];
  snprintf(pp, sizeof pp, "%s.ports", path);
  FILE *ports = fopen(pp, "w");
  out = fopen(path, "w");
  if (!ports || !out) fail("cannot write", path);
  setvbuf(out, NULL, _IOFBF, 1 << 22);
  const char *full = (const char *)vhpi_get_str(vhpiFullNameP, inst);
  vhpiHandleT du = vhpi_handle(vhpiDesignUnit, inst), pu = du ? vhpi_handle(vhpiPrimaryUnit, du) : NULL;
  fprintf(ports, "I %s\n", full);
  fprintf(ports, "E %s %s %s\n", du && vhpi_get_str(vhpiLibLogicalNameP, du) ? (const char *)vhpi_get_str(vhpiLibLogicalNameP, du) : "?", pu ? (const char *)vhpi_get_str(vhpiNameP, pu) : "?",
          du ? (const char *)vhpi_get_str(vhpiNameP, du) : "?");
  fprintf(ports, "F %s\n", pu ? (const char *)vhpi_get_str(vhpiFileNameP, pu) : "?");
  vhpiHandleT it = vhpi_iterator(vhpiGenericDecls, inst), h;
  while (it && (h = vhpi_scan(it))) generic(ports, h);
  size_t strip = strlen(full) + 1;
  it = vhpi_iterator(vhpiPortDecls, inst);
  while (it && (h = vhpi_scan(it))) {
    int mode = vhpi_get(vhpiModeP, h);
    fprintf(ports, "P %s %s\n", (const char *)vhpi_get_str(vhpiNameP, h), mode == vhpiInMode ? "in" : mode == vhpiOutMode ? "out" : mode == vhpiBufferMode ? "buffer" : "inout");
    if (mode == vhpiInMode) leaf(ports, h, strip);
    else if (mode != vhpiOutMode && mode != vhpiBufferMode) fail("an inout or linkage port: the replay drives in ports only", (const char *)vhpi_get_str(vhpiFullNameP, h));
  }
  fclose(ports);
  /* the values at the start: written as time 0, before the first delta */
  fprintf(out, "T 0 fs\nD -1\n");
  for (int i = 0; i < nleaf; i++) fprintf(out, "%d %s\n", i, text(leaves[i].h, leaves[i].k));
  last_t = 0;
  last_d = -1;
}

static void startup(void) {
  vhpiCbDataT cb = {.reason = vhpiCbEndOfInitialization, .cb_rtn = start};
  vhpi_register_cb(&cb, 0);
  vhpiCbDataT e = {.reason = vhpiCbEndOfSimulation, .cb_rtn = at_end};
  vhpi_register_cb(&e, 0);
}

void (*vhpi_startup_routines[])(void) = {startup, NULL};
