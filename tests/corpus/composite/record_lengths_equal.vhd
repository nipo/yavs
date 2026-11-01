-- difftest: top=record_lengths_equal std=2008
-- Every assignment path of records with unconstrained fields, all lengths equal: values arrive
-- and the target keeps its own bounds (LRM 10.6.2.1, #1005).
library ieee; use ieee.std_logic_1164.all;
entity record_lengths_equal is end;
architecture a of record_lengths_equal is
  type rec_t is record x : bit_vector; end record;
  type two_t is record a : std_logic_vector; b : integer_vector; end record;
  type outer_t is record r : rec_t; n : natural; end record;
  type arr_t is array (natural range <>) of rec_t;
  type acc_t is access rec_t;
  subtype r3 is rec_t(x(1 to 3));
  function fn return rec_t is begin return (x => "011"); end;
  function len (r : r3) return integer is begin return r.x'left * 10 + r.x'length; end;
  procedure put (p : out rec_t) is variable l : rec_t(x(7 to 9)) := (x => "100"); begin p := l; end;
  procedure swap (p : inout rec_t) is begin p.x := not p.x; end;
  procedure dflt (p : r3 := (x => "111")) is begin report "dflt " & to_string(p.x); end;
  signal sr : rec_t(x(0 to 2));
begin
  process
    variable s : rec_t(x(3 downto 1));
    variable o : rec_t(x(1 to 3)) := (x => "110");
    variable t : two_t(a(1 to 2), b(0 to 1)) := (a => "01", b => (5, 6));
    variable u : two_t(a(2 downto 1), b(1 downto 0));
    variable n1 : outer_t(r(x(1 to 2)));
    variable n2 : outer_t(r(x(5 to 6))) := (r => (x => "10"), n => 4);
    variable a1 : arr_t(0 to 1)(x(1 to 2));
    variable a2 : arr_t(1 to 2)(x(0 to 1)) := (1 => (x => "01"), 2 => (x => "10"));
    variable ptr : acc_t;
    variable nl : rec_t(x(1 to 0));
    variable nr : rec_t(x(5 to 4));
  begin
    s := o;
    report "whole " & to_string(s.x) & " " & integer'image(s.x'left);
    u := t;
    report "two " & to_string(u.a) & " " & integer'image(u.b(1)) & " " & integer'image(u.a'left);
    n1 := n2;
    report "nested " & to_string(n1.r.x) & " " & integer'image(n1.n) & " " & integer'image(n1.r.x'left);
    n1.r := n2.r;
    report "field " & to_string(n1.r.x);
    a1 := a2;
    report "array " & to_string(a1(0).x) & " " & to_string(a1(1).x);
    a1(0 to 0) := a2(2 to 2);
    report "slice " & to_string(a1(0).x);
    a1(1) := n2.r;
    report "element " & to_string(a1(1).x);
    s := fn;
    report "fn " & to_string(s.x);
    s := o when s.x(1) = '1' else fn;
    report "cond " & to_string(s.x);
    put(s);
    report "out " & to_string(s.x) & " " & integer'image(s.x'left);
    swap(s);
    report "inout " & to_string(s.x);
    report "in " & integer'image(len(s));
    report "qual " & integer'image(len(r3'(s)));
    dflt;
    ptr := new r3'(s);
    report "new " & to_string(ptr.all.x);
    ptr.all := o;
    report "deref " & to_string(ptr.all.x);
    nl := nr;
    report "null " & integer'image(nl.x'length);
    sr <= o;
    wait for 1 ns;
    report "signal " & to_string(sr.x) & " " & integer'image(sr.x'left);
    wait;
  end process;
end;
