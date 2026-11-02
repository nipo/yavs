-- LRM 6.4.2.2: a function of a package body reads 'length of a deferred constant of an
-- unconstrained array subtype and is called, to initialise a constant, before the full
-- declaration is elaborated: the bounds do not exist yet.
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body p is
  function h return integer is begin return U'length; end;
  constant X1 : integer := h;
  constant U : vec_t := (1, 2, 3, 4);
  function x return integer is begin return X1; end;
end package body;
use work.p.all;
entity top is end entity;
architecture a of top is
begin
  process
  begin
    report "RES " & integer'image(x);
    wait;
  end process;
end architecture;
