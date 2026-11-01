-- difftest: top=package_path_names std=2008 ref=hand
-- 'path_name and 'instance_name of a package: every form ends in a colon like any region, a
-- library package is named by its library from wherever it is read, and an item of a package or
-- instance nested in a library package has that package in its path (#1010, LRM 16.2.5, L110).
-- ref=hand: the first reference simulator rejects a package label as the prefix and stops on the
-- nested forms; the expected text is the second's, measured on these forms with a package body added
-- (it fails to link a generic package without one).
package package_path_names_gp is generic (g : integer);
  constant k : integer := g;
  constant kp : string := k'path_name;
  package n is
    constant m : integer := 1;
    constant mp : string := m'path_name;
  end package n;
end package;
package package_path_names_gi is new work.package_path_names_gp generic map (3);
package package_path_names_plain is
  package n is
    constant ci : string := n'instance_name;
    constant cn : string := package_path_names_plain'path_name;
  end package n;
  package nl is new work.package_path_names_gp generic map (6);
end package;
package package_path_names_outer is generic (h : integer);
  package ni is new work.package_path_names_gp generic map (h);
end package;
package package_path_names_oi is new work.package_path_names_outer generic map (7);
entity package_path_names is end;
architecture test of package_path_names is
  package li is new work.package_path_names_gp generic map (4);
  package lp is
    constant sn : string := lp'path_name;
    constant si : string := lp'instance_name;
  end package lp;
  constant c1 : string := li'path_name;
  constant c2 : string := li'instance_name;
  constant c3 : string := lp'path_name;
  constant c4 : string := work.package_path_names_plain'path_name;
  procedure rp(tag : string; p : string) is begin report tag & " " & p; end;
begin
  process begin
    rp("a.li.p", c1); rp("a.li.i", c2); rp("a.lp.p", c3); rp("a.plain.p", c4);
    rp("p.li.p", li'path_name); rp("p.li.i", li'instance_name);
    rp("p.lp.p", lp'path_name); rp("p.lp.i", lp'instance_name);
    rp("p.lp.sn", lp.sn); rp("p.lp.si", lp.si);
    rp("p.gi.p", work.package_path_names_gi'path_name); rp("p.gi.i", work.package_path_names_gi'instance_name);
    rp("p.pl.p", work.package_path_names_plain'path_name); rp("p.pl.i", work.package_path_names_plain'instance_name);
    rp("n.ci", work.package_path_names_plain.n.ci); rp("n.cn", work.package_path_names_plain.n.cn);
    rp("nl.kp", work.package_path_names_plain.nl.kp);
    rp("ni.kp", work.package_path_names_oi.ni.kp);
    rp("li.n.mp", li.n.mp);
    wait;
  end process;
  lab : process
    package pi is new work.package_path_names_gp generic map (8);
  begin
    wait for 1 ns;
    rp("pi.kp", pi.kp); rp("pi.p", pi'path_name); rp("pi.i", pi'instance_name);
    wait;
  end process;
  process
    package pu is new work.package_path_names_gp generic map (9);
  begin
    wait for 2 ns;
    rp("pu.kp", pu.kp); rp("pu.p", pu'path_name);
    wait;
  end process;
end;
