-- PSL formals of different kinds are separated by `;` (IEEE 1850, PSL 1.1 Box 38; #1078).
library ieee;
use ieee.std_logic_1164.all;
entity e is end;
architecture rtl of e is
  signal clk, a, b : std_logic := '0';
  default clock is rising_edge(clk);
  property then_y (sequence p, boolean y) is always (p |=> y);
begin
  p1: assert then_y({a = '1'}, b = '1');
end architecture;
