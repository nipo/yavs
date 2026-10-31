-- yavs-test: std=2008
-- expect: E1403 8:26 element value 'B' is outside subtype `lower`
-- LRM 9.3.3.3: an enumeration element is named by its literal, not by its position
package p is
  subtype lower is character range 'a' to 'z';
  type word is array (1 to 2) of lower;
  constant ok : word := ('a', 'b');
  constant bad : word := ('a', 'B');
end package;
