-- lint_720: what makes a loop. Run with that rule on (see tests/lint.rs).
entity loops is
  port (a, b : in bit; out_a, out_b : out bit);
end entity;

architecture concurrent of loops is
  signal y, z, w : bit;
  signal q : bit_vector(3 downto 0);
  signal count : bit_vector(3 downto 0);
begin
  y <= y and a; -- expect: lint_720
  z <= w and a; -- expect: lint_720
  w <= z or b;
  -- A chain with no cycle, two elements, an attribute of the target, a delay.
  out_a <= y and z;
  out_b <= out_a;
  q(0) <= q(1);
  q(1) <= q(2);
  count <= (count'range => a);
end architecture;

architecture in_process of loops is
  signal mid, chained, fed, carry, sel, kept, clk, reg : bit;
  signal v_in, v_out, w_out, v_a, v_b, v_part, picked, looped, s1, s2, s3 : bit;
  signal fed2, ev_in, counted : bit;
  type pair_t is record
    x, y : bit;
  end record;
  signal copied : pair_t;
begin
  -- A later statement reads what an earlier one assigned: no signal depends on itself.
  assigned_then_read : process (all) is
  begin
    mid <= a;
    out_a <= not mid;
  end process;

  -- Other statements of one process are not each other's inputs.
  two_chains : process (all) is
  begin
    chained <= out_a;
    fed2 <= b;
  end process;

  -- `if` and `case` say what the statements below them depend on.
  condition : process (all) is
  begin
    if kept = '1' then
      kept <= '0'; -- expect: lint_720
    end if;
    case sel is
      when '1' => sel <= a; -- expect: lint_720
      when others => null;
    end case;
  end process;

  through_a_later_branch : process (all) is
  begin
    if a = '1' then
      carry <= '1';
    elsif carry = '1' then
      s1 <= b;
    end if;
  end process;

  -- A register or a wait or a delay breaks the loop.
  register_p : process (clk) is
  begin
    if clk'event and clk = '1' then
      reg <= fed;
    end if;
  end process;
  fed <= reg and a;

  waits : process is
  begin
    wait for 5 ns;
    clk <= not clk;
  end process;

  delay : process (all) is
  begin
    picked <= not picked after 5 ns;
    s2 <= picked;
  end process;

  -- The source project's own case: a process and a statement.
  statement_and_process : process (a, looped) is
  begin
    v_in <= looped and a; -- expect: lint_720
  end process;
  looped <= v_in;

  -- A variable carries what was assigned to it, and only that.
  through_a_variable : process (all) is
    variable v : bit;
  begin
    v := v_out;
    v_out <= v; -- expect: lint_720
  end process;

  overwritten : process (all) is
    variable v : bit;
  begin
    v := w_out;
    s3 <= v;
    v := a;
    w_out <= v;
  end process;

  by_activation : process (all) is
    variable v : bit;
  begin
    v_a <= v; -- expect: lint_720
    v := v_a;
  end process;

  parts : process (all) is
    variable v : bit_vector(1 downto 0);
  begin
    v(0) := v_b;
    v(1) := a;
    v_b <= v(1);
  end process;

  -- Every part assigned after a whole copy: nothing of the copy is left.
  whole_then_parts : process (all) is
    variable v : pair_t;
  begin
    v := copied;
    v.x := a;
    v.y := b;
    copied <= v;
  end process;

  -- Asking about a change is no combinational logic.
  on_event : process (ev_in) is
  begin
    if ev_in'event then
      counted <= not counted;
    end if;
  end process;

  in_a_loop : process (all) is
    variable v : bit;
  begin
    v := a;
    for i in 0 to 1 loop
      v_part <= v; -- expect: lint_720
      v := v_part;
    end loop;
  end process;
end architecture;
