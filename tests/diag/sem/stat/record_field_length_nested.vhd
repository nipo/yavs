-- yavs-test: std=2008
-- expect: E1405 9:36 a nested field, also in a declaration
-- LRM 5.3.2.2, 10.6.2.1: a static record value whose nested field has another length.
entity e is end;
architecture a of e is
  type ru is record x : bit_vector; end record;
  type rn is record r : ru; end record;
  constant ok : rn(r(x(1 to 2))) := (r => (x => "11"));
  constant c : rn(r(x(1 to 3))) := (r => (x => "1111"));
begin
end;
