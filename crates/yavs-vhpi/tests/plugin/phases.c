/* Test VHPI plugin for the tool phases, the reset, the error and the interrupt callbacks (#832),
 * loaded by `yavs run --vhpi=LIB:ENTRY`.
 * SPDX-License-Identifier: MIT OR Apache-2.0 */

#include <string.h>

#include "vhpi_user.h"

#ifdef _WIN32
#define PLUGIN_EXPORT __declspec(dllexport)
#else
#define PLUGIN_EXPORT
#endif

static int err(void) {
  vhpiErrorInfoT e;
  return vhpi_check_error(&e) != 0;
}

static void phase(const vhpiCbDataT *cb) {
  vhpiTimeT t;
  vhpi_get_time(&t, NULL);
  vhpi_printf("%s: phase %d tool %d at %u\n", (const char *)cb->user_data, vhpi_get(vhpiPhaseP, NULL),
              vhpi_get(vhpiPhaseP, vhpi_handle(vhpiTool, NULL)), t.low);
}

static vhpiHandleT on(int reason, const char *tag, void (*f)(const struct vhpiCbDataS *)) {
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = reason;
  cb.cb_rtn = f;
  cb.user_data = (void *)tag;
  return vhpi_register_cb(&cb, vhpiReturnCb);
}

static vhpiHandleT after(unsigned ns, void (*f)(const struct vhpiCbDataS *), int reason) {
  static vhpiTimeT t;
  t.high = 0;
  t.low = ns;
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = reason;
  cb.cb_rtn = f;
  cb.time = &t;
  return vhpi_register_cb(&cb, vhpiReturnCb);
}

static void read_count(const char *what) {
  vhpiValueT v;
  memset(&v, 0, sizeof v);
  v.format = vhpiIntVal;
  vhpi_get_value(vhpi_handle_by_name("top:count", NULL), &v);
  vhpi_printf("%s: count %d\n", what, v.value.intg);
}

static void all_phases(void) {
  on(vhpiCbStartOfTool, "start of tool", phase);
  on(vhpiCbStartOfElaboration, "start of elaboration", phase);
  on(vhpiCbEndOfElaboration, "end of elaboration", phase);
  on(vhpiCbStartOfSimulation, "start of simulation", phase);
  on(vhpiCbStartOfInitialization, "start of initialization", phase);
  on(vhpiCbEndOfInitialization, "end of initialization", phase);
  on(vhpiCbEndOfSimulation, "end of simulation", phase);
  on(vhpiCbEndOfTool, "end of tool", phase);
}

/* phases: every phase that comes, and what a plugin at load time sees. */
PLUGIN_EXPORT void phases(void) {
  vhpi_printf("load: phase %d\n", vhpi_get(vhpiPhaseP, NULL));
  all_phases();
}

/* refused: what stays out, each with its error. */
PLUGIN_EXPORT void refused(void) {
  static const int reasons[] = {vhpiCbStartOfAnalysis, vhpiCbEndOfAnalysis, vhpiCbStartOfSave, vhpiCbEndOfSave,
                                vhpiCbStartOfRestart,  vhpiCbEndOfRestart,  vhpiCbQuiescense};
  for (unsigned i = 0; i < sizeof reasons / sizeof *reasons; i++) {
    vhpiHandleT h = on(reasons[i], "refused", phase);
    vhpi_printf("reason %d: handle %d error %d\n", reasons[i], h != NULL, err());
  }
  char buf[4];
  size_t n = vhpi_get_data(1, buf, sizeof buf);
  vhpi_printf("vhpi_get_data: %d error %d\n", (int)n, err());
  n = vhpi_put_data(1, buf, sizeof buf);
  vhpi_printf("vhpi_put_data: %d error %d\n", (int)n, err());
  int rc = vhpi_control(vhpiReset);
  vhpi_printf("reset at load: %d error %d\n", rc, err());
}

static void reset_report(const vhpiCbDataT *cb) {
  phase(cb);
  read_count((const char *)cb->user_data);
}

static void change(const vhpiCbDataT *cb) {
  (void)cb;
  vhpiTimeT t;
  vhpi_get_time(&t, NULL);
  vhpi_printf("change at %u\n", t.low);
}

static void do_reset(const vhpiCbDataT *cb) {
  (void)cb;
  static int times;
  vhpi_printf("capable %d\n", (vhpi_get(vhpiCapabilitiesP, vhpi_handle(vhpiTool, NULL)) & vhpiProvidesReset) != 0);
  read_count("before reset");
  if (times++ == 0) {
    int rc = vhpi_control(vhpiReset);
    vhpi_printf("vhpi_control: %d error %d\n", rc, err());
  }
}

