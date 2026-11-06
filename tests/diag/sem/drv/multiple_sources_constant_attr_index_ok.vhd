-- yavs-test: std=2008
-- expect: none
-- LRM 9.4.3 (an array attribute of a constant is globally static), 8.1, 14.7.2
entity e is end entity;
architecture a of e is
  type int_vector is array (natural range <>) of integer;
  constant c : int_vector := (1, 2, 3);
  signal s : bit_vector(0 to 3);
begin
  p1: process begin s(0) <= '1'; wait; end process;
  p2: process begin s(c'length) <= '1'; wait; end process;
  p3: process
    constant d : int_vector := (1, 2);
  begin
    s(d'high) <= '1';
    wait;
  end process;
end architecture;
