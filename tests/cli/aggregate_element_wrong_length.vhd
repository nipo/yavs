-- A folded aggregate with a null field of the wrong length: a length error (#1105).
entity aggregate_element_wrong_length is
end;
architecture t of aggregate_element_wrong_length is
  type cell is record
    ok : boolean;
    b : bit_vector(2 downto 0);
  end record;
  type cells is array (1 to 2) of cell;
  signal cs : cells;
begin
  process begin
    cs <= ((false, "111"), (true, ""));
    wait;
  end process;
end;
