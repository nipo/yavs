-- External names in generate branches that are not elaborated: the architecture is still
-- lowered as a whole, so these statements need aliases even though nothing binds them.
library ieee;
use ieee.std_logic_1164.all;
entity external_untaken_dut is
end;
architecture a of external_untaken_dut is
  signal s : std_logic := '0';
begin
end;
library ieee;
use ieee.std_logic_1164.all;
entity external_untaken_inner is
  generic (en : boolean);
end;
architecture a of external_untaken_inner is
  signal x : std_logic := '1';
begin
  d : entity work.external_untaken_dut;
  g : if en generate
    << signal ^.d.s : std_logic >> <= x;
  end generate;
  keep : process begin wait; end process;
end;
library ieee;
use ieee.std_logic_1164.all;
entity external_untaken_branch is
  generic (peek : boolean := false);
end;
architecture t of external_untaken_branch is
  signal stim : std_logic := '1';
  signal o1, o2, o3 : std_logic;
begin
  u : entity work.external_untaken_dut;
  w1 : if peek generate
    << signal .external_untaken_branch.u.s : std_logic >> <= stim;
  end generate;
  r1 : if peek generate
    o1 <= << signal .external_untaken_branch.u.s : std_logic >>;
  end generate;
  s1 : if peek generate
    process (<< signal .external_untaken_branch.u.s : std_logic >>) begin
      o2 <= '1';
    end process;
  end generate;
  f1 : if peek generate
    process begin
      << signal .external_untaken_branch.u.s : std_logic >> <= force stim;
      wait;
    end process;
  end generate;
  p1 : if peek generate
    impure function rd return std_logic is
    begin
      return << signal .external_untaken_branch.u.s : std_logic >>;
    end;
  begin
    o3 <= rd;
  end generate;
  n1 : for i in 1 to 0 generate
    << signal .external_untaken_branch.u.s : std_logic >> <= stim;
  end generate;
  e1 : if true generate
    o2 <= 'Z';
  else generate
    << signal .external_untaken_branch.u.s : std_logic >> <= stim;
  end generate;
  c1 : case 1 generate
    when 1 =>
      o3 <= 'Z';
    when others =>
      b : block begin
        << signal .external_untaken_branch.u.s : std_logic >> <= stim;
      end block;
  end generate;
  -- One instance elaborates the branch and the other does not, in both orders.
  i1 : entity work.external_untaken_inner generic map (en => true);
  i2 : entity work.external_untaken_inner generic map (en => false);
  i3 : entity work.external_untaken_inner generic map (en => true);
  process begin
    wait for 1 ns;
    assert << signal .external_untaken_branch.u.s : std_logic >> = '0';
    assert << signal .external_untaken_branch.i1.d.s : std_logic >> = '1';
    assert << signal .external_untaken_branch.i2.d.s : std_logic >> = '0';
    assert << signal .external_untaken_branch.i3.d.s : std_logic >> = '1';
    report "done";
    std.env.finish;
  end process;
end;
