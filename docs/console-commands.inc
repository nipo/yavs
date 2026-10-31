..
   Generated from the console's command table: YAVS_BLESS=1 cargo test -p yavs --bin yavs cli_reference

Run and step
~~~~~~~~~~~~

.. list-table::
   :widths: 40 60

   * - ``run | run TIME | run until CONDITION``
     - To the end or a breakpoint, for TIME, or until a change of a signal in the condition makes it true.
   * - ``step | step delta | step timestep``
     - To the next statement, entering calls; or to the end of the delta or time step.
   * - ``next``
     - To the next statement, over calls.
   * - ``stop``
     - Pause a running simulation (Ctrl-C on a terminal).

Breakpoints
~~~~~~~~~~~

.. list-table::
   :widths: 40 60

   * - ``break [FILE:]LINE [if CONDITION] | break SIGNAL [= VALUE] [if CONDITION] | break assert [note|warning|error|failure] | break list | break delete N | break delete assert | break delete all``
     - Stop at a line, at a change of a signal, or at a report of this severity and worse; list or delete them.

Inspect
~~~~~~~

.. list-table::
   :widths: 40 60

   * - ``examine NAME | examine NAME @ TIME``
     - The value of a local or a signal, or a field, element or slice of one; with a time, from the wave file (also `print`).
   * - ``describe NAME``
     - Type, bounds, resolution function, mode and declaration of a signal or variable.
   * - ``where``
     - Time, delta, process and source line.

Change values
~~~~~~~~~~~~~

.. list-table::
   :widths: 40 60

   * - ``force NAME VALUE [-out]``
     - Hold a signal at a value, from the next cycle; with `-out` the driving value, not what readers see.
   * - ``deposit NAME VALUE``
     - Give a signal a value once, in the next cycle.
   * - ``release NAME [-out]``
     - End a force, or with `-out` only one on the driving value.

Waves
~~~~~

.. list-table::
   :widths: 40 60

   * - ``wave add NAME|PATTERN... | wave cursor [TIME] | wave save [FILE] | wave load [FILE]``
     - With `--surfer`, show signals or scopes in the viewer; move its cursor (default: now); save or load the signal list.

Find
~~~~

.. list-table::
   :widths: 40 60

   * - ``find signals PATTERN | find variables PATTERN``
     - Search the hierarchy; `*` within a level, `**` across, a pattern without a dot matches the last name.

Diagnose
~~~~~~~~

.. list-table::
   :widths: 40 60

   * - ``drivers NAME``
     - What drives a signal: each driver's value and source line, the resolved value, any force.
   * - ``waits``
     - Where each process stands and what it waits for.
   * - ``trace x NAME``
     - Follow an unknown value back through drivers and the signals they read.
   * - ``activity``
     - The processes that ran most often and the signals that changed most often, so far.
   * - ``asserts``
     - Assertion failures and reports so far, by severity and source line.

Session and scripts
~~~~~~~~~~~~~~~~~~~

.. list-table::
   :widths: 40 60

   * - ``restart``
     - Back to time 0 with the same design and breakpoints (also `reset`).
   * - ``source FILE``
     - Run the commands of a file; the first failure ends it.
   * - ``help | help COMMAND``
     - This list, or one command.
   * - ``quit [STATUS]``
     - Leave; the exit status is STATUS, else that of the ended run, else 0 (also `exit`).

VUnit commands
~~~~~~~~~~~~~~

These are only available through the VUnit interface, that is ``vunit-yavs`` with ``python run.py lib.tb.test --yavs-interactive``, not in a plain ``yavs run --interactive``.

.. list-table::
   :widths: 40 60

   * - ``vunit_help``
     - Prints this help.
   * - ``vunit_load [yavs_extra_args]``
     - Loads design with correct generics for the test(s); Optional first argument are passed as extra flags to yavs; Re-runs the user-defined after_load init file.
   * - ``vunit_user_init``
     - Re-runs the user-defined GUI init file.
   * - ``vunit_run``
     - Runs the user-defined before_load init file; Runs the test(s).
   * - ``vunit_compile``
     - Re-compiles the source files.
   * - ``vunit_restart``
     - Re-compiles the source files; and re-runs the simulation if the compile was successful.
