-- yavs-test: std=2008
-- expect: E1406 8:21
-- E1406 (#1092): the dimensions come from the constrained subtype of the context
package p is
  type big is range 0 to 4294967295;
  type m is array (big range <>, big range <>) of bit;
  subtype s is m(big, big);
  constant c : s := (others => (others => '0'));
end package;
