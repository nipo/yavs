-- difftest: top=init_leaves
-- The initial values of signals of every composite shape reach the right leaf and element (#1008):
-- arrays of records, records of arrays, repeated and differing elements, null parts, a value from
-- a function, and the default of a signal with none. Each signal is read at time 0 and after a
-- delta, element by element.
library ieee; use ieee.std_logic_1164.all;
entity init_leaves is end;
architecture a of init_leaves is
  type color is (red, green, blue);
  type inner is record
    b : bit;
    l : std_logic_vector(3 downto 0);
    n : integer range -5 to 500;
  end record;
  type inners is array (natural range <>) of inner;
  type outer is record
    k : color;
    xs : inners(0 to 2);
    r : real;
    t : time;
    e : std_logic_vector(1 to 0);
    o : boolean;
  end record;
  type outers is array (1 to 3) of outer;
  type mem_t is array (0 to 7) of std_logic_vector(7 downto 0);
  type grid_t is array (0 to 1, 0 to 2) of std_ulogic;
  type ints is array (2 downto 0) of integer;

  constant i0 : inner := ('1', "01XZ", 7);
  constant i1 : inner := ('0', "1010", -5);

  function ramp return mem_t is
    variable m : mem_t;
  begin
    for i in m'range loop
      for j in m(i)'range loop
        m(i)(j) := '1' when (i + j) mod 3 = 0 else 'L' when (i + j) mod 3 = 1 else 'Z';
      end loop;
    end loop;
    return m;
  end function;

  signal u : mem_t := (others => (others => '1'));         -- one value throughout
  signal v : mem_t := (2 => x"5a", 5 => "ZZZZ0000", others => (others => '0'));
  signal w : mem_t := ramp;
  signal none : mem_t;
  signal g : grid_t := (('0', '1', 'X'), ('H', 'L', 'W'));
  signal rec : outer := (blue, (i0, i1, i0), 2.5, 7 ns, "", true);
  signal recs : outers := (2 => (green, (others => i1), -1.5, 3 ps, "", false),
                           others => (red, (0 => i0, others => i1), 0.25, 1 us, "", true));
  signal nd : outers;
  signal im : ints := (2 => 500, others => -1);
  signal nul : mem_t;
  signal nv : std_logic_vector(0 downto 1) := (others => '1');

  procedure out_mem(name : string; m : mem_t) is
  begin
    for i in m'range loop
      report name & "(" & integer'image(i) & ")=" & to_string(m(i));
    end loop;
  end procedure;

  procedure out_outer(name : string; o : outer) is
  begin
    report name & " " & color'image(o.k) & " " & boolean'image(o.r = 2.5) & boolean'image(o.r = -1.5)
      & boolean'image(o.r = 0.25) & boolean'image(o.r = real'low) & " "
      & time'image(o.t) & " " & boolean'image(o.o) & " " & integer'image(o.e'length);
    for i in o.xs'range loop
      report "  " & bit'image(o.xs(i).b) & " " & to_string(o.xs(i).l) & " "
        & integer'image(o.xs(i).n);
    end loop;
  end procedure;
begin
  process
  begin
    for pass in 0 to 1 loop
      out_mem("u", u); out_mem("v", v); out_mem("w", w); out_mem("none", none);
      for i in 0 to 1 loop
        for j in 0 to 2 loop
          report "g=" & std_ulogic'image(g(i, j));
        end loop;
      end loop;
      out_outer("rec", rec);
      for i in recs'range loop
        out_outer("recs", recs(i));
        out_outer("nd", nd(i));
      end loop;
      for i in im'range loop
        report "im=" & integer'image(im(i));
      end loop;
      report "nv=" & integer'image(nv'length);
      wait for 0 ns;
    end loop;
    wait;
  end process;
end architecture;
