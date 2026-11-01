-- lint_601: one signal driven from more than one statement.
library ieee;
use ieee.std_logic_1164.all;

entity drivers is
  port (a, b : in std_logic);
end entity;

architecture rtl of drivers is
  signal both, by_two, conc, whole, kept : std_logic;
  signal elems, parts : std_logic_vector(1 downto 0);
begin
  blk1 : block is
    signal own : std_logic;
  begin
    own <= a;
    kept <= own;
  end block;

  blk2 : block is
    signal own : std_logic;
  begin
    own <= b;
    whole <= own; -- expect: lint_601
  end block;

  whole <= a;

  both <= a; -- expect: lint_601
  blk3 : block is
  begin
    both <= b;
  end block;

  p1 : process (a) is -- expect: lint_601
  begin
    by_two <= a;
  end process;

  p2 : process (b) is
  begin
    by_two <= b;
  end process;

  p3 : process (a) is -- expect: lint_601
  begin
    conc <= a;
  end process;

  conc <= b;

  elems(0) <= a;
  elems(1) <= b;

  parts <= "00"; -- expect: lint_601
  parts(0) <= a;
end architecture;
