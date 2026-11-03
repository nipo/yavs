-- The same for a generic of the guarded block: the implicit signal GUARD (11.2) and the generic are
-- homographs in one region (12.3).
entity top is
end entity;

architecture a of top is
  signal c : boolean;
begin
  b : block (c)
    generic (guard : integer := 1);
    generic map (guard => 1);
  begin
    p : process begin report "RES accepted"; wait; end process;
  end block;
end architecture;
