-- LRM 14.4.2: a function of an instance's package body reads 'length of the deferred constant
-- and is called, to initialise a constant, before the full declaration is elaborated (the
-- instance is declared in an architecture): erroneous, as in a package.
package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body gp is
  function mk(m : integer) return vec_t is
    variable r : vec_t(1 to m) := (others => 7);
  begin return r; end;
  function h return integer is begin return U'length; end;
  constant X1 : integer := h;
  constant U : vec_t := mk(n);
  function x return integer is begin return X1; end;
end package body;
entity top is end entity;
architecture a of top is
  package ip is new work.gp generic map (n => 4);
begin
  process begin report "RES " & integer'image(ip.x); wait; end process;
end architecture;
