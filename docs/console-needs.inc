..
   Generated from the console's command table: YAVS_BLESS=1 cargo test -p yavs --bin yavs cli_reference

.. list-table::
   :widths: 40 60

   * - ``wave add NAME|PATTERN...``, ``wave cursor [TIME]``, ``wave save [FILE]``, ``wave load [FILE]``
     - A viewer: run with ``--surfer`` (:doc:`waveforms`)
   * - ``examine NAME @ TIME``
     - A wave file: run with ``--wave=FILE.fst``
   * - ``vunit_help``, ``vunit_load``, ``vunit_user_init``, ``vunit_run``, ``vunit_compile``, ``vunit_restart``
     - The test runner's interface (``vunit-yavs``, ``--yavs-interactive``)
