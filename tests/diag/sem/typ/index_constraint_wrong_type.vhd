-- yavs-test: std=2008
-- expect: E1300 16:20 enumeration type mark on a natural index
-- expect: E1300 17:20 enumeration subtype
-- expect: E1300 18:20 enumeration subtype indication with a range
-- expect: E1300 19:20 boolean
-- expect: E1300 20:30 second index of a two-dimensional array
-- expect: E1300 21:20 index of an array of arrays, with an element constraint
-- LRM 5.3.2.2 (#1086): the type of each discrete range is the type of its index
package p is
  type nv is array (natural range <>) of bit;
  type nv2 is array (natural range <>, integer range <>) of bit;
  type sv is array (natural range <>) of string;
  type e is (a, b, c);
  subtype se is e range a to b;
  constant ok1 : nv(natural) := (others => '0');
  constant c1 : nv(e) := (others => '0');
  constant c2 : nv(se) := (others => '0');
  constant c3 : nv(e range a to b) := (others => '0');
  constant c5 : nv(boolean) := (others => '0');
  constant c6 : nv2(natural, e) := (others => (others => '0'));
  constant c7 : sv(severity_level)(1 to 7) := (others => (others => 'x'));
  constant ok2 : nv2(integer range 0 to 1, natural) := (others => (others => '0'));
  constant ok3 : nv(integer range 0 to 1) := (others => '0');
  constant ok4 : nv(positive) := (others => '0');
end package;
