Debugger tutorial
=================

This tutorial debugs a small FIFO from VS Code, with the waves in Surfer, and then again from a terminal prompt. A test
fails on purpose. You find out why, one step at a time: each step says what to do and what you should see.

It needs a release of yavs and ``vunit-yavs`` newer than 0.11.0, and:

* the yavs VS Code extension of the same release (:doc:`debugging`),
* the `Surfer <https://surfer-project.org>`__ viewer, 0.6.0 or later, on ``PATH`` (:doc:`waveforms`),
* ``vunit-yavs`` (``pip install vunit-yavs``, :doc:`vunit`), which brings VUnit,
* ``yavs`` on ``PATH``, or its directory in ``VUNIT_YAVS_PATH``.

.. contents:: On this page
   :local:

The design
----------

The files are in ``docs/examples/fifo-debug`` of the source tree. Copy that directory and work in it. ``fifo.vhd`` is a
FIFO with a memory, a write pointer and a read pointer:

.. literalinclude:: examples/fifo-debug/fifo.vhd
   :language: vhdl
   :linenos:
   :caption: fifo.vhd

``tb_fifo.vhd`` has three tests. ``fails_on_purpose`` writes the words 1 and 2 and then reads word 2:

.. literalinclude:: examples/fifo-debug/tb_fifo.vhd
   :language: vhdl
   :linenos:
   :caption: tb_fifo.vhd

``run.py`` is a VUnit run script. It selects yavs as the simulator itself, so the commands below need no environment:

.. literalinclude:: examples/fifo-debug/run.py
   :language: python
   :caption: run.py

VS Code needs no configuration: the extension attaches to the port that ``--yavs-debug`` listens on.

Run all three tests once. Two pass; ``fails_on_purpose`` fails with
``word 2 - Got 0010_0110 (38). Expected 0100_1011 (75).`` The rest of this page finds out which side is wrong.

.. code-block:: bash

    python run.py

In VS Code
----------

1. Start the failing test and wait for the debugger
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

In a terminal in the demo directory:

.. code-block:: bash

    python run.py lib.tb_fifo.fails_on_purpose --yavs-debug --yavs-surfer

You should see the files compile and then this line, and nothing more: the simulation waits for a debugger on port 4711.

.. code-block:: text

    Debugging lib.tb_fifo.fails_on_purpose: attach a DAP client to 127.0.0.1:4711

2. Open the folder and set a breakpoint
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Open the demo directory as a folder in VS Code. Open ``fifo.vhd`` and click in the gutter of ``fifo.vhd`` line 40 (``next_count := next_count + 1;``). A red dot marks it.

3. Attach
~~~~~~~~~

Press F5, or run "yavs: Attach" from the command palette. The simulation stops at the breakpoint: the editor shows
line 40 highlighted, and Surfer opens next to it.

4. Look at the variables, then step
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The Variables view shows, under Locals, ``wp`` = 1, ``rp`` = 0 and ``next_count`` = 0; under Signals ``count`` = 0 and
``mem`` (four words, all ``"00000000"`` for now); under Generics ``DEPTH`` = 4. The first word is being written.

Press F10 to step over the line. ``next_count`` is now 1.

