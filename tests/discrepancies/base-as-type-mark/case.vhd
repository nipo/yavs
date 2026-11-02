-- LRM 16.2.2: T'BASE "is allowed only as the prefix of the name of another attribute", so it is no
-- type mark of a qualified expression (`word'base'(...)`, `integer'base'(5)`).
entity top is end entity;
architecture a of top is
  subtype word is bit_vector(3 downto 0);
begin
  process
    variable b : bit_vector(3 downto 0);
    variable i : integer;
  begin
    b := word'base'("1010");
    i := integer'base'(5);
    report "RES " & to_string(b) & " " & integer'image(i);
    wait;
  end process;
end architecture;
