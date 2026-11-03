-- yavs-test: std=2008
-- expect: E1101 24:5 label vs block generic
-- expect: E1101 25:5 label vs block port
-- expect: E1101 26:5 label vs block declaration
-- expect: E1101 27:5 label vs block type
-- expect: E1101 32:5 label vs generate parameter
-- expect: E1101 33:5 label vs for generate declaration
-- expect: E1101 38:5 label vs if generate declaration
-- expect: E1101 44:7 label vs case generate declaration
-- expect: E1101 48:5 label vs the implicit GUARD of the block
-- LRM 2008 11.1, 11.8, 12.3: a block or generate statement is a region of its own, with its generics, ports, parameter and declarations (#1084)
entity label_regions is end entity;
architecture a of label_regions is
  signal s : bit;
begin
  b : block
    generic (a_generic : integer := 1);
    generic map (a_generic => 1);
    port (a_port : in bit);
    port map (a_port => s);
    signal in_block : bit;
    type a_type is (u, v);
  begin
    a_generic : process begin wait; end process;
    a_port : process begin wait; end process;
    in_block : process begin wait; end process;
    a_type : process begin wait; end process;
  end block;
  f : for param in 0 to 1 generate
    signal in_for : bit;
  begin
    param : process begin wait; end process;
    in_for : process begin wait; end process;
  end generate;
  i : if true generate
    signal in_if : bit;
  begin
    in_if : process begin wait; end process;
  end generate;
  c : case true generate
    when true =>
      signal in_case : bit;
    begin
      in_case : process begin wait; end process;
    when others =>
  end generate;
  g : block (true) begin
    guard : process begin wait; end process;
  end block;
end architecture;
