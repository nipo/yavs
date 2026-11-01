entity external_runtime_constant_mismatch is end;
architecture a of external_runtime_constant_mismatch is
begin
  b1 : block is
    constant c2 : string := b1'path_name;
  begin
  end block;
  process
  begin
    assert << constant b1.c2 : string(1 to 2) >> = "ab" severity note;
    wait;
  end process;
end;
