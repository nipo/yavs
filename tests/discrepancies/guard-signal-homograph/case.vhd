-- A guarded block implicitly declares the signal GUARD (11.2). An explicit signal of that name in the
-- block is a homograph of it (12.3), which lets only a predefined operation be hidden.
entity top is
end entity;

architecture a of top is
  signal c : boolean;
begin
  b : block (c)
    signal guard : boolean;
  begin
    p : process begin report "RES accepted"; wait; end process;
  end block;
end architecture;
