-- yavs-test: std=2008
-- expect: E1226 19:29 deferred constant `U` is used before its full declaration
-- expect: E1226 20:24 deferred constant `U` is used before its full declaration
-- expect: E1226 21:27 deferred constant `U` is used before its full declaration
-- expect: E1226 22:47 deferred constant `U` is used before its full declaration
-- expect: E1226 24:29 deferred constant `U` is used before its full declaration
-- expect: E1226 26:45 deferred constant `K` is used before its full declaration
-- LRM 6.4.2.2: the bounds of a deferred constant of an unconstrained subtype come from its value, so its attributes are uses wherever the bounds are needed before the full declaration is elaborated: a declaration of the package body before it. An indexed prefix reads the value too (#1072, #1076)
package gp is
  generic (N : natural);
  constant D : natural := 2 * N;
end package;
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  constant K : vec_t(0 to 3);
end package;
package body p is
  subtype s_t is bit_vector(U'range);
  type arr_t is array (U'reverse_range) of bit;
  constant V : natural := U'length;
  package gi is new work.gp generic map (N => U'length);
  package nest is
    constant Z : natural := U'high;
  end package;
  function g return integer is begin return K(K'low); end;
  constant U : vec_t := (1, 2, 3, 4);
  constant K : vec_t(0 to 3) := (1, 2, 3, 4);
end package body;
