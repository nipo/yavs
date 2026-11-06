-- A transaction that keeps the value (`s <= s;`) makes `s` active: the guard is evaluated again
-- (LRM 14.7.4), GUARD of `s'event` turns FALSE and the guarded assignment ignores x = 2 (11.6).
entity top is end entity;
architecture t of top is
  signal s : bit := '0';
  signal x, y : integer := 0;
begin
  s <= '1' after 1 ns, '1' after 2 ns, '0' after 4 ns;
  x <= 1 after 1 ns, 2 after 2 ns, 3 after 4 ns;
  b : block (s'event) is
  begin
    y <= guarded x;
    process (guard) is
    begin
      report "RES guard=" & boolean'image(guard) & " at " & time'image(now);
    end process;
  end block;
  process (y) is
  begin
    report "RES y=" & integer'image(y);
  end process;
end architecture;
