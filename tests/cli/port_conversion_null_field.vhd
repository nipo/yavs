-- A conversion of a composite with a null array field (#1104).
entity port_conversion_child is
  port (f : out integer);
end;
architecture a of port_conversion_child is
begin
  f <= 3;
end;
entity port_conversion_null_field is
end;
architecture t of port_conversion_null_field is
  type empty_t is array (-1 downto 0) of integer;
  type rec_t is record
    a : integer;
    b : empty_t;
  end record;
  function conv(i : integer) return rec_t is
  begin
    return (a => i, b => (others => 0));
  end;
  signal s : rec_t;
begin
  u : entity work.port_conversion_child port map (conv(f) => s);
  process begin
    wait for 1 ns;
    assert s.a = 3;
    std.env.finish;
  end process;
end;
