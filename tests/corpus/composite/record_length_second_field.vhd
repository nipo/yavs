-- difftest: top=record_length_second_field std=2008 exit=1 ignore=path
-- ignore=path: sim1 also names the process of a length error outside a subprogram.
-- Records of equal total size: the second field's length differs (LRM 10.6.2.1, #1005).
entity record_length_second_field is end;
architecture a of record_length_second_field is
  type rec_t is record a : bit_vector; b : bit_vector; end record;
begin
  process
    variable s : rec_t(a(1 to 3), b(1 to 2));
    variable o : rec_t(a(1 to 2), b(1 to 3));
  begin
    s := o;
    report "not reached";
    wait;
  end process;
end;
