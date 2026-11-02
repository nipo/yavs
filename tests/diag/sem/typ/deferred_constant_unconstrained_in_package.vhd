-- yavs-test: std=2008
-- expect: E1226 7:27 deferred constant `U` is used before its full declaration
-- LRM 6.4.2.2: the length of a deferred constant of an unconstrained subtype is not known in its package declaration (#1072)
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  constant M : integer := U'length;
end package;
package body p is
  constant U : vec_t := (1, 2, 3, 4);
end package body;
