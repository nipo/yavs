-- Byte codes of records: the record packs `p0` to `p3` (vector fields only), the arrays of records
-- `a` (vectors, read element by element through a run-time index) and `m` (a scalar field, which
-- keeps its codes), the forced field `f.re` and `e`, whose field has a read of one element. The
-- report is the same with the codes of every signal (YAVS_CODES=all).
library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
package record_codes_pkg is
  type pair is record
    re, im : signed(17 downto 0);
  end record;
  type pairs is array (0 to 3) of pair;
  type tagged is record
    v : std_logic;
    d : unsigned(7 downto 0);
  end record;
  type taggeds is array (0 to 3) of tagged;
end;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use work.record_codes_pkg.all;
entity record_codes is end;
architecture a of record_codes is
  signal clk : std_logic := '0';
  signal n : natural := 0;
  signal p0 : pair := (to_signed(3, 18), to_signed(-4, 18));
  signal p1, p2, p3 : pair := (to_signed(-5, 18), to_signed(6, 18));
  signal a : pairs := (others => (to_signed(1, 18), to_signed(2, 18)));
  signal m : taggeds := (others => ('0', x"01"));
  signal f : pair := (to_signed(9, 18), to_signed(10, 18));
  signal e : pair := (to_signed(11, 18), to_signed(12, 18));
  signal h : pair := (to_signed(0, 18), to_signed(0, 18));
  signal q : signed(17 downto 0);
  signal w : natural := 0;
  signal done : boolean := false;
  signal sum : signed(23 downto 0) := (others => '0');
begin
  clk <= not clk after 5 ns when not done;
  process (clk) is
    variable v : pair;
    variable t : tagged;
    variable i : natural;
  begin
    if rising_edge(clk) then
      n <= n + 1;
      i := n mod 4;
      p0 <= (re => p0.im + n, im => p0.re - 1);
      p1 <= p0;
      p2 <= (re => p1.re, im => p1.im);
      p3.re <= p2.im; p3.im <= p2.re;
      -- A part of an array of records read element by element.
      v := a((i + 1) mod 4);
      a(i) <= (re => v.im + 1, im => v.re xor p3.re);
      t := m((i + 3) mod 4);
      m(i) <= (v => not t.v, d => t.d + unsigned(v.re(7 downto 0)));
      f <= (re => f.im + 1, im => f.re - p1.im);
      e <= (re => e.im + 3, im => e.re - 2);
      if n mod 4 = 0 then
        h <= (re => h.re + 1, im => h.im);
      else
        h.im <= h.im - n;
      end if;
      if e.re(2) = '1' then
        sum <= sum + p3.re - f.re + to_integer(m(i).d);
      else
        sum <= sum + p3.im + a(i).re;
      end if;
    end if;
  end process;
  q <= h.re;
  process (h.re) is
  begin
    w <= w + to_integer(h.re);
  end process;
  process is
    variable t : tagged;
  begin
    wait until n = 300;
    t := m(1);
    report "n=" & integer'image(n) & " s=" & to_hstring(sum) & " p3=" & to_hstring(p3.re)
      & to_hstring(p3.im) & " a=" & to_hstring(a(2).re) & to_hstring(a(3).im) & " m="
      & std_logic'image(t.v) & to_hstring(t.d) & " f=" & to_hstring(f.re) & to_hstring(f.im)
      & " h=" & to_hstring(q) & to_hstring(h.im) & " w=" & integer'image(w);
    done <= true;
    wait;
  end process;
end;
