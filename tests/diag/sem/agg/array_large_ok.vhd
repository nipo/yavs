-- yavs-test: std=2008
-- expect: none
-- #1092: a million times a million elements stays an accepted, folded constant
package p is
  type big is range 0 to 999999;
  type m is array (big, big) of bit;
  constant c : m := (others => (others => '0'));
end package;
