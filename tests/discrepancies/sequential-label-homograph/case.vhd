-- A sequential label is declared in the process (10.1), so it may not repeat a variable of it (12.3).
entity top is end entity;
architecture a of top is
begin
  p : process
    variable v : bit;
  begin
    v : v := '1';
    report "RES accepted";
    wait;
  end process;
end architecture;
