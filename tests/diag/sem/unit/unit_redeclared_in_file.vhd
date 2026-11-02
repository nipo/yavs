-- yavs-test: std=2008
-- expect: W0005 10:14 architecture `a` of `d` is declared a second time in this file (LRM 13.1)
-- expect: E1100 12:3 `zz` is not declared
-- LRM 13.5: a unit of the same name later in the file replaces the earlier one, which is analyzed too (#810, VESTs tc3064)
entity d is
end entity;
architecture a of d is
begin
end architecture;
architecture a of d is
begin
  zz <= '1';
end architecture;
