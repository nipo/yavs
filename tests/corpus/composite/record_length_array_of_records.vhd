-- difftest: top=record_length_array_of_records std=2008 exit=1 ignore=path
-- ignore=path: sim1 also names the process of a length error outside a subprogram.
-- An array of records whose element lengths differ (LRM 10.6.2.1, #1005).
entity record_length_array_of_records is end;
architecture a of record_length_array_of_records is
  type rec_t is record x : bit_vector; end record;
  type arr_t is array (natural range <>) of rec_t;
begin
  process
    variable s : arr_t(0 to 1)(x(1 to 3));
    variable o : arr_t(0 to 1)(x(1 to 4));
  begin
    s := o;
    report "not reached";
    wait;
  end process;
end;
