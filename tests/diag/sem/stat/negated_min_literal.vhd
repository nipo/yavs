-- yavs-test: std=2008
-- expect: E1402 9:24 the integer literal is too large
-- expect: E1402 10:25 the physical literal is too large
-- expect: E1402 12:25 the same in another unit
-- expect: E1402 13:23 a based literal in a type definition
-- LRM 9.5, 9.3.6, 5.2.3.1, 5.2.4.1: the literal is a value before the minus applies, and no 64-bit type holds 2**63 (L58)
package p is
  type big is range -9223372036854775807 - 1 to 9223372036854775807;
  constant a : big := -9223372036854775808;
  constant c : time := -9223372036854775808 fs;
  constant d : big := -9223372036854775807 - 1 + 1;
  constant e : time := -9223372036854775.808 ps;
  type big2 is range -16#8000_0000_0000_0000# to 0;
end package;
