-- A slice of a null array as a static name: of a signal, a record field, an array element and
-- as a port actual. No part is selected.
library ieee;
use ieee.std_logic_1164.all;
entity null_slice_child is
  port (i : in std_logic_vector; o : out std_logic_vector);
end;
architecture a of null_slice_child is
begin
  o <= i;
end;
library ieee;
use ieee.std_logic_1164.all;
entity null_slice_of_null is
end;
architecture t of null_slice_of_null is
  type rec_t is record
    v : std_logic;
    i : std_logic_vector;
  end record;
  type vv_t is array (natural range <>) of std_logic_vector;
  signal n, m, k, q : std_logic_vector(-1 downto 0);
  signal r : rec_t(i(-1 downto 0));
  signal vv : vv_t(0 to 1)(-1 downto 0);
  signal e1, e2 : vv_t(1 to 0)(7 downto 0);
begin
  m <= n(-1 downto 0);
  k(-1 downto 0) <= r.i(-1 downto 0);
  q <= vv(0)(-1 downto 0);
  e1(1 to 0) <= e2(1 to 0);
  u : entity work.null_slice_child port map (i => n(-1 downto 0), o => vv(1));
  process begin
    wait for 1 ns;
    report integer'image(m'length + k'length + q'length + e1'length);
    std.env.finish;
  end process;
end;
