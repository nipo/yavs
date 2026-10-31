-- yavs-test: std=2019
-- expect: E1502 6:3 no actual for v
-- LRM 6.5.6.2: a generic without default needs an actual; the uses do not cascade
entity e is end;
architecture a of e is
  type pt is protected
    generic (w : integer; v : integer);
    generic map (w => 4);
    function get return integer;
  end protected;
  type pt is protected body
    function get return integer is begin return w; end;
  end protected body;
  shared variable s : pt;
  constant c : integer := s.get;
begin
end;
