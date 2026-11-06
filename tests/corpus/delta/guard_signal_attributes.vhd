-- difftest: top=guard_signal_attributes std=2008 ignore=order
-- ignore=order: reports of different processes in one delta cycle (LRM 14.7.5.3 leaves their order open).
-- A guard condition may read any signal attribute (LRM 11.2). GUARD starts as its condition on the
-- values before simulation (14.7.5.2: 'event FALSE, 'last_event TIME'HIGH, 'stable TRUE, 16.2), with no
-- event, and keeps its value while no signal of the condition is active (14.7.4), so a guarded
-- assignment under `clk'event and clk = '1'` acts on the edge's GUARD (11.6).
library ieee;
use ieee.std_logic_1164.all;

entity guard_signal_attributes_sub is
  port (p : in std_ulogic; q : out boolean);
end;

architecture a of guard_signal_attributes_sub is
begin
  b : block (not p'event and p'last_value = '1') is
  begin
    q <= guard;
  end block;
end;

library ieee;
use ieee.std_logic_1164.all;

entity guard_signal_attributes is end;

architecture t of guard_signal_attributes is
  type rec is record
    f : bit;
    n : integer;
  end record;
  type int_vec is array (natural range <>) of integer;
  function sum(v : int_vec) return integer is
    variable r : integer := 0;
  begin
    for i in v'range loop
      r := r + v(i);
    end loop;
    return r;
  end;
  subtype rint is sum integer;

  signal clk : bit := '0';
  signal sl : std_ulogic := '1';
  signal still : bit := '0';
  signal r : rec := ('1', 3);
  signal x, y : integer := 0;
  signal q : boolean;
  signal reg : rint register := 0;
  signal bs : rint bus := 0;
  disconnect reg : rint after 2 ns;
  disconnect bs : rint after 1 ns;

  function ev(signal s : bit) return boolean is
  begin
    return s'event;
  end;
begin
  clk <= not clk after 5 ns when now < 30 ns;
  process (clk) begin
    if clk = '1' then
      x <= x + 1;
    end if;
  end process;
  sl <= '0' after 12 ns, '1' after 22 ns;

  -- The usual clocked guard: y follows x one edge behind, as the edge's GUARD stays TRUE.
  edge : block (clk'event and clk = '1') is
  begin
    y <= guarded x;
    reg <= guarded x * 10;
  end block;
  process (y) begin report "y=" & integer'image(y); end process;
  process (reg) begin report "reg=" & integer'image(reg); end process;

  -- Initial values of guards on attributes, each read at 0 ns and on every change.
  b1 : block (clk'event) is begin process (guard) begin report "event=" & boolean'image(guard); end process; end block;
  b2 : block (not clk'event) is begin process (guard) begin report "not_event=" & boolean'image(guard); end process; end block;
  b3 : block (clk'active) is begin process (guard) begin report "active=" & boolean'image(guard); end process; end block;
  -- On a signal that never changes: no evaluation after time 0.
  b4 : block (still'last_event = time'high) is begin process (guard) begin report "last_event=" & boolean'image(guard); end process; end block;
  b5 : block (still'last_value = '0') is begin process (guard) begin report "last_value=" & boolean'image(guard); end process; end block;
  b13 : block (still'last_active = time'high) is begin process (guard) begin report "last_active=" & boolean'image(guard); end process; end block;
  b6 : block (clk'stable) is begin process (guard) begin report "stable=" & boolean'image(guard); end process; end block;
  b7 : block (clk'quiet(2 ns)) is begin process (guard) begin report "quiet=" & boolean'image(guard); end process; end block;
  b8 : block (clk'delayed(1 ns) = '0') is begin process (guard) begin report "delayed=" & boolean'image(guard); end process; end block;
  b9 : block (not rising_edge(sl)) is begin process (guard) begin report "not_rising=" & boolean'image(guard); end process; end block;
  b10 : block (falling_edge(sl)) is begin process (guard) begin report "falling=" & boolean'image(guard); end process; end block;
  b11 : block (r.f'last_value = '1' and not r.n'event) is begin process (guard) begin report "record=" & boolean'image(guard); end process; end block;
  b12 : block (not ev(clk)) is begin process (guard) begin report "func=" & boolean'image(guard); end process; end block;
  b14 : block (<<signal .guard_signal_attributes.sl : std_ulogic>> = '1') is
  begin
    process (guard) begin report "external=" & boolean'image(guard); end process;
  end block;

  -- An inner guard reads the outer GUARD and an attribute.
  outer : block (not sl'event) is
  begin
    inner : block (guard and clk'stable) is
    begin
      process (guard) begin report "inner=" & boolean'image(guard); end process;
    end block;
  end block;

  -- A bus driven under an edge guard, and a guard that reads an attribute of that guarded signal.
  drv : block (clk'event and clk = '0') is
  begin
    bs <= guarded x + 100;
  end block;
  watch : block (bs'stable(500 ps)) is
  begin
    process (guard) begin report "bus_watch=" & boolean'image(guard); end process;
  end block;
  process (bs) begin report "bs=" & integer'image(bs); end process;

  u : entity work.guard_signal_attributes_sub port map (sl, q);
  process (q) begin report "port=" & boolean'image(q); end process;
end;
