-- State machine coverage (#1020). Every form the detection knows, and the forms it reports as
-- not measured.
package cov_fsm_pkg is
  type pkg_state_t is (p_idle, p_run);
end package;

use work.cov_fsm_pkg.all;

entity cov_fsm_dut is
  generic (go_far : boolean := true);
  port (clk, rst, go : in bit; held : buffer pkg_state_t := p_idle);
end entity;

architecture a of cov_fsm_dut is
  -- One process, with a reset that the `case` does not name.
  type one_t is (idle, busy, done, spare);
  signal one : one_t := idle;

  -- Two processes: a next state with a default assignment, copied into the register.
  type two_t is (a0, a1, a2);
  signal two, two_next : two_t := a0;

  -- A type from a package.
  signal pk : pkg_state_t := p_idle;

  -- A reset into a state no alternative names as a way out of `r2`.
  type three_t is (r0, r1, r2);
  signal three : three_t := r0;

  -- Not measured: a register with no `case`, a part of a signal, a variable.
  type if_t is (i0, i1);
  signal by_if : if_t := i0;
  type rec_t is record
    st : one_t;
    n : natural;
  end record;
  signal r : rec_t := (idle, 0);
  -- Not a machine: an enumeration nothing clocks, and a clocked one that does not read itself.
  signal mode : one_t := idle;
  signal copy : one_t := idle;
begin
  process (clk)
  begin
    if rising_edge(clk) then
      if rst = '1' then
        one <= idle;
      else
        case one is
          when idle =>
            if go = '1' then
              one <= busy;
            end if;
          when busy =>
            if go_far then
              one <= done;
            else
              one <= idle;
            end if;
          when done | spare =>
            one <= idle;
        end case;
      end if;
    end if;
  end process;

  process (clk)
  begin
    if rising_edge(clk) then
      if rst = '1' then
        three <= r0;
      else
        case three is
          when r0 => three <= r1;
          when r1 => three <= r2;
          when r2 => three <= r2;
        end case;
      end if;
    end if;
  end process;

  -- One machine per iteration of a generate.
  gen : for i in 0 to 1 generate
    signal g : if_t := i0;
  begin
    process (clk)
    begin
      if rising_edge(clk) then
        case g is
          when i0 =>
            if i = 1 then
              g <= i1;
            end if;
          when i1 => null;
        end case;
      end if;
    end process;
  end generate;

  process (clk)
  begin
    if rising_edge(clk) then
      two <= two_next;
    end if;
  end process;

  process (all)
  begin
    two_next <= two;
    case two is
      when a0 =>
        if go = '1' then
          two_next <= a1;
        end if;
      when a1 =>
        two_next <= a2;
      when others =>
        two_next <= a0;
    end case;
  end process;

  pkg_register : process
  begin
    wait until rising_edge(clk);
    case pk is
      when p_idle => pk <= p_run;
      when p_run => null;
    end case;
  end process;

  process (clk)
    variable v : if_t := i0;
  begin
    if rising_edge(clk) then
      if by_if = i0 then
        by_if <= i1;
      end if;
      case v is
        when i0 => v := i1;
        when i1 => v := i0;
      end case;
      case r.st is
        when idle => r.st <= busy;
        when others => r.st <= idle;
      end case;
      copy <= one;
    end if;
  end process;

  process (clk)
  begin
    if rising_edge(clk) then
      case held is
        when p_idle => held <= p_run;
        when p_run => held <= p_idle;
      end case;
    end if;
  end process;

  process (mode)
  begin
    case mode is
      when idle => null;
      when others => null;
    end case;
  end process;
end architecture;

entity cov_fsm is
  generic (runs : natural := 3; do_reset : boolean := true);
end entity;

architecture tb of cov_fsm is
  signal clk, rst, go : bit := '0';
  signal stop : boolean := false;
begin
  clk <= not clk after 5 ns when not stop;
  u_far : entity work.cov_fsm_dut port map (clk, rst, go, open);
  u_near : entity work.cov_fsm_dut generic map (go_far => false) port map (clk, rst, go, open);

  process
  begin
    wait for 12 ns;
    go <= '1';
    wait for 10 ns * runs;
    go <= '0';
    wait for 20 ns;
    rst <= '1' when do_reset else '0';
    wait for 10 ns;
    rst <= '0';
    wait for 10 ns;
    stop <= true;
    wait;
  end process;
end architecture;
