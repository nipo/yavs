-- yavs-test: std=2008
-- Near miss: the same lengths, other bounds; an open target takes the value's.
entity e is end;
architecture a of e is
  type ru is record x : bit_vector; end record;
  constant c : ru(x(3 downto 1)) := (x => "101");
  constant d : ru := (x => "1111");
  constant f : ru(x(1 to 3)) := c;
begin
end;
