-- One stage of an adder tree as a function on an array of `signed`, called on signals from a
-- clocked process: the element operands and targets are copied in compiled code, the local's
-- default is no fill, and the signal copies in the process's frame are found at the entry.
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity clift_elem_ops is end;
architecture a of clift_elem_ops is
  type t_arr is array (natural range <>) of signed(15 downto 0);
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
  signal clk : std_logic := '0';
  signal t : t_arr(0 to 12) := (others => (others => '0'));
  signal l1 : t_arr(0 to 6) := (others => (others => '0'));
  signal l2 : t_arr(0 to 3) := (others => (others => '0'));
begin
  clk <= not clk after 5 ns when now < 2 us;
  process (clk) is
  begin
    if rising_edge(clk) then
      for k in t'range loop
        t(k) <= t(k) + to_signed(3 * k + 1, 16);
      end loop;
      l1 <= reduce(t);
      l2 <= reduce(l1);
    end if;
  end process;
  process is
  begin
    wait for 2 us;
    report "sum=" & to_hstring(l2(0) + l2(1) + l2(2) + l2(3));
    wait;
  end process;
end;
