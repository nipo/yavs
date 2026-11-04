-- pgo: top=element_ports_tb std=2008 stop=4us
-- A wide vector signal with one instance's out port on each element: many sources on one leaf at elaboration,
-- as a register bank or a generated bus has. The cells form a Johnson counter, checked against a model each cycle.
library ieee;
use ieee.std_logic_1164.all;

entity ep_cell is
  generic (POL : boolean);
  port (clk : in std_logic; d : in std_logic; q : out std_logic);
end entity;

architecture rtl of ep_cell is
  signal r : std_logic := '0';
begin
  process (clk)
  begin
    if rising_edge(clk) then
      r <= d when POL else not d;
    end if;
  end process;
  q <= r when POL else not r;
end architecture;

library ieee;
use ieee.std_logic_1164.all;

entity element_ports_tb is
  generic (N : positive := 1500; CYCLES : positive := 300);
end entity;

architecture sim of element_ports_tb is
  signal clk  : std_logic := '0';
  signal done : boolean := false;
  signal bank : std_logic_vector(N - 1 downto 0);
begin
  clk <= not clk after 5 ns when not done;

  cells : for i in 0 to N - 1 generate
    signal d : std_logic;
  begin
    first : if i = 0 generate
      d <= not bank(N - 1);
    end generate;
    others_g : if i > 0 generate
      d <= bank(i - 1);
    end generate;
    u : entity work.ep_cell
      generic map (POL => i mod 2 = 0)
      port map (clk => clk, d => d, q => bank(i));
  end generate;

  check : process
    variable model : std_logic_vector(N - 1 downto 0);
  begin
    for i in 0 to N - 1 loop  -- a cell with POL false starts at '1'
      model(i) := '1' when i mod 2 = 1 else '0';
    end loop;
    for c in 1 to CYCLES loop
      wait until rising_edge(clk);
      wait for 1 ns;
      model := model(N - 2 downto 0) & not model(N - 1);
      assert bank = model report "bank differs from the model at cycle " & integer'image(c) severity failure;
    end loop;
    report "PASS";
    done <= true;
    wait;
  end process;
end architecture;
