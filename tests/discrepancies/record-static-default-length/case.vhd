-- LRM 5.3.2.2, 6.5.2: a static record default whose field has another length than the constrained
-- formal. A plain array default of this kind is rejected at analysis by yavs and sim1.
entity top is end entity;
architecture a of top is
  type ru is record x : bit_vector; end record;
  constant d : ru(x(1 to 4)) := (x => "1111");
  procedure pa (p : ru(x(1 to 3)) := d) is begin report "RES in " & integer'image(p.x'length); end;
begin
  process
  begin
    report "RES before";
    pa;
    report "RES after";
    wait;
  end process;
end architecture;
