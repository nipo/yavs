-- difftest: top=subcopy_bounds std=2008 exit=1 ignore=fatalmsg,path
-- ignore=fatalmsg,path: the references word the error and name its process differently.
-- An index out of the range of a matrix inside a subprogram whose descriptor the caller knows:
-- a copy for that descriptor fails at the same time, after the same reports (LRM 8.4).
package subcopy_bounds_p is
  type mat is array (natural range <>, natural range <>) of integer;
  function at(m : mat; i, j : natural) return integer;
end package;

package body subcopy_bounds_p is
  function at(m : mat; i, j : natural) return integer is
    variable s : integer := 0;
  begin
    for k in m'range(1) loop
      for l in m'range(2) loop
        s := s + m(k, l) * (k + l);
      end loop;
    end loop;
    return s + m(i, j);
  end function;
end package body;

use work.subcopy_bounds_p.all;

entity subcopy_bounds is end entity;

architecture a of subcopy_bounds is
  constant m : mat(1 to 2, 4 downto 2) := ((1, 2, 3), (4, 5, 6));
  signal clk : bit := '0';
begin
  clk <= not clk after 5 ns;
  process (clk)
    variable n : natural := 0;
  begin
    if clk = '1' then
      n := n + 1;
      -- The column leaves 4 downto 2 at the sixth rising edge.
      report integer'image(n) & " " & integer'image(at(m, 1 + n mod 2, 2 + n mod 3 + n / 6 * 3));
    end if;
  end process;
end architecture;
