-- difftest: top=record_length_var std=2008 exit=1 ignore=path
-- ignore=path: sim1 also names the process of a length error outside a subprogram.
-- A record assigned to a variable whose unconstrained field has another length is a length
-- error (LRM 10.6.2.1, #1005); equal lengths with other bounds are fine.
entity record_length_var is end;
architecture a of record_length_var is
  type rec_t is record x : bit_vector; end record;
begin
  p : process
    variable s : rec_t(x(3 downto 1));
    variable eq : rec_t(x(1 to 3)) := (x => "110");
    variable o : rec_t(x(1 to 4)) := (x => "1111");
  begin
    s := eq;
    report to_string(s.x) & " " & integer'image(s.x'left);
    s := o;
    report "not reached: " & to_string(s.x);
    wait;
  end process;
end;
