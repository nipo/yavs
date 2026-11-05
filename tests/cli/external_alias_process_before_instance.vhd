-- An alias of an external name declared in a process binds once the hierarchy is complete, so
-- the process may come before the instance it names, as a name in its statements may.
library ieee;
use ieee.std_logic_1164.all;
entity external_alias_process_dut is
end;
architecture a of external_alias_process_dut is
  signal s : std_logic := '0';
  signal v : std_logic_vector(7 downto 0) := x"a5";
  constant c : integer := 7;
begin
end;
library ieee;
use ieee.std_logic_1164.all;
entity external_alias_process_before_instance is
end;
architecture t of external_alias_process_before_instance is
begin
  p : process
    alias sa is << signal .external_alias_process_before_instance.u.s : std_logic >>;
    alias va is << signal u.v : std_logic_vector >>;
    alias part is << signal u.v : std_logic_vector(7 downto 0) >>(3 downto 0);
    alias ka is << constant u.c : integer >>;
    variable x : std_logic_vector(va'range);
  begin
    sa <= '1';
    wait on sa;
    x := va;
    assert sa = '1';
    assert x = x"a5" and x'length = 8;
    assert part = x"5";
    assert ka = 7;
    std.env.finish;
  end process;
  u : entity work.external_alias_process_dut;
end;
