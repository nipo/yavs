-- One stage of an adder tree as a function on an array of vectors, called from a clocked
-- process: its result comes back as data and bounds, no pair and no descriptor copy (#1122).
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity elem_result is end;
architecture t of elem_result is
  type t_arr is array (natural range <>) of signed(15 downto 0);
  signal clk : bit := '0';
  signal t  : t_arr(0 to 12) := (others => (others => '0'));
  signal l1 : t_arr(0 to 6) := (others => (others => '0'));
  function reduce (src : t_arr) return t_arr is
    variable r : t_arr(0 to (src'length + 1) / 2 - 1);
  begin
    for k in r'range loop
      if 2 * k + 1 <= src'high then
        r(k) := src(2 * k) + src(2 * k + 1);
      else
        r(k) := src(2 * k);
      end if;
    end loop;
    return r;
  end function;
begin
  clk <= not clk after 5 ns when now < 2 us;
  process (clk)
    variable n : integer := 0;
  begin
    if clk = '1' then
      for k in t'range loop
        t(k) <= t(k) + to_signed(3 * k + 1, 16);
      end loop;
      l1 <= reduce(t);
      n := n + 1;
      if n = 200 then
        report "sum=" & integer'image(to_integer(l1(0) + l1(6)));
      end if;
    end if;
  end process;
end architecture;
