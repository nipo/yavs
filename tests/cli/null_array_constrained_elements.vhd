-- A folded aggregate of a null array has no element to take the element bounds from (#1098).
library ieee;
use ieee.std_logic_1164.all;
entity null_array_constrained_elements is
end;
architecture t of null_array_constrained_elements is
  type vv is array (natural range <>) of std_logic_vector;
  signal s : vv(1 to 0)(3 downto 0);
  signal q : vv(0 to 1)(-1 downto 0);
  signal r : vv(1 to 0)(3 downto 0);
begin
  s <= (others => (others => '0'));
  q <= (others => (others => '0'));
  r <= (others => "0000");
  process
    variable m : vv(1 to 0)(7 downto 0) := (others => "10100101");
  begin
    wait for 1 ns;
    assert s'length = 0 and q(0)'length = 0 and m'length = 0;
    std.env.finish;
  end process;
end;
