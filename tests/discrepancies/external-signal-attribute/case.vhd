-- LRM 8.1, 16.2: the signal attributes with a signal external name written directly as the prefix.
entity top is end;
architecture test of top is
  signal s : bit_vector(3 downto 0) := "0000";
begin
  process begin
    wait for 1 ns;
    s <= "0001";
    wait;
  end process;
  process begin
    wait until <<signal .top.s : bit_vector(3 downto 0)>>'event;
    report "RES event " & boolean'image(<<signal .top.s : bit_vector(3 downto 0)>>'event);
    report "RES active " & boolean'image(<<signal .top.s : bit_vector(3 downto 0)>>'active);
    report "RES last_value " & to_string(<<signal .top.s : bit_vector(3 downto 0)>>'last_value);
    wait for 1 ns;
    report "RES last_event " & time'image(<<signal .top.s : bit_vector(3 downto 0)>>'last_event);
    report "RES stable " & boolean'image(<<signal .top.s : bit_vector(3 downto 0)>>'stable(5 ns));
    wait;
  end process;
end;
