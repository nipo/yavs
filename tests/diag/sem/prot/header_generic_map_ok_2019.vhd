-- yavs-test: std=2019
-- expect: none
-- LRM 5.6.2 (2019): near miss; the generic visible in the body
entity e is end;
architecture a of e is
  type pt is protected
    generic (w : integer);
    generic map (w => 4);
    function get return integer;
  end protected;
  type pt is protected body
    function get return integer is begin return w; end;
  end protected body;
  shared variable s : pt;
begin
end;
