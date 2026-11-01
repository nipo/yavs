-- LRM 16.2.5: a package instance, a local package and a library package as the prefix of 'path_name, from an architecture.
package plain is
  constant k : integer := 1;
end package;
entity top is end;
architecture test of top is
  package lp is
    constant k : integer := 1;
  end package lp;
begin
  process begin
    report "RES " & lp'path_name;
    report "RES " & lp'instance_name;
    report "RES " & work.plain'path_name;
    wait;
  end process;
end;
