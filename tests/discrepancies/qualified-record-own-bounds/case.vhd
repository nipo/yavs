-- LRM 9.3.5: a qualified expression converts to the subtype of its type mark, so a constrained
-- record subtype gives the field the mark's bounds, not the operand's.
entity top is end entity;
architecture a of top is
  type ru is record x : bit_vector; end record;
  subtype r3 is ru(x(1 to 3));
  type acc is access ru;
begin
  process
    variable s : ru(x(3 downto 1));
    variable p : acc;
  begin
    p := new r3'(s);
    report "RES " & integer'image(p.all.x'left);
    wait;
  end process;
end architecture;
