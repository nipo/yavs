/* Test VHPI plugin for records as a whole, raw and small physical formats, protected objects and
 * conversions across kinds (#833), loaded by `yavs run --vhpi=LIB:ENTRY`.
 * SPDX-License-Identifier: MIT OR Apache-2.0 */

#include <stdint.h>
#include <stdio.h>
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

static void hex(const char *what, const unsigned char *b, int n) {
  vhpi_printf("%s:", what);
  for (int i = 0; i < n; i++) {
    vhpi_printf(" %02x", b[i]);
  }
  vhpi_printf("\n");
}

static vhpiHandleT h(const char *name) {
  return vhpi_handle_by_name(name, NULL);
}

static void raw(const char *name) {
  unsigned char b[128];
  vhpiValueT v;
  memset(&v, 0, sizeof v);
  v.format = vhpiObjTypeVal;
  v.bufSize = sizeof b;
  v.value.ptr = b;
  int rc = vhpi_get_value(h(name), &v);
  char what[64];
  snprintf(what, sizeof what, "%s rc=%d format=%d elems=%d", name, rc, (int)v.format, v.numElems);
  hex(what, b, rc == 0 ? 16 : 0);
}

static int user(vhpiHandleT var, void *data) {
  vhpi_printf("in protected call on %s, data %d\n", vhpi_get_str(vhpiCaseNameP, var), *(int *)data);
  return 7;
}

static void later(const vhpiCbDataT *d) {
  (void)d;
  raw("top:r");
  raw("top:a");
  /* The size asked for, with no buffer. */
  int rc0;
  vhpiValueT v;
  memset(&v, 0, sizeof v);
  v.format = vhpiRawDataVal;
  vhpi_printf("size of top:r: %d\n", vhpi_get_value(h("top:r"), &v));
  /* A scalar format on a record. */
  memset(&v, 0, sizeof v);
  v.format = vhpiIntVal;
  rc0 = vhpi_get_value(h("top:r"), &v);
  vhpi_printf("int of a record: %d error=%d\n", rc0, err());
  /* Raw of a scalar and a vector. */
  raw("top:r.i");
  unsigned char b[8];
  memset(&v, 0, sizeof v);
  v.format = vhpiRawDataVal;
  v.bufSize = sizeof b;
  v.value.ptr = b;
  int rc = vhpi_get_value(h("top:r.bv"), &v);
  hex(rc == 0 ? "raw of r.bv" : "raw failed", b, 2);
  /* Small physical formats. */
  memset(&v, 0, sizeof v);
  v.format = vhpiSmallPhysVal;
  int rcs = vhpi_get_value(h("top:len"), &v);
  vhpi_printf("small phys of len: rc=%d %d\n", rcs, (int)v.value.smallphys);
  rcs = vhpi_get_value(h("top:big"), &v);
  vhpi_printf("small phys of big: rc=%d error=%d\n", rcs, err());
  int32_t small[2];
  memset(&v, 0, sizeof v);
  v.format = vhpiSmallPhysVecVal;
  v.bufSize = sizeof small;
  v.value.ptr = small;
  rcs = vhpi_get_value(h("top:lens"), &v);
  vhpi_printf("small phys vec: rc=%d %d %d\n", rcs, (int)small[0], (int)small[1]);
  /* Pointer formats. */
  memset(&v, 0, sizeof v);
  v.format = vhpiPtrVal;
  rc0 = vhpi_get_value(h("top:r.b"), &v);
  vhpi_printf("ptr of a bit: %d error=%d\n", rc0, err());
  /* A whole record written raw: b, i, re, bv. */
  unsigned char in[1 + 4 + 8 + 2];
  double re = 2.5;
  int32_t i = -3;
  in[0] = 1; /* '1' */
  memcpy(in + 1, &i, 4);
  memcpy(in + 5, &re, 8);
  in[13] = 1;
  in[14] = 0;
  memset(&v, 0, sizeof v);
  v.format = vhpiRawDataVal;
  v.bufSize = sizeof in;
  v.value.ptr = in;
  vhpi_printf("put raw: %d\n", vhpi_put_value(h("top:w"), &v, vhpiDepositPropagate));
  v.bufSize = 3;
  rc0 = vhpi_put_value(h("top:w"), &v, vhpiDepositPropagate);
  vhpi_printf("put short: %d error=%d\n", rc0, err());
  /* Protected objects. */
  vhpiHandleT p = h("top:sv");
  vhpi_printf("protected: %d kind %s\n", vhpi_get(vhpiIsProtectedTypeP, p),
              vhpi_get_str(vhpiKindStrP, vhpi_handle(vhpiType, p)));
  vhpiHandleT decl = vhpi_handle(vhpiProtectedTypeDecl, p);
  vhpi_printf("protected type: %s\n", decl ? vhpi_get_str(vhpiCaseNameP, decl) : (const vhpiCharT *)"none");
  int data = 5;
  vhpi_printf("call: %d\n", vhpi_protected_call(p, (vhpiUserFctT)user, &data));
  rc0 = vhpi_protected_call(h("top:r"), (vhpiUserFctT)user, &data);
  vhpi_printf("call on a signal: %d error=%d\n", rc0, err());
  memset(&v, 0, sizeof v);
  v.format = vhpiIntVal;
  rc0 = vhpi_get_value(p, &v);
  vhpi_printf("value of protected: %d error=%d\n", rc0, err());
}

static void readback(const vhpiCbDataT *d) {
  (void)d;
  raw("top:w");
}

static void convert(void) {
  vhpiValueT in, out;
  char text[32];
  memset(&in, 0, sizeof in);
  memset(&out, 0, sizeof out);
  in.format = vhpiIntVal;
  in.value.intg = 3;
  out.format = vhpiRealVal;
  int rc = vhpi_format_value(&in, &out);
  vhpi_printf("int to real: rc=%d %.1f\n", rc, out.value.real);
  in.format = vhpiRealVal;
  in.value.real = 2.5;
  out.format = vhpiIntVal;
  rc = vhpi_format_value(&in, &out);
  vhpi_printf("real to int: rc=%d %d\n", rc, out.value.intg);
  out.format = vhpiDecStrVal;
  out.bufSize = sizeof text;
  out.value.str = (vhpiCharT *)text;
  in.value.real = -1.25;
  rc = vhpi_format_value(&in, &out);
  vhpi_printf("real to decimal text: rc=%d %s\n", rc, text);
  in.format = vhpiBinStrVal;
  in.value.str = (vhpiCharT *)"1010";
  out.format = vhpiIntVal;
  rc = vhpi_format_value(&in, &out);
  vhpi_printf("bits to int: rc=%d %d\n", rc, out.value.intg);
  in.value.str = (vhpiCharT *)"1X";
  rc = vhpi_format_value(&in, &out);
  vhpi_printf("metavalue to int: rc=%d error=%d\n", rc, err());
  in.format = vhpiCharVal;
  in.value.ch = 'a';
  out.format = vhpiRealVal;
  rc = vhpi_format_value(&in, &out);
  vhpi_printf("char to real: rc=%d error=%d\n", rc, err());
}

PLUGIN_EXPORT void start(void) {
  static vhpiTimeT t1 = {0, 1}, t2 = {0, 2};
  vhpiCbDataT cb;
  memset(&cb, 0, sizeof cb);
  cb.reason = vhpiCbAfterDelay;
  cb.cb_rtn = later;
  cb.time = &t1;
  vhpi_register_cb(&cb, 0);
  cb.cb_rtn = readback;
  cb.time = &t2;
  vhpi_register_cb(&cb, 0);
  convert();
}
