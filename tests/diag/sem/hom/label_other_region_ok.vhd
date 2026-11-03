-- yavs-test: std=2008
-- expect: none
-- LRM 2008 12.1, 12.3: a homograph in a nested region only hides; a label of the same name there is legal (#1084)
library ieee;
package label_ok_pkg is
  constant from_pkg : bit := '0';
  procedure tick;
  component a_comp is end component;
end package;
package body label_ok_pkg is
  procedure tick is begin null; end procedure;
end package body;
use work.label_ok_pkg.all;
entity label_ok is
  port (a_port : in bit := '0');
end entity;
architecture a of label_ok is
  signal x : bit;
  procedure q is begin x : null; end procedure;
  function own return bit is begin own : return '0'; end function;
begin
  label_ok : process begin wait; end process;
  a : process begin wait; end process;
  work : process begin wait; end process;
  ieee : process begin wait; end process;
  from_pkg : process begin wait; end process;
  tick : process begin wait; end process;
  a_comp : process begin wait; end process;
  b : block
    signal b : bit;
  begin
    x : process begin wait; end process;
    a_port : process begin wait; end process;
    c : block
      signal c : bit;
      signal y : bit;
    begin
    end block;
  end block;
  g : for g in 0 to 1 generate
    x : process begin wait; end process;
  end generate;
  h : if x : true generate
    a_port : process begin wait; end process;
  end generate;
  p : process
    variable p : bit;
    variable v : bit;
  begin
    x : null;
    a_port : null;
    for i in 0 to 1 loop
      i : null;
    end loop;
    l : for l in 0 to 1 loop
      null;
    end loop;
    wait;
  end process;
  r : process begin
    r : null;
    wait;
  end process;
end architecture;
