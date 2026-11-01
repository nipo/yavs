-- LRM 5.3.2.2, 6.5.7.1: an actual whose field has another length than a constrained record formal.
entity top is end entity;
architecture a of top is
  type ru is record x : bit_vector; end record;
  procedure pa (p : in ru(x(1 to 3))) is begin report "RES in " & integer'image(p.x'length); end;
begin
  process
    variable o : ru(x(1 to 4));
  begin
    report "RES before";
    pa(o);
    report "RES after";
    wait;
  end process;
end architecture;
