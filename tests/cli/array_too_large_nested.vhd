-- The sizes of nested arrays multiply: 2**32 rows of 2**32 bits is E1406 (#1092).
entity array_too_large_nested is
end entity;

architecture a of array_too_large_nested is
  type big is range 0 to 4294967295;
  type row is array (big) of bit;
  type m is array (big) of row;
  signal s : m;
begin
  process
  begin
    report "unreachable";
    wait;
  end process;
end architecture;
