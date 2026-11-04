-- yavs-test: std=2008
-- expect: E1406 7:21
-- E1406 (#1092): positional rows then `others`
package p is
  type big is range 0 to 4294967295;
  type m is array (big, big) of bit;
  constant c : m := ((others => '0'), others => (others => '1'));
end package;
