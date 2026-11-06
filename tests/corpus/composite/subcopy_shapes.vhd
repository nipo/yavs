-- difftest: top=subcopy_shapes std=2008
-- Subprograms on descriptors a caller knows (matrices, arrays of unconstrained arrays, records
-- with unconstrained fields, arrays of those) from package, architecture and process objects:
-- copies of the subprograms for those descriptors must give what the subprogram gives.
package subcopy_shapes_p is
  type mat is array (integer range <>, integer range <>) of integer;
  type bvs is array (natural range <>) of bit_vector;
  type rec is record
    tag : bit_vector;
    row : bvs;
    n   : integer;
  end record;
  type recs is array (natural range <>) of rec;
  constant pm : mat(3 downto 1, -1 to 0) := ((1, 2), (3, 4), (5, 6));
  function msum(m : mat) return integer;
  function mshape(m : mat) return integer;
  function bsum(x : bvs) return integer;
  function rsum(r : rec) return integer;
  function asum(x : recs) return integer;
  function adeep(x : recs; d : natural) return integer;
  procedure mfill(m : out mat; k : integer);
  procedure rbump(r : inout rec);
end package;

package body subcopy_shapes_p is
  function msum(m : mat) return integer is
    variable s : integer := 0;
  begin
    for i in m'range(1) loop
      for j in m'range(2) loop
        s := s + m(i, j) * (3 * i + j + 7);
      end loop;
    end loop;
    return s;
  end function;

  -- Every attribute of both dimensions: what a copy folds.
  function mshape(m : mat) return integer is
    variable s : integer := m'length(1) * 1000 + m'length(2) * 100;
  begin
    s := s + m'left(1) * 37 + m'right(1) * 41 + m'left(2) * 43 + m'right(2) * 47;
    s := s + m'low(1) * 3 + m'high(2) * 5;
    if m'ascending(1) then
      s := s + 1;
    end if;
    if m'ascending(2) then
      s := s + 2;
    end if;
    return s;
  end function;

  function bsum(x : bvs) return integer is
    variable s : integer := x'length * 100 + x'element'length * 10;
  begin
    for i in x'range loop
      for j in x(i)'range loop
        if x(i)(j) = '1' then
          s := s + (i + 1) * (j + 2);
        end if;
      end loop;
    end loop;
    if x'element'ascending then
      s := s + 5;
    end if;
    return s;
  end function;

  function rsum(r : rec) return integer is
  begin
    return r.tag'length * 1000 + r.tag'left * 50 + r.n + bsum(r.row);
  end function;

  -- An element of the array and a field of a record handed on: copies called from copies.
  function asum(x : recs) return integer is
    variable s : integer := x'length * 10000 + x'left * 7;
  begin
    for i in x'range loop
      s := s + rsum(x(i)) * (i + 1);
    end loop;
    return s + bsum(x(x'right).row);
  end function;

  -- Recursion on the same descriptor.
  function adeep(x : recs; d : natural) return integer is
  begin
    if d = 0 then
      return x(x'left).n;
    end if;
    return x'length + adeep(x, d - 1);
  end function;

  procedure mfill(m : out mat; k : integer) is
  begin
    for i in m'range(1) loop
      for j in m'range(2) loop
        m(i, j) := k * i - j;
      end loop;
    end loop;
  end procedure;

  procedure rbump(r : inout rec) is
  begin
    r.n := r.n + r.row'length;
    r.tag(r.tag'right) := not r.tag(r.tag'right);
    r.row(r.row'left)(r.row'element'left) := r.tag(r.tag'left);
  end procedure;
end package body;

use work.subcopy_shapes_p.all;

entity subcopy_shapes is end entity;

architecture a of subcopy_shapes is
  constant am : mat(0 to 1, 5 downto 2) := ((1, 2, 3, 4), (5, 6, 7, 8));
  constant nm : mat(0 to 2, 1 to 0) := (others => (others => 0));
  constant ab : bvs(1 to 3)(4 downto 0) := ("10101", "01100", "11111");
  constant ar : rec := (tag => "110", row => ("0110", "1001"), n => 9);
  constant aa : recs(2 to 3)(tag(0 to 1), row(0 to 2)(1 downto 0)) :=
    ((tag => "01", row => ("01", "10", "11"), n => 4), (tag => "11", row => ("00", "01", "10"), n => 6));
  signal clk : bit := '0';
  signal done : boolean := false;
begin
  clk <= not clk after 5 ns when not done;

  process (clk)
    variable vm : mat(1 to 3, 0 to 1) := ((1, 1), (2, 2), (3, 3));
    variable vr : rec(tag(3 downto 0), row(1 to 2)(0 to 2)) := (tag => "1000", row => ("001", "010"), n => 1);
    variable va : recs(0 to 2)(tag(1 downto 0), row(0 to 1)(2 downto 0)) :=
      (others => (tag => "10", row => ("100", "011"), n => 2));
    variable n : natural := 0;
    variable s : integer := 0;
  begin
    if clk = '1' then
      n := n + 1;
      mfill(vm, n);
      rbump(vr);
      va(n mod 3).n := n;
      va(n mod 3).tag := vr.tag(1 downto 0);
      s := msum(am) + msum(pm) + msum(vm) + mshape(am) + mshape(pm) + mshape(nm) + mshape(vm);
      s := s + bsum(ab) + rsum(ar) + rsum(vr) + asum(aa) + asum(va) + adeep(aa, n mod 4);
      report integer'image(n) & " " & integer'image(s) & " " & integer'image(msum(vm)) & " " &
        integer'image(rsum(vr)) & " " & integer'image(asum(va));
      assert mshape(nm) = 3 * 1000 + 0 * 37 + 2 * 41 + 1 * 43 + 0 * 47 + 0 * 3 + 0 * 5 + 1 + 2
        report "null dimension" severity failure;
      if n = 12 then
        done <= true;
      end if;
    end if;
  end process;
end architecture;
