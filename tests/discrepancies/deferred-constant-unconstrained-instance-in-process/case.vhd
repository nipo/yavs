-- LRM 4.8, 6.4.2.2: the same instance declared in a process, a function and a generate.
package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function x return integer;
end package;
package body gp is
  function mk(m : integer) return vec_t is
    variable r : vec_t(1 to m) := (others => 7);
  begin r(m) := n; return r; end;
  constant U : vec_t := mk(n);
  function x return integer is begin return U'length; end;
end package body;

entity top is end entity;
architecture a of top is
  function f return integer is
    package ifn is new work.gp generic map (n => 6);
  begin
    return ifn.U'length * 10 + ifn.U(ifn.U'right);
  end function;
begin
  g : for i in 1 to 2 generate
    package ig is new work.gp generic map (n => i + 1);
  begin
    process begin report "RES generate " & integer'image(ig.U'length) & " " & integer'image(ig.U(i + 1)); wait; end process;
  end generate;
  process
    package ip is new work.gp generic map (n => 5);
  begin
    report "RES " & integer'image(ip.U'length) & " " & integer'image(ip.U'right) & " " & integer'image(ip.U(5)) & " " & integer'image(ip.x);
    report "RES function " & integer'image(f);
    wait;
  end process;
end architecture;
