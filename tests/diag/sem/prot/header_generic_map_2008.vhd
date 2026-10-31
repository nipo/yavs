-- yavs-test: std=2008
-- expect: E0004 7:5 a generic clause in a protected type is VHDL-2019
-- LRM 5.6.2 (2008): the declaration has no header; LCS-2016-034 adds it in 2019
entity e is end;
architecture a of e is
  type pt is protected
    generic (w : integer);
    function get return integer;
  end protected;
  type pt is protected body
    function get return integer is begin return w; end;
  end protected body;
begin
end;
