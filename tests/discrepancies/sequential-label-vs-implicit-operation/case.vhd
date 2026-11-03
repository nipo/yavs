-- 12.3 exempts an implicit predefined operation from the homograph rule: MINIMUM of the type would be
-- hidden by the label. A concurrent label there is rejected by every reference (lrm-checks L179).
entity top is end entity;
architecture a of top is
begin
  p : process
    type t is (u, v);
  begin
    minimum : report "RES accepted";
    wait;
  end process;
end architecture;
