-- difftest: top=driver_constant_attr_index std=2008
-- Processes driving distinct elements of an unresolved signal, indexed by array attributes of
-- constants whose bounds come from their initial values (globally static, LRM 9.4.3): each
-- process drives its own element only. A process constant with a globally static initial value
-- is left out: the references disagree (tests/discrepancies/process-constant-attribute-driver-index).
library ieee;
use ieee.std_logic_1164.all;

entity driver_constant_attr_index is
end entity;

architecture a of driver_constant_attr_index is
  type int_vector is array (natural range <>) of integer;

  impure function make(n : natural) return int_vector is
    variable r : int_vector(0 to n - 1) := (others => 0);
  begin
    return r;
  end function;

  constant a_c : int_vector := (1, 2, 3);
  constant b_c : int_vector := (4, 5);
  constant m_c : int_vector := make(2);
  signal done_s : std_ulogic_vector(0 to a_c'length + b_c'length + m_c'length) := (others => '0');
begin
  gen_a: for i in a_c'range generate
    p: process
    begin
      done_s(i) <= '1';
      wait;
    end process;
  end generate;

  gen_b: for i in b_c'range generate
    p: process
    begin
      done_s(a_c'length + i) <= '1';
      wait;
    end process;
  end generate;

  pm: process
  begin
    done_s(a_c'length + b_c'length + m_c'high) <= '1';
    wait;
  end process;

  pl: process
    constant l_c : int_vector := (1, 2, 3, 4, 5, 6, 7);
  begin
    done_s(l_c'length) <= '1';
    wait;
  end process;

  pf: process
  begin
    done_s(a_c'length + b_c'length) <= '1';
    wait;
  end process;

  check: process
  begin
    wait for 1 ns;
    assert done_s = "11111111" report "wrong drivers" severity failure;
    report "done";
    wait;
  end process;
end architecture;
