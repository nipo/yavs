-- yavs-test: std=2008
-- expect: none
-- LRM 16.2: S'SUBTYPE of an object name whose prefix has an indexed name before a selection
-- (a(0).v'subtype); 'element and 'range likewise; the parentheses are not a constraint
entity r11 is end entity;
architecture a of r11 is
  type rec is record v : bit_vector(3 downto 0); end record;
  type arr_t is array (0 to 1) of rec;
  signal a : arr_t;
  signal e : a(0).v'subtype;
  signal f : a(1).v'element;
  subtype st is a(0).v'subtype;
begin
  process
    variable v : a(0).v'subtype;
    variable w : bit_vector(a(0).v'range);
  begin
    for i in a(0).v'range loop
      v(i) := w(i);
    end loop;
    wait;
  end process;
end architecture;
