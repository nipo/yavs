-- lint_700 to lint_703: one clock or reset has nothing to cross.
entity domains is
  port (clk, clk_a, clk_b, rst, en, d : in bit);
end entity;

architecture rtl of domains is
  signal a1, y1, a2, b2, a3, b3, a4, a5, a6, b6 : bit;
begin
  one_clock : process (clk) is
  begin
    if clk'event and clk = '1' then
      a1 <= not a1;
      y1 <= a1 and y1;
    end if;
  end process;

  one_reset : process (clk, rst) is
  begin
    if rst = '1' then
      a2 <= '0';
      b2 <= '0';
    elsif clk'event and clk = '1' then
      a2 <= d;
      b2 <= a2 and d;
    end if;
  end process;

  no_reset : process (clk) is
  begin
    if clk'event and clk = '1' then
      a3 <= d;
      b3 <= a3 and d;
    end if;
  end process;

  combinational : process (en, d) is
  begin
    a4 <= '0';
    if en = '1' then
      a4 <= d;
    end if;
  end process;

  no_reset_branch : process (clk) is
  begin
    if clk'event and clk = '1' then
      a5 <= d;
    end if;
  end process;

  two_a : process (clk_a) is
  begin
    if clk_a'event and clk_a = '1' then
      a6 <= d;
    end if;
  end process;

  two_b : process (clk_b) is
  begin
    if clk_b'event and clk_b = '0' then
      b6 <= d;
    end if;
  end process;
end architecture;
