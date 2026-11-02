-- yavs-test: std=1993
-- expect: E1302 12:25 `=` is declared by std_logic_1164 and std_logic_unsigned; 1993 hides neither
-- help: --std=2008
-- LRM 1993 10.4, 10.5: two potentially visible subprograms with the same designator are both made directly visible
library ieee;
use ieee.std_logic_1164.all;
use ieee.std_logic_unsigned.all;
entity e is
end entity;
architecture a of e is
  signal a, b : std_logic_vector(3 downto 0);
  signal y : boolean := a = b;
begin
end architecture;
