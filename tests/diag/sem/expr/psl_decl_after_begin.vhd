-- yavs-test: std=2008
-- expect: E1241 13:12 a PSL sequence declaration after `begin`
-- expect: E1241 16:14 a PSL property declaration in the statement part of a block
-- help: move the declaration before `begin`
-- LRM 2008 3.3.2 (PSL declarations are block declarative items), 11.1 (a concurrent statement is
-- only a PSL directive); user decision 2026-11-02 (rule 47 review)
entity e is
end entity;
architecture t of e is
  signal clk, a : bit := '0';
  default clock is clk'event and clk = '1';
begin
  sequence s is {a; a};
  c1: cover s report "s";
  b : block is begin
    property p is always a;
    a1: assert p;
  end block;
end architecture;
