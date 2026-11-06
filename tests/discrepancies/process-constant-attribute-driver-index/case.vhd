-- LRM 9.4.3, 8.1, 14.7.2: two processes drive distinct elements of an unresolved signal; one
-- indexes it by 'length of a constant declared in the process, whose initial value is globally
-- (not locally) static.
entity top is end entity;
architecture a of top is
  type int_vector is array (natural range <>) of integer;
  constant a_c : int_vector := (1, 2);
  signal s : bit_vector(0 to 3) := "0000";
begin
  p0: process
  begin
    s(0) <= '1';
    wait;
  end process;

  p1: process
    constant l_c : int_vector := a_c & a_c(0);
  begin
    s(l_c'length) <= '1';
    wait;
  end process;

  check: process
  begin
    wait for 1 ns;
    report "RES " & bit'image(s(0)) & bit'image(s(3));
    wait;
  end process;
end architecture;
