-- yavs-test: std=2008
-- expect: E1101 11:3 label vs implicit operation
-- expect: E1101 12:3 label vs implicit operation
-- expect: E1101 16:5 label vs implicit operation
-- LRM 2008 12.3 exempts an implicit predefined operation; yavs still rejects, open for the maintainer (lrm-checks L179, #1084)
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
