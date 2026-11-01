-- LRM 7.4, 11.6: a disconnection specification of guarded record signals assigned through an aggregate target.
library ieee; use ieee.std_logic_1164.all;
entity top is end entity;
architecture a of top is
  type lc is record x : std_ulogic; end record;
  type la is array (natural range <>) of lc;
  function res(v : la) return lc is begin
    if v'length = 0 then return (x => 'Z'); end if;
    return v(v'left);
  end function;
  subtype rlc is res lc;
  signal g1, g2 : rlc bus;
  signal c : std_ulogic := '1';
  disconnect g1, g2 : rlc after 2 ns;
begin
  b : block (c = '1') begin
    (g1, g2) <= guarded la'((x => '1'), (x => '0'));
  end block;
  process begin
    wait for 1 ns;
    c <= '0';
    wait for 1 ns;
    report "RES " & to_string(g1.x) & to_string(g2.x);
    wait for 2 ns;
    report "RES " & to_string(g1.x) & to_string(g2.x);
    wait;
  end process;
end architecture;
