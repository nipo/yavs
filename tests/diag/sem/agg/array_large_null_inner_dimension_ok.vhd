-- yavs-test: std=2008
-- expect: none
-- #1092: rows of no elements add none, however many rows there are (the fold once looped over every row)
package p is
  type big is range 0 to 1099511627775;
  type m is array (big range <>, natural range <>) of bit;
  constant c : m(big, 1 to 0) := (others => (others => '0'));
  constant d : m(big, 1 to 0) := (0 => (others => '0'), others => (others => '1'));
end package;
