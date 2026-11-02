-- LRM 6.4.2.2: 'right of a deferred constant of an unconstrained array subtype, in a function of
-- a package nested in its package body, after the full declaration.
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function f return integer;
end package;
package body p is
  constant U : vec_t := (5 => 1, 6 => 2, 7 => 3, 8 => 4);
  package nest is
    function nf return integer;
  end package;
  package body nest is
    function nf return integer is begin return U'right; end;
  end package body;
  function f return integer is begin return nest.nf; end;
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
