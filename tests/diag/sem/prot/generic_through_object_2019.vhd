-- yavs-test: std=2019
-- expect: E1105 18:29 a constant generic is not a method
-- expect: E1105 19:29 a subprogram generic is not a method
-- LRM 5.6.2 (2019): only methods are visible through an object
entity e is end;
architecture a of e is
  function plus1(x : integer) return integer is begin return x + 1; end;
  type pt is protected
    generic (w : integer; function f(x : integer) return integer);
    generic map (w => 4, f => plus1);
    impure function get return integer;
  end protected;
  type pt is protected body
    impure function get return integer is begin return f(w); end;
  end protected body;
  shared variable s : pt;
  constant ok : integer := s.get;
  constant c : integer := s.w;
  constant d : integer := s.f(1);
begin
end;
