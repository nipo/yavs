-- difftest: top=guard_activity std=2008 ignore=order ref=hand
-- ignore=order: reports of different processes and instances in one delta cycle.
-- ref=hand: a guard is evaluated again in every cycle in which a signal of its condition is
-- active (LRM 14.7.4), also for a transaction that keeps the value (`s <= s;`); one reference
-- simulator does so only on an event, the other rejects 'event in a guard. Nothing here reads
-- 'active, so unchanged assignments may be skipped elsewhere in the design (delta/guard_activity_attr).
entity guard_activity_sub is
  port (p : in bit);
end;

architecture a of guard_activity_sub is
begin
  b : block (p'event) is
  begin
    process (guard) begin report "port guard=" & boolean'image(guard); end process;
  end block;
end;

entity guard_activity_u is
  generic (g : natural);
end;

architecture a of guard_activity_u is
  type rec is record
    f : bit;
    n : integer;
  end record;
  function ev(signal a : bit) return boolean is
  begin
    return a'event;
  end;
  signal s : bit := '0';
  signal v : bit_vector(0 to 1) := "00";
  signal r : rec := ('0', 0);
  signal x, y : integer := 0;
begin
  -- Each held drive in a process that reads a generic, so compiled code compares it inline.
  stim : process
  begin
    if g > 1000 then report "g"; end if;
    wait for 1 ns; s <= '1'; v(0) <= '1'; r.f <= '1'; x <= 1;
    wait for 1 ns; s <= s; x <= 2;
    wait for 1 ns; v(1) <= v(1); r.n <= r.n;
    wait for 1 ns; s <= '0'; x <= 3;
    wait for 1 ns; x <= 4;
    wait;
  end process;
  -- FALSE again at `s <= s`; the guarded assignment does not take x = 2.
  be : block (s'event) is
  begin
    process (guard) begin report "be guard=" & boolean'image(guard); end process;
    y <= guarded x;
  end block;
  bf : block (ev(s)) is
  begin
    process (guard) begin report "bf guard=" & boolean'image(guard); end process;
  end block;
  bl : block (s'last_event > 0 ns) is
  begin
    process (guard) begin report "bl guard=" & boolean'image(guard); end process;
  end block;
  bq : block (s'quiet) is
  begin
    process (guard) begin report "bq guard=" & boolean'image(guard); end process;
  end block;
  -- One element or field active: the whole composite is, the other element is not.
  bv : block (v'event) is
  begin
    process (guard) begin report "bv guard=" & boolean'image(guard); end process;
  end block;
  bv0 : block (v(0)'event) is
  begin
    process (guard) begin report "bv0 guard=" & boolean'image(guard); end process;
  end block;
  br : block (r'event) is
  begin
    process (guard) begin report "br guard=" & boolean'image(guard); end process;
  end block;
  brf : block (r.f'event) is
  begin
    process (guard) begin report "brf guard=" & boolean'image(guard); end process;
  end block;
  u : entity work.guard_activity_sub port map (p => s);
  process (y) begin report "y=" & integer'image(y); end process;
end;

entity guard_activity is end;

architecture a of guard_activity is
begin
  u0 : entity work.guard_activity_u generic map (0);
  u1 : entity work.guard_activity_u generic map (1);
end;
