-- yavs-test: std=2008
-- expect: E1101 38:3 process label vs signal
-- expect: E1101 39:3 block label vs signal
-- expect: E1101 40:3 for generate label vs signal
-- expect: E1101 41:3 if generate label vs signal
-- expect: E1101 42:3 case generate label vs signal
-- expect: E1101 43:3 instance label vs signal
-- expect: E1101 44:3 assertion label vs signal
-- expect: E1101 45:3 assignment label vs signal
-- expect: E1101 46:3 procedure call label vs signal
-- expect: E1101 47:3 label vs constant
-- expect: E1101 48:3 label vs type
-- expect: E1101 49:3 label vs enumeration literal
-- expect: E1101 50:3 label vs subtype
-- expect: E1101 51:3 label vs physical unit
-- expect: E1101 52:3 label vs component
-- expect: E1101 53:3 label vs procedure
-- expect: E1101 54:3 label vs function
-- expect: E1101 55:3 label vs alias
-- expect: E1101 56:3 label vs attribute
-- expect: E1101 57:3 instance label vs its own component
-- LRM 2008 11.1, 12.3: a concurrent label is declared in the architecture, so it may not repeat a declaration of it (#1084)
entity label_conc is end entity;
architecture a of label_conc is
  signal in_process, in_block, in_for, in_if, in_case, in_inst, in_assert, in_assign, in_call : bit;
  signal t : bit;
  constant a_constant : bit := '0';
  type a_type is (a_literal, other_literal);
  subtype a_subtype is bit;
  type a_unit_type is range 0 to 9 units a_unit; end units;
  component a_component is end component;
  component own is end component;
  procedure a_procedure is begin null; end procedure;
  function a_function return bit is begin return '0'; end function;
  alias an_alias is t;
  attribute an_attribute : integer;
begin
  in_process : process begin wait; end process;
  in_block : block begin end block;
  in_for : for i in 0 to 1 generate end generate;
  in_if : if true generate end generate;
  in_case : case true generate when others => end generate;
  in_inst : own;
  in_assert : assert true;
  in_assign : t <= '1';
  in_call : a_procedure;
  a_constant : process begin wait; end process;
  a_type : process begin wait; end process;
  a_literal : process begin wait; end process;
  a_subtype : process begin wait; end process;
  a_unit : process begin wait; end process;
  a_component : process begin wait; end process;
  a_procedure : process begin wait; end process;
  a_function : process begin wait; end process;
  an_alias : process begin wait; end process;
  an_attribute : process begin wait; end process;
  own : own;
end architecture;
