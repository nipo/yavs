-- yavs-test: std=2008
-- expect: E1406 7:21
-- E1406 (#1092): named rows take the segment path of the folding
package p is
  type big is range 0 to 4294967295;
  type m is array (big, big) of bit;
  constant c : m := (0 => (others => '0'), others => (others => '1'));
end package;
