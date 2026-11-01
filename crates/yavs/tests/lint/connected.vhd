-- lint_730, lint_750, lint_760, lint_001, lint_002: instances, recursion and sensitivity.
entity sink is
  port (i : in bit; o : out bit);
end entity;

architecture rtl of sink is
begin
  o <= i;
end architecture;

entity connected is
end entity;

architecture rtl of connected is
  component sink is
    port (i : in bit; o : out bit);
  end component;
  component reordered is -- expect: lint_750
    port (o : out bit; i : in bit);
  end component;
  signal from_inst, only_in, from_comp, to_comp, q, y : bit;
  signal clk, d : bit; -- expect: lint_730, lint_730
  signal never_driven : bit; -- expect: lint_730

  function leaf (x : integer) return integer is
  begin
    return x + 1;
  end function;

  function caller (x : integer) return integer is
  begin
    return leaf(x);
  end function;

  constant k : integer := caller(1);
begin
  -- An output of an instance drives, an input does not.
  u1 : entity work.sink port map (i => only_in, o => from_inst);
  u2 : component sink port map (i => to_comp, o => from_comp);
  y <= from_inst and from_comp and never_driven;

  only_in <= '0';
  to_comp <= '0';

  -- A clocked process is not checked for its list.
  clocked : process (clk) is
  begin
    if clk'event and clk = '1' then
      q <= d;
    end if;
  end process;
end architecture;

-- The same ports in another order are the same ports.
entity reordered is
  port (i : in bit; o : out bit);
end entity;

architecture rtl of reordered is
begin
  o <= i;
end architecture;
