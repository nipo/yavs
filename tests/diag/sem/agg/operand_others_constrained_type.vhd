-- yavs-test: std=2008
-- expect: E1325 8:31 an operand of a predefined operator is no constrained context
-- LRM 9.3.3.3: `others` needs an index range from the context, also for a constrained array type.
entity e is end;
architecture a of e is
  type t is array (3 downto 0) of integer;
  constant c : t := (others => 0);
  constant b : boolean := c = (others => 1);
begin
end;
