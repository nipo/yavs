-- yavs-test: std=2019
-- expect: E1501 14:17 instantiating a type that is not uninstantiated
-- LRM 5.6.2 (2019): with a generic map the type is already an instance
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
  type q is new pt generic map (w => 5);
begin
end;
