-- An `or` of two properties is outside the PSL simple subset (IEEE 1850 4.4.4, 1076-2008 1.3.5).
library ieee;
use ieee.std_logic_1164.all;
entity e is end;
architecture rtl of e is
  signal clk, a, b, c : std_logic := '0';
  default clock is rising_edge(clk);
begin
  bad: assert always ((a = '1' -> next (b = '1')) or (a = '1' -> next[2] (c = '1')));
  ok: assert always ((next (b = '1')) or (a = '1'));
end architecture;
