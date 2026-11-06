-- Narrow signals (#1111) asked through a signal parameter and an external name: those keep their
-- history, the registers nothing asks keep none.
library ieee; use ieee.std_logic_1164.all;
entity narrow_ext_sub is
  port (clk : in std_logic);
end;
architecture a of narrow_ext_sub is
  signal x : std_logic_vector(5 downto 0) := "000111";
  signal y : std_logic_vector(5 downto 0) := "010101";
begin
  process (clk) begin
    if rising_edge(clk) then
      x <= x(4 downto 0) & not x(5);
      y <= y(4 downto 0) & y(5);
    end if;
  end process;
end;

library ieee; use ieee.std_logic_1164.all;
entity narrow_history_ext is end;
architecture a of narrow_history_ext is
  signal clk : std_logic := '0';
  signal cnt : natural := 0;
  signal sp : std_logic_vector(5 downto 0) := "010101";
  procedure older(signal s : in std_logic_vector; t : time; n : inout natural) is
  begin
    if s'last_event < t then n := n + 1; end if;
  end;
begin
  clk <= not clk after 5 ns when cnt < 40;
  sub : entity work.narrow_ext_sub port map (clk => clk);
  process (clk) begin
    if rising_edge(clk) then
      cnt <= cnt + 1;
      if cnt mod 3 = 0 then sp <= not sp; end if;
    end if;
  end process;
  process
    variable n, q : natural := 0;
    alias ex is <<signal .narrow_history_ext.sub.x : std_logic_vector(5 downto 0)>>;
  begin
    while cnt < 40 loop
      wait until clk = '0';
      older(sp(4 downto 1), 25 ns, n);
      if ex(2)'last_event < 12 ns then q := q + 1; end if;
      if ex(4 downto 1)'last_value /= ex(4 downto 1) then q := q + 100; end if;
    end loop;
    report "n=" & integer'image(n) & " q=" & integer'image(q);
    wait;
  end process;
end;
