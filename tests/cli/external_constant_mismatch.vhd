entity external_constant_mismatch is end;
architecture a of external_constant_mismatch is
begin
  b1 : block is
    constant c1 : string := "hello";
  begin
  end block;
  process
  begin
    assert << constant b1.c1 : string(1 to 6) >> = "hello";
    wait;
  end process;
end;
