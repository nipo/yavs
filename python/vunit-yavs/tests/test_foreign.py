# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this file,
# You can obtain one at http://mozilla.org/MPL/2.0/.
#
# Copyright (c) 2014-2026, Lars Asplund lars.anders.asplund@gmail.com

"""
VHDL calls a native library, and Python embedded in it, through VHPIDIRECT foreign subprograms on a real
yavs: a VUnit package builds the library and registers the hooks `native_library_hooks` gives (#120).
"""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from test_fake_e2e import report
from test_real_yavs import TIMEOUT, YAVS_DIR

PROJECT = Path(__file__).parent / "project_foreign"
sys.path.insert(0, str(PROJECT))
import native_demo_setup  # noqa: E402  pylint: disable=wrong-import-position

AVAILABLE = native_demo_setup.available()


@unittest.skipIf(YAVS_DIR is None, "no yavs: set VUNIT_YAVS_PATH or put yavs on PATH")
@unittest.skipUnless(AVAILABLE["native"], "no C compiler: set CC or put gcc, cc or clang on PATH")
class TestForeignSubprograms(unittest.TestCase):
    """A package's native library is loaded into each simulation and its functions are called from VHDL"""

    def _run(self, python, args=None, vunit_args=()):
        with tempfile.TemporaryDirectory() as tempdir:
            xml = Path(tempdir) / "x.xml"
            env = dict(
                os.environ,
                VUNIT_SIMULATOR="yavs",
                VUNIT_YAVS_PATH=str(YAVS_DIR),
                PYTHONPATH=os.pathsep.join([str(PROJECT), os.environ.get("PYTHONPATH", "")]),
                NATIVE_DEMO_PYTHON="1" if python else "0",
            )
            if args:
                env["NATIVE_DEMO_ARGS"] = args
            proc = subprocess.run(
                [sys.executable, str(PROJECT / "run.py"), "-o", str(Path(tempdir) / "out"), "-x", str(xml)]
                + list(vunit_args),
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                timeout=TIMEOUT,
                check=False,
            )
            self.assertEqual(proc.returncode, 0, proc.stdout)
            return proc.stdout if vunit_args else report(xml)

    def test_native_function(self):
        self.assertEqual(
            self._run(python=False),
            {
                "lib.tb_init.A foreign function gives a signal its initial value": "passed",
                "lib.tb_native.A native library function is called": "passed",
            },
        )

    def test_elaborate_only_loads_the_library(self):
        # A foreign call in a signal's initial value is evaluated while elaborating: --elaborate needs the library too
        output = self._run(python=False, vunit_args=("--elaborate",))
        self.assertIn("lib.tb_init", output)

    def test_fat_pointer_layout(self):
        self.assertEqual(
            self._run(python=False, args="fat-pointer"),
            {
                "lib.tb_init.A foreign function gives a signal its initial value": "passed",
                "lib.tb_native.A native library function is called": "passed",
                "lib.tb_fat.A string reaches the library as a fat pointer": "passed",
            },
        )

    @unittest.skipUnless(AVAILABLE["python"], "needs a shared libpython and its headers (Linux)")
    def test_python_from_vhdl(self):
        self.assertEqual(
            self._run(python=True),
            {
                "lib.tb_init.A foreign function gives a signal its initial value": "passed",
                "lib.tb_native.A native library function is called": "passed",
                "lib.tb_python.Python is called from VHDL": "passed",
                "lib.tb_python.A Python error is printed by Python and returned": "passed",
            },
        )
