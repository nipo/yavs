-- difftest: top=aggregate_target_records_cover std=2008 exit=1 ref=hand
-- The length of each element of a record value is checked against its target of an aggregate
-- target, before anything is driven (#1011, LRM 10.5.2.1). ref=hand: the first reference simulator
-- names the whole value's variable instead of the target element.
entity aggregate_target_records_cover is end;
architecture t of aggregate_target_records_cover is
  type ru is record x : bit_vector; end record;
  type pair is record p : ru; q : ru; end record;
  signal a, b : ru(x(1 to 2));
begin
  process
    variable v : pair(p(x(1 to 2)), q(x(1 to 2))) := (p => (x => "10"), q => (x => "01"));
    variable w : pair(p(x(1 to 2)), q(x(1 to 3))) := (p => (x => "11"), q => (x => "111"));
  begin
    (a, b) <= v;
    wait for 1 ns;
    report to_string(a.x) & " " & to_string(b.x);
    (a, b) <= w;
    wait for 1 ns;
    report "not reached";
    wait;
  end process;
end;
