-- yavs-test: std=2019
-- expect: none
-- LRM 2019 6.5.2, 6.5.7.1: user operators with anonymous parameter types, called in function
-- notation; in operator notation the predefined operators of other types are used (`i + i`),
-- and the user operators are E1505 (anonymous_operator_notation.vhd, user 2026-11-02)
package p is
  function "<" (a : type is private; b : a'subtype) return boolean;
  function "-" (a : type is private) return boolean;
  function "+" (a : type is range <>; b : a'subtype) return a'subtype;
  function "&" (a : type is (<>); b : a'subtype) return a'subtype;
end package;
package body p is
  function "<" (a : type is private; b : a'subtype) return boolean is begin return true; end function;
  function "-" (a : type is private) return boolean is begin return false; end function;
  function "+" (a : type is range <>; b : a'subtype) return a'subtype is begin return a - b; end function;
  function "&" (a : type is (<>); b : a'subtype) return a'subtype is begin return b; end function;
end package body;
use work.p.all;
entity top is end;
architecture a of top is
  type rec is record x : integer; end record;
  type color is (red, green);
begin
  process
    variable r : rec;
    variable b : boolean;
    variable i : integer;
    variable c : color;
  begin
    b := "-"(r);
    b := "<"(r, r);
    i := i + i;
    i := 1 + 2;
    c := "&"(red, green);
    wait;
  end process;
end architecture;
