-- LRM 8.7: the object an external name denotes has the name's subtype, so another length of a
-- constant is an error when the name is bound.
entity top is end entity;
architecture a of top is
begin
  b1 : block is
    constant c1 : string := "hello";
  begin
  end block;
  process
  begin
    report "RES before";
    assert << constant b1.c1 : string(1 to 6) >> = "hello" severity note;
    report "RES after";
    wait;
  end process;
end architecture;
