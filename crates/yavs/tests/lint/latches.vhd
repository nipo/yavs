-- lint_600: a signal or variable a combinational process does not settle on every path.
entity latches is
  generic (implement_g : boolean := true; width_g : natural := 3);
  port (a, b, sel : in bit; d : in bit_vector(1 downto 0));
end entity;

architecture rtl of latches is
  type rec_t is record
    x, y : bit;
  end record;
  signal s1, s2, s3, s4, s5, s6, s7, s8, s9, s10, s11, s12, s13, s14, s15, s16, s17, s18 : bit;
  signal r : rec_t;
  signal v1 : bit_vector(1 downto 0);
  constant width_c : natural := width_g * 2;
begin
  -- The source project's cases.
  a_generic_decides : process (all) is
  begin
    if implement_g then
      s1 <= a;
    end if;
  end process;

  a_constant_of_a_generic_decides : process (all) is
  begin
    if width_c > 4 then
      s2 <= a;
    end if;
  end process;

  a_signal_decides : process (all) is
  begin
    if sel = '1' then
      s3 <= a; -- expect: lint_600
    end if;
  end process;

  a_default : process (all) is
  begin
    s4 <= '0';
    if sel = '1' then
      s4 <= a;
    end if;
  end process;

  waits : process is
  begin
    if sel = '1' then
      s5 <= a;
    end if;
    wait on sel;
  end process;

  a_default_in_a_branch : process (all) is
  begin
    if sel = '1' then
      s6 <= '0';
      if a = '1' then
        s6 <= b;
      end if;
    else
      s6 <= '1';
    end if;
  end process;

  a_nested_if : process (all) is
  begin
    if sel = '1' then
      if a = '1' then
        s7 <= b; -- expect: lint_600
      end if;
    else
      s7 <= '1';
    end if;
  end process;

  a_case_everywhere : process (all) is
  begin
    case sel is
      when '1' => s8 <= a;
      when others => s8 <= b;
    end case;
  end process;

  whole_record : process (all) is
  begin
    r <= ('0', '0');
    if sel = '1' then
      r.x <= a;
    end if;
  end process;

  read_too_early : process (all) is
    variable v : bit;
  begin
    if a = '1' then
      v := b;
    end if;
    s9 <= v; -- expect: lint_600
  end process;

  written_first : process (all) is
    variable v : bit;
  begin
    v := b;
    s10 <= v;
  end process;

  initial_value : process (all) is
    variable v : bit := '0';
  begin
    if a = '1' then
      v := b;
    end if;
    s11 <= v;
  end process;

  both_branches : process (all) is
    variable v : bit;
  begin
    if a = '1' then
      v := b;
    else
      v := sel;
    end if;
    s12 <= v;
  end process;

  read_in_the_branch_that_wrote : process (all) is
    variable v : bit;
  begin
    if a = '1' then
      v := b;
      s13 <= v;
    else
      s13 <= '0';
    end if;
  end process;

  clocked_variable : process (sel) is
    variable v : bit;
  begin
    if sel'event and sel = '1' then
      if a = '1' then
        v := b;
      end if;
      s14 <= v;
    end if;
  end process;

  a_for_loop : process (all) is
  begin
    for i in 1 downto 0 loop
      v1(i) <= d(i);
    end loop;
  end process;

  a_for_loop_assigns_a_variable : process (all) is
    variable v : bit;
  begin
    for i in 1 downto 0 loop
      v := d(i);
    end loop;
    s15 <= v;
  end process;

  an_attribute_is_no_read : process (all) is
    variable v : bit_vector(1 downto 0);
    variable w : bit_vector(1 downto 0);
  begin
    for i in v'range loop
      v(i) := d(i);
    end loop;
    w := d(w'range);
    s16 <= v(0) and w(0);
  end process;

  carried_by_a_loop : process (all) is
    variable v : bit_vector(1 downto 0);
  begin
    for i in 0 to 1 loop
      if i = 0 then
        v(i) := d(i);
      else
        v(i) := v(i - 1);
      end if;
    end loop;
    s17 <= v(1);
  end process;

  a_generic_chooses_the_value : process (all) is
    variable v : bit;
  begin
    if implement_g then
      v := a;
    end if;
    s18 <= v;
  end process;
end architecture;
