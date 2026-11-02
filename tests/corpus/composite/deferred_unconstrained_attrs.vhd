-- difftest: top=deferred_unconstrained_attrs std=2008
-- Attributes of a deferred constant of an unconstrained subtype in subprograms of its package
-- body, before the full declaration (#1076): a function body, a parameter subtype, a variable
-- and a local subtype; after it also a subtype, a type, a constant and a package instantiation.
-- The bounds come from the full declaration's value. A nested package is the discrepancy case
-- deferred-constant-unconstrained-nested.
package deferred_unconstrained_attrs_g is
  generic (N : natural);
  constant D : natural := 2 * N;
end package;
package deferred_unconstrained_attrs_p is
  type vec_t is array (natural range <>) of integer;
  type mat_t is array (natural range <>, natural range <>) of bit;
  constant U : vec_t;
  constant M : mat_t;
  function f return integer;
  function g return string;
  function h return integer;
  function later return integer;
  procedure q (constant d : in bit_vector(U'range); s : out integer);
end package;
package body deferred_unconstrained_attrs_p is
  function f return integer is
    variable v : U'subtype;
    variable e : U'element;
    variable w : bit_vector(U'reverse_range);
  begin
    v := (others => 7);
    e := v(U'low) + U'high;
    assert U'ascending severity failure;
    return U'length * 1000 + U'left * 100 + U'right * 10 + e + w'length + M'length(2) + M'right(2);
  end;
  procedure q (constant d : in bit_vector(U'range); s : out integer) is
    variable n : integer := 0;
  begin
    for i in d'range loop
      if d(i) = '1' then n := n + 10 ** (i - U'left); end if;
    end loop;
    s := n;
  end;
  function g return string is
    subtype sub_t is bit_vector(U'range);
    variable a : sub_t := (others => '1');
  begin
    return integer'image(a'left) & " " & integer'image(a'right) & " " & integer'image(a'length);
  end;
  function h return integer is begin return U'length; end;
  constant U : vec_t := (5 => 1, 6 => 2, 7 => 3, 8 => 4);
  constant M : mat_t := (1 to 2 => (3 to 5 => '1'));
  subtype late_t is bit_vector(U'range);
  constant LK : natural := U'length + M'length(2);
  type arr_t is array (U'reverse_range) of bit;
  package gi is new work.deferred_unconstrained_attrs_g generic map (N => U'length);
  function later return integer is begin return gi.D + LK + late_t'length + arr_t'length; end;
end package body;
use work.deferred_unconstrained_attrs_p.all;
entity deferred_unconstrained_attrs is end entity;
architecture a of deferred_unconstrained_attrs is
begin
  process
    variable s : integer;
  begin
    report "f=" & integer'image(f);
    report "g=" & g;
    report "h=" & integer'image(h);
    q("1011", s);
    report "q=" & integer'image(s);
    report "later=" & integer'image(later);
    report "U'length=" & integer'image(U'length) & " " & integer'image(U'left);
    wait;
  end process;
end architecture;
