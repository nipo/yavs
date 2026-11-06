-- One adder-tree stage written once as a procedure with signal parameters of an unconstrained
-- array type: an element of the formal is read as that element of the actual (no read of the
-- whole signal), its bounds come from the actual's descriptor, and compiled code slices the
-- reference and drives the element without the interpreter.
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity sigparam_tree is end;
architecture a of sigparam_tree is
  type t_arr is array (natural range <>) of signed(15 downto 0);
  signal clk : std_logic := '0';
  signal t : t_arr(0 to 12) := (others => (others => '0'));
  signal l1 : t_arr(0 to 6) := (others => (others => '0'));
  signal l2 : t_arr(0 to 3) := (others => (others => '0'));
  signal l3 : t_arr(0 to 1) := (others => (others => '0'));
  signal done : boolean := false;
begin
  clk <= not clk after 5 ns when not done;
  process (clk) is
    procedure reduce (signal src : in t_arr; signal dst : out t_arr) is
    begin
      for k in dst'range loop
        if 2 * k + 1 <= src'high then
          dst(k) <= src(2 * k) + src(2 * k + 1);
        else
          dst(k) <= src(2 * k);
        end if;
      end loop;
    end procedure;
  begin
    if rising_edge(clk) then
      for k in t'range loop
        t(k) <= t(k) + to_signed(3 * k + 1, 16);
      end loop;
      reduce(t, l1);
      reduce(l1, l2);
      reduce(l2, l3);
    end if;
  end process;
  process is
  begin
    for i in 1 to 100 loop
      wait until rising_edge(clk);
    end loop;
    report "q=" & to_hstring(l3(0) + l3(1));
    done <= true;
    wait;
  end process;
end;
