-- yavs-test: std=2019
-- expect: E1406 7:54
-- LRM 5.3.2.2 (#1092, fuzz sem 833598c9): in VHDL-2019 NATURAL has 2**63 values, so two rows of it are 2**64 elements
package p is
  type nv2 is array (natural range <>, integer range <>) of bit;
  constant ok1 : nv2(integer range 0 to 1, positive) := (others => (others => '0'));
  constant c : nv2(integer range 0 to 1, natural) := (others => (others => '0'));
end package;
