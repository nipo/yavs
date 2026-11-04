-- yavs-test: std=2008
-- expect: E1621 13:37 attribute `event` requires a signal prefix
-- expect: E1621 14:37 attribute `last_value` requires a signal prefix
-- LRM 16.2 (#1091): signal attributes take a signal; an external constant or variable name is not one.
package p is
  constant k : bit := '0';
  signal s : bit;
end package;
entity external_class_attribute_mismatch is end entity;
architecture a of external_class_attribute_mismatch is
  signal t : boolean;
begin
  t <= <<constant @work.p.k : bit>>'event;
  t <= <<constant @work.p.k : bit>>'last_value = '0';
end architecture;
