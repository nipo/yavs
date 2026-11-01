-- difftest: top=generic_package_own_name std=2008 ref=hand
-- A generic package named by itself keeps its own library name in every instance, in the package and
-- in its body (#1010, LRM 16.2.5, lrm-checks L173); an item of an instance is named by the instance.
-- ref=hand: the expected text is the first reference simulator's, with the library named `work`
-- (it names the library after its directory); the second gives the instance's name in the
-- declaration (catalogue `generic-package-own-name`).
package generic_package_own_name_gp is generic (g : integer);
  constant ci : string := generic_package_own_name_gp'instance_name;
  constant cp : string := generic_package_own_name_gp'path_name;
  constant k : integer := g;
  constant kp : string := k'path_name;
  function fi return string;
  function fp return string;
  function fk return string;
  function fself return string;
end package;
package body generic_package_own_name_gp is
  function fi return string is begin return generic_package_own_name_gp'instance_name; end;
  function fp return string is begin return generic_package_own_name_gp'path_name; end;
  function fk return string is begin return k'instance_name; end;
  function fself return string is begin return fself'path_name; end;
end package body;
package generic_package_own_name_gi is new work.generic_package_own_name_gp generic map (3);
package generic_package_own_name_plain is
  constant ci : string := generic_package_own_name_plain'instance_name;
  function fp return string;
end package;
package body generic_package_own_name_plain is
  function fp return string is begin return generic_package_own_name_plain'path_name; end;
end package body;
entity generic_package_own_name is end;
architecture test of generic_package_own_name is
  package li is new work.generic_package_own_name_gp generic map (4);
  procedure rp(tag : string; p : string) is begin report tag & " " & p; end;
begin
  b1 : block
    package bi is new work.generic_package_own_name_gp generic map (5);
  begin
    p : process begin
      wait for 1 ns;
      rp("b.ci", bi.ci); rp("b.cp", bi.cp); rp("b.fi", bi.fi); rp("b.fp", bi.fp);
      wait;
    end process;
  end block;
  g1 : for i in 0 to 0 generate
    b2 : block
      package gg is new work.generic_package_own_name_gp generic map (6);
    begin
      p : process begin
        wait for 2 ns;
        rp("g.ci", gg.ci); rp("g.cp", gg.cp); rp("g.fi", gg.fi); rp("g.fp", gg.fp);
        wait;
      end process;
    end block;
  end generate;
  process begin
    rp("gi.ci", work.generic_package_own_name_gi.ci); rp("gi.cp", work.generic_package_own_name_gi.cp);
    rp("gi.fi", work.generic_package_own_name_gi.fi); rp("gi.fp", work.generic_package_own_name_gi.fp);
    rp("gi.kp", work.generic_package_own_name_gi.kp); rp("gi.fk", work.generic_package_own_name_gi.fk);
    rp("gi.fself", work.generic_package_own_name_gi.fself);
    rp("li.ci", li.ci); rp("li.cp", li.cp); rp("li.fi", li.fi); rp("li.fp", li.fp);
    rp("plain.ci", work.generic_package_own_name_plain.ci); rp("plain.fp", work.generic_package_own_name_plain.fp);
    wait;
  end process;
end;
