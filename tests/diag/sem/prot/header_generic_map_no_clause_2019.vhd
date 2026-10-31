-- yavs-test: std=2019
-- expect: E1501 7:18 a generic map with no generic clause
-- LRM 5.6.2 (2019): a generic map aspect only follows a generic clause
entity e is end;
architecture a of e is
  type pt is protected
    generic map (w => 4);
    function get return integer;
  end protected;
begin
end;
