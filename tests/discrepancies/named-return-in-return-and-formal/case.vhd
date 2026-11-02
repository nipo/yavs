-- LRM 2019 4.2.1: the result subtype of a named-return function call as the actual of a
-- constrained formal (LCS-2016-072b item c). The call in a return statement is an error
-- (tests/diag/sem/v2019/named_return_in_return.vhd).
entity top is end entity;
architecture a of top is
  function iota return r of integer_vector is
    variable v : r;
  begin
    for i in v'range loop v(i) := i; end loop;
    return v;
  end function;
  procedure show(x : integer_vector(1 to 3)) is
  begin
    report "RES show " & integer'image(x(3));
  end procedure;
begin
  process
  begin
    show(iota);
    wait;
  end process;
end architecture;
