-- yavs-test: std=2008
-- expect: E1301 23:11 the sequential label hides MAXIMUM of the process type
-- expect: E1301 26:8 the label of the architecture hides MINIMUM of its type
-- expect: E1301 27:9 the label of the architecture hides MINIMUM of the entity type
-- LRM 2008 12.3, 12.1: the label hides the implicit operation of the type of its region, and a call
-- after the label no longer finds it; an operation of a nested region stays (maintainer ruling
-- 2026-11-03, lrm-checks L179, #1084)
entity label_hides is
  type et is (e1, e2);
end entity;
architecture a of label_hides is
  type t is (u, v);
  signal s : t;
  signal es : et;
  signal c : boolean;
begin
  minimum : process begin wait; end process;
  other : process
    type q is (r, w);
    variable vq : q;
  begin
    maximum : null;
    vq := maximum(r, w);
    wait;
  end process;
  s <= minimum(u, v);
  es <= minimum(e1, e2);
  nested : process
    type q is (r, w);
    variable vq : q;
  begin
    vq := minimum(r, w);
    wait;
  end process;
end architecture;
