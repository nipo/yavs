/* Test VHPI plugin for connectivity (#830): drivers, contributors, loads and transactions of
 * signals, and transaction, force and release callbacks. Loaded by `yavs run --vhpi=LIB:ENTRY`.
 * SPDX-License-Identifier: MIT OR Apache-2.0 */

#include <stdio.h>
#include <string.h>

#include "vhpi_user.h"

#ifdef _WIN32
#define PLUGIN_EXPORT __declspec(dllexport)
#else
#define PLUGIN_EXPORT
#endif

#define STR(prop, h) ((const char *)vhpi_get_str(prop, h))

static int err(void) {
  vhpiErrorInfoT e;
  return vhpi_check_error(&e) != 0;
}

static void list(const char *what, int rel, vhpiHandleT o) {
  vhpi_printf("  %s:", what);
  vhpiHandleT it = vhpi_iterator(rel, o);
  for (vhpiHandleT x; it && (x = vhpi_scan(it));) {
    vhpiHandleT f = NULL;
    int kind = vhpi_get(vhpiKindP, x);
    if (kind == vhpiInPortK || kind == vhpiOutPortK) {
      vhpi_printf(" %s(%s<-%s)", vhpi_get_str(vhpiKindStrP, x),
                  vhpi_get_str(vhpiFullCaseNameP, vhpi_handle(vhpiFormal, x)),
                  vhpi_get_str(vhpiFullCaseNameP, vhpi_handle(vhpiActual, x)));
    } else if (kind == vhpiDriverK) {
      f = vhpi_handle(vhpiEqProcessStmt, x);
      vhpi_printf(" driver(%s of %s)", f ? STR(vhpiFullCaseNameP, f) : "?",
                  vhpi_get_str(vhpiFullCaseNameP, vhpi_handle(vhpiSignal, x)));
    } else {
      vhpi_printf(" %s", vhpi_get_str(vhpiFullCaseNameP, x));
    }
  }
  vhpi_printf("\n");
}

static void show(const char *name) {
  vhpiHandleT s = vhpi_handle_by_name(name, NULL);
  vhpi_printf("%s: resolved=%d kind=%d basic=%d forced=%d\n", name, vhpi_get(vhpiIsResolvedP, s),
              vhpi_get(vhpiSigKindP, s), vhpi_get(vhpiIsBasicP, s), vhpi_get(vhpiIsForcedP, s));
  list("drivers", vhpiDrivers, s);
  list("local contributors", vhpiLocalContributors, s);
  list("optimized contributors", vhpiOptimizedContributors, s);
  list("local loads", vhpiLocalLoads, s);
  list("optimized loads", vhpiOptimizedLoads, s);
  vhpiHandleT net = vhpi_handle(vhpiSimNet, s);
  vhpi_printf("  net: %s\n", net ? STR(vhpiFullCaseNameP, net) : "none");
  vhpiHandleT rf = vhpi_handle(vhpiResolFunc, s);
  vhpi_printf("  resolution: %s %s\n", rf ? STR(vhpiKindStrP, rf) : "none",
              rf ? STR(vhpiCaseNameP, rf) : "");
}

/* Drivers and transactions of each driver when initialization ends. */
static void graph_cb(const vhpiCbDataT *d) {
  (void)d;
  show("top:bus_s");
  show("top:v");
  show("top:v(2)");
  show("top:u1:o");
  show("top:u1:i");
  show("top:clk");
  show("top:u2:o");
  list("basic signals of r", vhpiBasicSignals, vhpi_handle_by_name("top:r", NULL));
  list("signals p1 drives", vhpiDrivenSigs, vhpi_handle_by_name("top:p1", NULL));
  vhpiHandleT it = vhpi_iterator(vhpiDrivers, vhpi_handle_by_name("top:bus_s", NULL));
  for (vhpiHandleT dr; it && (dr = vhpi_scan(it));) {
    char buf[8];
    vhpiValueT v;
    memset(&v, 0, sizeof v);
    v.format = vhpiBinStrVal;
    v.bufSize = sizeof buf;
    v.value.str = (vhpiCharT *)buf;
    vhpi_get_value(dr, &v);
    vhpi_printf("driver value %s size %d, transactions:\n", buf, vhpi_get(vhpiSizeP, dr));
    vhpiHandleT ti = vhpi_iterator(vhpiTransactions, dr);
    for (vhpiHandleT t; ti && (t = vhpi_scan(ti));) {
      vhpiPhysT at = vhpi_get_phys(vhpiTimeP, t);
      vhpi_get_value(t, &v);
      vhpi_printf("  %s at %u null=%d value %s\n", vhpi_get_str(vhpiKindStrP, t), at.low,
                  vhpi_get(vhpiIsNullP, t), buf);
    }
  }
}

