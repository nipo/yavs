-- yavs-test: std=2008
-- expect: none
-- LRM 2008 11.8: an alternative label is declared in the block implied at elaboration, so a declaration of the
-- alternative with the same name is accepted; both references accept (maintainer ruling 2026-11-03, lrm-checks L179, #1084)
entity gen_alt is end entity;
architecture a of gen_alt is
  constant c : boolean := true;
begin
  g : if x : c generate
    signal x : bit;
  begin
  end generate;
  h : case 1 generate
    when y : 1 =>
      signal y : bit;
    begin
    end;
    when others =>
    begin
    end;
  end generate;
end architecture;
