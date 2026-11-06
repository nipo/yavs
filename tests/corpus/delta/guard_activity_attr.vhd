-- difftest: top=guard_activity_attr std=2008 ref=hand
-- ref=hand: GUARD of `a'active` is evaluated again when `a` is active (LRM 14.7.4), at the first
-- transaction, which keeps the value, and stays TRUE from then on; one reference simulator
-- evaluates a guard again only on an event, the other rejects 'active in a guard.
entity guard_activity_attr is end;

architecture t of guard_activity_attr is
  signal a : bit := '0';
begin
  a <= a after 1 ns, a after 2 ns;
  b : block (a'active) is
  begin
    process (guard) begin report "guard=" & boolean'image(guard); end process;
  end block;
end;
