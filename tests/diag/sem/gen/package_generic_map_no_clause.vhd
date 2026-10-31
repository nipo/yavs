-- yavs-test: std=2008
-- expect: E1501 5:16 a generic map with no generic clause
-- LRM 4.7: a generic map aspect only follows a generic clause
package pm is
  generic map (w => 4);
end package;
