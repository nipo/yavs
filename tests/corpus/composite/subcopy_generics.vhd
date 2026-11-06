-- difftest: top=subcopy_generics std=2008 ignore=order
-- ignore=order: the instances report in the same cycles, in an order the LRM leaves open.
-- Descriptors the generics of an instance fix, passed to subprograms of a package and of the
-- architecture (which also read a generic): instances with the same and other generics, a
-- generate whose parameter sizes the objects, a signal, an access value, out and inout records.
-- Copies of the subprograms for those descriptors must give what the subprograms give.
package subcopy_generics_p is
  type mat is array (natural range <>, natural range <>) of integer;
  type bvs is array (natural range <>) of bit_vector;
  type rec is record
    key : bit_vector;
    row : bvs;
  end record;
  type mat_ptr is access mat;
  function weigh(m : mat) return integer;
  function pick(r : rec; i : natural) return integer;
  procedure stamp(r : inout rec; k : natural);
  procedure fill(m : out mat; k : integer);
end package;

package body subcopy_generics_p is
  function weigh(m : mat) return integer is
    variable s : integer := m'length(1) * 97 + m'length(2) * 13;
  begin
    for i in m'range(1) loop
      for j in m'reverse_range(2) loop
        s := (s * 3 + m(i, j) * (i + 1) + j) mod 100003;
      end loop;
    end loop;
    return s;
  end function;

  function pick(r : rec; i : natural) return integer is
    variable s : integer := r.key'length * 10 + r.row'length;
  begin
    for j in r.row(i)'range loop
      if r.row(i)(j) = r.key(r.key'low + (j mod r.key'length)) then
        s := s + j + 1;
      end if;
    end loop;
    return s;
  end function;

  procedure stamp(r : inout rec; k : natural) is
  begin
    r.row(k mod r.row'length) := not r.row(k mod r.row'length);
    r.key(r.key'high - (k mod r.key'length)) := '1';
  end procedure;

  procedure fill(m : out mat; k : integer) is
  begin
    for i in m'range(1) loop
      for j in m'range(2) loop
        m(i, j) := (k + i * 7 + j * 3) mod 11;
      end loop;
    end loop;
  end procedure;
end package body;

use work.subcopy_generics_p.all;

entity subcopy_generics_u is
  generic (w : positive; l : positive; tag : natural);
  port (clk : in bit);
end entity;

architecture a of subcopy_generics_u is
  constant cm : mat(0 to l - 1, 1 to w) := (others => (others => w));
  constant cr : rec(key(w - 1 downto 0), row(1 to l)(0 to w)) :=
    (key => (others => '0'), row => (others => (others => '1')));
  signal sm : mat(1 to l, 0 to w - 1) := (others => (others => l));
  -- Reads a generic as well as its argument's descriptor.
  function local_sum(m : mat) return integer is
    variable s : integer := w * 1000 + tag;
  begin
    for i in m'range(1) loop
      s := s + m(i, m'high(2)) + m'length(2);
    end loop;
    return s;
  end function;
begin
  process (clk)
    variable vm : mat(2 to l + 1, 0 to w - 1);
    variable vr : rec(key(0 to w - 1), row(0 to l)(w downto 1)) :=
      (key => (others => '0'), row => (others => (others => '0')));
    variable p : mat_ptr := new mat(0 to 1, 0 to w);
    variable n : natural := 0;
    variable s : integer := 0;
  begin
    if clk = '1' then
      n := n + 1;
      fill(vm, n);
      fill(p.all, n + tag);
      stamp(vr, n);
      sm <= vm;
      s := weigh(cm) + weigh(vm) + weigh(sm) + weigh(p.all) + local_sum(cm) + local_sum(vm);
      s := s + pick(cr, 1) + pick(vr, n mod (l + 1)) + pick(vr, 0);
      report integer'image(tag) & " " & integer'image(n) & " " & integer'image(s);
    end if;
  end process;
end architecture;

entity subcopy_generics is end entity;

architecture a of subcopy_generics is
  signal clk : bit := '0';
  signal done : boolean := false;
begin
  clk <= not clk after 5 ns when not done;
  done <= true after 100 ns;
  u1 : entity work.subcopy_generics_u generic map (3, 2, 1) port map (clk);
  u2 : entity work.subcopy_generics_u generic map (5, 1, 2) port map (clk);
  u3 : entity work.subcopy_generics_u generic map (3, 2, 3) port map (clk);
  g : for i in 1 to 3 generate
    ug : entity work.subcopy_generics_u generic map (i + 1, i, 10 + i) port map (clk);
  end generate;
end architecture;
