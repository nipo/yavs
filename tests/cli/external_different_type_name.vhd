package p2 is
  type rc is record i : integer; end record;
end package;
use work.p2.all;
entity inner2 is end;
architecture a of inner2 is
  signal s : rc;
begin
end;
entity external_different_type_name is end;
architecture t of external_different_type_name is
  type other is record i : integer; end record;
  signal q : other;
begin
  ei : entity work.inner2;
  process begin
    q <= <<signal .external_different_type_name.ei.s : other>>;
    wait;
  end process;
end;
