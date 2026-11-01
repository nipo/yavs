package p is
  type rc is record i : integer; end record;
end package;
use work.p.all;
entity inner is end;
architecture a of inner is
  signal s : rc;
begin
end;
entity external_same_type_name is end;
architecture t of external_same_type_name is
  type rc is record i : integer; end record;
  signal q : rc;
begin
  ei : entity work.inner;
  process begin
    q <= <<signal .external_same_type_name.ei.s : rc>>;
    wait;
  end process;
end;
