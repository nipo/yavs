-- lint_710 to lint_713: what is a state machine, and which `others` is dead.
package machines_pkg is
  type shared_t is (s_idle, s_run, s_orphan);
end package;

use work.machines_pkg.all;

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

-- Two processes: the next state is read through the copy, a default that holds is no exit.
architecture two_process of machines is
  type state_t is (idle, run, stuck, orphan); -- expect: lint_710
  signal state, next_state : state_t := idle;
  signal clk, go : bit;
begin
  reg : process (clk) is
  begin
    if rising_edge(clk) then
      state <= next_state;
    end if;
  end process;

  comb : process (all) is
  begin
    next_state <= state;
    case state is
      when idle =>
        if go = '1' then
          next_state <= run;
        end if;
      when run => next_state <= stuck;
      when stuck => null; -- expect: lint_711
      when orphan => next_state <= idle;
    end case;
  end process;
end architecture;

-- A default that is a state is the exit of every alternative.
architecture default_state of machines is
  type state_t is (idle, run);
  signal state, next_state : state_t := idle;
  signal clk : bit;
begin
  reg : process (clk) is
  begin
    if rising_edge(clk) then
      state <= next_state;
    end if;
  end process;

  comb : process (all) is
  begin
    next_state <= idle;
    case state is
      when idle => next_state <= run;
      when run => null;
    end case;
  end process;
end architecture;

-- A type of a package: the finding is at the register, whatever else uses the type.
architecture from_package of machines is
  signal state : shared_t := s_idle; -- expect: lint_710
  signal clk : bit;
begin
  p : process (clk) is
  begin
    if rising_edge(clk) then
      case state is
        when s_idle => state <= s_run;
        when s_run => state <= s_idle;
        when s_orphan => state <= s_idle;
      end case;
    end if;
  end process;
end architecture;

-- A wait, and two registers of one type judged apart.
architecture two_registers of machines is
  type state_t is (idle, run, last); -- expect: lint_710
  signal a : state_t := idle;
  signal b : state_t := idle;
  signal clk : bit;
begin
  pa : process is
  begin
    wait until rising_edge(clk);
    case a is
      when idle => a <= run;
      when run => a <= last;
      when last => a <= idle;
    end case;
  end process;

  pb : process is
  begin
    wait until rising_edge(clk);
    case b is
      when idle => b <= run;
      when run => b <= idle;
      when last => b <= idle;
    end case;
  end process;
end architecture;

-- A subtype leaves the rest of its type out, and a next state from a function leaves it unread.
architecture narrowed of machines is
  type state_t is (idle, run, outside);
  subtype small_t is state_t range idle to run;
  signal state : small_t := idle;
  type other_t is (o0, o1, o2);
  signal other, other_next : other_t := o0;
  signal clk : bit;
  function pick return other_t is
  begin
    return o1;
  end function;
begin
  p : process (clk) is
  begin
    if rising_edge(clk) then
      case state is
        when idle => state <= run;
        when run => state <= idle;
        when others => null; -- expect: lint_712
      end case;
      other <= other_next;
    end if;
  end process;

  q : process (all) is
  begin
    other_next <= pick;
    case other is
      when o0 => null;
      when others => null; -- expect: lint_713
    end case;
  end process;
end architecture;

