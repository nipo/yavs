-- yavs-test: std=2008
-- expect: none
-- #1092: 2**32 * (2**32 - 1) elements fit 64 bits, and so do 2**64 - 1 in one dimension
package p is
  type big is range 0 to 4294967295;
  type less is range 1 to 4294967295;
  type m is array (big, less) of bit;
  constant c : m := (others => (others => '0'));
  type edge is range -9223372036854775807 to 9223372036854775807;
  type v is array (edge) of bit;
  constant e : v := (others => '0');
end package;
