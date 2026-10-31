..
   Generated from the console's command table: YAVS_BLESS=1 cargo test -p yavs --bin yavs cli_reference

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
