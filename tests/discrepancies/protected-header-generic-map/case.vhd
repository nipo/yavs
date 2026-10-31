-- A generic clause and a generic map in a protected type header (2019 5.6.2).
entity top is end;
architecture a of top is
  type pt is protected
    generic (w : integer);
    generic map (w => 4);
    impure function get return integer;
  end protected;
  type pt is protected body
    impure function get return integer is begin return w; end;
  end protected body;
  shared variable s : pt;
begin
  process begin report "RES " & integer'image(s.get); wait; end process;
end;
