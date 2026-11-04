-- 5.3.2: the LRM sets no limit on the number of elements of an array; an implementation may.
-- 2**32 * 2**32 elements do not fit a 64-bit count.
entity top is end entity;
architecture a of top is
  type big is range 0 to 4294967295;
  type m is array (big, big) of bit;
  constant c : m := (others => (others => '0'));
begin
  process
  begin
    report "RES elaborated";
    wait;
  end process;
end architecture;
