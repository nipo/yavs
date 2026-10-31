# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this file,
# You can obtain one at http://mozilla.org/MPL/2.0/.
#
# Copyright (c) 2014-2026, Lars Asplund lars.anders.asplund@gmail.com

"""
The console in a real terminal (a pseudo-terminal, standard library only), on a real yavs: what pipes
do not show, because there is no line editor on a pipe.
"""

import os
import select
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

try:
    import fcntl
    import pty
    import termios
except ImportError:  # not on Windows
    pty = None

HERE = Path(__file__).parent
YAVS_DIR = os.environ.get("VUNIT_YAVS_PATH") or (Path(shutil.which("yavs")).parent if shutil.which("yavs") else None)
PROMPT = b"yavs> "
CTRL_C = b"\x03"
CTRL_D = b"\x04"
TAB = b"\t"

ENDLESS = """
entity clocked is end;
architecture a of clocked is
  signal clk : bit;
begin
  clkgen : process begin
    wait for 5 ns; clk <= not clk;
  end process;
end;
"""

# Line 7 is the statement of the loop
SPIN = """entity spin is end;
architecture a of spin is
  procedure spin is
    variable i : natural := 0;
  begin
    loop
      i := (i + 1) mod 7;
    end loop;
  end procedure;
begin
  process begin
    spin;
    wait;
  end process;
end;
"""


class Terminal:
    """A command with a pseudo-terminal as its controlling terminal"""

    def __init__(self, args, cwd, env=None):
        self.fd, slave = pty.openpty()
        self.seen = b""

        def own_terminal():
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        self.proc = subprocess.Popen(  # pylint: disable=subprocess-popen-preexec-fn,consider-using-with
            args,
            cwd=cwd,
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            start_new_session=True,
            preexec_fn=own_terminal,
        )
        os.close(slave)

    def send(self, data):
        os.write(self.fd, data)

    def _read(self, timeout):
        if not select.select([self.fd], [], [], timeout)[0]:
            return False
        try:
            data = os.read(self.fd, 65536)
        except OSError:  # the other side has gone
            return False
        self.seen += data
        return bool(data)

    def expect(self, text, timeout=60):
        """Wait for `text` and forget all up to its end; the text before it is returned"""
        end = time.monotonic() + timeout
        while text not in self.seen:
            if time.monotonic() > end or (not self._read(0.1) and self.proc.poll() is not None):
                raise AssertionError(f"no {text!r} after {timeout} s, got: {self.seen[-600:]!r}")
        before, self.seen = self.seen.split(text, 1)
        return before

    def wait(self, timeout=30):
        end = time.monotonic() + timeout
        while self.proc.poll() is None:
            if time.monotonic() > end:
                raise AssertionError(f"still running after {timeout} s, last output: {self.seen[-600:]!r}")
            self._read(0.1)
        return self.proc.returncode

    def close(self):
        if self.proc.poll() is None:
            self.proc.kill()
        self.proc.wait()
        os.close(self.fd)


@unittest.skipIf(pty is None or YAVS_DIR is None, "needs a pseudo-terminal and yavs")
class TestConsolePty(unittest.TestCase):
    def setUp(self):
        self._tempdir = tempfile.TemporaryDirectory()  # pylint: disable=consider-using-with
        self.addCleanup(self._tempdir.cleanup)
        self.dir = Path(self._tempdir.name)
        self.yavs = str(Path(YAVS_DIR) / "yavs")
        self.env = dict(os.environ, YAVS_STD_CACHE=os.environ.get("YAVS_STD_CACHE", "off"), TERM="xterm")

    def _console(self, top, source):
        (self.dir / f"{top}.vhd").write_text(source, encoding="utf-8")
        subprocess.run(
            [self.yavs, "analyze", "--work=work:w", f"{top}.vhd"],
            cwd=self.dir,
            env=self.env,
            check=True,
            timeout=120,
        )
        term = Terminal([self.yavs, "run", "--work=work:w", top, "--interactive"], self.dir, self.env)
        self.addCleanup(term.close)
        term.expect(PROMPT)
        return term

    def test_ctrl_c_pauses_a_run_and_the_prompt_comes_back(self):
        term = self._console("clocked", ENDLESS)
        for line in (b"run\r", b"run 1000 sec\r"):
            term.send(line)
            term.expect(b"\r\n")  # the line was taken
            time.sleep(0.5)
            term.send(CTRL_C)
            self.assertIn(b"pause at", term.expect(PROMPT))
        term.send(b"where\r")
        self.assertIn(b"time ", term.expect(PROMPT))
        term.send(b"quit\r")
        self.assertEqual(term.wait(), 0)

    def test_ctrl_c_at_the_prompt_drops_the_line(self):
        term = self._console("clocked", ENDLESS)
        term.send(b"no such thing")
        term.send(CTRL_C)
        term.expect(PROMPT)
        term.send(b"quit\r")
        self.assertEqual(term.wait(), 0)

    def _stopped_in_the_loop(self):
        term = self._console("spin", SPIN)
        term.send(b"break spin.vhd:7\r")
        term.expect(PROMPT)
        term.send(b"run\r")
        self.assertIn(b"spin.vhd:7", term.expect(PROMPT))
        return term

    def test_leaving_a_stop_in_an_endless_subprogram_loop(self):
        for leave in (b"quit\r", CTRL_D):
            self.assertEqual(self._stopped_in_the_loop_status(leave), 0, leave)
        term = self._stopped_in_the_loop()
        term.send(b"restart\r")
        term.expect(PROMPT)
        term.send(b"where\r")
        self.assertIn(b"time 0 fs, delta 0", term.expect(PROMPT))
        term.send(b"quit\r")
        self.assertEqual(term.wait(), 0)

    def _stopped_in_the_loop_status(self, leave):
        term = self._stopped_in_the_loop()
        term.send(leave)
        return term.wait()

    def test_ctrl_c_pauses_an_endless_subprogram_loop(self):
        term = self._console("spin", SPIN)
        term.send(b"run\r")
        term.expect(b"\r\n")
        time.sleep(0.5)
        term.send(CTRL_C)
        self.assertIn(b"pause at", term.expect(PROMPT))
        term.send(b"quit\r")
        self.assertEqual(term.wait(), 0)


if __name__ == "__main__":
    unittest.main()
