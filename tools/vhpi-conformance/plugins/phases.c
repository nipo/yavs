/* Tool phases: vhpiPhaseP at load, in a foreign model's elaboration function and in every
 * callback, with the tool handle and with NULL; which tool-phase reasons register and fire;
 * vhpi_control(vhpiReset), vhpi_get_data and vhpi_put_data; the capabilities.
 * SPDX-License-Identifier: MIT OR Apache-2.0 */
#include "common.h"

/* One reference exports neither. */
#pragma weak vhpi_get_data
#pragma weak vhpi_put_data

static void phase(const char *what) {
  vhpiHandleT tool = vhpi_handle(vhpiTool, NULL);
  int a = tool ? vhpi_get(vhpiPhaseP, tool) : -1;
  int ea = err();
  int b = vhpi_get(vhpiPhaseP, NULL);
  int eb = err();
  T("%s: phase tool=%d err=%d null=%d err=%d\n", what, a, ea, b, eb);
}

static void f_elab(const vhpiCbDataT *cb) { (void)cb; phase("func elabf"); }
static void f_exec(const vhpiCbDataT *cb) {
  phase("func execf");
  vhpiValueT v;
  memset(&v, 0, sizeof v);
  v.format = vhpiIntVal;
  v.value.intg = 7;
  vhpi_put_value(cb->obj, &v, vhpiDepositPropagate);
}

static void fired(const vhpiCbDataT *cb) { phase((const char *)cb->user_data); }

static void probe(const vhpiCbDataT *cb);

/* The after-delay callback is registered here: one reference crashes on it at load. */
static void started(const vhpiCbDataT *cb) {
  fired(cb);
  static vhpiTimeT t = {0, 500000};
  vhpiCbDataT d;
  memset(&d, 0, sizeof d);
  d.reason = vhpiCbAfterDelay;
  d.cb_rtn = probe;
  d.time = &t;
  vhpi_register_cb(&d, 0);
}

static void on(int reason, const char *tag) {
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = reason;
  cb.cb_rtn = reason == vhpiCbStartOfSimulation ? started : fired;
  cb.user_data = (void *)tag;
  vhpiHandleT h = vhpi_register_cb(&cb, vhpiReturnCb);
  T("register %s: %d err=%d\n", tag, h != NULL, err());
}

static void probe(const vhpiCbDataT *cb) {
  (void)cb;
  char buf[8];
  phase("after delay");
  if (vhpi_put_data) {
    int d = (int)vhpi_put_data(1, buf, sizeof buf);
    T("vhpi_put_data: %d err=%d\n", d, err());
    d = (int)vhpi_get_data(1, buf, sizeof buf);
    T("vhpi_get_data: %d err=%d\n", d, err());
  }
  T("capabilities %d\n", vhpi_get(vhpiCapabilitiesP, vhpi_handle(vhpiTool, NULL)));
  int rc = vhpi_control(vhpiReset);
  T("vhpi_control reset: %d err=%d\n", rc, err());
}

static void init(void) {
  phase("startup");
  vhpiForeignDataT d;
  memset(&d, 0, sizeof d);
  d.kind = vhpiFuncF;
  d.libraryName = "phases";
  d.modelName = "f";
  d.elabf = f_elab;
  d.execf = f_exec;
  vhpi_register_foreignf(&d);
  on(vhpiCbStartOfTool, "start of tool");
  on(vhpiCbEndOfTool, "end of tool");
  on(vhpiCbStartOfAnalysis, "start of analysis");
  on(vhpiCbEndOfAnalysis, "end of analysis");
  on(vhpiCbStartOfElaboration, "start of elaboration");
  on(vhpiCbEndOfElaboration, "end of elaboration");
  on(vhpiCbStartOfInitialization, "start of initialization");
  on(vhpiCbEndOfInitialization, "end of initialization");
  on(vhpiCbStartOfSimulation, "start of simulation");
  on(vhpiCbEndOfSimulation, "end of simulation");
  on(vhpiCbQuiescense, "quiescense");
  on(vhpiCbPLIError, "pli error");
  on(vhpiCbSigInterrupt, "signal interrupt");
  on(vhpiCbEnterInteractive, "enter interactive");
  on(vhpiCbExitInteractive, "exit interactive");
  on(vhpiCbStartOfSave, "start of save");
  on(vhpiCbEndOfSave, "end of save");
  on(vhpiCbStartOfRestart, "start of restart");
  on(vhpiCbEndOfRestart, "end of restart");
  on(vhpiCbStartOfReset, "start of reset");
  on(vhpiCbEndOfReset, "end of reset");
}

void (*vhpi_startup_routines[])(void) = {init, NULL};
void conf_boot(void) { init(); }
