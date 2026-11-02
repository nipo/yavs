-- difftest: top=deferred_subtype_attrs std=2008
-- Attributes of the subtype of a deferred constant in its package body before the full
-- declaration (#1072): in a parameter subtype, a variable, a type, subtype and package
-- instantiation of the body, for a record element and a second dimension. The deferred
-- declarations constrain the subtypes fully, so no value is read. Globally static bounds are
-- the discrepancy case deferred-constant-global-bounds (both references fail to elaborate it).
package deferred_subtype_attrs_g is
  generic (N : natural);
  constant DOUBLE : natural := 2 * N;
end package;

package deferred_subtype_attrs_p is
  type vec_t is array (natural range <>) of integer;
  type rec_t is record
    f : bit_vector(2 to 7);
  end record;
  type mat_t is array (natural range <>, natural range <>) of bit;
  constant K : vec_t(0 to 3);
  constant G : vec_t(4 downto 1);
  constant R : rec_t;
  constant M : mat_t(1 to 2, 6 downto 4);
  constant KL : natural := K'length;
  procedure q (constant d : in bit_vector(K'range); s : out integer);
  function f return integer;
  function g_text return string;
  function double return natural;
end package;

package body deferred_subtype_attrs_p is
  type arr_t is array (K'reverse_range) of bit;
  subtype sub_t is bit_vector(G'range);
  subtype int_t is integer range G'low to G'high;
  package gi is new work.deferred_subtype_attrs_g generic map (N => K'length + G'length);

  procedure q (constant d : in bit_vector(K'range); s : out integer) is
    variable n : integer := 0;
  begin
    for i in d'range loop
      if d(i) = '1' then
        n := n + 10 ** i;
      end if;
    end loop;
    s := n;
  end;

  function f return integer is
    variable v : K'subtype;
    variable e : K'element;
    variable vr : bit_vector(R.f'range);
    variable vm : bit_vector(M'range(2));
  begin
    v := (others => 7);
    e := v(1) + K'high;
    assert K'ascending and not G'ascending severity failure;
    return e + v'length + vr'left + vr'length + vm'left + M'length(1) + K'left + K'right + K'low;
  end;

  function g_text return string is
    variable a : arr_t;
    variable s : sub_t := (others => '1');
    variable i : int_t := int_t'high;
  begin
    return integer'image(a'left) & " " & integer'image(a'right) & " " & integer'image(s'left)
      & " " & integer'image(s'right) & " " & integer'image(int_t'low) & " " & integer'image(i);
  end;

  function double return natural is begin return gi.DOUBLE; end;

  constant K : vec_t(0 to 3) := (1, 2, 3, 4);
  constant G : vec_t(4 downto 1) := (others => 9);
  constant R : rec_t := (f => "101010");
  constant M : mat_t(1 to 2, 6 downto 4) := (others => (others => '1'));
end package body;

use work.deferred_subtype_attrs_p.all;
entity deferred_subtype_attrs is
end entity;

architecture a of deferred_subtype_attrs is
begin
  process
    variable s : integer;
  begin
    q("1011", s);
    report "q=" & integer'image(s);
    report "f=" & integer'image(f);
    report "types=" & g_text;
    report "double=" & integer'image(double) & " kl=" & integer'image(KL);
    report "sum=" & integer'image(K(0) + K(3) + G(1) + G'length);
    wait;
  end process;
end architecture;
