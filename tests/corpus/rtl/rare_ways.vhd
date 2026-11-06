-- difftest: top=rtl_rare_ways std=2008 stop=2us
-- The rare ways of compiled code, which compiled code keeps out of line: reads of a short slice
-- that straddles a 64-element word of a wide signal (the codes, not the one plane word), in-place
-- numeric_std operations whose operands sometimes hold metavalues or are too wide (the native
-- call), and statements that make temporaries (the cut back of the temporary area).
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity rtl_rare_ways is end;

architecture t of rtl_rare_ways is
  signal clk  : std_logic := '0';
  signal n    : natural := 0;
  signal wide : std_logic_vector(127 downto 0) := (others => '0');
  signal a, b : unsigned(15 downto 0) := (others => '0');
  signal big  : unsigned(79 downto 0) := (others => '0');
  signal sum  : unsigned(15 downto 0);
  signal bsum : unsigned(79 downto 0);
  signal lo, hi, mid : std_logic_vector(15 downto 0);
  signal cat  : std_logic_vector(31 downto 0);
begin
  clk <= not clk after 5 ns;

  stim : process (clk)
  begin
    if rising_edge(clk) then
      n <= n + 1;
      wide <= wide(118 downto 0) & std_logic_vector(to_unsigned(n mod 509, 9));
      a <= to_unsigned((n * 37) mod 65536, 16);
      b <= to_unsigned((n * 91 + 5) mod 65536, 16);
      -- Every seventh cycle an operand with a metavalue.
      if n mod 7 = 3 then
        b(4) <= 'X';
      end if;
      big <= big + to_unsigned(n * 1001 + 7, 80);
    end if;
  end process;

  work : process (clk)
    variable bits : std_logic_vector(15 downto 0);
  begin
    if rising_edge(clk) then
      for k in 0 to 15 loop
        bits(k) := wide(56 + k);
      end loop;
      assert wide(71 downto 56) = bits report "straddling slice read wrong" severity failure;
      lo  <= wide(15 downto 0);
      mid <= wide(71 downto 56);      -- straddles element 64
      hi  <= wide(127 downto 112);
      sum <= a + b;
      bsum <= big + big;              -- wider than one word
      cat <= wide(63 downto 56) & wide(71 downto 64) & std_logic_vector(a);
    end if;
  end process;

  rep : process (clk)
  begin
    if falling_edge(clk) and n mod 10 = 0 then
      report to_hstring(lo) & " " & to_hstring(mid) & " " & to_hstring(hi) & " " &
        to_hstring(std_logic_vector(sum)) & " " & to_hstring(std_logic_vector(bsum)) & " " &
        to_hstring(cat);
    end if;
  end process;
end;
