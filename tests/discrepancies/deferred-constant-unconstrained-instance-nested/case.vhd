-- LRM 4.8, 6.4.2.2: an instance of a generic package declared in another generic package's
-- declarations, that one instantiated in an architecture: the inner deferred constant of an
-- unconstrained array subtype has the bounds and elements of its full declaration.
package gp is
  generic (n : integer);
  type vec_t is array (natural range <>) of integer;
  constant U : vec_t;
end package;
package body gp is
  constant U : vec_t := (1, 2, 3, n);
end package body;
package gs is
  generic (n : integer);
  package inner is new work.gp generic map (n => n + 2);
end package;
entity top is end entity;
architecture a of top is
  package os is new work.gs generic map (n => 5);
begin
  process begin
    report "RES " & integer'image(os.inner.U'length) & " " & integer'image(os.inner.U(3));
    wait;
  end process;
end architecture;
