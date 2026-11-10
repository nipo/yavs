-- A small function inlined into another small function and into the process, at two levels
-- of inlining: the optimiser makes the body of each once and hands it out again
-- (`opt_bodies_kept`), instead of optimising it at every level.
entity body_once is end;
architecture t of body_once is
  function inc(x : integer) return integer is
  begin
    return (x + 3) mod 1000;
  end function;
  function twice(x : integer) return integer is
  begin
    return inc(inc(x) * 2);
  end function;
begin
  process
    variable s : integer := 0;
  begin
    for i in 1 to 50 loop
      s := twice(s) + inc(i);
    end loop;
    report "s=" & integer'image(s);
    wait;
  end process;
end;
