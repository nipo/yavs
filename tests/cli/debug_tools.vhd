-- A real unknown value: a never driven signal through a port and a flip-flop, two drivers that disagree, reports of two severities.
library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
entity sub is port (d : in std_logic; q : out std_logic; clk : in std_logic); end;
architecture a of sub is
begin
  reg : process (clk) begin
    if rising_edge(clk) then q <= d; end if;
  end process;
end;
library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
entity debug_tools is end;
architecture a of debug_tools is
  signal clk : std_logic := '0';
  signal never : std_logic;          -- never driven: 'U'
  signal a, b : std_logic := '0';
  signal q, r : std_logic;
  signal bus_s : std_logic_vector(3 downto 0);
  signal cnt : unsigned(3 downto 0) := (others => '0');
  type st_t is (idle, busy);
  signal st : st_t := idle;
  signal num : integer range 0 to 15 := 0;
  type rec_t is record f : std_logic; g : integer; end record;
  signal rc : rec_t := ('0', 3);
begin
  clk <= not clk after 5 ns when cnt < 8;
  u1 : entity work.sub port map (d => never, q => q, clk => clk);
  r <= q and a;
  bus_s <= "0101";
  bus_s(1 downto 0) <= "1Z";
  counter : process (clk)
    variable total : integer := 0;
  begin
    if rising_edge(clk) then
      total := total + 1;
      cnt <= cnt + 1;
      num <= total mod 16;
      if total = 3 then st <= busy; end if;
    end if;
  end process;
  check : process begin
    wait until cnt = 3;
    report "three" severity note;
    assert false report "cnt reached 3" severity error;
    wait until cnt = 5;
    assert false report "fatal now" severity warning;
    wait;
  end process;
end;
