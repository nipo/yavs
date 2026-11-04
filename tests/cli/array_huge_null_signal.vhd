-- Arrays with a null dimension have no elements however large the others are: no E1406 (#1092).
entity array_huge_null_signal is
end entity;

architecture a of array_huge_null_signal is
  type big is range -9223372036854775807 - 1 to 9223372036854775807;
  type m is array (big range <>, natural range <>) of bit;
  signal s : m(big, 1 to 0);
  type row is array (natural range <>) of bit;
  type nested is array (big range <>) of row(1 to 0);
  signal n : nested(big);
begin
  process
  begin
    report "huge null arrays elaborate";
    wait;
  end process;
end architecture;