PLUGIN_EXPORT void graph(void) {
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = vhpiCbEndOfInitialization;
  cb.cb_rtn = graph_cb;
  vhpi_register_cb(&cb, 0);
}

static void event(const vhpiCbDataT *d) {
  vhpi_printf("%s %s at %u: %s\n", (const char *)d->user_data,
              vhpi_get_str(vhpiFullCaseNameP, d->obj), d->time->low,
              d->value ? (char *)d->value->value.str : "-");
}

static char values[8][32];
static vhpiValueT slots[8];
static int used;

static void watch(int reason, vhpiHandleT o, const char *tag) {
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = reason;
  cb.cb_rtn = event;
  cb.obj = o;
  slots[used].format = vhpiBinStrVal;
  slots[used].bufSize = sizeof values[used];
  slots[used].value.str = (vhpiCharT *)values[used];
  cb.value = &slots[used++];
  cb.user_data = (void *)tag;
  vhpi_register_cb(&cb, 0);
}

/* Each kind of callback on a resolved signal, a driver of it and a forced signal. */
PLUGIN_EXPORT void cbs(void) {
  vhpiHandleT bus = vhpi_handle_by_name("top:bus_s", NULL);
  watch(vhpiCbTransaction, bus, "transaction");
  vhpiHandleT it = vhpi_iterator(vhpiDrivers, bus);
  vhpiHandleT first = vhpi_scan(it);
  vhpi_release_handle(it);
  watch(vhpiCbTransaction, first, "driver");
  watch(vhpiCbTransaction, vhpi_handle_by_name("top:same", NULL), "unchanged");
  /* A port collapsed into one element of its actual reads that element. */
  watch(vhpiCbValueChange, vhpi_handle_by_name("top:u1:o", NULL), "changed");
  vhpiHandleT n = vhpi_handle_by_name("top:f", NULL);
  watch(vhpiCbForce, n, "force");
  watch(vhpiCbRelease, n, "release");
  /* Misuse: not a signal. */
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = vhpiCbForce;
  cb.cb_rtn = event;
  cb.obj = vhpi_handle_by_name("top:c", NULL);
  vhpiHandleT h = vhpi_register_cb(&cb, vhpiReturnCb);
  vhpi_printf("force on a constant: %s error=%d\n", h ? "handle" : "NULL", err());
}

static void put_force(const vhpiCbDataT *d) {
  vhpiValueT v;
  memset(&v, 0, sizeof v);
  v.format = vhpiLogicVal;
  v.value.enumv = vhpi1;
  vhpiHandleT s = vhpi_handle_by_name("top:g", NULL);
  vhpi_put_value(s, &v, *(int *)d->user_data);
}

/* A force and a release made through VHPI reach the same callbacks; a forced flag follows. */
static int fm = vhpiForcePropagate, rm = vhpiRelease;
static void forced_flag(const vhpiCbDataT *d) {
  vhpi_printf("%s at %u forced=%d\n", (const char *)d->user_data, d->time->low,
              vhpi_get(vhpiIsForcedP, d->obj));
}

PLUGIN_EXPORT void vhpiforce(void) {
  vhpiHandleT s = vhpi_handle_by_name("top:g", NULL);
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.cb_rtn = forced_flag;
  cb.obj = s;
  cb.reason = vhpiCbForce;
  cb.user_data = (void *)"force";
  vhpi_register_cb(&cb, 0);
  cb.reason = vhpiCbRelease;
  cb.user_data = (void *)"release";
  vhpi_register_cb(&cb, 0);
  static vhpiTimeT t1 = {0, 2}, t2 = {0, 5};
  vhpiCbDataT at;
  memset(&at, 0, sizeof at);
  at.reason = vhpiCbAfterDelay;
  at.cb_rtn = put_force;
  at.time = &t1;
  at.user_data = &fm;
  vhpi_register_cb(&at, 0);
  at.time = &t2;
  at.user_data = &rm;
  vhpi_register_cb(&at, 0);
}
