-- A whole record read and driven at every clock, one leaf of four changing (#1050): the three that
-- hold make no direct entry, whether the plan keeps images or not.
entity seam_runs is
end entity;

architecture t of seam_runs is
  type rec_t is record
    a : bit_vector(39 downto 0);
    b : bit_vector(39 downto 0);
    c : bit_vector(39 downto 0);
    d : bit_vector(39 downto 0);
  end record;
  signal clk : bit := '0';
  signal r : rec_t := ((others => '0'), (others => '0'), (others => '0'), (others => '0'));
  signal sum : natural := 0;
begin
  clk <= not clk after 5 ns when now < 2 us;

  drive : process (clk)
    variable v : rec_t;
  begin
    if clk = '1' then
      v := r;
      v.a := not r.a;
      r <= v;
    end if;
  end process;

  count : process (r)
  begin
    if r.a(0) = '1' then sum <= sum + 1; end if;
  end process;

  check : process
  begin
    wait for 2 us + 10 ns;
    assert sum > 90 report "sum " & integer'image(sum) severity failure;
    report "seam_runs " & integer'image(sum);
    wait;
  end process;
end architecture;
