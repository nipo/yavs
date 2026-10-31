-- The testbench of the worked examples of the interactive debugger page.
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity reg is
  port (clk, d : in std_logic; q : out std_logic);
end entity;

architecture rtl of reg is
begin
  flop : process (clk)
  begin
    if rising_edge(clk) then
      q <= d;
    end if;
  end process;
end architecture;

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity tb is
end entity;

architecture sim of tb is
  signal clk    : std_logic := '0';
  signal enable : std_logic;  -- nothing drives it
  signal q      : std_logic;
  signal cnt    : unsigned(3 downto 0) := (others => '0');
  signal level  : std_logic_vector(1 downto 0);
begin
  clk <= not clk after 5 ns when cnt < 8;

  u_reg : entity work.reg port map (clk => clk, d => enable, q => q);

  counter : process (clk)
    variable total : integer := 0;
  begin
    if rising_edge(clk) then
      total := total + 1;
      cnt <= cnt + 1;
    end if;
  end process;

  level <= "01";
  level <= "11" when cnt > 2 else "01";

  check : process
  begin
    wait until cnt = 3;
    assert q = '1' report "q is not high" severity error;
    wait;
  end process;
end architecture;
