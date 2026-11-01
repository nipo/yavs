-- difftest: top=record_length_out_param std=2008 exit=1 ignore=path
-- ignore=path: sim1 also names the process of a length error outside a subprogram.
-- An unconstrained record formal has its actual's bounds (LRM 6.5.7.1), so a value of another
-- length is an error inside the procedure (#1005; catalogue record-out-param-length).
entity record_length_out_param is end;
architecture a of record_length_out_param is
  type rec_t is record x : bit_vector; end record;
  procedure put (p : out rec_t) is
    variable o : rec_t(x(1 to 4));
  begin
    p := o;
  end;
begin
  process
    variable s : rec_t(x(1 to 3));
  begin
    put(s);
    report "not reached";
    wait;
  end process;
end;
