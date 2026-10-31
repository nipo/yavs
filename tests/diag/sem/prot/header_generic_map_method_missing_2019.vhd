-- yavs-test: std=2019
-- expect: E1230 9:14 method without body
-- LRM 5.6.3: every method has a body
entity e is end;
architecture a of e is
  type pt is protected
    generic (w : integer);
    generic map (w => 4);
    function get return integer;
  end protected;
  type pt is protected body
  end protected body;
begin
end;
