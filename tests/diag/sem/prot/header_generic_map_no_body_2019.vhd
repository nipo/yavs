-- yavs-test: std=2019
-- expect: E1230 5:3 no body
-- LRM 5.6.3: the body of the generic is still needed
package p is
  type pt is protected
    generic (w : integer);
    generic map (w => 4);
    function get return integer;
  end protected;
end package;
package body p is
end package body;
