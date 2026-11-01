-- A clock read through an object alias has the edge trigger of the clock itself (#1001): each
-- process below runs only on its edge. Every form of edge_forms, through an alias of the signal, of
-- an element, and of a renumbered slice.
library ieee; use ieee.std_logic_1164.all;
entity edge_alias is end;
architecture a of edge_alias is
  signal x : std_ulogic := '0';
  signal v : std_ulogic_vector(3 downto 0) := "0000";
  signal rc : std_ulogic := '0';
  alias c : std_logic is x;
  alias e : std_ulogic is v(2);
  alias s : std_ulogic_vector(0 to 3) is v;
  alias a2 : std_ulogic is c;
  type cnt_t is array (1 to 10) of natural;
  signal n : cnt_t := (others => 0);
  signal done : boolean := false;
begin
  x <= not x after 5 ns when not done;
  v(2) <= not v(2) after 5 ns when not done;
  v(3) <= not v(3) after 5 ns when not done;
  process (c) begin if rising_edge(c) then n(1) <= n(1) + 1; end if; end process;
  process (c) begin if falling_edge(c) then n(2) <= n(2) + 1; end if; end process;
  process (c) begin if c'event and c = '1' then n(3) <= n(3) + 1; end if; end process;
  process (c) begin if not c'stable and c = '1' then n(4) <= n(4) + 1; end if; end process;
  process begin wait until rising_edge(c); n(5) <= n(5) + 1; end process;
  process begin wait until c = '1'; n(6) <= n(6) + 1; end process;
  process (e) begin if rising_edge(e) then n(7) <= n(7) + 1; end if; end process;
  process (s(0)) begin if falling_edge(s(0)) then n(8) <= n(8) + 1; end if; end process;
  process (a2) begin if a2 = '1' and a2'event then n(9) <= n(9) + 1; end if; end process;
  process (x) begin if c'event and x = '1' then n(10) <= n(10) + 1; end if; end process;
  process
    variable sum : natural := 0;
  begin
    wait for 1000 ns;
    done <= true;
    wait for 10 ns;
    for i in n'range loop sum := sum + n(i); end loop;
    report "edges=" & integer'image(sum);
    wait;
  end process;
end;
