-- A package path through a nested package (#1103).
library ieee;
use ieee.std_logic_1164.all;
package external_nested_pkg is
  package inner is
    signal gs : std_logic := '0';
    constant c : integer := 7;
  end package;
end;
library ieee;
use ieee.std_logic_1164.all;
entity external_nested_package is
end;
architecture t of external_nested_package is
begin
  process begin
    << signal @work.external_nested_pkg.inner.gs : std_logic >> <= '1';
    wait for 1 ns;
    assert << signal @work.external_nested_pkg.inner.gs : std_logic >> = '1';
    assert << constant @work.external_nested_pkg.inner.c : integer >> = 7;
    std.env.finish;
  end process;
end;
