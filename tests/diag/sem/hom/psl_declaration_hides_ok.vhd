-- yavs-test: std=2008
-- expect: none
-- LRM 12.3: a PSL declaration in an architecture hides a declaration of that name in a package
-- made visible by a use clause, and does not clash with it (#1077)
package p is
  constant pa : integer := 1;
end package;
use work.p.all;
entity e is
end entity;
architecture t of e is
  signal clk, a : bit := '0';
  default clock is clk'event and clk = '1';
  sequence pa is {a};
begin
  c1: cover pa report "x";
end architecture;
