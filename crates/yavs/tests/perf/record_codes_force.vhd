-- Byte codes of a forced record field: `f` is no pack (it is forced), and its vector fields keep
-- no codes. The report is the same with the codes of every signal (YAVS_CODES=all).
library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
entity record_codes_force is end;
architecture a of record_codes_force is
  type pair is record
    re, im : signed(17 downto 0);
  end record;
  signal clk : std_logic := '0';
  signal n : natural := 0;
  signal f, g : pair := (to_signed(9, 18), to_signed(10, 18));
  signal sum : signed(23 downto 0) := (others => '0');
  signal done : boolean := false;
begin
  clk <= not clk after 5 ns when not done;
  process (clk) is
  begin
    if rising_edge(clk) then
      n <= n + 1;
      f <= (re => f.im + 1, im => f.re - n);
      g <= f;
      sum <= sum + g.re - f.im;
    end if;
  end process;
  process is
  begin
    wait until n = 40;
    f.re <= force to_signed(77, 18);
    wait until n = 50;
    g <= force (to_signed(5, 18), to_signed(6, 18));
    wait until n = 60;
    f.re <= release;
    wait until n = 70;
    g <= release;
    wait until n = 300;
    report "n=" & integer'image(n) & " s=" & to_hstring(sum) & " f=" & to_hstring(f.re)
      & to_hstring(f.im) & " g=" & to_hstring(g.re) & to_hstring(g.im);
    done <= true;
    wait;
  end process;
end;
