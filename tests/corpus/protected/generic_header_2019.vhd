-- difftest: top=generic_header std=2019 ref=hand
-- ref=hand: no reference simulator parses a generic map in a protected type header; the
-- expected output was worked out by hand from the LRM.
-- 2019 protected type header with a generic clause and a generic map (5.6.2): the type is
-- an instance, usable at once; constant, type, subprogram and package generics.
package generic_header_k is
  generic (c : integer);
  constant kk : integer := c * 10;
end package;

package generic_header_p is
  package k3 is new work.generic_header_k generic map (c => 3);
  function plus2(x : integer) return integer;
  -- All generic classes, the body in the package body.
  type all_kinds is protected
    generic (
      type t;
      init : t;
      function f(a : t) return t;
      function "+"(a, b : t) return t is <>;
      package k is new work.generic_header_k generic map (<>)
    );
    generic map (t => integer, init => 40, f => plus2, k => k3);
    procedure add(x : t);
    impure function get return t;
    impure function get_f return t;
    impure function get_k return integer;
    -- Its own name in a method profile.
    procedure copy_from(variable other : inout all_kinds);
    impure function fresh return t;
  end protected all_kinds;
end package;

package body generic_header_p is
  function plus2(x : integer) return integer is
  begin
    return x + 2;
  end function;

  type all_kinds is protected body
    variable v : t := init;
    procedure add(x : t) is
    begin
      v := v + x;
    end procedure;
    impure function get return t is
    begin
      return v;
    end function;
    impure function get_f return t is
    begin
      return f(v);
    end function;
    impure function get_k return integer is
    begin
      return k.kk;
    end function;
    procedure copy_from(variable other : inout all_kinds) is
    begin
      v := other.get;
    end procedure;
    -- A local object of its own type: a new instance with the same generics.
    impure function fresh return t is
      variable tmp : all_kinds;
    begin
      return tmp.get;
    end function;
  end protected body all_kinds;
end package body;

-- A generic package whose generic gives the actual: one type per package instance.
package generic_header_g is
  generic (depth : positive);
  type counter is protected
    generic (wrap : positive);
    generic map (wrap => depth);
    procedure inc;
    impure function get return natural;
  end protected counter;
end package;

package body generic_header_g is
  type counter is protected body
    variable n : natural := 0;
    procedure inc is
    begin
      n := (n + 1) mod wrap;
    end procedure;
    impure function get return natural is
    begin
      return n;
    end function;
  end protected body counter;
end package body;

use work.generic_header_p.all;

entity generic_header is
end entity;

architecture a of generic_header is
  package g2 is new work.generic_header_g generic map (depth => 2);
  package g5 is new work.generic_header_g generic map (depth => 5);

  -- Two types of one name in different regions, with different actuals, and positional.
  type pt is protected
    generic (w : integer; s : string);
    generic map (4, "arch");
    impure function get return integer;
    impure function name return string;
  end protected;
  type pt is protected body
    impure function get return integer is
    begin
      return w;
    end function;
    impure function name return string is
    begin
      return s;
    end function;
  end protected body;

  shared variable sh : all_kinds;
  shared variable arch_pt : pt;
  shared variable c2 : g2.counter;
begin
  blk : block
    type pt is protected
      generic (w : integer);
      generic map (w => 9);
      impure function get return integer;
    end protected;
    type pt is protected body
      impure function get return integer is
      begin
        return w;
      end function;
    end protected body;
    shared variable blk_pt : pt;
  begin
    process
    begin
      report "block pt " & integer'image(blk_pt.get);
      wait;
    end process;
  end block;

  -- The actual depends on the generate parameter.
  gen : for i in 1 to 2 generate
    type pg is protected
      generic (w : integer);
      generic map (w => i * 100);
      impure function get return integer;
    end protected;
    type pg is protected body
      impure function get return integer is
      begin
        return w;
      end function;
    end protected body;
    shared variable g : pg;
  begin
    process
    begin
      wait for i * 1 ns;
      report "gen " & integer'image(i) & " " & integer'image(g.get);
      wait;
    end process;
  end generate;

  process
    -- In a process, an object per type, variables not shared.
    type lp is protected
      generic (type e; first : e);
      generic map (e => character, first => 'q');
      impure function head return e;
    end protected;
    type lp is protected body
      impure function head return e is
      begin
        return first;
      end function;
    end protected body;
    variable l : lp;
    variable other : all_kinds;
    variable c5 : g5.counter;
  begin
    wait for 5 ns;
    sh.add(2);
    report "sh " & integer'image(sh.get) & " " & integer'image(sh.get_f) & " " & integer'image(sh.get_k)
      & " " & integer'image(sh.fresh);
    other.add(100);
    sh.copy_from(other);
    report "copied " & integer'image(sh.get);
    report "arch pt " & integer'image(arch_pt.get) & " " & arch_pt.name;
    report "lp " & l.head;
    for i in 1 to 3 loop
      c2.inc;
      c5.inc;
    end loop;
    report "counters " & integer'image(c2.get) & " " & integer'image(c5.get);
    report "generic_header ok";
    wait;
  end process;
end architecture;
