-- yavs-test: std=2008
-- expect: E1410 18:6 an element of an array of arrays in an aggregate target is an aggregate, not a name
-- expect: E1410 19:6 the same for a signal target
-- help: assign the elements one by one
-- LRM 10.6.2.1, 10.5.2.1: "the expression in each element association of the aggregate shall be a
-- locally static name"; subaggregates of a matrix target are not element associations (9.3.3.3)
entity e is
end entity;
architecture a of e is
  type row is array (0 to 1) of integer;
  type mat is array (0 to 1) of row;
  type m2 is array (0 to 1, 0 to 1) of integer;
  signal s0, s1, s2, s3 : integer;
begin
  process
    variable p, q, r, s : integer;
  begin
    ((p, q), (r, s)) := mat'((1, 2), (3, 4));
    ((s0, s1), (s2, s3)) <= mat'((1, 2), (3, 4));
    ((p, q), (r, s)) := m2'((1, 2), (3, 4));
    wait;
  end process;
end architecture;
