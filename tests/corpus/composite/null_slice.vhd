-- difftest: top=tb_null_slice
-- Null slices of null arrays: as a source, a target, a port actual, through a record field with
-- a derived range, and in an array of records.
library ieee;
use ieee.std_logic_1164.all;

package null_slice_pkg is
  type t_rec is record
    valid : std_logic;
    tid   : std_logic_vector;
  end record;
  type t_rec_array is array (natural range <>) of t_rec;
end package;

library ieee;
use ieee.std_logic_1164.all;

entity null_slice_sub is
  port (d : in std_logic_vector);
end entity;

architecture a of null_slice_sub is
begin
  process
  begin
    assert d'length = 0 report "port length" severity failure;
    wait;
  end process;
end architecture;

library ieee;
use ieee.std_logic_1164.all;
use work.null_slice_pkg.all;

entity null_slice_dut is
  port (s : in t_rec);
end entity;

architecture a of null_slice_dut is
  signal by_length : std_logic_vector(s.tid'length - 1 downto 0);
begin
  by_length <= s.tid(s.tid'length - 1 downto 0);
  process
  begin
    wait for 1 ns;
    assert s.valid = '1' report "field beside the null one" severity failure;
    assert by_length'length = 0 and by_length'left = -1 and by_length'right = 0
      report "derived bounds" severity failure;
    report "dut ok";
    wait;
  end process;
end architecture;

library ieee;
use ieee.std_logic_1164.all;
use work.null_slice_pkg.all;

entity tb_null_slice is
end entity;

architecture t of tb_null_slice is
  signal down_a, down_b, down_c : std_logic_vector(-1 downto 0);
  signal up_a, up_b             : std_logic_vector(0 to -1);
  signal wide                   : std_logic_vector(7 downto 0) := x"5a";
  signal from_wide              : std_logic_vector(-1 downto 0);
  signal r                      : t_rec(tid(-1 downto 0));
  signal rs                     : t_rec_array(0 to -1)(tid(7 downto 0));
  signal rs_b                   : t_rec_array(0 to -1)(tid(7 downto 0));
begin
  down_b               <= down_a(-1 downto 0);
  down_c(-1 downto 0)  <= down_a;
  up_b                 <= up_a(0 to -1);
  from_wide            <= wide(-1 downto 0);
  rs_b                 <= rs(0 to -1);
  sub : entity work.null_slice_sub port map (d => down_a(-1 downto 0));
  dut : entity work.null_slice_dut port map (s => r);
  r.valid <= '1';

  process
  begin
    wait for 2 ns;
    assert down_b'length = 0 and up_b'length = 0 and from_wide'length = 0 and rs_b'length = 0
      report "lengths" severity failure;
    assert wide = x"5a" report "wide" severity failure;
    report "done";
    wait;
  end process;
end architecture;
