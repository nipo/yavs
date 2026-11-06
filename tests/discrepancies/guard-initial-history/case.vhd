-- GUARD's history at 0 ns: it is set to its condition during initialization, which is no
-- simulation cycle, so it had no event: 'last_event is TIME'HIGH and 'last_value its current
-- value (LRM 14.7.5.2, 16.2).
entity top is end entity;
architecture t of top is
  signal c : bit := '0';
begin
  b : block (c = '0') is
  begin
    process is
    begin
      report "RES guard=" & boolean'image(guard) & " last_value=" & boolean'image(guard'last_value)
        & " no_event=" & boolean'image(guard'last_event = time'high);
      wait;
    end process;
  end block;
end architecture;
