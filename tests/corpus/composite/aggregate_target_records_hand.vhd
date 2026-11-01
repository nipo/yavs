-- difftest: top=aggregate_target_records_hand std=2008 ref=hand
-- Aggregate targets with composite elements the first reference simulator does not run correctly
-- (#1011, LRM 10.5.2.1, 10.6.2.1): it assigns record fields by position, gives every row of a matrix
-- the first row's elements, fails 2008 pieces of an array of records with a length error, ignores
-- a disconnection specification below an aggregate target and starts a waveform with a null
-- element late. The second crashes on these forms. The values below are the LRM's.
package aggregate_target_records_hand_pkg is
  type rc is record i : integer; l : bit_vector(0 to 1); end record;
  type rca is array (natural range <>) of rc;
  type wrap is record r : rc; s : rca(0 to 1); end record;
  type wp is record a, b : wrap; end record;
  procedure pf(signal w : out wrap; v : rc; vs : rca);
  procedure pg(signal o : out rca; v : rca);
  procedure ph(signal o : inout wp; v : wp);
end package;
package body aggregate_target_records_hand_pkg is
  procedure pf(signal w : out wrap; v : rc; vs : rca) is begin
    (w.r, w.s(1), w.s(0)) <= rca'(v, vs(0), vs(1));
  end;
  procedure pg(signal o : out rca; v : rca) is begin
    (o(2 to 3), o(0 to 1)) <= v;
  end;
  procedure ph(signal o : inout wp; v : wp) is begin
    (o.b, o.a) <= v;
  end;
end package body;

library ieee; use ieee.std_logic_1164.all;
use work.aggregate_target_records_hand_pkg.all;
entity aggregate_target_records_hand is end;
architecture t of aggregate_target_records_hand is
  type pair is record p, q : rc; end record;
  type mat is array (0 to 1, 0 to 1) of rc;
  type mbv is array (0 to 1, 0 to 1) of bit_vector(0 to 1);
  type ru is record x : bit_vector; end record;
  type mru is array (0 to 1, 0 to 1) of ru(x(1 to 2));
  type lc is record x : std_ulogic; y : std_ulogic; end record;
  type la is array (natural range <>) of lc;
  function res(v : la) return lc is begin
    if v'length = 0 then return ('Z', 'Z'); end if;
    return v(v'left);
  end;
  subtype rlc is res lc;
  type lp is record p, q : lc; end record;
  signal a, b, c, d, e, f : rc;
  signal arr : rca(0 to 3);
  signal w : wrap;
  signal pp : wp;
  signal v1, v2, v3, v4 : bit_vector(0 to 1);
  signal u1, u2, u3, u4 : ru(x(1 to 2));
  signal g1, g2 : rlc bus;
  signal n1, n2 : rlc bus;
  signal guard_on : std_ulogic := '0';
  disconnect g1, g2 : rlc after 2 ns;
begin
  blk : block (guard_on = '1') begin
    (g1, g2) <= guarded lp'(p => ('1', '0'), q => ('0', '1'));
  end block;
  process
    variable v : pair := (p => (1, "10"), q => (2, "01"));
    variable va, vb : rc;
    variable a4 : rca(0 to 3) := ((1, "00"), (2, "01"), (3, "10"), (4, "11"));
    variable vs : rca(0 to 3);
    variable mm : mat := (((1, "00"), (2, "01")), ((3, "10"), (4, "11")));
    variable vc, vd, ve, vf : rc;
    variable mb : mbv := (("00", "01"), ("10", "11"));
    variable w1, w2, w3, w4 : bit_vector(0 to 1);
    variable mu : mru := (((x => "00"), (x => "01")), ((x => "10"), (x => "11")));
    variable wv : wp := (a => (r => (1, "00"), s => ((2, "01"), (3, "10"))),
                         b => (r => (4, "11"), s => ((5, "10"), (6, "01"))));
    variable lv : lp := (p => ('1', '0'), q => ('0', '1'));
  begin
    -- named fields select their field
    (q => va, p => vb) := v;
    report "named v " & integer'image(va.i) & " " & integer'image(vb.i);
    (q => a, p => b) <= v;
    wait for 1 ns;
    report "named s " & integer'image(a.i) & " " & integer'image(b.i);
    -- 2008 pieces of an array of records
    (vs(2 to 3), vs(0 to 1)) := a4;
    report "piece v " & integer'image(vs(0).i) & integer'image(vs(1).i) & integer'image(vs(2).i) & integer'image(vs(3).i);
    (arr(2 to 3), arr(0 to 1)) <= a4;
    wait for 1 ns;
    report "piece s " & integer'image(arr(0).i) & integer'image(arr(1).i) & integer'image(arr(2).i) & integer'image(arr(3).i);
    -- range and element choices
    (0 to 1 => vs(1 to 2), 2 => vs(3), 3 => vs(0)) := a4;
    report "range v " & integer'image(vs(0).i) & integer'image(vs(1).i) & integer'image(vs(2).i) & integer'image(vs(3).i);
    (0 to 1 => arr(1 to 2), 2 => arr(3), 3 => arr(0)) <= a4;
    wait for 1 ns;
    report "range s " & integer'image(arr(0).i) & integer'image(arr(1).i) & integer'image(arr(2).i) & integer'image(arr(3).i);
    -- matrix of records
    ((vc, vd), (ve, vf)) := mm;
    report "mat v " & integer'image(vc.i) & integer'image(vd.i) & integer'image(ve.i) & integer'image(vf.i);
    ((c, d), (e, f)) <= mm;
    wait for 1 ns;
    report "mat s " & integer'image(c.i) & integer'image(d.i) & integer'image(e.i) & integer'image(f.i) & to_string(f.l);
    -- matrix of records with unconstrained fields
    ((u1, u2), (u3, u4)) <= mu;
    wait for 1 ns;
    report "mat u " & to_string(u1.x) & to_string(u2.x) & to_string(u3.x) & to_string(u4.x);
    -- matrix of logic arrays
    ((w1, w2), (w3, w4)) := mb;
    report "mat l v " & to_string(w1) & to_string(w2) & to_string(w3) & to_string(w4);
    ((v1, v2), (v3, v4)) <= mb;
    wait for 1 ns;
    report "mat l s " & to_string(v1) & to_string(v2) & to_string(v3) & to_string(v4);
    -- signal parameters: parts of a record, pieces, whole records
    pf(w, (9, "01"), ((8, "10"), (7, "11")));
    wait for 1 ns;
    report "par f " & integer'image(w.r.i) & integer'image(w.s(0).i) & integer'image(w.s(1).i);
    pg(arr, a4);
    wait for 1 ns;
    report "par g " & integer'image(arr(0).i) & integer'image(arr(1).i) & integer'image(arr(2).i) & integer'image(arr(3).i);
    ph(pp, wv);
    wait for 1 ns;
    report "par h " & integer'image(pp.a.r.i) & integer'image(pp.b.s(1).i) & integer'image(pp.a.s(0).i);
    -- guarded: the guard goes false, each leaf disconnects after the specified time
    guard_on <= '1';
    wait for 1 ns;
    report "guard on " & to_string(g1.x) & to_string(g2.y);
    guard_on <= '0';
    wait for 1 ns;
    report "guard off, 1 ns " & to_string(g1.x) & to_string(g2.y);
    wait for 2 ns;
    report "guard off, 3 ns " & to_string(g1.x) & to_string(g2.y);
    -- a null waveform element
    (n1, n2) <= lv, null after 1 ns;
    wait for 0 ns;
    report "null 0 " & to_string(n1.x) & to_string(n2.y);
    wait for 2 ns;
    report "null 2 " & to_string(n1.x) & to_string(n2.y);
    wait;
  end process;
end;
