-- Two processes wake each other through two fields of a record that share a kernel signal
-- (#686). Nothing asks the history of the record, so it keeps none: the delta cycle limit names
-- the whole record as still changing; with every history kept, the fields.
library ieee;
use ieee.std_logic_1164.all;
entity stuck_record_loop is end entity;
architecture a of stuck_record_loop is
  type rec is record
    a, b : std_logic;
  end record;
  signal r : rec := ('0', '0');
begin
  p : process (r.b)
  begin
    r.a <= not r.b;
  end process;
  q : process (r.a)
  begin
    r.b <= r.a;
  end process;
end architecture;
