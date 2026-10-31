/* Test VHPI plugin for the declarations and units of #831: aliases, files, implicit signals,
 * components, configuration specifications, subprograms, libraries, use clauses and user
 * attributes of units and labels. Loaded by `yavs run --vhpi=LIB:ENTRY`.
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
#define OR(h, prop) ((h) ? STR(prop, h) : "none")

static vhpiHandleT h(const char *name) {
  return vhpi_handle_by_name(name, NULL);
}

static void names(const char *what, int rel, vhpiHandleT o) {
  vhpi_printf("%s:", what);
  vhpiHandleT it = vhpi_iterator(rel, o);
  for (vhpiHandleT x; it && (x = vhpi_scan(it));) {
    vhpi_printf(" %s", STR(vhpiCaseNameP, x));
  }
  vhpi_printf("\n");
}

static const char *bin(vhpiHandleT o) {
  static char buf[32];
  vhpiValueT v;
  memset(&v, 0, sizeof v);
  v.format = vhpiBinStrVal;
  v.bufSize = sizeof buf;
  v.value.str = (vhpiCharT *)buf;
  return vhpi_get_value(o, &v) == 0 ? buf : "none";
}

static void at3(const vhpiCbDataT *d) {
  (void)d;
  vhpiHandleT s = h("top:s");
  vhpiHandleT it = vhpi_iterator(vhpiSigAttrs, s);
  for (vhpiHandleT x; it && (x = vhpi_scan(it));) {
    if (vhpi_get(vhpiPredefAttrP, x) == vhpiStablePA) {
      vhpi_printf("stable value at 3: %s\n", bin(x));
    }
  }
  const char *al[] = {"top:sa", "top:part"};
  for (int i = 0; i < 2; i++) {
    vhpiHandleT a = h(al[i]);
    vhpiHandleT t = vhpi_handle(vhpiAliasedName, a);
    vhpi_printf("alias %s -> %s value %s\n", STR(vhpiCaseNameP, a), OR(t, vhpiFullCaseNameP), bin(a));
  }
}

static void comp(const char *name) {
  vhpiHandleT i = h(name);
  vhpiHandleT c = vhpi_handle(vhpiCompDecl, i);
  const vhpiCharT *cn = vhpi_get_str(vhpiCompNameP, i);
  vhpi_printf("%s: kind=%d comp=%s compdecl=%s label=%s\n", STR(vhpiCaseNameP, i),
              vhpi_get(vhpiCompInstKindP, i), cn ? (const char *)cn : "none", OR(c, vhpiKindStrP),
              STR(vhpiCompInstNameP, i));
}

PLUGIN_EXPORT void start(void) {
  vhpiHandleT root = vhpi_handle(vhpiRootInst, NULL);
  names("aliases", vhpiAliasDecls, root);
  vhpi_printf("file: %s %s\n", STR(vhpiKindStrP, h("top:out_f")), STR(vhpiCaseNameP, h("top:out_f")));
  vhpiHandleT it = vhpi_iterator(vhpiSigAttrs, h("top:s"));
  for (vhpiHandleT x; it && (x = vhpi_scan(it));) {
    vhpiHandleT p = vhpi_handle(vhpiPrefix, x);
    vhpi_printf("sig attr %s %s attr %d prefix %s\n", STR(vhpiKindStrP, x), STR(vhpiCaseNameP, x),
                vhpi_get(vhpiPredefAttrP, x), STR(vhpiFullCaseNameP, p));
  }
  names("attr of unit", vhpiAttrSpecs, vhpi_handle(vhpiDesignUnit, root));
  names("attr of label", vhpiAttrSpecs, h("top:inst"));
  names("attr decls", vhpiAttrDecls, root);
  comp("top:inst");
  comp("top:d");
  it = vhpi_iterator(vhpiConfigSpecs, root);
  for (vhpiHandleT x; it && (x = vhpi_scan(it));) {
    vhpi_printf("config: %s %s comp=%s\n", STR(vhpiKindStrP, x), STR(vhpiCaseNameP, x),
                OR(vhpi_handle(vhpiCompDecl, x), vhpiKindStrP));
  }
  vhpiHandleT f = h("top:local");
  vhpi_printf("func: %s %s params=%d pure=%d body=%s ret=%s\n", STR(vhpiKindStrP, f), STR(vhpiCaseNameP, f),
              vhpi_get(vhpiNumParamsP, f), vhpi_get(vhpiIsPureP, f), OR(vhpi_handle(vhpiSubpBody, f), vhpiKindStrP),
              STR(vhpiNameP, vhpi_handle(vhpiReturnType, f)));
  it = vhpi_iterator(vhpiLibraryDecls, NULL);
  for (vhpiHandleT l; it && (l = vhpi_scan(it));) {
    vhpi_printf("lib %s logical=%s\n", STR(vhpiNameP, l), STR(vhpiLibLogicalNameP, l));
    vhpiHandleT u = vhpi_iterator(vhpiDesignUnits, l);
    for (vhpiHandleT x; u && (x = vhpi_scan(u));) {
      if (vhpi_get(vhpiKindP, x) == vhpiPackBodyK) {
        vhpi_printf("unit %s %s primary=%s\n", STR(vhpiKindStrP, x), STR(vhpiCaseNameP, x),
                    OR(vhpi_handle(vhpiPrimaryUnit, x), vhpiKindStrP));
      }
      if (!strcmp(STR(vhpiCaseNameP, x), "top")) {
        vhpiHandleT c = vhpi_iterator(vhpiUseClauses, x);
        for (vhpiHandleT y; c && (y = vhpi_scan(c));) {
          vhpi_printf("use %s\n", STR(vhpiCaseNameP, y));
        }
      }
    }
  }
  vhpiHandleT p = h("@work:p:f");
  it = vhpi_iterator(vhpiParamDecls, p);
  for (vhpiHandleT x; it && (x = vhpi_scan(it));) {
    vhpi_printf("param %s %s\n", STR(vhpiKindStrP, x), STR(vhpiCaseNameP, x));
  }
  vhpi_printf("end line of top: %d\n", vhpi_get(vhpiEndLineNoP, vhpi_handle(vhpiDesignUnit, root)));
  static vhpiTimeT t3 = {0, 3};
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = vhpiCbAfterDelay;
  cb.cb_rtn = at3;
  cb.time = &t3;
  vhpi_register_cb(&cb, 0);
}
