library vunit_lib;
context vunit_lib.vunit_context;
library native_demo;
use native_demo.native_pkg.all;

entity tb_init is
  generic (runner_cfg : string);
end entity;

architecture a of tb_init is
  -- Evaluated while the design is elaborated: the library must be loaded then
  signal initial : integer := native_add1(41);
begin
  main : process
  begin
    test_runner_setup(runner, runner_cfg);
    while test_suite loop
      if run("A foreign function gives a signal its initial value") then
        check_equal(initial, 42);
      end if;
    end loop;
    test_runner_cleanup(runner);
  end process;
end architecture;
