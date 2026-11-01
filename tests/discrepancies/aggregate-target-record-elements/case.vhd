-- LRM 10.5.2.1: record signals as elements of an aggregate signal target.
entity top is end entity;
architecture a of top is
  type ru is record x : bit_vector; end record;
  type pair is record p, q : ru; end record;
  signal s, t : ru(x(1 to 2));
begin
  process
    variable v : pair(p(x(1 to 2)), q(x(1 to 2))) := (p => (x => "10"), q => (x => "01"));
  begin
    (s, t) <= v;
    wait for 1 ns;
    report "RES " & to_string(s.x) & " " & to_string(t.x);
    wait;
  end process;
end architecture;
