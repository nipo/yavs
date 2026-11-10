-- A function whose result has the bounds of its string parameter or of a new string, so the body
-- reads the parameter's descriptor in the frame, called 300 times: from its 128th call on the
-- call is lean (`lean_late`), writing the descriptor's slots and no other.
entity lean_late is end;
architecture t of lean_late is
  impure function count_char(s : string; c : character) return natural is
    variable n : natural := 0;
  begin
    for i in s'range loop
      if s(i) = c then
        n := n + 1;
      end if;
    end loop;
    return n;
  end function;

  impure function quote(msg : string) return string is
  begin
    if msg'length /= 0 and count_char(msg, ' ') /= 1 then
      if msg(msg'left) = ':' then
        return " " & msg;
      else
        return " :" & msg & ":";
      end if;
    end if;
    return "";
  end function;
begin
  process
    variable n : integer := 0;
  begin
    for k in 1 to 300 loop
      n := n + quote(integer'image(k))'length + quote(":" & integer'image(k))'length;
    end loop;
    report "n=" & integer'image(n);
    wait;
  end process;
end;
