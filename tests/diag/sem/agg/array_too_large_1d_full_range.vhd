-- yavs-test: std=2008
-- expect: E1406 7:21
-- E1406 (#1092): a one-dimensional array over the whole 64-bit range has 2**64 elements
package p is
  type big is range -9223372036854775807 - 1 to 9223372036854775807;
  type v is array (big) of bit;
  constant c : v := (others => '0');
end package;
