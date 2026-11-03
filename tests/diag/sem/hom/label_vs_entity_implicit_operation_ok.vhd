-- yavs-test: std=2008
-- expect: none
-- LRM 2008 12.1, 12.3: the operation is hidden by the label; both references reject, open for the maintainer (lrm-checks L179, #1084)
entity label_op_ent is
  type t is (x, y);
end entity;
architecture a of label_op_ent is
begin
  minimum : process begin wait; end process;
end architecture;
