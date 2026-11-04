-- yavs-test: std=2008
-- expect: none
-- #1092: an array with a null dimension has no elements however large the others are
package p is
  type big is range -9223372036854775807 - 1 to 9223372036854775807;
  type m is array (natural range <>, big range <>) of bit;
  constant c : m(1 to 0, big) := (others => (others => '0'));
end package;
