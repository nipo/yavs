-- yavs-test: std=2008
-- expect: E1101 29:12 signal vs implicit GUARD
-- expect: E1101 33:14 generic vs implicit GUARD
-- expect: E1101 38:11 port vs implicit GUARD
-- expect: E1101 43:14 constant vs implicit GUARD
-- expect: E1101 47:10 type vs implicit GUARD
-- expect: E1101 51:14 function vs implicit GUARD
-- expect: E1101 55:11 alias vs implicit GUARD
-- LRM 2008 11.2, 12.3: the implicit signal GUARD is not a predefined operation, so an explicit
-- homograph in the block is an error, not a hiding (#1085)
entity guard_homograph is end entity;
architecture a of guard_homograph is
  signal c : boolean;
  signal d : bit;
begin
  ok_unguarded : block
    signal guard : boolean;
  begin
  end block;
  ok_nested : block (c)
    signal x : bit;
  begin
    inner : block (c)
      signal x : bit;
    begin
    end block;
  end block;
  gs : block (c)
    signal guard : boolean;
  begin
  end block;
  gg : block (c)
    generic (guard : integer := 1);
    generic map (guard => 1);
  begin
  end block;
  gp : block (c)
    port (guard : in bit);
    port map (guard => d);
  begin
  end block;
  gc : block (c)
    constant guard : boolean := true;
  begin
  end block;
  gt : block (c)
    type guard is (u, v);
  begin
  end block;
  gf : block (c)
    function guard return boolean is begin return true; end function;
  begin
  end block;
  ga : block (c)
    alias guard : boolean is c;
  begin
  end block;
end architecture;
