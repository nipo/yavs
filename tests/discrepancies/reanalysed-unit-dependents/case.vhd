-- An architecture analysed again after a unit that names it (LRM 13.5): the unit is obsolete and is
-- analysed again before it is used, so the last architecture runs. One file here; analysing the two
-- architectures from two files gives the same on yavs, see the catalogue row for the references.
entity e is end entity;
architecture a of e is
begin
  process begin report "RES first"; wait; end process;
end architecture;

entity top is end entity;
architecture t of top is
begin
  u : entity work.e(a);
end architecture;

architecture a of e is
begin
  process begin report "RES last"; wait; end process;
end architecture;
