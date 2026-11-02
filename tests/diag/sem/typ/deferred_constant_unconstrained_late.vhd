-- yavs-test: std=2008
-- expect: none
-- LRM 6.4.2.2: an attribute of the subtype alone of a deferred constant of an unconstrained subtype may be used in the package body, once the full declaration exists: in a subprogram (parameter subtypes, variables, bodies), which runs after elaboration, and in any declaration after the full declaration (#1076)
package gp is
  generic (N : natural);
  constant D : natural := 2 * N;
end package;
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function f return integer;
  procedure q (constant d : in bit_vector(U'range));
end package;
package body p is
  function f return integer is
    variable v : U'subtype;
    variable w : bit_vector(U'reverse_range);
    variable e : U'element;
  begin
    return U'length + U'left + U'right + U'low + U'high + w'length + v'length + e;
  end;
  procedure q (constant d : in bit_vector(U'range)) is begin null; end;
  procedure r (constant d : in bit_vector(U'range)) is begin null; end;
  constant U : vec_t := (1, 2, 3, 4);
  subtype s_t is bit_vector(U'range);
  type arr_t is array (U'reverse_range) of bit;
  constant V : natural := U'length;
  package gi is new work.gp generic map (N => U'length);
  package nest is
    constant Z : natural := U'high;
  end package;
end package body;
