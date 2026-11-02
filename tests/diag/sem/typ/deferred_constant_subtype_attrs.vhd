-- yavs-test: std=2008
-- expect: none
-- LRM 6.4.2.2: an attribute of the subtype alone of a deferred constant whose declaration constrains it fully reads no value, before the full declaration too (#1072)
package p is
  type vec_t is array (natural range <>) of integer;
  type rec_t is record
    f : bit_vector(0 to 5);
  end record;
  constant K : vec_t(3 downto 0);
  constant R : rec_t;
  constant L : integer := K'length + K'high;
  subtype s is bit_vector(K'range);
  procedure q (constant d : in bit_vector(K'range));
  component c is
    generic (g : bit_vector(K'range) := (others => '0'));
    port (x : in bit_vector(K'range));
  end component;
end package;
package body p is
  type arr_t is array (K'reverse_range) of bit;
  subtype int_t is integer range K'low to K'high;
  procedure q (constant d : in bit_vector(K'range)) is begin null; end;
  function f return integer is
    variable a : bit_vector(K'length - 1 downto 0);
    variable b : integer range K'right to K'left;
    variable v : K'subtype;
    variable e : K'element;
    variable x : bit_vector(R.f'range);
  begin
    if K'ascending then return 0; end if;
    return a'length + b + v'length + e + x'length;
  end;
  package inner is
    procedure q2 (constant d : in bit_vector(K'range));
  end package;
  package body inner is
    procedure q2 (constant d : in bit_vector(K'range)) is begin null; end;
  end package body;
  constant K : vec_t(3 downto 0) := (1, 2, 3, 4);
  constant R : rec_t := (f => (others => '1'));
end package body;
