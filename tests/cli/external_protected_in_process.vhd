-- A method of a protected type declared in a process drives for that process (#1097).
library ieee;
use ieee.std_logic_1164.all;
entity external_protected_dut is
end;
architecture a of external_protected_dut is
  signal s : std_logic := '0';
begin
end;
library ieee;
use ieee.std_logic_1164.all;
entity external_protected_in_process is
end;
architecture t of external_protected_in_process is
  signal loc : std_logic := '0';
begin
  u : entity work.external_protected_dut;
  p : process
    type pt is protected
      procedure wr;
    end protected;
    type pt is protected body
      procedure wr is
      begin
        << signal .external_protected_in_process.u.s : std_logic >> <= '1';
        loc <= '1';
      end;
    end protected body;
    variable sv : pt;
  begin
    sv.wr;
    wait for 1 ns;
    assert << signal .external_protected_in_process.u.s : std_logic >> = '1';
    assert loc = '1';
    std.env.finish;
  end process;
end;
