-- difftest: top=deferred_instance_bounds std=2008 ignore=order
-- The bounds and elements of an unconstrained deferred constant of a generic package
-- instantiated in an architecture, a block and another instance (LRM 4.8, 4.9,
-- 6.4.2.2; #1079). ignore=order: same-cycle reports of different processes.
package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body gp is
  constant U : vec_t := (1, 2, 3, n);
  function x return integer is begin return U'length; end function;
end package body;

package gw is
  generic (n : integer);
  function y return integer;
end package;
package body gw is
  package inner is new work.gp generic map (n => n + 1);
  function y return integer is
  begin
    return inner.U'length * 100 + inner.U'right * 10 + inner.U(inner.U'left);
  end function;
end package body;

entity deferred_instance_bounds is end entity;

architecture a of deferred_instance_bounds is
  package ia is new work.gp generic map (n => 4);
  package ow is new work.gw generic map (n => 6);
begin
  process begin
    assert ia.U'length = 4 and ia.U'left = 0 and ia.U'right = 3 report "arch bounds" severity failure;
    assert ia.U(0) = 1 and ia.U(2) = 3 and ia.U(3) = 4 report "arch elements" severity failure;
    assert ia.x = 4 report "arch x" severity failure;
    report "ARCH " & integer'image(ia.U'length) & " " & integer'image(ia.U(3));
    report "BODY " & integer'image(ow.y);
    assert ow.y = 431 report "nested" severity failure;
    wait;
  end process;

  b : block
    package ib is new work.gp generic map (n => 9);
  begin
    process begin
      assert ib.U'length = 4 and ib.U(3) = 9 report "block" severity failure;
      report "BLOCK " & integer'image(ib.U'length) & " " & integer'image(ib.U(3));
      wait;
    end process;
  end block;
end architecture;
