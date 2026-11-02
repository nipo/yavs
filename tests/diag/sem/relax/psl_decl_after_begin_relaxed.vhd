-- yavs-test: std=2008 flags=--relaxed
-- expect: W1241 13:12 `--relaxed` (R8) accepts a PSL declaration after `begin`
-- expect: W1241 15:12 the same, and its name is still a declaration of the region
-- expect: E1101 15:12 property `k` is a homograph of constant `k` (#1077)
-- LRM 2008 3.3.2, 11.1; 6.1, 12.3
entity e is
end entity;
architecture t of e is
  signal clk, a : bit := '0';
  constant k : integer := 1;
  default clock is clk'event and clk = '1';
begin
  sequence s is {a; a};
  c1: cover s report "s";
  property k is always a;
end architecture;
