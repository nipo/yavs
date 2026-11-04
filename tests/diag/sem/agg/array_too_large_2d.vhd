-- yavs-test: std=2008
-- expect: E1406 8:21
-- help: narrow
-- E1406 (#1092): an array holds at most 2**64 - 1 elements; here 2**32 * 2**32
package p is
  type big is range 0 to 4294967295;
  type m is array (big, big) of bit;
  constant c : m := (others => (others => '0'));
end package;
