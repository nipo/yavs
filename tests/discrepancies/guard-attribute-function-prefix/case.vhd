-- A guard condition on 'last_value of a signal: GUARD is evaluated again in every cycle in which
-- that signal is active (LRM 14.7.4: the condition contains a reference to it).
entity top is end entity;
architecture t of top is
  signal c : bit := '0';
begin
  c <= '1' after 5 ns, '0' after 10 ns;
  b : block (c'last_value = '0') is
  begin
    process (guard) is
    begin
      report "RES guard=" & boolean'image(guard) & " at " & time'image(now);
    end process;
  end block;
end architecture;