5. Stop in the testbench: this is the bug
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Click the red dot in ``fifo.vhd`` to remove that breakpoint. In ``tb_fifo.vhd`` set a breakpoint on the ``check_equal(dout, ...)`` line,
``tb_fifo.vhd`` line 49 (``check_equal(dout, word(n), "word " & integer'image(n));``), and press F5. The simulation stops
there, at 60 ns. Under Locals, ``n`` = 2. Under Signals, ``dout`` = ``"00100110"``.

The test expects word 2, ``"01001011"``. The FIFO returns the oldest word it holds, word 1, and that is what a FIFO
should do. The FIFO is right and the testbench is wrong: ``fails_on_purpose`` should read word 1.

6. Ask the Debug Console
~~~~~~~~~~~~~~~~~~~~~~~~

Type these in the Debug Console. They are the commands of the :doc:`interactive` (all of them in :doc:`console-reference`).

.. code-block:: text

    describe dout
    drivers wr
    find signals *count*

You should see:

.. code-block:: text

    tb_fifo.dout
      kind        signal
      type        STD_ULOGIC_VECTOR
      bounds      7 downto 0 (8 elements)
      resolution  none (one driver at most)
      declared    tb_fifo.vhd:20
      value       "00100110"

    tb_fifo.wr : STD_ULOGIC = '0', declared at tb_fifo.vhd:17
      driver                value  assigned at
      process tb_fifo.main  '0'    tb_fifo.vhd:35

      signal             kind    type     value
      tb_fifo.dut.count  signal  NATURAL  1

7. Add a signal to the wave
~~~~~~~~~~~~~~~~~~~~~~~~~~~

In the Variables view right-click ``dout`` and pick "Add to Wave". ``dout`` appears in Surfer, with its history from
time 0 up to the stop.

.. note::

   Keep Surfer and VS Code side by side, so that Surfer's window stays visible. If Add to Wave reports "Surfer does not
   answer", bring its window to the front: :doc:`waveforms` says why.

8. Find a process variable and add it
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Open the command palette and run "yavs: Find Signals or Variables...". Enter ``*next_count*``. The list holds one result,
``tb_fifo.dut.main.next_count`` (a variable of ``tb_fifo.dut.main``). Pick it. It appears in Surfer with its values from
time 0.

The procedure parameter ``n`` cannot be added. Right-click ``n`` under Locals and pick "Add to Wave": yavs refuses
and a notification says why: a parameter exists only during a call.

.. code-block:: text

    yavs: `n` is a local of a subprogram, which exists only during a call: only signals, process variables and shared variables are recorded

9. Watch a signal by its path
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

In the Watch view add ``tb_fifo.dut.count``. Its value is 1. A name that does not exist shows an error with the nearest
name instead; remove that entry from the view.

10. Stop only for the case you want
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Press Shift+F5 to end the session. In the terminal run the test that passes, the one that reads four words:

.. code-block:: bash

    python run.py lib.tb_fifo.fill_and_drain --yavs-debug --yavs-surfer

Right-click the red dot on the ``check_equal(dout, ...)`` line, pick "Edit Breakpoint...", choose Expression and enter ``n = 2``.
Press F5. The simulation skips the reads of words 0 and 1 and stops at 100 ns with ``n`` = 2.

11. Stop when a signal changes
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

In the Variables view right-click ``empty`` and pick "Break on Value Change". Press F5. The simulation stops at 105 ns,
after the last word was read, and ``empty`` is ``'1'``.

12. Force a signal and watch the test fail
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Right-click ``empty`` and pick "Force a signal...". Enter ``0``. Press F5. The value breakpoint can
stop first, now at ``empty`` = ``'0'``; press F5 again. The test fails, and the Debug Console shows:

.. code-block:: text

    ERROR - empty after DEPTH reads - Got 0. Expected 1.

The session ends with a line giving exit status 1.

From a terminal prompt
----------------------

13. Do the same without an editor
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The console of :doc:`interactive` has the same debugger. Start the passing test at the prompt:

.. code-block:: bash

    python run.py lib.tb_fifo.fill_and_drain --yavs-interactive

After VUnit's list of commands you get a ``yavs>`` prompt at time 0. Break on the ``check_equal(full, ...)`` line,
``tb_fifo.vhd`` line 62 (``check_equal(full, '1', "full after DEPTH writes");``), and run:

.. code-block:: text

    yavs> break tb_fifo.vhd:62
    breakpoint 1: /demo/tb_fifo.vhd:62
    yavs> run
    breakpoint at 70 ns, delta 0, /demo/tb_fifo.vhd:62
    yavs> examine full
    '1'

Type ``examine fu`` and press Tab: it completes to ``examine full``. Resume with ``run``; ``continue`` is not a command
here. The test runs to its end and passes:

.. code-block:: text

    yavs> run
    ** Note: 110000000fs+10: STOP called with status 0
    the simulation ended at 110 ns, delta 10
    yavs> quit

``quit`` prints VUnit's summary, with ``lib.tb_fifo.fill_and_drain`` passed.

See also
--------

* :doc:`debugging`: the VS Code debugger and other editors.
* :doc:`interactive`: the prompt and worked examples.
* :doc:`console-reference`: every console command, with its arguments, errors and examples.
* :doc:`waveforms`: Surfer and waveform files.
* :doc:`vunit-yavs`: the options of ``--yavs-debug``, ``--yavs-surfer`` and ``--yavs-interactive``.
