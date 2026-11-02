-- yavs-test: std=2008
-- expect: E1101 13:12 sequence `pa` is a homograph of constant `PA` in the same region
-- expect: E1101 14:3 property `clk` is a homograph of port `clk` of the entity
-- expect: E1101 16:12 two PSL declarations named `pq`
-- LRM 2008 6.1 (a PSL sequence or property declaration is a declaration), 12.3 (#1077)
entity psl_dup is
  port (clk : in bit);
end entity;
architecture t of psl_dup is
  signal a : bit := '0';
  constant PA : string := "10";
  default clock is clk'event and clk = '1';
  sequence pa is {a};
  property clk is always a;
  property pq is always a;
  sequence pq is {a; a};
begin
  c1: cover {a} report "x";
end architecture;
