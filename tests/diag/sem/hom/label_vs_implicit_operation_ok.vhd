-- yavs-test: std=2008
-- expect: none
-- LRM 2008 12.3: the implicit declaration of a predefined operation is the one exception to the homograph rule, so a label hides it (maintainer ruling 2026-11-03, lrm-checks L179, #1084)
entity label_op is
end entity;
architecture a of label_op is
  type t is (x, y);
begin
  minimum : process begin wait; end process;
  to_string : process begin wait; end process;
  p : process
    type q is (r, s);
  begin
    minimum : null;
    wait;
  end process;
end architecture;
