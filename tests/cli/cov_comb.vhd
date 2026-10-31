-- Combination coverage and the in-source exclusions of its decisions
-- (crates/yavs/tests/coverage_comb.rs): the requirements of every decision below are pinned there.
library ieee;
use ieee.std_logic_1164.all;

entity cov_comb is end entity;

architecture sim of cov_comb is
  signal a, b, c : boolean := false;
  signal x, y    : std_logic := '0';
  signal q       : std_logic;
begin
  -- Concurrent assignment of a std_logic expression: no short-circuit, four combinations.
  -- yavs cov exclude 11: x and y are never high together
  q <= x and y;

  stim : process
    variable n, calls : integer := 0;
    variable v        : boolean;
    impure function tick(t : boolean) return boolean is
    begin
      calls := calls + 1;
      return t;
    end function;
  begin
    -- (a, b, c) over all eight combinations.
    for i in 0 to 7 loop
      a <= (i / 4) mod 2 = 1;
      b <= (i / 2) mod 2 = 1;
      c <= i mod 2 = 1;
      wait for 1 ns;
      -- With `a` false the rest is not evaluated; `b or c` is skipped when `b` is true.
      -- yavs coverage exclude condition (a=false): a low is covered elsewhere
      -- yavs cov exclude (b=false, c=false)
      if a and (b or c) then
        n := n + 1;
      -- yavs cov exclude 1-
      elsif b and c then
        n := n + 2;
      end if;
      -- Both operands of xor are evaluated, the or after it is not when the xor holds.
      v := not (a xor b) or c;
      -- Function calls are inputs and are addressed by position; `tick(b)` is called only
      -- when `tick(a)` is true.
      if tick(a) and tick(b) then
        n := n + 10;
      end if;
      -- The same object twice: its two readings agree.
      if a and not a then
        n := n + 100;
      end if;
    end loop;
    -- std_logic inputs: 'H' is 1, 'L' is 0, 'X' and 'U' are no combination.
    for i in 0 to 3 loop
      case i is
        when 0 => x <= '0'; y <= '1';
        when 1 => x <= 'H'; y <= 'L';
        when 2 => x <= 'X'; y <= '1';
        when others => x <= '1'; y <= 'U';
      end case;
      wait for 1 ns;
      if x and y then
        n := n + 1000;
      end if;
    end loop;
    report "n=" & integer'image(n) & " calls=" & integer'image(calls);
    wait;
  end process;
end architecture;
