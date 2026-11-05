-- Individual association of an unconstrained formal: a slice gives its direction, and null
-- slices alone its bounds (#1100, #1101).
library ieee;
use ieee.std_logic_1164.all;
package individual_null_slices_pkg is
  type rr is record
    v : std_logic;
    d : std_logic_vector;
    i : std_logic_vector;
  end record;
end package;
library ieee;
use ieee.std_logic_1164.all;
use work.individual_null_slices_pkg.all;
entity individual_null_slices_port is
  port (r : inout rr);
end;
architecture a of individual_null_slices_port is
begin
  process begin
    wait for 1 ns;
    assert r.d'left = 7 and r.d'length = 8 and r.i'length = 0;
    wait;
  end process;
end;
library ieee;
use ieee.std_logic_1164.all;
use work.individual_null_slices_pkg.all;
entity individual_null_slices is
end;
architecture t of individual_null_slices is
  signal r : rr(d(7 downto 0), i(-1 downto 0));
  signal m : std_logic_vector(-1 downto 0);
begin
  a : entity work.individual_null_slices_port
    port map (r.v => r.v, r.d(7 downto 0) => r.d, r.i(-1 downto 0) => m);
  b : entity work.individual_null_slices_port
    port map (r.v => r.v, r.d(7 downto 0) => r.d, r.i => r.i(-1 downto 0));
  process begin
    wait for 2 ns;
    std.env.finish;
  end process;
end;
