-- LRM 9.3.5, 9.3.7, 16.2.2: an allocator of a qualified expression whose type mark is `'subtype` of a
-- simple object name or `'element` of an object or a type, `new x'subtype'(...)`, `new m'element'(...)`.
entity top is end entity;
architecture a of top is
  type vec_t is array (natural range <>) of bit_vector(3 downto 0);
  type bv_acc is access bit_vector;
begin
  process
    variable x : bit_vector(3 downto 0) := "1100";
    variable m : vec_t(0 to 1) := ("0001", "0010");
    variable p, q, s : bv_acc;
  begin
    p := new x'subtype'("1010");
    q := new m'element'("0110");
    s := new vec_t'element'(0 => '1', others => '0');
    report "RES " & to_string(p.all) & " " & to_string(q.all) & " " & to_string(s.all);
    wait;
  end process;
end architecture;
