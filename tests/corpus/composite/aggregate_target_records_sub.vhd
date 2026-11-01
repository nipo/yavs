-- difftest: top=aggregate_target_records_sub std=2008
-- Records as elements of an aggregate signal target through signal parameters of a package, a
-- process and a local procedure, a procedure driving signals of its process, and an out port of
-- an instance (#1011, LRM 10.5.2.1).
package aggregate_target_records_pkg is
  type ru is record x : bit_vector; end record;
  type pair is record p, q : ru; end record;
  type rc is record i : integer; l : bit_vector(0 to 1); end record;
  type pc is record p : rc; b : bit; q : rc; end record;
  type rca is array (natural range <>) of rc;
  procedure drive(signal a, b : out ru; v : pair);
  procedure drive2(signal a : out rc; signal s : out bit; signal b : out rc; v : pc);
  procedure driveo(signal o : out rca; v : rca);
end package;
package body aggregate_target_records_pkg is
  procedure drive(signal a, b : out ru; v : pair) is begin
    (a, b) <= v;
  end;
  procedure drive2(signal a : out rc; signal s : out bit; signal b : out rc; v : pc) is begin
    (a, s, b) <= v after 1 ns;
  end;
  procedure driveo(signal o : out rca; v : rca) is begin
    (o(1), o(0)) <= v;
  end;
end package body;

use work.aggregate_target_records_pkg.all;
entity sub4 is
  generic (n : natural := 2);
  port (ao, bo : out ru(x(1 to n)); i : in pair(p(x(1 to n)), q(x(1 to n))));
end;
architecture r of sub4 is begin
  (ao, bo) <= i;
end;

use work.aggregate_target_records_pkg.all;
entity aggregate_target_records_sub is end;
architecture t of aggregate_target_records_sub is
  signal a, b : ru(x(1 to 2));
  signal c, d, po, qo : ru(x(1 to 2));
  signal i : pair(p(x(1 to 2)), q(x(1 to 2)));
  signal r1, r2 : rc;
  signal s1 : bit;
  signal o : rca(0 to 1);
  signal e, f : pair(p(x(1 to 1)), q(x(1 to 3)));
begin
  u : entity work.sub4 port map (ao => po, bo => qo, i => i);
  process
    type pp is record m, n : pair(p(x(1 to 1)), q(x(1 to 3))); end record;
    variable v : pair(p(x(1 to 2)), q(x(1 to 2))) := (p => (x => "10"), q => (x => "01"));
    variable pv : pc := (p => (1, "10"), b => '1', q => (2, "01"));
    variable ca : rca(0 to 1) := ((3, "01"), (4, "10"));
    variable w : pp := (m => (p => (x => "1"), q => (x => "110")), n => (p => (x => "0"), q => (x => "011")));
    procedure local(signal a, b : out ru; v : pair) is begin
      (a, b) <= v;
    end;
    procedure inproc is begin
      (c, d) <= v;
    end;
  begin
    drive(a, b, v);
    wait for 1 ns;
    report "1 " & to_string(a.x) & " " & to_string(b.x);
    drive2(r1, s1, r2, pv);
    wait for 2 ns;
    report "2 " & integer'image(r1.i) & to_string(r1.l) & to_string(s1) & integer'image(r2.i) & to_string(r2.l);
    driveo(o, ca);
    wait for 1 ns;
    report "3 " & integer'image(o(0).i) & integer'image(o(1).i);
    local(a, b, (p => (x => "11"), q => (x => "00")));
    wait for 1 ns;
    report "4 " & to_string(a.x) & " " & to_string(b.x);
    inproc;
    wait for 1 ns;
    report "5 " & to_string(c.x) & " " & to_string(d.x);
    i <= (p => (x => "01"), q => (x => "11"));
    wait for 1 ns;
    report "6 " & to_string(po.x) & " " & to_string(qo.x);
    (e, f) <= w;
    wait for 1 ns;
    report "7 " & to_string(e.p.x) & to_string(e.q.x) & " " & to_string(f.p.x) & to_string(f.q.x);
    wait;
  end process;
end;
