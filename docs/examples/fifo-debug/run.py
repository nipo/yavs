#!/usr/bin/env python3
"""Run script of the debugger tutorial (docs/debugger-tutorial.rst): a FIFO with a testbench that fails on purpose"""

import os
from pathlib import Path

os.environ.setdefault("VUNIT_SIMULATOR", "yavs")  # yavs comes from PATH, or from VUNIT_YAVS_PATH

from vunit import VUnit  # pylint: disable=wrong-import-position

ROOT = Path(__file__).parent

vu = VUnit.from_argv()
vu.add_package("vunit-yavs", allow_setup=True)  # first: it provides the simulator
vu.add_vhdl_builtins()
vu.add_library("lib").add_source_files([ROOT / "fifo.vhd", ROOT / "tb_fifo.vhd"])
vu.main()
