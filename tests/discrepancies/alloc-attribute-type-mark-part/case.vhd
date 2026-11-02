-- LRM 9.3.5, 9.3.7, 16.2.2: an allocator of a qualified expression whose type mark is `'subtype` of an
-- indexed or selected object name, `new r(0).v'subtype'(...)`, `new m(1)'subtype'(...)`.
entity top is end entity;
architecture a of top is
  type vec_t is array (natural range <>) of bit_vector(3 downto 0);
  type rec is record v : bit_vector(3 downto 0); end record;
  type rec_a is array (0 to 1) of rec;
  type bv_acc is access bit_vector;
begin
  process
    variable r : rec_a;
    variable m : vec_t(0 to 1) := ("0001", "0010");
    variable p, q : bv_acc;
  begin
    p := new r(0).v'subtype'("1010");
    q := new m(1)'subtype'("0110");
    report "RES " & to_string(p.all) & " " & integer'image(p'length) & " " & to_string(q.all);
    wait;
  end process;
end architecture;
