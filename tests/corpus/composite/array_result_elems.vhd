-- difftest: top=array_result_elems std=2008
-- Array results whose elements are composites (vectors, records) of a type with one level of
-- bounds: a signal or a variable as the argument, nested calls, the result in a variable, in
-- `=` and `&`, as a procedure actual, in a constant at elaboration, from a protected method, the
-- memo of pure calls, recursion and a result of other bounds than the target's (a conversion).
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

package array_result_elems_pkg is
  type t_arr is array (natural range <>) of signed(15 downto 0);
  type pt is record x : integer; y : std_ulogic_vector(3 downto 0); end record;
  type pt_vec is array (integer range <>) of pt;
  function reduce(src : t_arr) return t_arr;
  function seq(n, seed : integer) return t_arr;
  function sum_all(v : t_arr) return signed;
  function flip(v : pt_vec) return pt_vec;
  function halves(v : t_arr) return t_arr;
  type acc is protected
    procedure add(v : t_arr);
    impure function get return t_arr;
  end protected;
end package;

package body array_result_elems_pkg is
  function reduce(src : t_arr) return t_arr is
    variable r : t_arr(0 to (src'length + 1) / 2 - 1);
  begin
    for k in r'range loop
      if 2 * k + 1 <= src'length - 1 then
        r(k) := src(src'low + 2 * k) + src(src'low + 2 * k + 1);
      else
        r(k) := src(src'low + 2 * k);
      end if;
    end loop;
    return r;
  end function;

  function seq(n, seed : integer) return t_arr is
    variable r : t_arr(1 to n);
  begin
    for k in r'range loop
      r(k) := to_signed(seed * k - 7, 16);
    end loop;
    return r;
  end function;

  function sum_all(v : t_arr) return signed is
    variable s : signed(15 downto 0) := (others => '0');
  begin
    for k in v'range loop
      s := s + v(k);
    end loop;
    return s;
  end function;

  function flip(v : pt_vec) return pt_vec is
    variable r : pt_vec(v'high downto v'low);
  begin
    for k in v'range loop
      r(k) := (x => v(k).x * 2, y => not v(k).y);
    end loop;
    return r;
  end function;

  -- Recursion: a result built from the results of calls of itself.
  function halves(v : t_arr) return t_arr is
  begin
    if v'length <= 1 then
      return v;
    end if;
    return halves(reduce(v));
  end function;

  type acc is protected body
    variable st : t_arr(0 to 3) := (others => (others => '0'));
    procedure add(v : t_arr) is
    begin
      for k in st'range loop
        st(k) := st(k) + v(v'low + k);
      end loop;
    end procedure;
    impure function get return t_arr is
    begin
      return st;
    end function;
  end protected body;
end package body;

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.array_result_elems_pkg.all;

entity array_result_elems is end;

architecture t of array_result_elems is
  constant C : t_arr := reduce(seq(9, 5));
  signal t  : t_arr(0 to 12) := (others => (others => '0'));
  signal l1 : t_arr(0 to 6)  := (others => (others => '0'));
  signal l2 : t_arr(0 to 3)  := (others => (others => '0'));
  signal l3 : t_arr(0 to 1)  := (others => (others => '0'));
  signal lw : t_arr(10 to 13) := (others => (others => '0'));
  signal pv : pt_vec(1 to 3) := (others => (0, "0000"));
  shared variable a : acc;

  procedure show(name : string; v : t_arr) is
  begin
    report name & " " & integer'image(v'left) & " " & integer'image(v'right) & " "
      & to_hstring(sum_all(v)) & " " & to_hstring(v(v'left)) & " " & to_hstring(v(v'right));
  end procedure;
begin
  process is
    variable v  : t_arr(0 to 3);
    variable w  : t_arr(0 to 1);
    variable p  : pt_vec(1 to 3) := ((1, "0001"), (2, "0010"), (3, "0100"));
    variable q  : pt_vec(1 to 3);
  begin
    show("C", C);
    for c in 1 to 4 loop
      for k in t'range loop
        t(k) <= t(k) + to_signed(3 * k + c, 16);
      end loop;
      l1 <= reduce(t);
      l2 <= reduce(l1);
      l3 <= reduce(reduce(l1));
      lw <= reduce(l1);
      wait for 1 ns;
      show("l1", l1);
      show("l2", l2);
      show("l3", l3);
      show("lw", lw);
      v := reduce(t(0 to 7));
      show("v", v);
      w := reduce(reduce(t(0 to 7)));
      show("w", w);
      show("halves", halves(t));
      show("cat", reduce(t(0 to 3)) & reduce(l1));
      report "eq " & boolean'image(reduce(l1) = l2) & " " & boolean'image(reduce(l2) = l3);
      a.add(reduce(t));
      show("acc", a.get);
      show("memo", reduce(seq(5, c)));
      show("memo", reduce(seq(5, c)));
    end loop;
    q := flip(p);
    for k in q'range loop
      report "flip " & integer'image(k) & " " & integer'image(q(k).x) & " " & to_hstring(q(k).y);
    end loop;
    pv <= flip(p);
    wait for 1 ns;
    report "pv " & integer'image(pv(1).x) & " " & to_hstring(pv(3).y);
    wait;
  end process;
end;
