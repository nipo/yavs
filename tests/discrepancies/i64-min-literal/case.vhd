-- LRM 9.5, 9.3.6, 5.2.4.1: 9223372036854775808 fs is a value before the minus applies, too large for 64 bits.
entity top is end entity;
architecture a of top is
  constant k : time := -9223372036854775808 fs;
begin
  process begin report "RES ok"; wait; end process;
end architecture;
