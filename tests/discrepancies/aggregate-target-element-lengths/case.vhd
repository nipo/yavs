-- LRM 10.5.2.1: an array of arrays as an element of an aggregate signal target: its element lengths are checked.
entity top is end entity;
architecture a of top is
  type iv is array (natural range <>) of integer_vector;
  type ip is record u, w : iv(0 to 1)(0 to 2); end record;
  signal c, d : iv(0 to 1)(0 to 1);
begin
  process
    variable v : ip := (u => ((1, 2, 3), (4, 5, 6)), w => ((1, 2, 3), (4, 5, 6)));
  begin
    report "RES before";
    (c, d) <= v;
    wait for 1 ns;
    report "RES not reached";
    wait;
  end process;
end architecture;
