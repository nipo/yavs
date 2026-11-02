-- LRM 6.4.2.2: 'range of a deferred constant of an unconstrained array subtype in a parameter
-- subtype of a procedure of its package body, before the full declaration.
package p is
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
  function f return integer;
end package;
package body p is
  procedure q (constant d : in bit_vector(U'range); s : out integer) is
  begin
    s := d'left * 10 + d'right;
  end;
  function f return integer is
    variable s : integer;
  begin
    q("1011", s);
    return s;
  end;
  constant U : vec_t := (5 => 1, 6 => 2, 7 => 3, 8 => 4);
end package body;
use work.p.all;
entity top is end entity;
architecture a of top is
begin
  process
  begin
    report "RES " & integer'image(f);
    wait;
  end process;
end architecture;
