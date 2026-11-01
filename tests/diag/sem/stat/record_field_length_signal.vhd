-- yavs-test: std=2008
-- expect: E1405 8:12 a signal target as well
-- LRM 10.5.2.1: a static record value in a waveform.
entity e is end;
architecture a of e is
  type ru is record x : bit_vector; end record;
  signal t : ru(x(1 to 3));
begin t <= (x => "1111");
end;
