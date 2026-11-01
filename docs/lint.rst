Static analysis
===============

``yavs lint`` (needs a release newer than 0.11.0) is a static analysis tool, not a style guide or formatter: it
reports what can be proved wrong about a design before it is simulated, and it never looks at layout. A default run
reports definite errors only: the program cannot do what it says, whatever anyone intended. It
reports what it can derive and nothing it would have to guess, so an empty report means nothing
provable was found, and a line in it is something to correct.

.. code-block:: console

   $ yavs lint src/fifo.vhd
   error[lint_771]: `depth` can reach the end of its body without returning a value, which is an error when it does
     --> src/fifo.vhd:21:12
      |
   21 |   function depth (w : natural) return natural is
      |            ^^^^^
      |
      = help: `yavs lint --explain lint_771` says what this rule reports

   note: 1 finding in 1 file(s)

:doc:`lint-rules` lists every rule; ``yavs lint --explain lint_771`` prints one and
``yavs lint --list-rules`` prints them with what this configuration runs.

What to check
-------------

Check what is already in a library, or name files and directories:

.. code-block:: console

   $ yavs analyze --work=mylib:out/mylib src/*.vhd
   $ yavs lint --work=mylib:out/mylib              # every file of the library
   $ yavs lint --work=mylib:out/mylib src/fifo.vhd # one file, what it needs from the library
   $ yavs lint src/pkg.vhd src/fifo.vhd            # no library: in this order, as library work
   $ yavs lint --recursive src                     # a directory, sorted by name

Nothing is written into a library. Files a library records are read in its order, so analyze first
when a directory does not sort into dependency order. Give the ``--define``, ``--psl`` and
``--relaxed`` the files were analyzed with, as for ``yavs run``. A file that fails analysis prints
its errors and is skipped; the others are still checked.

The exit status is 0 without a finding of severity error, 1 with one, and 2 for a usage error, a
bad configuration or a file that fails analysis.

How sure a rule is
------------------

.. list-table::
   :header-rows: 1
   :widths: 20 60 20

   * - Class
     - What a finding means
     - Default
   * - ``definite``
     - The program cannot do what it says.
     - on
   * - ``advisory``
     - An exact fact, whose significance depends on what you meant.
     - off
   * - ``experimental``
     - Inferred design intent.
     - off
   * - ``policy``
     - A house convention.
     - off

Configuration
-------------

``yavs-lint.yaml`` in the working directory or an ancestor, or the file ``--config`` names, sets
which rules run and how a finding counts. A rule's own setting wins over its class's, which wins over
``global``:

.. code-block:: yaml

   rule:
     global:
       severity: warning      # a finding is reported, and does not fail the run
     group:
       advisory:
         disable: false       # one class on
     lint_772:
       disable: true          # one rule off
       severity: error

``lint_602`` takes ``suffixes`` and ``lint_603`` takes ``prefixes``, lists in which ``*`` and ``?``
are allowed, or a regular expression after ``re:`` (``re:_p[0-9]+``), matched without regard to case.
With no list these two rules say nothing. ``-Wno-RULE``, ``-Werror`` and
``-Werror=RULE`` do the same for one run; ``-Werror`` makes a finding of severity warning an error.

Other keys are warned about and ignored: warning W0139, at any level of this file and of the waiver file, for
a key, a rule id or a rule group yavs does not have. Settings for a formatter or a style guide mean
nothing here, so a configuration shared with such a tool works as it is. The run goes on, ``-Werror``
makes it fail, and ``-Wno-W0139`` silences it. A name close to a real one gets a suggestion, and many in
one section are one warning that lists them all:

.. code-block:: console

   $ yavs lint src/pkg.vhd
   warning[W0139]: `yavs-lint.yaml` line 3: unknown rule `lnt_771` in `rule` is ignored; a rule with a similar name exists: `lint_771`

   warning[W0139]: `yavs-lint.yaml` lines 4 to 8: 5 unknown rules in `rule` are ignored:
                   `style_1`, `style_2`, `style_3`, `style_4`, `style_5`

   warning[W0139]: `yavs-lint.yaml` line 9: unknown key `indent` is ignored

   note: 0 findings in 1 file(s)

A misspelled key therefore changes nothing, so read these warnings. A bad value of a key yavs does read,
such as ``severity: fatal``, a YAML error or a file that cannot be read still stops the run with E0134.

To leave files out, list them under ``exclude``: paths or globs, relative to the configuration file,
of files or directories. A pattern that starts with ``**`` also matches files outside that directory.
An excluded file is still read when another file needs it:

.. code-block:: yaml

   exclude:
     - generated                # a directory
     - 'src/**/old_*.vhd'       # `*` and `?` stop at `/`, `**` does not
     - '**/vunit/vhdl/**'       # a test runner's own libraries, wherever they are installed

