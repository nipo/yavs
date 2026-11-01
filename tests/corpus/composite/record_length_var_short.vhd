-- difftest: top=record_length_var_short std=2008 exit=1 ignore=path
-- ignore=path: sim1 also names the process of a length error outside a subprogram.
-- A shorter value is an error too, not a read past its end (LRM 10.6.2.1, #1005).
library ieee; use ieee.std_logic_1164.all;
entity record_length_var_short is end;
architecture a of record_length_var_short is
  type rec_t is record v : std_logic_vector; n : integer; end record;
begin
  process
    variable s : rec_t(v(1 to 4));
    variable o : rec_t(v(1 to 3)) := (v => "101", n => 7);
  begin
    s := o;
    report "not reached";
    wait;
  end process;
end;
