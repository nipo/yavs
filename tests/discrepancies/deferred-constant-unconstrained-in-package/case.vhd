-- LRM 6.4.2.2: 'length of a deferred constant of an unconstrained array subtype in an initial
-- value of its package declaration: the bounds come from the value, not known there.
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  constant M : integer := U'length;
end package;
package body p is
  constant U : vec_t := (1, 2, 3, 4);
end package body;
use work.p.all;
entity top is end entity;
architecture a of top is
begin
  process
  begin
    report "RES " & integer'image(M);
    wait;
  end process;
end architecture;