Testbenches and hardware
------------------------

A testbench is written to other rules than hardware: a signal driven from two places is often how it
works. The blocks ``rtl:`` and ``testbench:`` hold a ``rule:`` section each, which wins over the
top-level one for files of that kind. With neither block, files are not told apart.

.. code-block:: yaml

   testbench_files:
     - 'tb_*.vhd'           # a bare name: that file in any directory
     - 'verify/**'          # relative to this file, and below any directory
   testbench_libraries:
     - tb_lib
   testbench:
     rule:
       lint_601:
         disable: true
   rtl:
     rule:
       lint_600:
         disable: false

A file is a testbench, in this order, when ``testbench_files`` or ``testbench_libraries`` names it, when
it has the comment ``-- yavs: testbench`` in its first 2000 characters, when its code names
``vunit_lib``, ``osvvm``, ``uvvm_util``, ``bitvis`` or ``runner_cfg``, when it declares an entity with
no ports, or when its file name starts with ``tb_`` or ends in ``_tb``, or a directory on its path is
``test``, ``tests``, ``tb``, ``testbench``, ``testbenches``, ``sim``, ``bench`` or ``verification``. The
last three are inferences and can be wrong: list the files to decide it yourself. The run does not say
which files it took for testbenches. ``suffixes`` and ``prefixes`` hold for the whole run, wherever they
are set.

To name the files to check in the configuration, list them under ``file_list``: paths, directories or
globs. ``yavs lint`` with no file on the command line then checks them, and files on the command line
are checked as well as the list, each file once:

.. code-block:: yaml

   file_list:
     - modules/fifo/src/fifo.vhd     # a file
     - 'modules/*/src/*.vhd'         # `*` and `?` stop at `/`, sorted, hidden entries skipped
     - 'modules/**/test/*.vhd'       # `**` crosses directories
     - $IP_ROOT/src/*.vhd            # $NAME and ${NAME} come from the environment
     - generated                     # a directory, as on the command line

Entries are relative to the directory you run in, not to the configuration file, unlike ``exclude`` and
waivers. An entry that gives no file stops the run with ``E0134``, naming the entry, the configuration
and that directory. ``exclude`` wins over ``file_list``. ``--no-file-list`` leaves the list out, to
check one file, or the files a library records. Settings for one entry are not implemented; a waiver
turns a rule off for a file.

The files of a mapped library are read in the order of the library and into that library, whichever
way they were named. Other files are read in the order given, as library ``work`` or ``--work``, and a
glob gives them in sorted order: a file that uses a package that sorts after it fails analysis unless
that package is in an analyzed library given with ``--work`` or ``--map``. So analyze the design first
and give its libraries:

.. code-block:: console

   $ yavs analyze --work=fifo:out/fifo modules/fifo/src/fifo_pkg.vhd modules/fifo/src/fifo.vhd
   $ yavs lint --map=fifo:out/fifo

Waivers
-------

A waiver accepts a finding a project has decided about, with the reason. They live in
``yavs-lint-waivers.yaml`` next to the configuration, or the file ``--waivers`` names:

.. code-block:: yaml

   waivers:
     - rule: lint_772
       files: 'src/legacy/**/*.vhd'  # `*` and `?` stop at `/`, `**` does not
       lines: [12, 45]               # omit to waive the rule in the whole file
       reason: kept until the block is replaced

