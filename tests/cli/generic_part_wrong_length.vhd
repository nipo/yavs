-- A generic default with an element of the wrong length (#1105).
entity generic_part_wrong_length is
  generic (
    n : positive := 2
  );
end;
architecture t of generic_part_wrong_length is
  type word_vec_t is array (natural range <>) of bit_vector(0 downto 0);
begin
  b : block
    generic (g : word_vec_t(1 to n) := (others => (1 to 0 => '0')));
  begin
  end block;
end;
