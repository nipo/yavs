-- lint_710 to lint_713: what is a state machine, and which `others` is dead.
entity machines is
end entity;

architecture complete of machines is
  type state_t is (idle, run, done, orphan);
  signal state : state_t;
  signal clk, go : bit;
begin
  p : process (clk) is
  begin
    if clk'event and clk = '1' then
      case state is
        when idle =>
          if go = '1' then
            state <= run;
          end if;
        when run => state <= done;
        when done => state <= orphan;
        when orphan => state <= idle;
      end case;
    end if;
  end process;
end architecture;

architecture computed of machines is
  type state_t is (idle, run, done, orphan);
  signal state : state_t;
  signal clk : bit;
  function next_state (s : state_t) return state_t is
  begin
    return s;
  end function;
begin
  p : process (clk) is
  begin
    if clk'event and clk = '1' then
      case state is
        when idle => state <= run;
        when run => state <= done;
        when done => state <= next_state(state);
        when orphan => state <= idle;
      end case;
    end if;
  end process;
end architecture;

architecture multiplexer of machines is
  type state_t is (idle, run, done);
  signal state : state_t;
  signal clk, go, y : bit;
begin
  p : process (clk) is
  begin
    if clk'event and clk = '1' then
      if go = '1' then
        state <= run;
      else
        state <= idle;
      end if;
      if go = '0' then
        state <= done;
      end if;
      case state is
        when idle => y <= '0';
        when run => y <= '1';
        when done => y <= '0';
      end case;
    end if;
  end process;
end architecture;

-- The source project: no register, no machine.
architecture no_clock of machines is
  type state_t is (idle, run, orphan);
  signal state : state_t;
  signal go : bit;
begin
  p : process (go) is
  begin
    case state is
      when idle => state <= run;
      when others => state <= idle; -- expect: lint_713
    end case;
  end process;
end architecture;

architecture cases of machines is
  type state_t is (idle, run, done);
  type small_t is (a, b);
  type big_t is (a, b, c);
  signal state : state_t;
  signal byte : bit_vector(7 downto 0);
begin
  p : process is
  begin
    -- Exhaustive with no `others`, and alternatives that group their choices.
    case state is
      when idle | run => null;
      when done => null;
    end case;
    case state is
      when idle | run => null;
      when done => null;
      when others => null; -- expect: lint_712
    end case;
    -- Not an enumeration of this design: left alone.
    case byte is
      when "00000000" => null;
      when others => null;
    end case;
    -- A range names values too, and every value is named here: yavs knows, see docs/decisions.md.
    case state is
      when idle to done => null;
      when others => null; -- expect: lint_712
    end case;
    wait;
  end process;

  variable_of_a_type : process is
    variable v : state_t;
  begin
    case v is
      when idle => null;
      when run => null;
      when done => null;
      when others => null; -- expect: lint_712
    end case;
    wait;
  end process;

  -- One name, two types: each is resolved, see docs/decisions.md.
  first : process is
    variable same : small_t;
  begin
    case same is
      when a => null;
      when b => null;
      when others => null; -- expect: lint_712
    end case;
    wait;
  end process;

  second : process is
    variable same : big_t;
  begin
    case same is
      when a => null;
      when others => null; -- expect: lint_713
    end case;
    wait;
  end process;
end architecture;
