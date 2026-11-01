-- difftest: top=record_length_nested std=2008 exit=1 ignore=path
-- ignore=path: sim1 also names the process of a length error outside a subprogram.
-- A field of a nested record (LRM 10.6.2.1, #1005).
entity record_length_nested is end;
architecture a of record_length_nested is
  type inner_t is record x : bit_vector; end record;
  type outer_t is record r : inner_t; end record;
begin
  process
    variable s : outer_t(r(x(1 to 3)));
    variable o : outer_t(r(x(1 to 4)));
  begin
    s := o;
    report "not reached";
    wait;
  end process;
end;
