-- An entity and its architecture are one region (12.1 a): a statement label of the entity may not
-- repeat a declaration of the architecture (11.1, 12.3).
entity top is
begin
  x : process begin wait; end process;
end entity;

architecture a of top is
  signal x : bit;
begin
  p : process begin report "RES accepted"; wait; end process;
end architecture;