``rule: '*'`` covers every rule, and a bare file name matches that file in any directory. A waived
finding is in no report and changes no exit status. An unknown rule waives nothing and an unknown key is
ignored, each with warning W0139: a misspelled ``files`` or ``lines`` leaves a waiver that covers more than
it says, so run with ``-Werror`` where that matters.

Turning rules off for a directory or file pattern
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

``exclude`` takes files out for every rule. To leave them checked and turn off only some rules, give
one entry the rules and the places:

.. code-block:: yaml

   waivers:
     - rule:
         - lint_601
         - lint_004
       files:
         - src/legacy              # a directory: everything under it
         - 'third_party/**/*.vhd'  # a pattern
         - '**/vendor/**'          # a pattern that starts with `**` also matches outside this directory
       files_exclude:
         - src/legacy/current      # still checked: this wins over `files`
       reason: code we do not change

     - rule:
         - lint_713
       files_exclude:              # without `files`: every file except these
         - src/core
       reason: only the core names every value

``rule``, ``files`` and ``files_exclude`` take a list, dashed or as ``[a, b]``, or one word. Patterns
follow the rules of ``exclude`` and are relative to the waiver file, wherever the command runs and
whether it reads files or a library. A pattern that covers no file of the run and no VHDL file under
the waiver file gets the warning W0132, as it may be a typing mistake or a renamed directory. It is not
an error, because a shared waiver file may name generated files that do not exist yet; ``-Werror``
makes it one.

Reports
-------

Findings are ordinary diagnostics on standard error, so ``--message-format=short`` and ``json``
work as for every command. For a pipeline, write them to files as well:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Format
   * - ``--json=FILE``
     - Files with their findings: rule, line, severity, message.
   * - ``--sarif=FILE``
     - SARIF 2.1.0, for code scanning. The class of a rule is its tag.
   * - ``--junit=FILE``
     - JUnit, one test case per source file.
   * - ``--quality-report=FILE``
     - Code Climate, for GitLab code quality.
   * - ``--sonarqube=FILE``
     - SonarQube generic issues. A finding of a definite rule is a bug.

Design facts
------------

``--facts=FILE`` writes what the analysis knows about each architecture as JSON, whichever rules are
on, with a ``schema_version``:

.. code-block:: json

   {
     "schema_version": 1,
     "architectures": [{
       "unit": "fifo(rtl)",
       "file": "src/fifo.vhd",
       "state_machines": [{
         "type": "state_t", "signals": ["state"], "states": ["idle", "run"],
         "transitions": [{"from": "idle", "to": "run"}], "reset_state": "idle"
       }],
       "clock_domains": [{"clock": "clk", "registers": ["state"]}],
       "reset_domains": [{"reset": "rst", "registers": ["state"]}],
       "crossings": [{"kind": "clock", "signal": "flag", "from": "clk_a", "to": "clk_b", "line": 42}]
     }]
   }

With VUnit
----------

``python run.py --compile --yavs-lint`` checks every library the run compiled, after compiling, and
fails the run with a finding of severity error (:doc:`vunit-yavs`). It reads ``yavs-lint.yaml`` from
the directory you run in or an ancestor, so ``exclude`` applies. ``file_list`` does not: the project
says which files a library has. Warning W0139 is told for the first library only.

In VS Code
----------

The extension contributes the problem matcher ``$yavs``, so a task shows findings in the Problems
view and in the editor:

.. code-block:: json

   {
     "label": "yavs lint",
     "type": "shell",
     "command": "yavs lint --message-format=short --recursive src",
     "problemMatcher": "$yavs"
   }
