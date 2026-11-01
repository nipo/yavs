-- The conditions of `wait until` and `assert` under coverage (#1020), and the evaluations of a
-- `wait until` with an edge conjunct (#1022): the design's notes are its result.
entity cov_wait is
end entity;

architecture a of cov_wait is
  type counter_t is protected
    procedure bump;
    impure function value return natural;
  end protected;
  type counter_t is protected body
    variable n : natural := 0;
    procedure bump is
    begin
      n := n + 1;
    end procedure;
    impure function value return natural is
    begin
      return n;
    end function;
  end protected body;
  shared variable calls : counter_t;

  signal clk, x, en, ok : bit := '0';

  impure function f(b : bit) return boolean is
  begin
    calls.bump;
    report "impure call at " & time'image(now) & " " & bit'image(b);
    return true;
  end function;
begin
  clk <= not clk after 10 ns when now < 40 ns;
  x <= '1' after 5 ns, '0' after 7 ns;
  en <= '1' after 15 ns, '0' after 35 ns;
  ok <= '1' after 2 ns;

  -- The edge test is not first: `f` runs at every event of `clk` and `x`.
  process
  begin
    wait until f(x) and rising_edge(clk);
    report "woke on f at " & time'image(now);
  end process;

  -- Two inputs and an edge first, which has no rows.
  process
  begin
    wait until rising_edge(clk) and en = '1' and ok = '1';
    report "woke on en at " & time'image(now);
  end process;

  -- Two inputs, no edge, with a timeout at the end.
  process
  begin
    wait until x = '1' and en = '0';
    report "woke on x at " & time'image(now);
    wait until x = '1' and en = '1' for 100 ns;
    report "timed out at " & time'image(now);
    report "calls=" & integer'image(calls.value);
    wait;
  end process;

  -- A concurrent and a sequential assertion of two inputs, always true.
  assert ok = '1' or clk = '0' report "never" severity error;
  process (clk)
  begin
    assert en = '0' or ok = '1' report "never" severity error;
  end process;
end architecture;
