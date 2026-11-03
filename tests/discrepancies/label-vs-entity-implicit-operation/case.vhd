-- An entity and its architecture are one region (12.1), and 12.3 exempts an implicit predefined
-- operation: the label hides MINIMUM of the entity's type.
entity top is
  type t is (u, v);
end entity;
architecture a of top is
begin
  minimum : process begin report "RES accepted"; wait; end process;
end architecture;
