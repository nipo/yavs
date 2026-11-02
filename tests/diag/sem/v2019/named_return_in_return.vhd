-- yavs-test: std=2019
-- expect: E1304 19:12 `iota` has a named return value and cannot be the value of a return statement
-- expect: E1304 24:13 the same in a function with a constrained result subtype
-- help: return that variable
-- LRM 2019 4.2.1, LCS-2016-072b: "It is an error if a function has a return identifier and the
-- function is called from a context other than the ones above"; a return statement is not one.
-- The qualified form (item e) and a variable (item a) stay legal: `twice_ok`.
entity e is
end entity;
architecture a of e is
  subtype iv3 is integer_vector(1 to 3);
  function iota return r of integer_vector is
    variable v : r;
  begin
    return v;
  end function;
  function twice return r of integer_vector is
  begin
    return iota;
  end function;
  function three return iv3 is
  begin
    -- comment line
    return (iota);
  end function;
  function twice_ok return r of integer_vector is
    variable v : r := iota;
  begin
    return r'(iota);
  end function;
begin
end architecture;
