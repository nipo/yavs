-- The bounds of an object declared in a process can name an external object (#1099).
library ieee;
use ieee.std_logic_1164.all;
entity external_constraint_dut is
end;
architecture a of external_constraint_dut is
  signal v : std_logic_vector(7 downto 0) := x"a5";
begin
end;
library ieee;
use ieee.std_logic_1164.all;
entity external_in_process_constraint is
end;
architecture t of external_in_process_constraint is
begin
  p : process
    variable x : std_logic_vector(<< signal .external_in_process_constraint.u.v : std_logic_vector >>'range);
    procedure q is
      variable y : std_logic_vector(<< signal .external_in_process_constraint.u.v : std_logic_vector >>'range);
    begin
      assert y'length = 8;
    end;
  begin
    wait for 1 ns;
    x := << signal .external_in_process_constraint.u.v : std_logic_vector >>;
    assert x = x"a5";
    q;
    std.env.finish;
  end process;
  u : entity work.external_constraint_dut;
end;
