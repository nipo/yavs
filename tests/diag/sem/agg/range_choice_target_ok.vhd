-- yavs-test: std=2008
-- expect: none
-- LRM 10.5.2.1, 10.6.2.1 (#244, user decision 2026-10-19: accepted)
-- Range choices in aggregate targets, also as subtype names and range attributes, mixed with
-- element choices, in variable and signal assignments. A nested aggregate as an element of an
-- array of arrays is E1410 (user, 2026-11-02; expr/agg_target_nested_array.vhd).
entity e is end entity;
architecture a of e is
  subtype hr is natural range 5 downto 2;
  signal sa, sb, sc, sd : bit_vector(2 downto 0);
  signal v6 : bit_vector(5 downto 0);
begin
  (5 downto 3 => sa, 2 downto 0 => sb) <= v6;
  process
    variable v : bit_vector(5 downto 0);
    variable hi, w : bit_vector(3 downto 0);
    variable lo : bit_vector(1 downto 0);
    variable b, b2 : bit;
  begin
    (5 downto 2 => hi, 1 downto 0 => lo) := v;
    (0 to 1 => lo, 2 to 5 => hi) := v;
    (hr => hi, lo'range => lo) := v;
    (5 => b, 4 downto 1 => w, 0 => b2) := v;
    (5 downto 3 => sc, 2 downto 0 => sd) <= v;
    wait;
  end process;
end architecture;
