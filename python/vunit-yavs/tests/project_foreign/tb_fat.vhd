library vunit_lib;
context vunit_lib.vunit_context;
library native_demo;
use native_demo.native_pkg.all;

entity tb_fat is
  generic (runner_cfg : string);
end entity;

architecture a of tb_fat is
begin
  main : process
  begin
    test_runner_setup(runner, runner_cfg);
    while test_suite loop
      if run("A string reaches the library as a fat pointer") then
        check_equal(native_fat_len("hello"), 6);
      end if;
    end loop;
    test_runner_cleanup(runner);
  end process;
end architecture;