/* reset: back to time 0 once at 25 ns. */
PLUGIN_EXPORT void reset(void) {
  all_phases();
  on(vhpiCbStartOfReset, "start of reset", reset_report);
  on(vhpiCbEndOfReset, "end of reset", reset_report);
  after(25, do_reset, vhpiCbAfterDelay);
}

static void watch_count(const vhpiCbDataT *cb) {
  (void)cb;
  vhpiCbDataT w;
  memset(&w, 0, sizeof w);
  w.reason = vhpiCbValueChange;
  w.cb_rtn = change;
  w.obj = vhpi_handle_by_name("top:count", NULL);
  vhpi_register_cb(&w, 0);
}

/* reset_watch: reset, with a repeating value-change callback from the start of simulation. */
PLUGIN_EXPORT void reset_watch(void) {
  reset();
  on(vhpiCbStartOfInitialization, "watch", watch_count);
}

/* reset_refused: the option given makes the reset an error. */
PLUGIN_EXPORT void reset_refused(void) {
  after(25, do_reset, vhpiCbAfterDelay);
}

static void pli_error(const vhpiCbDataT *cb) {
  (void)cb;
  vhpiErrorInfoT e;
  int got = vhpi_check_error(&e);
  vhpi_printf("error callback: %d %s\n", got, got ? e.message : "");
  /* An error inside must not fire it again. */
  vhpi_handle_by_name("no_such_thing_either", NULL);
}

/* pli: an error fires vhpiCbPLIError, and the failing caller still sees its error. */
PLUGIN_EXPORT void pli(void) {
  on(vhpiCbPLIError, "error", pli_error);
  vhpiHandleT h = vhpi_handle_by_name("no_such_thing", NULL);
  vhpi_printf("failed: %d caller sees error %d\n", h != NULL, err());
}

/* interrupt: told about Ctrl-C, the signal readable, then the end of the simulation. */
static void interrupted(const vhpiCbDataT *cb) {
  phase(cb);
  read_count("interrupted");
}

static void ready(const vhpiCbDataT *cb) {
  (void)cb;
  vhpi_printf("running\n");
}

PLUGIN_EXPORT void interrupt(void) {
  on(vhpiCbSigInterrupt, "signal interrupt", interrupted);
  on(vhpiCbEndOfSimulation, "end of simulation", phase);
  on(vhpiCbEndOfTool, "end of tool", phase);
  on(vhpiCbStartOfSimulation, "ready", ready);
}

/* console: interactive callbacks around a debugger's stops. */
PLUGIN_EXPORT void console(void) {
  on(vhpiCbEnterInteractive, "enter interactive", phase);
  on(vhpiCbExitInteractive, "exit interactive", phase);
  on(vhpiCbEndOfTool, "end of tool", phase);
}

static void released_change(const vhpiCbDataT *cb) {
  vhpiValueT v;
  memset(&v, 0, sizeof v);
  v.format = vhpiIntVal;
  int rc = vhpi_get_value(cb->obj, &v);
  vhpi_printf("released trigger: %d count %d\n", rc, v.value.intg);
}

static void register_then_release(const vhpiCbDataT *cb) {
  (void)cb;
  vhpiHandleT count = vhpi_handle_by_name("top:count", NULL);
  vhpiCbDataT w;
  memset(&w, 0, sizeof w);
  w.reason = vhpiCbValueChange;
  w.cb_rtn = released_change;
  w.obj = count;
  vhpiHandleT h = vhpi_register_cb(&w, vhpiReturnCb);
  vhpi_release_handle(count);
  vhpiCbDataT info;
  vhpi_get_cb_info(h, &info);
  vhpi_printf("info obj valid: %d\n", vhpi_get(vhpiKindP, info.obj) == vhpiSigDeclK);
}

/* released: the plugin releases the handle it registered a callback with (#273). */
PLUGIN_EXPORT void released(void) {
  on(vhpiCbStartOfSimulation, "start", register_then_release);
}

/* text: printed UTF-8 stays, ISO-8859-1 as a VHPI string value is becomes UTF-8 (#984); a property
 * that only one reference has is an error (#983). */
PLUGIN_EXPORT void text(void) {
  vhpi_printf("utf8 W\xc3\xb6rks\n");
  vhpi_printf("latin1 W\xf6rks\n");
  int seed = vhpi_get(1100, vhpi_handle(vhpiTool, NULL));
  vhpi_printf("seed property: %d error %d\n", seed, err());
}
