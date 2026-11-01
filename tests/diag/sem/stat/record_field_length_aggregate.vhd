-- yavs-test: std=2008
-- expect: E1405 10:10 the field has 3 elements, the static value 4
-- LRM 10.6.2.1: a static record value whose field has another length than the constrained target.
entity e is end;
architecture a of e is
  type ru is record x : bit_vector; end record;
begin process
    variable s : ru(x(1 to 3));
  begin
    s := (x => "1111");
    wait;
  end process;
end;
