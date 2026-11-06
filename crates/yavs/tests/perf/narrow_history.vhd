-- Narrow signals (#1111): registers nothing asks the history of keep none, and the attributes of
-- the others give exact answers per element: 'event and edges of an element of a vector that
-- changes several elements at once, 'last_event and 'last_value of elements and slices, through an
-- alias and a port that is a slice of its actual, and the implicit signals. No signal parameter and
-- no external name (`narrow_history_ext`), which would keep every wide history.
library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
entity narrow_sub is
  port (clk : in std_logic; p : in std_logic_vector(3 downto 0); k : out natural);
end;
architecture a of narrow_sub is
  signal x : std_logic_vector(5 downto 0) := "000111";
begin
  process (clk)
    variable n : natural := 0;
  begin
    if rising_edge(clk) then
      x <= x(4 downto 0) & not x(5);
      if p(1)'last_event < 12 ns then n := n + 1; end if;
      if p'last_value(2) /= p(2) then n := n + 10; end if;
      k <= n;
    end if;
  end process;
end;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
entity narrow_history is end;
architecture a of narrow_history is
  signal clk : std_logic := '0';
  signal done : boolean := false;
  -- Nothing asks the history of these.
  signal r8 : std_logic_vector(7 downto 0) := x"5A";
  signal r18 : signed(17 downto 0) := to_signed(1234, 18);
  signal r64 : std_logic_vector(63 downto 0) := (0 => '1', 9 => '1', others => '0');
  signal cnt : natural := 0;
  -- These are asked.
  signal d : std_logic_vector(7 downto 0) := x"0F";
  signal e : std_logic_vector(11 downto 0) := x"0F0";
  signal f : std_logic_vector(15 downto 0) := x"00FF";
  signal g : std_logic_vector(7 downto 0) := x"33";
  signal h : std_logic_vector(7 downto 0) := x"C3";
  signal sp : std_logic_vector(5 downto 0) := "010101";
  -- Only the prefix of implicit signals: the kernel reads its history itself.
  signal sq : std_logic_vector(5 downto 0) := "000011";
  signal dev, ded, k : natural := 0;
  alias ga : std_logic_vector(3 downto 0) is g(7 downto 4);
begin
  clk <= not clk after 5 ns when not done;
  done <= true when cnt = 60;
  sub : entity work.narrow_sub port map (clk => clk, p => h(5 downto 2), k => k);
  process (clk) begin
    if rising_edge(clk) then
      cnt <= cnt + 1;
      r8 <= r8(6 downto 0) & (r8(7) xor r8(3));
      r18 <= r18 + 77;
      r64 <= r64(62 downto 0) & (r64(63) xor r64(4));
      d <= d(6 downto 0) & not d(7);
      if cnt mod 3 = 0 then e <= e(10 downto 0) & not e(11); end if;
      if cnt mod 4 /= 1 then f <= std_logic_vector(unsigned(f) + 37); end if;
      if cnt mod 5 < 2 then g <= not g; end if;
      h <= h(6 downto 0) & h(7);
      if cnt mod 7 = 3 then sp <= not sp; end if;
      if cnt mod 3 /= 0 then sq <= sq(4 downto 0) & sq(5); end if;
    end if;
  end process;
  -- Woken by d, not by the clock: compiled code answers these from the event stamps.
  process (d) begin
    if d(3)'event then dev <= dev + 1; end if;
    if rising_edge(d(5)) then ded <= ded + 1; end if;
  end process;
  process
    variable n, m, q : natural := 0;
    variable t : time := 0 ns;
  begin
    while not done loop
      wait until clk = '0';
      if e(4)'last_event < 25 ns then n := n + 1; end if;
      if e(6 downto 2)'last_event < 25 ns then n := n + 100; end if;
      if e'last_event < 15 ns then t := t + e'last_event; end if;
      if e(9 downto 3)'last_value /= e(9 downto 3) then m := m + 1; end if;
      if e'last_value(1) = '1' then m := m + 10; end if;
      if e'event then m := m + 1000; end if;
      if ga'last_event < 12 ns then q := q + 1; end if;
      if f(3)'last_active < 6 ns then q := q + 100; end if;
      if not f'stable(8 ns) then q := q + 1000; end if;
      if f(2 downto 1)'quiet(6 ns) then q := q + 10000; end if;
      if f'delayed(3 ns) /= f then q := q + 100000; end if;
      if sp(4 downto 1)'last_event < 30 ns then n := n + 10000; end if;
      if not sq'stable(12 ns) then m := m + 100000; end if;
      if sq'delayed(7 ns) /= sq then m := m + 10000000; end if;
      if sq(2 downto 1)'quiet(4 ns) then q := q + 1000000; end if;
    end loop;
    report "n=" & integer'image(n) & " m=" & integer'image(m) & " q=" & integer'image(q)
      & " t=" & time'image(t) & " dev=" & integer'image(dev) & " ded=" & integer'image(ded)
      & " k=" & integer'image(k) & " r8=" & to_hstring(r8) & " r18=" & to_hstring(r18)
      & " r64=" & to_hstring(r64);
    wait;
  end process;
  process (f'transaction) begin
    if f'transaction'event then null; end if;
  end process;
end;
