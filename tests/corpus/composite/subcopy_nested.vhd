-- difftest: top=subcopy_nested std=2008 exit=1 ignore=fatalmsg,path
-- ignore=fatalmsg,path: the references word the error and name its process differently.
-- Copies of subprograms for known descriptors around what they do not own: a nested function that
-- reads its parent's parameter and variables through the static link, a heap object made in the
-- declarative part and replaced by one of another shape, a parameter written in a loop that calls
-- out, a whole record assigned to an `inout` formal, and a length check that fails inside a copy.
package subcopy_nested_p is
  type mat is array (natural range <>, natural range <>) of integer;
  type bvs is array (natural range <>) of bit_vector;
  type rec is record
    key : bit_vector;
    row : bvs;
  end record;
  type mat_ptr is access mat;
  impure function outer(m : mat; k : integer) return integer;
  function weigh(m : mat) return integer;
  procedure swap(a : inout rec; b : in rec);
  procedure fold(m : inout mat; k : integer);
end package;

package body subcopy_nested_p is
  impure function outer(m : mat; k : integer) return integer is
    variable acc : integer := k;
    impure function cell(i, j : natural) return integer is
    begin
      acc := acc + 1;
      return m(i, j) * (m'length(2) + acc mod 3);
    end function;
  begin
    for i in m'range(1) loop
      for j in m'range(2) loop
        acc := acc + cell(i, j);
      end loop;
    end loop;
    return acc + m'high(1) * 10 + m'low(2);
  end function;

  function weigh(m : mat) return integer is
    variable s : integer := m'length(1) * 100 + m'length(2);
  begin
    for i in m'range(1) loop
      for j in m'range(2) loop
        s := (s * 7 + m(i, j)) mod 65521;
      end loop;
    end loop;
    return s;
  end function;

  -- `b` has the shape of `a` or not: the assignment checks it (LRM 10.6.2.1).
  procedure swap(a : inout rec; b : in rec) is
  begin
    a := b;
  end procedure;

  procedure fold(m : inout mat; k : integer) is
  begin
    for i in m'range(1) loop
      for j in m'range(2) loop
        m(i, j) := (m(i, j) + k + weigh(m)) mod 97;
      end loop;
    end loop;
  end procedure;
end package body;

use work.subcopy_nested_p.all;

entity subcopy_nested is end entity;

architecture a of subcopy_nested is
  constant cm : mat(1 to 2, 3 downto 1) := ((1, 2, 3), (4, 5, 6));
  constant good : rec(key(1 downto 0), row(0 to 1)(2 downto 0)) := (key => "10", row => ("110", "011"));
  constant bad : rec(key(2 downto 0), row(0 to 1)(2 downto 0)) := (key => "101", row => ("110", "011"));
  signal clk : bit := '0';
begin
  clk <= not clk after 5 ns;

  process (clk)
    variable p : mat_ptr := new mat(0 to 1, 0 to 2);
    variable vm : mat(0 to 2, 1 to 2) := ((1, 2), (3, 4), (5, 6));
    variable vr : rec(key(1 downto 0), row(0 to 1)(2 downto 0)) := (key => "00", row => ("000", "000"));
    variable n : natural := 0;
  begin
    if clk = '1' then
      n := n + 1;
      if n = 4 then
        deallocate(p);
        p := new mat(2 to 4, 0 to 0);
      end if;
      for i in p'range(1) loop
        for j in p'range(2) loop
          p(i, j) := n + i - j;
        end loop;
      end loop;
      fold(vm, n);
      swap(vr, good);
      report integer'image(n) & " " & integer'image(outer(cm, n)) & " " & integer'image(outer(p.all, n)) &
        " " & integer'image(weigh(p.all)) & " " & integer'image(weigh(vm)) & " " & integer'image(outer(vm, 1));
      if n = 7 then
        swap(vr, bad);
      end if;
    end if;
  end process;
end architecture;
