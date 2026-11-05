-- External names that depend on a generic of a subprogram declared in a process: each instance
-- has its own (#1096).
library ieee;
use ieee.std_logic_1164.all;
entity external_generic_dut is
end;
architecture a of external_generic_dut is
  signal v : std_logic_vector(7 downto 0) := x"00";
begin
end;
library ieee;
use ieee.std_logic_1164.all;
entity external_generic_subprogram is
end;
architecture t of external_generic_subprogram is
begin
  u0 : entity work.external_generic_dut;
  u1 : entity work.external_generic_dut;
  p : process
    procedure wr generic (n : natural) parameter (x : std_logic_vector) is
    begin
      << signal .external_generic_subprogram.u0.v : std_logic_vector(n - 1 downto 0) >> <= x;
    end;
    procedure wri is new wr generic map (n => 8);
    procedure typed generic (type t) parameter (x : t) is
    begin
      << signal .external_generic_subprogram.u1.v : t >> <= x;
    end;
    subtype byte_t is std_logic_vector(7 downto 0);
    procedure typedi is new typed generic map (t => byte_t);
    procedure never generic (n : natural) is
    begin
      << signal .external_generic_subprogram.nope.v : std_logic_vector(n - 1 downto 0) >> <= (others => '0');
    end;
  begin
    wri(x"3c");
    typedi(x"c3");
    wait for 1 ns;
    assert << signal .external_generic_subprogram.u0.v : std_logic_vector(7 downto 0) >> = x"3c";
    assert << signal .external_generic_subprogram.u1.v : std_logic_vector(7 downto 0) >> = x"c3";
    std.env.finish;
  end process;
end;
