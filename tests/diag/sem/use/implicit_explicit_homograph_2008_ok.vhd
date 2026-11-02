-- yavs-test: std=2008
-- expect: none
-- LRM 2008 12.4 b): of two potentially visible homographs the explicit one hides the implicit one
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
