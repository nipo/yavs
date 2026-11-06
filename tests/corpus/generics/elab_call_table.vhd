-- difftest: top=elab_call_table
-- Pure functions called while elaborating that read a package table built by a function: they
-- run against the package state, and from the second run on their results come from the call
-- cache. The run that first answers them from the cache must lower the same program as the run
-- that stored them (the compiled code of the one before is linked, not made again).
package elab_call_table_pkg is
  type iv is array (natural range <>) of integer;
  function pick (i : integer) return integer;
  function twice (x : integer) return integer;
end package;

package body elab_call_table_pkg is
  function mk (n : integer) return iv is
    variable r : iv(0 to n - 1);
  begin
    for i in r'range loop
      r(i) := i * i + 1;
    end loop;
    return r;
  end function;
  constant T : iv := mk(6);
  function pick (i : integer) return integer is
  begin
    return T(i);
  end function;
  function twice (x : integer) return integer is
  begin
    return 2 * x;
  end function;
end package body;

use work.elab_call_table_pkg.all;

entity elab_call_table is
end entity;

architecture a of elab_call_table is
  constant c1 : integer := pick(2);
  constant c2 : integer := twice(4);
  constant c3 : integer := pick(2);
  constant c4 : integer := pick(5);
  signal s1 : bit_vector(1 to c1 + c2);
  signal s2 : bit_vector(1 to c3 + c4);
  signal n : integer := 0;
begin
  process
  begin
    for k in 1 to 3 loop
      n <= n + pick(k);
      wait for 1 ns;
    end loop;
    report "widths " & integer'image(s1'length) & " " & integer'image(s2'length) & " sum " & integer'image(n);
    assert s1'length = 13 and s2'length = 31 and n = 17 severity failure;
    wait;
  end process;
end architecture;
