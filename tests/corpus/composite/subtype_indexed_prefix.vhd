-- difftest: top=subtype_indexed_prefix std=2008
-- 'subtype, 'element and 'range of an object name whose prefix has an indexed name before a
-- selection (a(0).v'subtype, #1067), in declarations, parameters, conversions and ranges.
package sip_pk is
  type r_t is record v : bit_vector(3 downto 0); w : integer; end record;
  type a_t is array (0 to 1) of r_t;
  type aa_t is array (0 to 1) of bit_vector(2 downto 0);
  type m_t is array (0 to 1, 0 to 2) of bit_vector(1 to 2);
  type rr_t is record a : a_t; end record;
  type ra_t is array (0 to 1) of rr_t;
  constant c : a_t := (others => (v => "1010", w => 3));
end package;

use work.sip_pk.all;
entity subtype_indexed_prefix is end;
architecture t of subtype_indexed_prefix is
  signal a : a_t := c;
  signal ra : ra_t;
  signal aa : aa_t;
  signal m : m_t;
  signal s : a(0).v'subtype;
  constant k : c(1).v'subtype := "0101";
  subtype st is ra(0).a(1).v'subtype;
  type t2 is array (0 to 1) of a(0).v'subtype;
  type r2 is record f : a(0).v'subtype; end record;
  alias al : a(0).v'subtype is a(0).v;
  function len(x : a(0).v'subtype) return integer is begin return x'length; end;
  procedure cp(x : in a(0).v'subtype; y : out a(0).v'subtype) is begin y := x; end;
  function ret return a(0).v'subtype is begin return "1001"; end;
begin
  process
    variable v : a(0).v'subtype;
    variable w : ra(0).a(1).v'subtype;
    variable x : a(0).v(3 downto 2)'subtype;
    variable y : m(0, 1)'subtype;
    variable z : a(0).v'element;
    variable e : aa(1)'element;
    variable q : bit_vector(a(0).v'range);
    variable n : integer := 0;
    variable cv : bit_vector(3 downto 0);
    variable ty : t2;
    variable rc : r2;
  begin
    v := "1010";
    report to_string(v) & " " & integer'image(v'length);
    w := "0110";
    report to_string(w) & " " & integer'image(w'length) & " " & integer'image(w'left);
    report integer'image(x'length) & " " & integer'image(y'length) & " " & integer'image(q'length);
    z := '1'; e := '0';
    report to_string(z) & to_string(e);
    report integer'image(s'length) & " " & integer'image(k'length) & " " & to_string(k);
    report integer'image(al'length) & " " & integer'image(len("1111")) & " " & to_string(ret);
    cp("1100", v);
    report to_string(v);
    for i in a(0).v'range loop n := n + 1; end loop;
    for i in ra(0).a(1).v'subtype'range loop n := n + 10; end loop;
    for i in aa(1)'range loop n := n + 100; end loop;
    report integer'image(n);
    cv := a(0).v'subtype(v);
    report to_string(cv);
    cv := a(0).v'subtype'("0011");
    report to_string(cv) & " " & integer'image(ty(0)'length) & " " & integer'image(rc.f'length);
    assert v = "1100" and n = 344 and cv = "0011" severity failure;
    wait;
  end process;
end;
