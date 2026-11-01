-- difftest: top=aggregate_target_array_lengths std=2008 exit=1 ref=hand
-- An array of arrays as an element of an aggregate signal target: the element lengths of the
-- value are checked too (#1011, LRM 10.5.2.1). ref=hand: the first reference simulator runs it
-- with no check, the second fails with an access error of its own.
entity aggregate_target_array_lengths is end;
architecture t of aggregate_target_array_lengths is
  type iv is array (natural range <>) of integer_vector;
  type ip is record u, w : iv(0 to 1)(0 to 2); end record;
  type iq is record u, w : iv(0 to 1)(0 to 1); end record;
  signal c, d : iv(0 to 1)(0 to 1);
begin
  process
    variable ok : iq := (u => ((1, 2), (3, 4)), w => ((5, 6), (7, 8)));
    variable bad : ip := (u => ((1, 2, 3), (4, 5, 6)), w => ((1, 2, 3), (4, 5, 6)));
  begin
    (c, d) <= ok;
    wait for 1 ns;
    report integer'image(c(1)(1)) & " " & integer'image(d(0)(1));
    (c, d) <= bad;
    wait for 1 ns;
    report "not reached";
    wait;
  end process;
end;
