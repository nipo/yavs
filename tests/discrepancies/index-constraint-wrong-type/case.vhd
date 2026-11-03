-- The type of each discrete range of an index constraint is the type of its index (5.3.2.2).
entity top is end entity;
architecture a of top is
  type nv is array (natural range <>) of bit;
  type e is (p, q, r);
  signal s : nv(e);
begin
end architecture;
