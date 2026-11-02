-- yavs-test: std=2019
-- expect: E1505 20:29 operator notation gives no association list to bind the anonymous type
-- expect: E1505 21:29 the same for a unary operator
-- help: call it in function notation: `"<"(r, r)`
-- LRM 2019 6.5.7.1 (LCS-2016-016): an anonymous type is bound through an association list;
-- function notation has one (line 22), operator notation does not. Other types keep the
-- predefined operator (line 23). User decision 2026-11-02 (rule 47 review).
package p is
  function "<" (a : type is private; b : a'subtype) return boolean;
  function "-" (a : type is private) return integer;
end package;
use work.p.all;
entity e is
end entity;
architecture a of e is
  type rec is record x : integer; end record;
begin
  process
    variable r : rec;
    variable b : boolean := r < r;
    variable n : integer := -r;
    variable c : boolean := "<"(r, r);
    variable d : boolean := 1 < 2 and -n < 0;
  begin
    wait;
  end process;
end architecture;
