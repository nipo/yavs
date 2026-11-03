-- The loop label is declared in the process (10.1), the parameter in the loop statement (12.1 i):
-- two regions, so no homograph (12.3).
entity top is end entity;
architecture a of top is
begin
  p : process
  begin
    x : for x in 0 to 0 loop
      report "RES accepted";
    end loop;
    wait;
  end process;
end architecture;
