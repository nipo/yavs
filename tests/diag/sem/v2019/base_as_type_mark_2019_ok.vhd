-- yavs-test: std=2019
-- expect: none
-- LRM 2008 16.2.2 bars T'BASE as a type mark (typ/base_as_type_mark.vhd); a 2019 file still
-- accepts it until LCS-2016-018 is checked (lrm-checks L177; the community 2019 suite's tb_018).
package p is
  type thing_t is (one, two);
  signal thing : thing_t'base;
  constant left : thing_t'base := thing_t'left;
end package;
