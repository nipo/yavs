-- Reports of every severity from two places, three times from a loop.
entity summary_asserts is
end entity;

architecture a of summary_asserts is
  signal clk : bit := '0';
  signal cnt : natural := 0;
begin
  clk <= not clk after 5 ns when cnt < 3;

  count : process (clk) is
  begin
    if clk'event and clk = '1' then
      cnt <= cnt + 1;
    end if;
  end process;

  check : process is
  begin
    for i in 1 to 3 loop
      wait until rising_edge(clk);
      report "tick" severity note;
      assert cnt > 1 report "too early" severity warning;
    end loop;
    assert false report "first error" severity error;
    wait;
  end process;
end architecture;
