-- LRM 10.6.2.1: matching elements, also of the elements of an array of unconstrained arrays.
entity top is end entity;
architecture a of top is
  type aa is array (natural range <>) of bit_vector;
begin
  process
    variable a : aa(0 to 1)(1 to 3);
    variable b : aa(0 to 1)(1 to 4);
  begin
    report "RES before";
    a := b;
    report "RES after " & integer'image(a(0)'length);
    wait;
  end process;
end architecture;
