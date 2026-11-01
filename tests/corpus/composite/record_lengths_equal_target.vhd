-- difftest: top=record_lengths_equal_target std=2008
-- An aggregate target takes the fields of a record value of the same lengths (LRM 10.6.2.1, #1005).
entity record_lengths_equal_target is end;
architecture a of record_lengths_equal_target is
  type rec_t is record x : bit_vector; end record;
  type pair_t is record p : rec_t; q : rec_t; end record;
begin
  process
    variable pr : pair_t(p(x(1 to 2)), q(x(1 to 2))) := (p => (x => "11"), q => (x => "00"));
    variable c, d : rec_t(x(2 downto 1));
  begin
    (c, d) := pr;
    report "target " & to_string(c.x) & " " & to_string(d.x) & " " & integer'image(c.x'left);
    wait;
  end process;
end;
