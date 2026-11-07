-- difftest: top=subcopy_edges std=2008 ignore=order
-- ignore=order: the processes of the generate report in the cycles of the main one.
-- Edges of copies of subprograms for known descriptors: null dimensions and arrays, a null field,
-- the same matrix data under other shapes (pure calls), recursion on slices, a generate's
-- parameter, a heap object of a new shape at every call, the same record as `in` and `inout`, a
-- signal parameter, partly constrained formals, two instances of a generic package, textio and
-- reports inside the subprograms.
use std.textio.all;

package subcopy_edges_p is
  type mat is array (natural range <>, natural range <>) of integer;
  type bvs is array (natural range <>) of bit_vector;
  type rec is record
    key : bit_vector;
    row : bvs;
  end record;
  type recs is array (natural range <>) of rec;
  type rec_ptr is access rec;
  pure function msum(m : mat) return integer;
  function rlen(r : rec) return integer;
  function rdeep(x : recs) return integer;
  function head(x : recs) return integer;
  procedure mirror(a : in rec; b : inout rec);
  function sig_len(signal s : rec) return integer;
  function fixed_rows(x : bvs(open)(3 downto 0)) return integer;
  procedure show(m : mat; tag : string);
end package;

package body subcopy_edges_p is
  pure function msum(m : mat) return integer is
    variable s : integer := m'length(1) * 1000 + m'length(2) * 100 + m'left(1) * 10 + m'left(2);
  begin
    for i in m'range(1) loop
      for j in m'range(2) loop
        s := s + m(i, j) * (i - m'left(1) + 1) * (j + 3);
      end loop;
    end loop;
    return s;
  end function;

  function rlen(r : rec) return integer is
    variable s : integer := r.key'length * 100 + r.row'length * 10 + r.row'element'length;
  begin
    for i in r.row'range loop
      for j in r.row(i)'range loop
        if r.row(i)(j) = '1' then
          s := s + i + j;
        end if;
      end loop;
    end loop;
    return s;
  end function;

  -- A shorter slice at every level: another descriptor each time.
  function rdeep(x : recs) return integer is
  begin
    if x'length = 0 then
      return 0;
    end if;
    return rlen(x(x'left)) + 3 * rdeep(x(x'left + 1 to x'right));
  end function;

  function head(x : recs) return integer is
    variable s : integer := x'length;
  begin
    for i in x'range loop
      s := s + rlen(x(i));
    end loop;
    return s;
  end function;

  -- `a` and `b` may be the same object.
  procedure mirror(a : in rec; b : inout rec) is
    variable k : bit_vector(a.key'range) := a.key;
  begin
    for i in b.row'range loop
      b.row(i) := not b.row(i);
    end loop;
    b.key := k(k'left) & b.key(b.key'left downto b.key'right + 1);
  end procedure;

  function sig_len(signal s : rec) return integer is
  begin
    return rlen(s) + s.key'length;
  end function;

  function fixed_rows(x : bvs(open)(3 downto 0)) return integer is
    variable s : integer := x'length * 10;
  begin
    for i in x'range loop
      if x(i)(x(i)'left) = '1' then
        s := s + i;
      end if;
    end loop;
    return s;
  end function;

  procedure show(m : mat; tag : string) is
    variable l : line;
  begin
    write(l, tag & " " & integer'image(m'length(1)) & "x" & integer'image(m'length(2)));
    for i in m'range(1) loop
      for j in m'range(2) loop
        write(l, " " & integer'image(m(i, j)));
      end loop;
    end loop;
    writeline(output, l);
    if m'length(1) = 0 then
      report tag & " has no rows";
    end if;
  end procedure;
end package body;

package subcopy_edges_gen is
  generic (type elem; function weight(e : elem) return integer);
  type grid is array (natural range <>, natural range <>) of elem;
  function total(g : grid) return integer;
end package;

package body subcopy_edges_gen is
  function total(g : grid) return integer is
    variable s : integer := g'length(1) * 7 + g'length(2);
  begin
    for i in g'range(1) loop
      for j in g'range(2) loop
        s := s + weight(g(i, j)) * (i + 1);
      end loop;
    end loop;
    return s;
  end function;
end package body;

package subcopy_edges_util is
  function bit_weight(b : bit) return integer;
  function int_weight(i : integer) return integer;
end package;

package body subcopy_edges_util is
  function bit_weight(b : bit) return integer is
  begin
    return bit'pos(b) * 5;
  end function;
  function int_weight(i : integer) return integer is
  begin
    return i * i;
  end function;
end package body;

use work.subcopy_edges_util.all;
package subcopy_edges_gb is new work.subcopy_edges_gen generic map (elem => bit, weight => bit_weight);
use work.subcopy_edges_util.all;
package subcopy_edges_gi is new work.subcopy_edges_gen generic map (elem => integer, weight => int_weight);

use std.textio.all;
use work.subcopy_edges_p.all;

entity subcopy_edges is end entity;

architecture a of subcopy_edges is
  -- One data, three shapes: a pure call memoised for one shape must not answer for another.
  constant m23 : mat(0 to 1, 0 to 2) := ((1, 2, 3), (4, 5, 6));
  constant m32 : mat(0 to 2, 0 to 1) := ((1, 2), (3, 4), (5, 6));
  constant m23b : mat(5 to 6, 1 to 3) := ((1, 2, 3), (4, 5, 6));
  constant m0x : mat(1 to 0, 0 to 3) := (others => (others => 0));
  constant cr : recs(0 to 3)(key(1 downto 0), row(0 to 1)(2 downto 0)) :=
    ((key => "01", row => ("001", "010")), (key => "10", row => ("011", "100")),
     (key => "11", row => ("101", "110")), (key => "00", row => ("111", "000")));
  constant none : recs(1 to 0)(key(1 downto 0), row(0 to 1)(2 downto 0)) := (others => (key => "00", row => (others => "000")));
  constant nullkey : rec(key(1 to 0), row(0 to 2)(1 downto 0)) := (key => "", row => ("01", "10", "11"));
  constant fr : bvs(2 to 4)(3 downto 0) := ("1000", "0111", "1111");
  constant gb : work.subcopy_edges_gb.grid(0 to 1, 0 to 3) := ("1010", "0110");
  constant gi : work.subcopy_edges_gi.grid(0 to 2, 2 to 3) := ((1, 2), (3, 4), (5, 6));
  signal sr : rec(key(3 downto 0), row(1 to 2)(0 to 1)) := (key => "1001", row => ("10", "01"));
  signal clk : bit := '0';
  signal done : boolean := false;
begin
  clk <= not clk after 5 ns when not done;

  process (clk)
    variable n : natural := 0;
    variable s : integer := 0;
    variable v : rec(key(2 downto 0), row(0 to 1)(1 downto 0)) := (key => "100", row => ("01", "11"));
    variable p : rec_ptr;
  begin
    if clk = '1' then
      n := n + 1;
      s := msum(m23) + 3 * msum(m32) + 5 * msum(m23b) + 7 * msum(m0x);
      s := s + rdeep(cr) + head(cr) + head(none) + rlen(nullkey) + fixed_rows(fr);
      s := s + work.subcopy_edges_gb.total(gb) + work.subcopy_edges_gi.total(gi);
      mirror(v, v);
      s := s + rlen(v) + sig_len(sr);
      -- A heap object of another shape at every call.
      p := new rec'(key => (n mod 4 downto 0 => '1'), row => (0 to n mod 3 => (n mod 2 downto 0 => '1')));
      s := s + rlen(p.all);
      deallocate(p);
      if n mod 4 = 1 then
        show(m23, "m23");
        show(m0x, "m0x");
      end if;
      sr.key <= sr.key(0) & sr.key(3 downto 1);
      report integer'image(n) & " " & integer'image(s);
      if n = 9 then
        done <= true;
      end if;
    end if;
  end process;

  -- Objects the generate parameter sizes.
  g : for i in 1 to 3 generate
    constant gm : mat(0 to i, 1 to i + 1) := (others => (others => i));
    constant gr : recs(0 to i - 1)(key(i downto 0), row(0 to 1)(i - 1 downto 0)) :=
      (others => (key => (others => '1'), row => (others => (others => '0'))));
  begin
    process (clk)
      variable n : natural := 0;
    begin
      if clk = '1' then
        n := n + 1;
        if n mod 3 = 0 then
          report "g" & integer'image(i) & " " & integer'image(msum(gm) + head(gr) + rdeep(gr));
        end if;
      end if;
    end process;
  end generate;
end architecture;
