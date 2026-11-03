-- 12.3 exempts an implicit predefined operation from the homograph rule: the label hides MINIMUM of
-- the type of its architecture. Both references reject (lrm-checks L179).
entity top is end entity;
architecture a of top is
  type t is (u, v);
begin
  minimum : process begin report "RES accepted"; wait; end process;
end architecture;
