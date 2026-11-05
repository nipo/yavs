-- An external signal in a block guard, evaluated at run time, and as the initial value of a
-- signal, which is the initial value of the target (#1102).
library ieee;
use ieee.std_logic_1164.all;
entity external_guard_dut is
end;
architecture a of external_guard_dut is
  signal s : std_logic := '0';
  signal k : std_logic := '1';
  signal v : std_logic_vector(3 downto 0) := x"5";
begin
  s <= '1' after 2 ns;
end;
library ieee;
use ieee.std_logic_1164.all;
entity external_guard_and_init is
end;
architecture t of external_guard_and_init is
  signal o : std_logic := '0';
begin
  g : block (<< signal .external_guard_and_init.u.s : std_logic >> = '1')
  begin
    o <= guarded '1';
  end block;
  u : entity work.external_guard_dut;
  i : block
    signal x : std_logic := << signal .external_guard_and_init.u.k : std_logic >>;
    signal y : std_logic_vector(3 downto 0) := << signal .external_guard_and_init.u.v : std_logic_vector(3 downto 0) >>;
  begin
    process begin
      assert x = '1' and y = x"5";
      wait for 1 ns;
      assert o = '0';
      wait for 2 ns;
      assert o = '1';
      std.env.finish;
    end process;
  end block;
end;
