-- 'event in a guard condition (LRM 11.2, 16.2): allowed, and the guarded assignment acts on the
-- GUARD of the edge, which stays TRUE until the clock is active again (14.7.4, 11.6).
entity top is end entity;
architecture t of top is
  signal clk : bit := '0';
  signal x, y : integer := 0;
begin
  clk <= '1' after 5 ns, '0' after 10 ns, '1' after 15 ns;
  x <= 1 after 5 ns + 1 fs, 2 after 15 ns + 1 fs;
  b : block (clk'event and clk = '1') is
  begin
    y <= guarded x;
  end block;
  process (y) is
  begin
    report "RES y=" & integer'image(y);
  end process;
end architecture;
