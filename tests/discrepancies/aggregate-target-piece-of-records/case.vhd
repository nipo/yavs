-- LRM 10.5.2.1, 10.6.2.1, 9.3.3.3: 2008 slice pieces of an array of records in an aggregate target.
entity top is end entity;
architecture a of top is
  type rc is record i : integer; end record;
  type rca is array (natural range <>) of rc;
  signal s : rca(0 to 3);
begin
  process
    variable v : rca(0 to 3) := ((i => 1), (i => 2), (i => 3), (i => 4));
    variable w : rca(0 to 3);
  begin
    (w(2 to 3), w(0 to 1)) := v;
    report "RES v " & integer'image(w(0).i) & integer'image(w(1).i) & integer'image(w(2).i) & integer'image(w(3).i);
    (s(2 to 3), s(0 to 1)) <= v;
    wait for 1 ns;
    report "RES s " & integer'image(s(0).i) & integer'image(s(1).i) & integer'image(s(2).i) & integer'image(s(3).i);
    wait;
  end process;
end architecture;
