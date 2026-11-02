-- yavs-test: std=1993 flags=--relaxed
-- expect: W1302 12:34 `--relaxed` (R7) uses the explicit `=` of std_logic_unsigned
-- LRM 1993 10.4, 10.5; `--relaxed` (R7) takes the explicit one, as 2008 12.4 does
library ieee;
use ieee.std_logic_1164.all;
use ieee.std_logic_unsigned.all;
entity e is
end entity;
architecture a of e is
  signal a, b : std_logic_vector(3 downto 0);
begin
  process (a, b) is begin assert a = b; end process;
end architecture;
