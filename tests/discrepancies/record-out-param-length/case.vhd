-- LRM 6.5.7.1, 10.6.2.1: an unconstrained record formal has its actual's bounds, so a value of
-- another field length is a length error, as for an array formal.
entity top is end entity;
architecture a of top is
  type ru is record x : bit_vector; end record;
  function four return bit_vector is begin return "1111"; end;
  procedure pa (p : out ru) is begin p := (x => four); end;
begin
  process
    variable s : ru(x(1 to 3));
  begin
    report "RES before";
    pa(s);
    report "RES after " & to_string(s.x);
    wait;
  end process;
end architecture;
