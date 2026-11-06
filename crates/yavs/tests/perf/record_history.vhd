-- Records whose history is asked, through a field or as a whole, beside records nothing asks the
-- history of, which keep none (#1117). Every answer goes into the report.
library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
package record_history_pkg is
  type rec is record
    a : std_logic_vector(7 downto 0);
    b : std_logic;
    c : unsigned(17 downto 0);
  end record;
  type recs is array (0 to 3) of rec;
  constant ZERO : rec := (x"00", '0', (others => '0'));
end package;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use work.record_history_pkg.all;
entity record_history_sub is
  port (clk : in std_logic; ri : in rec; ro : out rec);
end;
architecture a of record_history_sub is
  -- Asked through an external name only.
  signal x : rec := ZERO;
begin
  -- The history of the actual of `ri` is asked only here.
  process (ri)
    variable n : unsigned(17 downto 0) := (others => '0');
  begin
    if ri.b'event then n := n + 1; end if;
    if ri.c'last_event > 30 ns then n := n + 7; end if;
    ro <= (ri.a, ri.b, n);
  end process;
  process (clk) begin
    if rising_edge(clk) then
      x.c <= x.c + 3;
      x.b <= not x.b;
    end if;
  end process;
end;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use work.record_history_pkg.all;
entity record_history is end;
architecture a of record_history is
  signal clk : std_logic := '0';
  signal cnt : natural := 0;
  -- q, g, m, s are asked; p and pm are not.
  signal q, p, g, s : rec := ZERO;
  signal m, pm : recs := (others => ZERO);
  signal nq, nm, ng, np : natural := 0;
  -- pi is asked in the child through a port, w through a whole-record signal parameter; po is not.
  signal pi, po, w : rec := ZERO;
  -- Nanoseconds, 0 for never (TIME'HIGH).
  function in_ns(t : time) return natural is
  begin
    if t > 1 ms then return 0; end if;
    return t / 1 ns;
  end;
  procedure older(signal f : in unsigned; t : time; n : inout natural) is
  begin
    if f'last_event < t then n := n + 1; end if;
  end;
  procedure whole_age(signal r : in rec; n : inout natural) is
  begin
    if r'last_event < 20 ns then n := n + 1; end if;
    if r'last_value.b /= r.b then n := n + 10; end if;
  end;
begin
  clk <= not clk after 5 ns when cnt < 60;
  sub : entity work.record_history_sub port map (clk => clk, ri => pi, ro => po);

  drive : process (clk)
    variable x : unsigned(17 downto 0) := to_unsigned(12345, 18);
  begin
    if rising_edge(clk) then
      cnt <= cnt + 1;
      x := x(16 downto 0) & (x(17) xor x(10));
      -- Whole records and field by field.
      q <= (std_logic_vector(x(7 downto 0)), x(3), x);
      p <= (std_logic_vector(x(7 downto 0)), x(3), x);
      if x(0) = '1' then
        g.a <= std_logic_vector(x(15 downto 8));
      end if;
      g.b <= x(5);
      if x(2) = '1' then
        g.c <= x;
        s.c <= x;
      end if;
      m(to_integer(x(1 downto 0))) <= (std_logic_vector(x(9 downto 2)), x(4), x);
      pm(to_integer(x(1 downto 0))) <= (std_logic_vector(x(9 downto 2)), x(4), x);
      pm(to_integer(x(3 downto 2))).b <= x(0);
      pi <= (std_logic_vector(x(11 downto 4)), x(6), x);
      if x(1) = '1' then
        w <= (std_logic_vector(x(7 downto 0)), x(8), x);
      end if;
    end if;
  end process;

  -- In the delta of each event of q: field and whole attributes, an edge of a field.
  on_q : process (q)
    variable n, t : natural := 0;
  begin
    if q.b'event then n := n + 1; end if;
    if q'event then n := n + 10; end if;
    if q.c'active then n := n + 100; end if;
    if rising_edge(q.b) then n := n + 1000; end if;
    t := t + in_ns(q.a'last_event) + in_ns(q'last_active);
    t := t + to_integer(q.c'last_value(7 downto 0)) + to_integer(unsigned(q'last_value.a));
    nq <= n + t;
  end process;

  -- An array of records: an element, a field of an element, the whole.
  on_m : process (m)
    variable n, t : natural := 0;
  begin
    if m(1)'event then n := n + 1; end if;
    if falling_edge(m(0).b) then n := n + 10; end if;
    if m(3).b'last_value = '1' then n := n + 100; end if;
    t := t + in_ns(m(2).c'last_event) + in_ns(m'last_event);
    nm <= n + t;
  end process;

  -- On the falling edge, a delta after the changes: the whole record's history only.
  on_g : process (clk)
    variable t : natural := 0;
    variable v : rec;
  begin
    if falling_edge(clk) then
      v := g'last_value;
      t := t + in_ns(g'last_event) + in_ns(g'last_active) + to_integer(v.c(5 downto 0));
      older(s.c, 25 ns, t);
      ng <= t;
    end if;
  end process;

  -- Nothing asks the history of p: a process waits for one field and reads the whole.
  on_p : process
    variable n : natural := 0;
  begin
    wait on p.b;
    n := n + 1 + to_integer(p.c(3 downto 0)) + to_integer(pm(2).c(2 downto 0));
    np <= n;
  end process;

  final : process
    variable e, o : natural := 0;
    alias ex is <<signal .record_history.sub.x : rec>>;
  begin
    while cnt < 60 loop
      wait until clk = '0';
      if ex.c'last_event < 12 ns then e := e + 1; end if;
      if ex'last_value.b /= ex.b then e := e + 100; end if;
      whole_age(w, o);
      o := o + to_integer(po.c(7 downto 0));
    end loop;
    report "n=" & integer'image(nq) & " m=" & integer'image(nm) & " g=" & integer'image(ng)
      & " p=" & integer'image(np) & " e=" & integer'image(e) & " o=" & integer'image(o);
    std.env.finish;
  end process;
end;
