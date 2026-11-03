-- yavs-test: std=2019
-- expect: E1300 11:17 range record of another type than the index
-- LRM 5.3.2.2: the type of each discrete range is the type of its index (#1086)
entity e is end entity;
architecture a of e is
  type color is (red, green, blue);
  type cv is array (color range <>) of bit;
  signal s : bit_vector(7 downto 0);
  constant r : s'range'record := s'range'value;
  constant ok : cv(color) := (others => '0');
  signal y : cv(r);
begin
end architecture;
