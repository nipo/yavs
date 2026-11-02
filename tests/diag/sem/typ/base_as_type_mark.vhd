-- yavs-test: std=2008
-- expect: E1201 14:15 'base as the type mark of a subtype indication
-- expect: E1201 21:10 'base as the type mark of a qualified expression
-- expect: E1201 22:14 'base in an allocator
-- expect: E1201 23:10 'base as the type mark of a type conversion
-- help: name the type directly, e.g. `integer'(5)`
-- LRM 16.2.2: T'BASE "is allowed only as the prefix of the name of another attribute";
-- as a prefix (lines 17, 24) it stays legal. User decision 2026-11-02 (rule 47 review).
entity e is
end entity;
architecture a of e is
  subtype word is bit_vector(3 downto 0);
  type wa is access bit_vector;
  signal sv : integer'base;
begin
  process
    variable k : integer range natural'base'range;
    variable i : integer;
    variable p : wa;
  begin
    i := integer'base'(5);
    p := new word'base'("1010");
    i := integer'base(2.5);
    i := natural'base'high;
    wait;
  end process;
end architecture;
