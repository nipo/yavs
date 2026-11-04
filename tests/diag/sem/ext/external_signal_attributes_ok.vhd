-- yavs-test: std=2019
-- expect: none
-- LRM 8.1, 8.6, 16.2, 8.7 (#1091): a signal external name is a prefix for every signal
-- attribute and for the value and subtype attributes, as the same signal is through an alias.
package p is
  signal s : bit_vector(3 downto 0);
  signal b : bit;
  constant k : bit_vector(7 downto 0) := x"a5";
end package;
use work.p.all;
entity external_signal_attributes_ok is end entity;
architecture a of external_signal_attributes_ok is
begin
  process
    variable t : boolean;
    variable n : natural;
    variable d : time;
    variable v : bit_vector(3 downto 0);
    alias x is <<signal .external_signal_attributes_ok.s : bit_vector(3 downto 0)>>;
  begin
    t := x'event or x'active or x'stable or x'quiet(1 ns) or x'driving;
    t := <<signal .external_signal_attributes_ok.s : bit_vector(3 downto 0)>>'event;
    t := <<signal .external_signal_attributes_ok.s : bit_vector(3 downto 0)>>'active;
    t := <<signal @work.p.s : bit_vector(3 downto 0)>>'stable(1 ns);
    t := <<signal ^.s : bit_vector(3 downto 0)>>'quiet or <<signal @work.p.b : bit>>'driving;
    d := <<signal @work.p.s : bit_vector(3 downto 0)>>'last_event;
    d := <<signal @work.p.s : bit_vector(3 downto 0)>>'last_active;
    v := <<signal @work.p.s : bit_vector(3 downto 0)>>'last_value;
    v := <<signal @work.p.s : bit_vector(3 downto 0)>>'delayed(1 ns);
    t := <<signal @work.p.b : bit>>'transaction = '1';
    n := <<signal @work.p.s : bit_vector(3 downto 0)>>'length;
    n := <<signal @work.p.s : bit_vector(3 downto 0)>>'left + <<signal @work.p.s : bit_vector(3 downto 0)>>'high;
    t := <<signal @work.p.s : bit_vector(3 downto 0)>>'ascending;
    for i in <<signal @work.p.s : bit_vector(3 downto 0)>>'range loop null; end loop;
    n := <<signal @work.p.s : bit_vector(3 downto 0)>>'subtype'length;
    t := <<signal @work.p.s : bit_vector(3 downto 0)>>'element'high = '1';
    t := <<constant @work.p.k : bit_vector(7 downto 0)>>'length = 8;
    n := <<constant @work.p.k : bit_vector(7 downto 0)>>'subtype'length;
    wait on <<signal @work.p.s : bit_vector(3 downto 0)>>'delayed(1 ns);
    wait until <<signal @work.p.b : bit>>'event;
    wait;
  end process;
end architecture;
