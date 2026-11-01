Interactive debugger
====================

``yavs run --interactive`` (needs a release newer than 0.11.0) runs a simulation from a terminal prompt, without an editor. It
elaborates the design, stops at time 0 before any process has run, and waits at ``yavs>``:

.. code-block:: text

    $ yavs run --work=work:build/work tb --interactive
    yavs> break tb.vhd:24
    breakpoint 1: /home/me/tb.vhd:24
    yavs> run 10 ns
    step at 10 ns, delta 2
    yavs> examine cnt
    4
    yavs> force cnt 5
    tb.cnt = 5
    yavs> run
    breakpoint at 30 ns, delta 0, /home/me/tb.vhd:24
    yavs> restart
    yavs> quit

A design with no process has a session too: ``run`` ends at time 0, and its signals can be examined.

The prompt has line editing and history. Ctrl-C pauses a running simulation and returns to the
prompt; at the prompt it drops the line. ``exit`` is ``quit``, and so is Ctrl-D at an empty prompt or the end of piped
input.

Tab works as in a shell: one match completes, several complete what they have in common, and a second Tab lists them and
gives the line back. It offers what fits at the cursor: the first word is a command (or a test runner's command), never a
signal; after a command, what that command takes: signals and variables after ``examine``, ``describe``, ``drivers``,
``force`` and the like (``tb.`` then Tab goes down a dotted path, ``rec.`` then Tab through fields), files after ``source``
and ``wave save``, source files and signals after ``break``, subcommands after ``wave``, ``find`` and ``step``, and nothing
for a time after ``run`` or a value after ``force``. A bare name typed as the first word is still examined.

``help`` lists the commands in columns, their names in bold on a colour terminal (not with ``NO_COLOR`` or
``--color=never``, and never in piped output).

.. contents:: On this page
   :local:

All commands
------------

This is the overview. :doc:`console-reference` has every command and subcommand with its arguments, errors
and examples, and how names, values, times and conditions are written.

Several commands go on a line with ``;``. ``#`` at the start of a command and ``--`` start a
comment, except a ``--`` after a space and right before a letter inside a command, which is a flag
(``vunit_load --stop-time=30ns``). Reports and text output go to standard output as in a batch run,
and so do the results of commands. Notices of stops and errors go to standard error. A name that
means nothing gets a suggestion of what is close, whatever kind of name it is.

.. include:: console-commands.inc

More on some of them:

* ``examine`` takes a field, element or slice (``examine v(3 downto 0)``, ``examine rec.x``), goes
  through an access value (``examine p.next.value``) and takes an expression as breakpoint conditions
  do (``examine to_integer(cnt) + 1``). Inside a method of a protected type the variables of the
  object are in scope. Between cycles, simple names are looked up at the top level; inside a
  process, where that process sees them. A bare name also works.
* ``force``, ``deposit`` and ``release`` take effect from the next cycle. Values are VHDL literals, and a bit-like target
  needs no quotes: ``force clk 1``, ``force dout 0101``, the same in ``break rst = 1``. An integer target's ``1`` stays 1.
  ``force`` holds what readers of the signal see; ``-out`` at the end holds the driving value instead. ``release`` ends
  either force, ``release NAME -out`` only one on the driving value.
* ``restart`` keeps the breakpoints; nothing is compiled, and a wave file is written again from the
  start. Runs with ``--vhpi`` or ``--load`` refuse it: start the run again.
* ``find`` matches a pattern without a dot against the last name anywhere (``find signals *clk*``),
  and one with a dot against the path (``find signals u1.*``). What it lists, ``examine``,
  ``drivers`` and ``wave add`` take. A variable of a process is read at a stop in that process, or
  with ``examine NAME @ TIME``.
* ``break assert`` without a severity means ``error``. It stops after the report, in the statement
  that made it, with the stack intact.
* ``waits`` shows what each process waits for at any stop. A plain run gets the same from
  ``--hang-report`` (:doc:`cli`); a run that simply ends prints nothing.
* ``examine NAME @ TIME`` needs ``--wave``. What ``--wave-include``, ``--wave-exclude`` or
  ``--wave-max-array`` leave out cannot be read back. Composites are recorded element by element and
  are listed that way.
* ``trace x`` needs no wave file; with one it also says since when each signal has been unknown,
  else when it last changed. It follows the drivers of a signal, and from a process to the signals
  its statements name, and it marks where it starts: nothing drives a signal, two drivers hold
  opposite values, a force, or a process with no unknown input. It cannot see variables, values from
  files or subprogram parameters, and it does not know which of the signals read made the value.
* ``activity`` counts runs of a process and, per signal, delta cycles with an event. Activations the
  kernel avoids do not run, so they do not count. ``asserts`` counts per innermost statement.
* ``wave save`` and ``wave load`` without a file use the list of this testbench
  (:doc:`waveforms`).

Worked examples
---------------

All of these use ``docs/examples/debugger/tb.vhd`` of the source tree, analyzed with
``yavs analyze --std=2008 --work=work:lib tb.vhd``, with the design in ``/work``.

Stop at a line and inspect
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. example:

.. code-block:: text

    yavs> break tb.vhd:43
    breakpoint 1: /work/tb.vhd:43
    yavs> run
    breakpoint at 5 ns, delta 0, /work/tb.vhd:43
    yavs> where
    time 5 ns, delta 0
    tb.counter, /work/tb.vhd:43
    yavs> examine cnt
    "0000"
    yavs> examine total
    1
    yavs> describe cnt
    tb.cnt
      kind        signal
      type        UNSIGNED
      bounds      3 downto 0 (4 elements)
      resolution  resolved (per element)
      declared    tb.vhd:31
      value       "0000"

Break on a value
~~~~~~~~~~~~~~~~

.. example:

.. code-block:: text

    yavs> break cnt = "0011"
    breakpoint 1: tb.cnt
    yavs> run
    ** Error: 25000000fs+1: q is not high
        Process tb.check
    data breakpoint at 25 ns, delta 1: tb.cnt = "0011"
    yavs> examine cnt
    "0011"

Find who drives an X
~~~~~~~~~~~~~~~~~~~~

.. example: --wave=tb.fst

.. code-block:: text

    yavs> run 30 ns
    ** Error: 25000000fs+1: q is not high
        Process tb.check
    step at 30 ns, delta 0
    yavs> examine q
    'U'
    yavs> drivers q
    tb.q : STD_LOGIC = 'U', declared at tb.vhd:30
      driver           value  assigned at
      port tb.u_reg.q  'U'    tb.vhd:7
      resolved by resolved
    yavs> trace x q
    tb.q = 'U', since 0 fs, declared at tb.vhd:30
      tb.u_reg.q = 'U', since 0 fs, declared at tb.vhd:7
        driven 'U' by process tb.u_reg.flop, at tb.vhd:15
          tb.u_reg.d = 'U', since 0 fs, declared at tb.vhd:7
            tb.enable = 'U', since 0 fs, declared at tb.vhd:29
    Where it starts:
      tb.enable: nothing drives it, it keeps its initial value (tb.vhd:29)
    It follows drivers and the signals their processes name. It cannot see variables,
    values from files, subprogram parameters, or which signal read made this value.
    yavs> drivers level
    tb.level : STD_LOGIC_VECTOR = "X1", declared at tb.vhd:32
      driver          value  assigned at
      process tb._p1  "01"   tb.vhd:47
      process tb._p2  "11"   tb.vhd:48
      resolved by resolved (per element)
    yavs> trace x level
    tb.level = "X1", since 25 ns, declared at tb.vhd:32
      element (1): process tb._p1 drives '0' and process tb._p2 drives '1', which resolve to 'X'
    Where it starts:
      tb.level: element (1): process tb._p1 drives '0' and process tb._p2 drives '1', which resolve to 'X'
    It follows drivers and the signals their processes name. It cannot see variables,
    values from files, subprogram parameters, or which signal read made this value.

Stop on a failing assertion
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. example:

.. code-block:: text

    yavs> break assert error
    stop at assertions and reports of severity error, failure
    yavs> run
    ** Error: 25000000fs+1: q is not high
        Process tb.check
    assertion of severity error at 25 ns, delta 1, /work/tb.vhd:53: q is not high
    yavs> where
    time 25 ns, delta 1
    tb.check, /work/tb.vhd:53
    yavs> examine q
    'U'
    yavs> trace x q
    tb.q = 'U', never changed, declared at tb.vhd:30
      tb.u_reg.q = 'U', never changed, declared at tb.vhd:7
        driven 'U' by process tb.u_reg.flop, at tb.vhd:15
          tb.u_reg.d = 'U', never changed, declared at tb.vhd:7
            tb.enable = 'U', never changed, declared at tb.vhd:29
    Where it starts:
      tb.enable: nothing drives it, it keeps its initial value (tb.vhd:29)
    It follows drivers and the signals their processes name. It cannot see variables,
    values from files, subprogram parameters, or which signal read made this value.
    yavs> asserts
    Assertion summary: 0 failures, 1 error, 0 warnings, 0 notes
      severity  count  place      first message
      error         1  tb.vhd:53  q is not high

Read a past value
~~~~~~~~~~~~~~~~~

.. example: --wave=tb.fst

.. code-block:: text

    yavs> run 30 ns
    ** Error: 25000000fs+1: q is not high
        Process tb.check
    step at 30 ns, delta 0
    yavs> examine cnt @ 12 ns
    "0001"
    yavs> examine cnt @ 1 us
    error: 1 us is still to come: the simulation is at 30 ns
    yavs> find signals *cn*
      signal  kind    type      value
      tb.cnt  signal  UNSIGNED  "0011"
    yavs> find variables tot*
      variable          kind
      tb.counter.total  variable of tb.counter

Add signals and variables to the wave
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

With ``--surfer`` (:doc:`waveforms`); the viewer shows the history of what is added since time 0:

.. example: --surfer

.. code-block:: text

    yavs> run 20 ns
    note: the waveform is written to `tb.fst`
    step at 20 ns, delta 0
    yavs> wave add cnt
    1 added
    yavs> wave add *clk*
    2 added
    yavs> wave add tb.counter.total
    1 added
    yavs> wave save
    9 signals saved to `lib/yavs-wave-layout/tb.txt`

Change a value and continue
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. example:

.. code-block:: text

    yavs> run 22 ns
    step at 22 ns, delta 0
    yavs> force enable '1'
    tb.enable = '1'
    yavs> run 20 ns
    step at 42 ns, delta 0
    yavs> drivers enable
    tb.enable : STD_LOGIC = '1', declared at tb.vhd:29
      no driver: it keeps its initial value unless forced or deposited
      resolved by resolved
      force: '1'
    yavs> examine q
    '1'
    yavs> release enable
    tb.enable
    yavs> deposit cnt "0000"
    tb.cnt = "0000"
    yavs> run 1 ns
    step at 43 ns, delta 0
    yavs> examine cnt
    "0000"

Diagnose a run
~~~~~~~~~~~~~~

.. example:

.. code-block:: text

    yavs> run 30 ns
    ** Error: 25000000fs+1: q is not high
        Process tb.check
    step at 30 ns, delta 0
    yavs> waits
      process        state    waits for         at
      tb._p0         waiting  tb.clk, tb.cnt    tb.vhd:34
      tb.u_reg.flop  waiting  tb.u_reg.clk      tb.vhd:12
      tb.counter     waiting  tb.clk            tb.vhd:38
      tb._p1         ended    nothing: `wait;`  tb.vhd:47
      tb._p2         waiting  tb.cnt            tb.vhd:48
      tb.check       ended    nothing: `wait;`  tb.vhd:54
    yavs> activity
    Process activations: 33 runs in all, 6 shown
      runs      %  name           place
        10   30.3  tb._p0         tb.vhd:34
         7   21.2  tb.counter     tb.vhd:41
         7   21.2  tb.u_reg.flop  tb.vhd:14
         4   12.1  tb._p2         tb.vhd:48
         4   12.1  tb.check       tb.vhd:52
         1    3.0  tb._p1         tb.vhd:47
    Signal activity: 17 events in all, 4 shown
      events      %  name          place
           6   35.3  tb.clk        tb.vhd:28
           6   35.3  tb.u_reg.clk  tb.vhd:7
           3   17.6  tb.cnt        tb.vhd:31
           2   11.8  tb.level      tb.vhd:32

In the VS Code Debug Console
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The same lines, in the Debug Console of a stopped ``yavs`` session, give the same answers:
``drivers q``, ``trace x q``, ``find signals *cn*``, ``break assert error``. In the Variables view
the context menu of a signal has "Show Drivers", the command "yavs: Find Signals or Variables..."
lists matches in a quick pick and adds what you choose to the wave, or else examines it, and the
Breakpoints view has "Assertion: failure", "Assertion: error", "Assertion: warning" and "Report: note".

Scripts
-------

``--do`` runs commands without a terminal. It can be repeated:

.. code-block:: sh

    yavs run --work=work:build/work tb --do "break tb.vhd:24; run; examine cnt" --do "source more.do"

Without ``--interactive`` the run exits at the end of the script with its status. A command that
fails ends the script with status 1. With ``--interactive``, the prompt follows the script, and a
failed command only prints its error. ``--do "run; quit"`` prints what a plain run prints and exits
with the same status.

In VS Code
----------

The same commands work in the Debug Console of a ``yavs`` session (:doc:`debugging`): type
``run 10 ns``, ``break list`` or ``restart``. A plain name still shows its value. ``restart`` stops
at time 0 and keeps the breakpoints of the editor. Unlike at the prompt, the session ends with the
run. Breakpoints made with ``break`` belong to the session only: the editor's Breakpoints list does not
show them, or their deletion. Watch expressions and hovers take what ``examine`` takes.

With a test runner
------------------

A test runner can add its own commands to the console: it registers a name and help text, and yavs
calls it when the command is typed. The handler runs in the test runner's process and can print
on the console, run console commands and load the design again. The protocol is in the
`command line contract <https://github.com/ru551n/yavs/blob/main/docs/cli-contract.md>`__
("Console hook").

The Python package ``vunit-yavs`` uses it: ``python run.py lib.tb.test_name --yavs-interactive``
starts one test at the prompt, and ``vunit_help`` lists what it adds. See its README.

Options
-------

* ``--hang-report[=MAX]``: at the end of a session that reaches its stop time or stalls, where each
  process waits (W3101).
* ``--assert-summary`` and ``--activity[=FILE]``: the tables of ``asserts`` and ``activity`` at the
  end of the run. In a plain run, ``--message-format=json`` makes the summary one JSON object.

Limits
------

The console is a debug session: the interpreter on one thread (``--threads`` and ``--tier`` have no
effect), no ``--cov`` or ``--profile``. One driver per run: the console or a debugger, not both (``--dap`` is refused with
``--interactive`` and ``--do``). ``--surfer`` combines with either.
``run until`` watches signals and reads them between cycles, so a variable of a process in its condition is an unknown name.

See also
--------

* :doc:`debugger-tutorial`: a failing test debugged step by step, in VS Code and at this prompt.
* :doc:`console-reference`: every command, argument and error.
* :doc:`debugging`: the same session in VS Code.
* :doc:`vunit`: the console with a test of a run script.
* :doc:`waveforms`: ``--surfer`` and the ``wave`` commands.
