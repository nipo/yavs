-- yavs-test: std=2008
-- expect: E1406 7:21
-- E1406 (#1092): no two dimensions overflow, all three do
package p is
  type d is range 0 to 4194303;
  type m is array (d, d, d) of bit;
  constant c : m := (others => (others => (others => '0')));
end package;
