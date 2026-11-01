-- difftest: top=edge_alias ref=hand
-- ref=hand: the first reference simulator runs the process of `x'event and x = '0'` as true once at
-- time 0 (the second one and the LRM: no event at initialization); n(4) and n(17) are edited.
-- Every clock edge spelling read through an object alias (#1001): the same process fires, on the
-- same events, as with the signal itself. Each form counts its firings and sums their times, over
-- transitions that tell the forms apart ('H', 'L', 'U', 'X' on both sides of the edge).
library ieee; use ieee.std_logic_1164.all;

package edge_alias_pkg is
  signal pclk : std_ulogic := '0';
  alias palias : std_ulogic is pclk;
end package;

library ieee; use ieee.std_logic_1164.all;
use work.edge_alias_pkg.all;

entity edge_alias_port is
  generic (k : natural);
  port (clkin : in std_ulogic; vin : in std_ulogic_vector(7 downto 0);
        fired : out natural := 0);
end entity;

architecture a of edge_alias_port is
  alias pc : std_ulogic is clkin;
  alias pe : std_ulogic is vin(k);
  signal n1, n2 : natural := 0;
begin
  process (pc) begin if rising_edge(pc) then n1 <= n1 + 1; end if; end process;
  process (pe) begin if pe'event and pe = '1' then n2 <= n2 + 1; end if; end process;
  fired <= 100 * n1 + n2;
end architecture;

library ieee; use ieee.std_logic_1164.all;
use work.edge_alias_pkg.all;

entity edge_alias is
  generic (idx : natural := 2);
end entity;

architecture a of edge_alias is
  type mode_t is (m_a, m_b, m_c);
  type by_mode_t is array (mode_t) of std_ulogic;
  type rec_t is record
    f : std_ulogic;
    g : std_ulogic_vector(1 downto 0);
  end record;

  signal x : std_ulogic := '0';
  signal r : std_logic := '0'; -- resolved
  signal b : bit := '0';
  signal o : boolean := false;
  signal v : std_ulogic_vector(7 downto 0) := (others => '0');
  signal rc : rec_t := ('0', "00");
  signal bm : by_mode_t := (others => '0');
  signal rst, en : std_ulogic := '0';

  alias c : std_ulogic is x;                       -- subtype of the same type
  alias cl : std_logic is x;                       -- resolved subtype name of the same type
  alias cn is x;                                   -- no subtype
  alias c2 : std_ulogic is c;                      -- alias of an alias
  alias cr : std_ulogic is r;                      -- unresolved view of a resolved signal
  alias cb : bit is b;
  alias co : boolean is o;
  alias el : std_ulogic is v(idx);                 -- element, static through a generic
  alias sl : std_ulogic_vector(0 to 3) is v(7 downto 4); -- renumbered slice: sl(0) is v(7)
  alias sn is v(3 downto 0);                       -- slice keeping its bounds
  alias su : std_ulogic_vector is v(5 downto 2);   -- unconstrained subtype keeps the bounds
  alias wh : std_ulogic_vector(8 downto 1) is v;   -- whole, renumbered: wh(1) is v(0)
  alias ss : std_ulogic_vector(1 downto 0) is sl(1 to 2); -- alias of a slice of an alias
  alias fd : std_ulogic is rc.f;
  alias fv : std_ulogic is rc.g(1);
  alias me : std_ulogic is bm(m_b);
  alias ar : std_ulogic is rst;
  alias cx : x01 is x;                             -- a subtype that does not hold every value

  signal n : integer_vector(1 to 38) := (others => 0);
  signal q : integer_vector(1 to 2) := (others => 0);
  signal d, reg : std_ulogic := '0';
  alias ad : std_ulogic is d;
  alias ae : std_ulogic is en;
  signal p1, p2 : natural;
  signal done : boolean := false;
begin
  process (c) begin if rising_edge(c) then n(1) <= n(1) + 1; end if; end process;
  process (c) begin if falling_edge(c) then n(2) <= n(2) + 1; end if; end process;
  process (c) begin if c'event and c = '1' then n(3) <= n(3) + 1; end if; end process;
  process (c) begin if c = '0' and c'event then n(4) <= n(4) + 1; end if; end process;
  process (c) begin if not c'stable and c = '1' then n(5) <= n(5) + 1; end if; end process;
  process begin wait until rising_edge(c); n(6) <= n(6) + 1; end process;
  process begin wait until c = '1'; n(7) <= n(7) + 1; end process;
  process begin wait on c until c = '0'; n(8) <= n(8) + 1; end process;
  process begin wait until c'event and c = '1'; n(9) <= n(9) + 1; end process;
  -- The two names of one signal mixed.
  process (x) begin if c'event and x = '1' then n(10) <= n(10) + 1; end if; end process;
  process (c) begin if x'event and c = '0' then n(11) <= n(11) + 1; end if; end process;
  -- Other spellings of the alias.
  process (cl) begin if rising_edge(cl) then n(12) <= n(12) + 1; end if; end process;
  process (cn) begin if falling_edge(cn) then n(13) <= n(13) + 1; end if; end process;
  process (c2) begin if c2'event and c2 = '1' then n(14) <= n(14) + 1; end if; end process;
  process (cr) begin if rising_edge(cr) then n(15) <= n(15) + 1; end if; end process;
  process (cb) begin if rising_edge(cb) then n(16) <= n(16) + 1; end if; end process;
  process (cb) begin if cb'event and cb = '0' then n(17) <= n(17) + 1; end if; end process;
  process (co) begin if falling_edge(co) then n(18) <= n(18) + 1; end if; end process;
  process (co) begin if co'event and co then n(19) <= n(19) + 1; end if; end process;
  -- Elements: one wrong element here would count another bit's edges.
  process (el) begin if rising_edge(el) then n(20) <= n(20) + 1; end if; end process;
  process (sl) begin if rising_edge(sl(0)) then n(21) <= n(21) + 1; end if; end process;
  process (sn) begin if sn(1)'event and sn(1) = '1' then n(22) <= n(22) + 1; end if; end process;
  process (su) begin if falling_edge(su(3)) then n(23) <= n(23) + 1; end if; end process;
  process (wh) begin if rising_edge(wh(1)) then n(24) <= n(24) + 1; end if; end process;
  process begin wait until rising_edge(ss(1)); n(25) <= n(25) + 1; end process;
  process (fd) begin if rising_edge(fd) then n(26) <= n(26) + 1; end if; end process;
  process (fv) begin if not fv'stable and fv = '0' then n(27) <= n(27) + 1; end if; end process;
  process (me) begin if rising_edge(me) then n(28) <= n(28) + 1; end if; end process;
  -- Reset branch through an alias, and an alias declared in the process.
  process (c, ar) begin
    if ar = '1' then
      n(29) <= n(29) + 100;
    elsif rising_edge(c) then
      n(29) <= n(29) + 1;
    end if;
  end process;
  process (x)
    alias loc : std_ulogic is x;
  begin
    if rising_edge(loc) then n(30) <= n(30) + 1; end if;
  end process;
  -- A package alias, the edge with another operand, and a list that does not name the alias.
  process (palias) begin if rising_edge(palias) then n(31) <= n(31) + 1; end if; end process;
  process (c) begin if rising_edge(c) and en = '1' then n(32) <= n(32) + 1; end if; end process;
  process (en) begin if rising_edge(c) then n(33) <= n(33) + 1; end if; end process;
  process (cx) begin if rising_edge(cx) then n(35) <= n(35) + 1; end if; end process;
  process begin wait until rising_edge(<<signal .edge_alias.x : std_ulogic>>); n(36) <= n(36) + 1; end process;
  process begin wait until <<signal .edge_alias.x : std_ulogic>> = '1'; n(37) <= n(37) + 1; end process;
  -- The edge has the same event as a concurrent conditional assignment.
  q(1) <= q(1) + 1 when rising_edge(c);
  -- A register that reads through aliases under a level: it may sleep, never miss a value.
  process (c) begin
    if rising_edge(c) then
      if ae = '1' then reg <= ad; end if;
    end if;
  end process;
  process (reg) begin n(38) <= n(38) + 1; end process;
  -- An edge of the alias in a process with work after the test: not a trigger, same count.
  process (c) begin
    if rising_edge(c) then n(34) <= n(34) + 1; end if;
    if c = 'X' then q(2) <= q(2) + 1; end if;
  end process;

  u1 : entity work.edge_alias_port
    generic map (k => 1) port map (clkin => x, vin => v, fired => p1);
  u2 : entity work.edge_alias_port
    generic map (k => 3) port map (clkin => r, vin => v, fired => p2);

  process
    type seq_t is array (natural range <>) of std_ulogic;
    constant seq : seq_t :=
      ('1', '0', 'H', 'L', '1', 'X', '1', 'L', 'H', '0', 'U', '1', 'Z', '0', 'W', '1', '-', '0');
    variable sum : natural := 0;
  begin
    for i in seq'range loop
      wait for 5 ns;
      x <= seq(i);
      r <= seq(i);
      pclk <= seq((i + 1) mod seq'length);
      en <= '1' when i mod 4 < 2 else '0';
      d <= '1' when i mod 8 < 3 else '0';
      rst <= '1' when i = 8 else '0';
      b <= not b;
      o <= not o;
      rc.f <= seq((i + 2) mod seq'length);
      rc.g <= seq((i + 3) mod seq'length) & seq(i);
      bm(m_a) <= seq((i + 1) mod seq'length);
      bm(m_b) <= seq((i + 5) mod seq'length);
      bm(m_c) <= seq((i + 2) mod seq'length);
      -- A different walk for every bit of the vector.
      for k in v'range loop
        v(k) <= seq((i + 3 * k) mod seq'length);
      end loop;
    end loop;
    wait for 5 ns;
    for i in n'range loop
      report "n(" & integer'image(i) & ")=" & integer'image(n(i));
    end loop;
    report "cond=" & integer'image(q(1)) & " x=" & integer'image(q(2));
    report "ports=" & integer'image(p1) & "," & integer'image(p2);
    wait;
  end process;
end architecture;
