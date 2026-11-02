-- difftest: top=aggregate_target_nested_array std=2008 ref=hand
-- Aggregate targets of arrays of arrays and of arrays of records (#350, LRM 10.5.2.1, 10.6.2.1;
-- lrm-checks L98): each element is a name, a whole row of an array of arrays, positional or by
-- index choice, for variables and signals; an element of an array of records may be a nested
-- record target. A nested aggregate as an element of an array of arrays is E1410 (user,
-- 2026-11-02; tests/diag/sem/expr/agg_target_nested_array.vhd).
-- ref=hand: the first reference simulator gives every row the first row's elements (and
-- ignores index choices), so the .expected is the LRM result.
entity aggregate_target_nested_array is end entity;
architecture t of aggregate_target_nested_array is
  type row is array (0 to 1) of integer;
  type mat is array (0 to 2) of row;
  type mrec is record
    r : row;
    k : integer;
  end record;
  type rows is array (0 to 1) of mrec;
  function mk(b : integer) return mat is
  begin
    return ((b, b + 1), (b + 2, b + 3), (b + 4, b + 5));
  end function;
  signal s0, s1, s2 : row := (0, 0);
begin
  process
    variable p0, p1 : integer;
    variable w0, w1, w2 : row;
    variable m : mat;
  begin
    (w0, w1, w2) := mk(20);
    assert w0 = (20, 21) and w1 = (22, 23) and w2 = (24, 25) severity failure;
    (2 => w0, 1 => m(0), 0 => w2) := mk(40);
    assert w2 = (40, 41) and m(0) = (42, 43) and w0 = (44, 45) severity failure;
    ((w1, p0), (m(1), p1)) := rows'(((50, 51), 52), ((53, 54), 55));
    assert w1 = (50, 51) and p0 = 52 and m(1) = (53, 54) and p1 = 55 severity failure;
    (s0, s1, s2) <= mk(60);
    wait for 1 ns;
    assert s0 = (60, 61) and s1 = (62, 63) and s2 = (64, 65) severity failure;
    (2 => s0, 1 => s2, 0 => s1) <= mk(80) after 1 ns;
    wait for 2 ns;
    assert s1 = (80, 81) and s2 = (82, 83) and s0 = (84, 85) severity failure;
    report "p0=" & integer'image(p0) & " s0=" & integer'image(s0(0)) & " s2=" & integer'image(s2(1));
    wait;
  end process;
end architecture;
