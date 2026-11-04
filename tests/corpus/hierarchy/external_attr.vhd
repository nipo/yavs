-- difftest: top=external_attr std=2008 ref=hand
-- Attributes of a signal or constant external name written directly (LRM 8.1, 8.6, 8.7, 16.2,
-- #1091): the signal attributes, the array and subtype attributes, in expressions, waits and
-- sensitivity-driven processes.
-- ref=hand: one reference simulator rejects the signal attributes of an external name, the
-- other rejects 'length, 'left and 'range of one; the expected output is the LRM result.
library ieee;
use ieee.std_logic_1164.all;
entity external_attr is end;
architecture a of external_attr is
  type rec_t is record f : std_logic_vector(3 downto 0); end record;
  signal s : std_logic_vector(3 downto 0) := "0000";
  signal b : bit := '0';
  signal r : rec_t := (f => "0000");
  signal n : integer := 0;
  constant c : std_logic_vector(7 downto 0) := x"a5";
begin
  stim : process
  begin
    wait for 1 ns;
    s <= "0001"; b <= '1'; r.f <= "0010"; n <= 7;
    wait for 3 ns;
    b <= '0';
    wait;
  end process;

  chk : process
    alias xs is <<signal .external_attr.s : std_logic_vector(3 downto 0)>>;
  begin
    assert not <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'event severity failure;
    wait until <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'event;
    assert now = 1 ns severity failure;
    assert xs'event severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'active severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'last_event = 0 ns severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'last_value = "0000" severity failure;
    assert <<signal .external_attr.r : rec_t>>.f'last_value = "0000" severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'length = 4 severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'left = 3 severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'range'right = 0 severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'subtype'length = 4 severity failure;
    assert <<signal .external_attr.n : integer>>'subtype'high = integer'high severity failure;
    assert <<constant .external_attr.c : std_logic_vector(7 downto 0)>>'length = 8 severity failure;
    assert <<constant .external_attr.c : std_logic_vector(7 downto 0)>>'subtype'length = 8 severity failure;
    wait for 1 ns;
    assert not <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'event severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'last_event = 1 ns severity failure;
    assert <<signal .external_attr.s : std_logic_vector(3 downto 0)>>'last_active = 1 ns severity failure;
    assert not <<signal .external_attr.b : bit>>'stable(10 ns) severity failure;
    assert <<signal .external_attr.b : bit>>'delayed(1 ns) = '1' severity failure;
    wait for 5 ns;
    assert <<signal .external_attr.b : bit>>'quiet(2 ns) severity failure;
    assert <<signal .external_attr.b : bit>>'stable(2 ns) severity failure;
    report "checked";
    wait;
  end process;

  watch : process (all)
  begin
    if <<signal .external_attr.n : integer>>'event then
      report "n event at " & time'image(now);
    end if;
  end process;
end;
