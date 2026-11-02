-- yavs-test: std=2008
-- expect: E1226 14:45 deferred constant `U` is used before its full declaration
-- expect: E1226 15:43 deferred constant `U` is used before its full declaration
-- expect: E1226 16:45 deferred constant `K` is used before its full declaration
-- LRM 6.4.2.2: the bounds of a deferred constant of an unconstrained subtype come from its value, so its attributes are uses before the full declaration; an indexed prefix reads the value too (#1072)
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  constant K : vec_t(0 to 3);
  function f return integer;
  function g return integer;
end package;
package body p is
  function f return integer is begin return U'length; end;
  procedure q (constant d : in bit_vector(U'range)) is begin null; end;
  function g return integer is begin return K(K'low); end;
  constant U : vec_t := (1, 2, 3, 4);
  constant K : vec_t(0 to 3) := (1, 2, 3, 4);
end package body;
