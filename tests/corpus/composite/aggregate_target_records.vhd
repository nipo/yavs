-- difftest: top=aggregate_target_records std=2008
-- Records as elements of an aggregate signal target (#1011, LRM 10.5.2.1): a record signal, an
-- element or a whole array of records, a nested record field, mixed with scalars, with
-- unconstrained fields, several waveform elements, delay mechanisms, a concurrent assignment.
entity aggregate_target_records is end;
architecture t of aggregate_target_records is
  type ru is record x : bit_vector; end record;
  type pair is record p : ru; q : ru; end record;
  type rc is record i : integer; l : bit_vector(0 to 1); end record;
  type mix is record s : bit; r : rc; n : integer; end record;
  type rca is array (natural range <>) of rc;
  type amix is record a : rca(0 to 1); r : rc; end record;
  type ra2 is array (natural range <>) of ru;
  signal a, b : ru(x(1 to 2));
  signal c1, c2 : rc;
  signal s1 : bit; signal n1 : integer;
  signal arr : rca(0 to 3);
  signal arr2 : rca(0 to 1);
  signal u : ra2(0 to 1)(x(1 to 2));
  signal h : pair(p(x(0 to 1)), q(x(5 downto 4)));
  signal ha, hb : ru(x(1 to 2));
begin
  process
    variable v : pair(p(x(1 to 2)), q(x(1 to 2))) := (p => (x => "10"), q => (x => "01"));
    variable m : mix := (s => '1', r => (7, "01"), n => 9);
    variable ca : rca(0 to 1) := ((1, "01"), (2, "10"));
    variable am : amix := (a => ((3, "11"), (4, "00")), r => (5, "10"));
    variable uv : ra2(0 to 1)(x(1 to 2)) := ((x => "11"), (x => "01"));
  begin
    -- issue case
    (a, b) <= v;
    wait for 1 ns;
    report "1 " & to_string(a.x) & " " & to_string(b.x);
    -- mixed scalar and record
    (s1, c1, n1) <= m;
    wait for 1 ns;
    report "2 " & to_string(s1) & " " & integer'image(c1.i) & " " & to_string(c1.l) & " " & integer'image(n1);
    -- array of records, elements records
    (c1, c2) <= ca;
    wait for 1 ns;
    report "3 " & integer'image(c1.i) & to_string(c1.l) & " " & integer'image(c2.i) & to_string(c2.l);
    -- elements of an array of records as targets
    (arr(1), arr(3)) <= ca;
    wait for 1 ns;
    report "4 " & integer'image(arr(1).i) & to_string(arr(1).l) & " " & integer'image(arr(3).i) & to_string(arr(3).l);
    -- a whole array of records element of a record
    (arr2, c2) <= am;
    wait for 1 ns;
    report "5 " & integer'image(arr2(0).i) & integer'image(arr2(1).i) & " " & integer'image(c2.i);
    -- nested
    ((arr(0), arr(1)), c1) <= am;
    wait for 1 ns;
    report "6 " & integer'image(arr(0).i) & integer'image(arr(1).i) & " " & integer'image(c1.i);
    -- unconstrained fields in an array
    (a, b) <= uv;
    wait for 1 ns;
    report "8 " & to_string(a.x) & " " & to_string(b.x);
    -- nested record fields of a signal
    (h.p, h.q) <= v;
    wait for 1 ns;
    report "9 " & to_string(h.p.x) & " " & to_string(h.q.x);
    -- several waveform elements
    (a, b) <= v after 1 ns, pair'(p => (x => "00"), q => (x => "11")) after 2 ns;
    wait for 1 ns;
    report "10 " & to_string(a.x) & " " & to_string(b.x);
    wait for 1 ns;
    report "11 " & to_string(a.x) & " " & to_string(b.x);
    -- swap: value reads the targets
    (a, b) <= pair'(p => b, q => a);
    wait for 1 ns;
    report "12 " & to_string(a.x) & " " & to_string(b.x);
    -- transport and reject
    (a, b) <= transport v after 1 ns;
    wait for 2 ns;
    report "13 " & to_string(a.x) & " " & to_string(b.x);
    (a, b) <= reject 1 ns inertial pair'(p => (x => "11"), q => (x => "11")) after 2 ns;
    wait for 3 ns;
    report "14 " & to_string(a.x) & " " & to_string(b.x);
    wait;
  end process;
  -- concurrent, conditional form
  (ha, hb) <= pair'(p => (x => "01"), q => (x => "10")) after 1 ns;
  process begin
    wait for 2 ns;
    report "c " & to_string(ha.x) & " " & to_string(hb.x);
    wait;
  end process;
end;
