-- yavs-test: std=2008
-- expect: E1230 6:3 no body, reported once
-- LRM 5.6.3
entity e is end;
architecture a of e is
  type pt is protected
    impure function get return integer;
  end protected;
begin
end;
