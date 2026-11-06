-- difftest: top=guard_activity_late std=2008 ignore=order ref=hand
-- ignore=order: reports of different processes and instances in one delta cycle.
-- ref=hand: a guard is evaluated again in every cycle in which a signal of its condition is
-- active (LRM 14.7.4): `a` of an instance elaborated after the blocks, through an external name
-- and a function, and `s` beside a part no evaluation reaches. Each takes a transaction that keeps
-- the value; one reference simulator evaluates a guard again only on an event, the other rejects
-- 'active in a guard.
entity guard_activity_late_src is
  generic (g : natural := 0);
end;

architecture a of guard_activity_late_src is
  signal a : bit := '0';
begin
  -- Held drives in a process that reads a generic, so compiled code compares them inline.
  stim : process
  begin
    if g > 1000 then report "g"; end if;
    wait for 1 ns; a <= a;
    wait for 1 ns; a <= a;
    wait;
  end process;
end;

entity guard_activity_late is
  generic (n : natural := 5);
end;

architecture t of guard_activity_late is
  function act(signal x : bit) return boolean is
  begin
    return x'active;
  end;
  signal s : bit := '0';
  signal v : bit_vector(0 to 3) := "0000";
begin
  stim : process
  begin
    if n > 1000 then report "n"; end if;
    wait for 4 ns; s <= s;
    -- An event on another element of `v`: `bo` reads no part of it and stays TRUE.
    wait for 1 ns; v(0) <= '1';
    wait;
  end process;
  be : block (<<signal .guard_activity_late.u.a : bit>>'active) is
  begin
    process (guard) begin report "be guard=" & boolean'image(guard); end process;
  end block;
  g : for i in 0 to 1 generate
    bf : block (act(<<signal .guard_activity_late.u.a : bit>>)) is
    begin
      process (guard) begin report "bf guard=" & boolean'image(guard); end process;
    end block;
  end generate;
  -- `v(n)` is never evaluated: n = 5 is outside its range.
  bo : block ((n < 4 and v(n)'event) or s'active) is
  begin
    process (guard) begin report "bo guard=" & boolean'image(guard); end process;
  end block;
  u : entity work.guard_activity_late_src;
end;
