-- difftest: top=lean_array_params std=2008
-- Calls of subprograms with array parameters whose bounds are read only through the descriptor
-- (the length, the bounds, the direction, a slice, an index), parameters that are never read,
-- a parameter returned whole or passed on to another call, to a report and to textio, 1-D strings
-- ascending, descending and null, heap strings, a 2-D array parameter, a callee that allocates
-- and frees while the argument is in use, and a protected method with a string parameter.
use std.textio.all;

entity lean_array_params is end entity;

architecture a of lean_array_params is
  type matrix is array (natural range <>, natural range <>) of integer;
  type int_ptr is access integer;

  -- The position of the first `target` in `vector`, counted from 1, through an alias.
  function pos_first(target : character; vector : string; not_found : natural := 0) return natural is
    alias v : string(1 to vector'length) is vector;
  begin
    assert vector'length > 0 report "empty string" severity failure;
    for i in v'left to v'right loop
      if v(i) = target then
        return i;
      end if;
    end loop;
    return not_found;
  end function;

  -- Only the bounds of `s` are read, and `unused` not at all.
  function shape(s : string; unused : integer; k : integer) return integer is
    variable r : integer := k;
  begin
    for i in 1 to 3 loop
      if s'ascending then
        r := r + s'length * i + s'left;
      else
        r := r - s'length * i - s'right;
      end if;
    end loop;
    return r;
  end function;

  -- Either the parameter itself or a new string built from a slice of it.
  function trim_first(msg : string) return string is
  begin
    if msg'length > 1 and msg(msg'left) = ' ' then
      if msg'ascending then
        return msg(msg'left + 1 to msg'right);
      else
        return msg(msg'left - 1 downto msg'right);
      end if;
    end if;
    return msg;
  end function;

  function slen(s : string) return natural is
  begin
    return s'length;
  end function;

  function sleft(s : string) return integer is
  begin
    return s'left;
  end function;

  -- Passes its parameter on: to another function, to a report and to a line.
  function describe(msg : string; tag : integer) return string is
    variable l : line;
    variable n : natural;
  begin
    n := pos_first(':', msg);
    write(l, msg);
    write(l, string'(" #"));
    write(l, tag);
    write(l, string'(" at "));
    write(l, n);
    if tag mod 50 = 0 then
      report "describe " & msg & " " & integer'image(msg'length);
    end if;
    return l.all;
  end function;

  -- Allocates and frees on every element while `s` is in use.
  function churn(s : string) return integer is
    variable p : int_ptr;
    variable sum : integer := 0;
  begin
    for i in s'range loop
      p := new integer'(character'pos(s(i)));
      sum := sum + p.all;
      deallocate(p);
    end loop;
    return sum + s'length;
  end function;

  function corner(m : matrix) return integer is
  begin
    return m(m'left(1), m'left(2)) + m(m'right(1), m'right(2)) * 10 + m'length(1) * 100 + m'length(2) * 1000;
  end function;

  -- An access parameter that is never read, and `s` only through its bounds.
  procedure bounds(s : in string; variable p : inout int_ptr; lo, hi : out integer) is
  begin
    lo := s'low;
    hi := s'high;
  end procedure;

  procedure measure(s : in string; n : out integer; lo : out integer) is
  begin
    n := 0;
    for i in s'range loop
      if s(i) /= ' ' then
        n := n + 1;
      end if;
    end loop;
    lo := s'low;
  end procedure;

  type counter is protected
    procedure add(s : string; unused : integer);
    impure function total return integer;
  end protected;

  type counter is protected body
    variable t : integer := 0;
    procedure add(s : string; unused : integer) is
    begin
      for k in 1 to 2 loop
        t := t + s'length * k;
      end loop;
    end procedure;
    impure function total return integer is
    begin
      return t;
    end function;
  end protected body;

  shared variable cnt : counter;
begin
  process
    variable up : string(1 to 12) := "hello: world";
    variable down : string(12 downto 1) := "dlrow :olleh";
    variable off : string(5 to 9) := " abcd";
    variable l : line;
    variable acc, n, lo : integer := 0;
    variable m : matrix(2 to 3, 0 to 2) := ((1, 2, 3), (4, 5, 6));
    variable p : int_ptr := new integer'(7);
  begin
    l := new string'(" from the heap: yes");
    for k in 1 to 400 loop
      -- New contents every round: the memo of pure calls does not answer these calls.
      up(3) := character'val(97 + k mod 26);
      down(4) := character'val(65 + k mod 26);
      off(7) := character'val(48 + k mod 10);
      l.all(4) := character'val(97 + k mod 7);
      acc := acc + pos_first(':', up) + pos_first(':', down) + pos_first('z', off, 99);
      acc := acc + shape(up, k, k) + shape(down, -k, 1) + shape(l.all, 0, 2);
      acc := acc + slen(trim_first(up)) + slen(trim_first(down)) + slen(trim_first(off));
      acc := acc + slen(trim_first(l.all)) + sleft(trim_first(l.all));
      acc := acc + churn(off) + churn(l.all(2 to 5));
      acc := acc + corner(m) + slen(describe(up, k));
      measure(down, n, lo);
      acc := acc + n + lo;
      bounds(off, p, lo, n);
      acc := acc + lo * n;
      cnt.add(off, k);
      cnt.add(l.all, 0);
      acc := acc mod 1000003;
    end loop;
    report "trim " & trim_first(off) & "|" & trim_first(down) & "|" & trim_first(l.all) & "|" & trim_first("x");
    report "describe " & describe(down, 7);
    report "pos " & integer'image(pos_first(':', l.all)) & " " & integer'image(pos_first('q', "q"));
    report "shape " & integer'image(shape(off, 0, 0)) & " " & integer'image(shape("", 0, 3));
    report "acc " & integer'image(acc) & " total " & integer'image(cnt.total);
    assert acc = acc report "unreachable" severity failure;
    wait;
  end process;
end architecture;
