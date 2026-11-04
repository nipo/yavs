-- yavs-test: std=2008
-- expect: E1406 6:21
-- E1406 (#1092): INTEGER squared, exactly 2**64 elements
package p is
  type m is array (integer, integer) of bit;
  constant c : m := (others => (others => '0'));
end package;
