-- difftest: top=tb_null_record_field
-- A null field of a constrained record through an unconstrained port: its attributes, ranges
-- derived from it, and concurrent assignments with static null operands, next to nonnull widths.
library ieee;
use ieee.std_logic_1164.all;

package null_record_field_pkg is
  type t_axi4_stream is record
    tvalid : std_logic;
    tdata  : std_logic_vector;
    tid    : std_logic_vector;
  end record;
end package;

library ieee;
use ieee.std_logic_1164.all;
use work.null_record_field_pkg.all;

entity null_field_dut is
  generic (
    name : string;
    n    : natural;
    l, r : integer;
    asc  : boolean;
    at   : time);
  port (axi_stream : in t_axi4_stream; en : in std_logic);
end entity;

architecture a of null_field_dut is
  constant zeros : std_logic_vector(axi_stream.tid'length - 1 downto 0) := (others => '0');
  constant ones  : std_logic_vector(axi_stream.tid'length - 1 downto 0) := (others => '1');
  signal mux_else, mux_then, mux_str : std_logic_vector(axi_stream.tid'length - 1 downto 0);
  signal mux_range                   : std_logic_vector(axi_stream.tid'range);
  signal arms, gate, tree, copy      : std_logic_vector(axi_stream.tid'length - 1 downto 0);
  signal is_zero, is_ones            : std_logic;
begin
  mux_else  <= axi_stream.tid when en = '1' else (others => '0');
  mux_then  <= (others => '1') when en = '1' else axi_stream.tid;
  mux_str   <= axi_stream.tid when en = '1' else zeros;
  mux_range <= axi_stream.tid when en = '1' else (others => '0');
  arms      <= axi_stream.tid when en = '1' else ones when axi_stream.tvalid = '1' else zeros;
  gate      <= axi_stream.tid and ones;
  tree      <= (axi_stream.tid and mux_else) or zeros;
  copy      <= axi_stream.tid;
  is_zero   <= '1' when axi_stream.tid = zeros else '0';
  is_ones   <= '1' when ones = axi_stream.tid else '0';

  process
  begin
    assert axi_stream.tid'length = n report name & " length" severity failure;
    assert axi_stream.tid'left = l report name & " left" severity failure;
    assert axi_stream.tid'right = r report name & " right" severity failure;
    assert axi_stream.tid'ascending = asc report name & " ascending" severity failure;
    assert mux_else'length = n and mux_else'left = n - 1 and mux_else'right = 0
      and not mux_else'ascending report name & " derived" severity failure;
    assert mux_range'left = l and mux_range'right = r and mux_range'ascending = asc
      report name & " range" severity failure;
    assert axi_stream.tdata'length = 8 report name & " tdata" severity failure;
    wait for at;
    report name & ": " & to_string(axi_stream.tvalid) & " " & to_string(axi_stream.tdata)
      & " tid=" & to_string(axi_stream.tid)
      & " else=" & to_string(mux_else) & " then=" & to_string(mux_then)
      & " str=" & to_string(mux_str) & " range=" & to_string(mux_range)
      & " arms=" & to_string(arms) & " gate=" & to_string(gate)
      & " tree=" & to_string(tree) & " copy=" & to_string(copy)
      & " zero=" & to_string(is_zero) & " ones=" & to_string(is_ones);
    wait for 10 ns;
    report name & ": " & to_string(axi_stream.tvalid) & " " & to_string(axi_stream.tdata)
      & " tid=" & to_string(axi_stream.tid)
      & " else=" & to_string(mux_else) & " then=" & to_string(mux_then)
      & " str=" & to_string(mux_str) & " range=" & to_string(mux_range)
      & " arms=" & to_string(arms) & " gate=" & to_string(gate)
      & " tree=" & to_string(tree) & " copy=" & to_string(copy)
      & " zero=" & to_string(is_zero) & " ones=" & to_string(is_ones);
    wait;
  end process;
end architecture;

library ieee;
use ieee.std_logic_1164.all;
use work.null_record_field_pkg.all;

entity tb_null_record_field is
end entity;

architecture t of tb_null_record_field is
  signal first_null  : t_axi4_stream(tdata(7 downto 0), tid(-1 downto 0));
  signal wide        : t_axi4_stream(tdata(7 downto 0), tid(7 downto 0));
  signal one         : t_axi4_stream(tdata(7 downto 0), tid(0 downto 0));
  signal second_null : t_axi4_stream(tdata(7 downto 0), tid(-1 downto 0));
  signal ascending   : t_axi4_stream(tdata(7 downto 0), tid(0 to -1));
  signal en          : std_logic := '0';
begin
  -- Null, wide, null again: no instance may take the bounds or shape of another.
  u0 : entity work.null_field_dut
    generic map ("u0", 0, -1, 0, false, 1 ns)
    port map (axi_stream => first_null, en => en);
  u8 : entity work.null_field_dut
    generic map ("u8", 8, 7, 0, false, 2 ns)
    port map (axi_stream => wide, en => en);
  u1 : entity work.null_field_dut
    generic map ("u1", 1, 0, 0, false, 3 ns)
    port map (axi_stream => one, en => en);
  u0b : entity work.null_field_dut
    generic map ("u0b", 0, -1, 0, false, 4 ns)
    port map (axi_stream => second_null, en => en);
  ua : entity work.null_field_dut
    generic map ("ua", 0, 0, -1, true, 5 ns)
    port map (axi_stream => ascending, en => en);

  process
  begin
    first_null.tvalid  <= '1';
    first_null.tdata   <= x"a5";
    wide.tvalid        <= '1';
    wide.tdata         <= x"3c";
    wide.tid           <= x"c3";
    one.tvalid         <= '0';
    one.tdata          <= x"0f";
    one.tid            <= "1";
    second_null.tvalid <= '0';
    second_null.tdata  <= x"f0";
    ascending.tvalid   <= '1';
    ascending.tdata    <= x"81";
    wait for 6 ns;
    en <= '1';
    wide.tid <= x"ff";
    wait for 20 ns;
    report "done";
    wait;
  end process;
end architecture;
