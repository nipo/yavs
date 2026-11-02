-- LRM 6.4.2.2: 'length of a deferred constant of an unconstrained array subtype in a subtype of
-- its package body before the full declaration: the bounds are needed when the subtype is
-- elaborated, before the value.
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function f return integer;
end package;
package body p is
  subtype s_t is bit_vector(U'range);
  function f return integer is begin return s_t'length; end;
  constant U : vec_t := (1, 2, 3, 4);
end package body;
use work.p.all;
entity top is end entity;
architecture a of top is
begin
  process
  begin
    report "RES " & integer'image(f);
    wait;
  end process;
end architecture;
