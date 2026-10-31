-- yavs-test: std=2019
-- expect: E1230 11:19 shared variable before the body
-- LRM 14.4.2: the instance needs its body elaborated first
entity e is end;
architecture a of e is
  type pt is protected
    generic (w : integer);
    generic map (w => 4);
    function get return integer;
  end protected;
  shared variable s : pt;
  type pt is protected body
    function get return integer is begin return w; end;
  end protected body;
begin
end;
