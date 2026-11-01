-- LRM 9.3.3.3: a named aggregate as an actual of a declared operator with a constrained formal.
-- A predefined operator's operand takes the index subtype's direction (both references); here
-- the formal's subtype is a context as for any function call.
entity top is end entity;
architecture a of top is
  type a5 is array (3 downto 0) of integer;
  function "+" (l, r : a5) return integer is begin return l(3) * 100 + r(3); end;
begin
  process
  begin
    report "RES " & integer'image((3 => 2, 2 => 0, 1 => 0, 0 => 0) + (3 => 3, 2 => 0, 1 => 0, 0 => 0));
    wait;
  end process;
end architecture;
