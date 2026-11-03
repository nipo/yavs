-- yavs-test: std=2008
-- expect: E1101 23:5 label vs variable of the procedure
-- expect: E1101 27:5 label vs parameter
-- expect: E1101 31:5 label vs function parameter
-- expect: E1101 36:5 label vs parameter of a declared subprogram
-- expect: E1101 46:5 assignment label vs variable
-- expect: E1101 47:5 null label vs variable
-- expect: E1101 48:5 loop label vs variable
-- expect: E1101 49:5 if label vs variable
-- expect: E1101 50:5 case label vs variable
-- expect: E1101 52:7 label in a loop vs variable of the process
-- expect: E1101 54:5 label vs constant
-- expect: E1101 55:5 label vs type
-- expect: E1101 56:5 label vs enumeration literal
-- expect: E1101 57:5 label vs procedure
-- expect: E1101 58:5 label vs function
-- LRM 2008 10.1, 12.1 c) h), 12.3: a sequential label is declared in the innermost process or subprogram body (#1084)
entity label_seq is end entity;
architecture a of label_seq is
  procedure in_sub is
    variable a_var : bit;
  begin
    a_var : null;
  end procedure;
  procedure with_param (a_param : bit) is
  begin
    a_param : null;
  end procedure;
  function with_fparam (f_param : bit) return bit is
  begin
    f_param : return f_param;
  end function;
  procedure declared (d_param : bit);
  procedure declared (d_param : bit) is
  begin
    d_param : null;
  end procedure;
begin
  p : process
    variable v_assign, v_null, v_loop, v_if, v_case, v_nested : bit;
    constant a_constant : bit := '0';
    type a_type is (a_literal, other_literal);
    procedure a_procedure is begin null; end procedure;
    function a_function return bit is begin return '0'; end function;
  begin
    v_assign : v_assign := '1';
    v_null : null;
    v_loop : for i in 0 to 1 loop null; end loop;
    v_if : if true then null; end if;
    v_case : case true is when others => null; end case;
    for i in 0 to 1 loop
      v_nested : null;
    end loop;
    a_constant : null;
    a_type : null;
    a_literal : null;
    a_procedure : null;
    a_function : null;
    wait;
  end process;
end architecture;
