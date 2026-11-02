-- LRM 6.4.2.2: 'range of a deferred constant whose subtype has globally static bounds, in a
-- subtype of its package body before the full declaration; the deferred declaration fixes them.
package w is
  function width return natural;
end package;
package body w is
  function width return natural is begin return 5; end;
end package body;
use work.w.all;
package p is
  type vec_t is array (natural range <>) of integer;
  constant G : vec_t(width - 1 downto 1);
  function text return string;
end package;
package body p is
  subtype sub_t is bit_vector(G'range);
  function text return string is
  begin
    return integer'image(sub_t'left) & " " & integer'image(sub_t'right);
  end;
  constant G : vec_t(width - 1 downto 1) := (others => 9);
end package body;
use work.p.all;
entity top is end entity;
architecture a of top is
begin
  process
  begin
    report "RES " & text;
    wait;
  end process;
end architecture;
