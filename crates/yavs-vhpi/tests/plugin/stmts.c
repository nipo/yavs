/* Test VHPI plugin for statements (#829): equivalent processes as regions, their sensitivity
 * and flags, sequential statements, and what still has no implementation.
 * Loaded by `yavs run --vhpi=LIB:ENTRY`. SPDX-License-Identifier: MIT OR Apache-2.0 */

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

static void seq(vhpiHandleT o, int depth) {
  vhpiHandleT it = vhpi_iterator(vhpiSeqStmts, o);
  for (vhpiHandleT x; it && (x = vhpi_scan(it));) {
    vhpi_printf("%*s%s line=%d end=%d seq=%d\n", depth, "", STR(vhpiKindStrP, x), vhpi_get(vhpiLineNoP, x),
                vhpi_get(vhpiEndLineNoP, x), vhpi_get(vhpiIsSeqStmtP, x));
    seq(x, depth + 2);
  }
}

PLUGIN_EXPORT void start(void) {
  vhpiHandleT root = vhpi_handle(vhpiRootInst, NULL);
  vhpiHandleT it = vhpi_iterator(vhpiEqProcessStmts, root);
  for (vhpiHandleT x; it && (x = vhpi_scan(it));) {
    vhpi_printf("%s %s label=%s postponed=%d passive=%d guarded=%d sens:", STR(vhpiKindStrP, x), STR(vhpiCaseNameP, x),
                STR(vhpiLabelNameP, x), vhpi_get(vhpiIsPostponedP, x), vhpi_get(vhpiIsPassiveP, x),
                vhpi_get(vhpiIsGuardedP, x));
    vhpiHandleT s = vhpi_iterator(vhpiSensitivities, x);
    for (vhpiHandleT y; s && (y = vhpi_scan(s));) {
      vhpi_printf(" %s", STR(vhpiCaseNameP, y));
    }
    vhpi_printf("\n");
  }
  vhpi_printf("old spelling: %s\n", vhpi_iterator(vhpiSensitivitys, vhpi_handle_by_name("top:ca", NULL)) ? "ok" : "null");
  seq(vhpi_handle_by_name("top:bd", NULL), 0);
  vhpiHandleT sg = vhpi_handle_by_name("top:a", NULL);
  vhpi_printf("staticness: signal %d const %d\n", vhpi_get(vhpiStaticnessP, sg),
              vhpi_get(vhpiStaticnessP, vhpi_handle_by_name("top:k", NULL)));
  /* Not implemented: a warning and the standard's error. */
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = vhpiCbStmt;
  cb.obj = root;
  cb.cb_rtn = (void (*)(const vhpiCbDataT *))start;
  vhpiHandleT r = vhpi_register_cb(&cb, vhpiReturnCb);
  vhpi_printf("statement callback: %s error=%d\n", r ? "handle" : "NULL", err());
  r = vhpi_handle(vhpiCurStackFrame, NULL);
  vhpi_printf("stack frame: %s error=%d\n", r ? "handle" : "NULL", err());
}
