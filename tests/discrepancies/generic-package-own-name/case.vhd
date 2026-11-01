-- LRM 16.2.5, 4.9: a generic package named by itself inside its instances, in the declaration and in the body.
package gp is generic (g : integer);
  constant gn : string := gp'instance_name;
  function fg return string;
end package;
package body gp is
  function fg return string is begin return gp'path_name; end;
end package body;
package gi is new work.gp generic map (3);
entity top is end;
architecture test of top is
  package li is new work.gp generic map (4);
begin
  process begin
    report "RES h " & work.gi.gn; report "RES i " & work.gi.fg;
    report "RES j " & li.gn;      report "RES k " & li.fg;
    wait;
  end process;
end;
