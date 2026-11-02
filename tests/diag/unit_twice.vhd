-- A unit declared twice in one file (LRM 13.1, 13.5): the second replaces the first.
entity e is end entity;
architecture a of e is begin end architecture;
entity top is end entity;
architecture t of top is
begin
  u : entity work.e(a);
end architecture;
architecture a of e is begin end architecture;
architecture b of e is begin end architecture;
package p is end package;
package body p is end package body;
package body p is end package body;
