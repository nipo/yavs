-- yavs-test: std=2008
-- expect: E1101 12:3 entity statement label vs port
-- expect: E1101 25:10 signal vs entity statement label
-- expect: E1101 27:3 architecture label vs generic
-- expect: E1101 28:3 architecture label vs port
-- expect: E1101 29:3 architecture label vs entity constant
-- expect: E1101 30:3 architecture label vs entity statement label
-- LRM 2008 11.1, 12.1 a), 12.3: an entity and its architecture are one declarative region (#1084)
entity in_entity is
  port (a_port : in bit);
begin
  a_port : process begin wait; end process;
end entity;

entity across is
  generic (a_generic : integer := 1);
  port (b_port : in bit);
  constant a_const : bit := '0';
begin
  ent_vs_sig : process begin wait; end process;
  ent_vs_lbl : process begin wait; end process;
end entity;

architecture a of across is
  signal ent_vs_sig : bit;
begin
  a_generic : process begin wait; end process;
  b_port : process begin wait; end process;
  a_const : process begin wait; end process;
  ent_vs_lbl : process begin wait; end process;
end architecture;
