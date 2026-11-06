-- A pipeline of record registers that changes in every leaf at every clock (#1110): past the
-- review of the record images, compiled code reads and drives each whole record on the kernel
-- itself (`yavs_rt_gather`, `yavs_rt_scatter`), not through the signal callback.
library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
entity rec_direct is generic (CYC : natural := 1300); end;
architecture a of rec_direct is
  type cplx is record
    re, im : signed(17 downto 0);
  end record;
  signal clk : std_ulogic := '0';
  signal s0 : cplx := (to_signed(7, 18), to_signed(-8, 18));
  signal s1, s2, s3 : cplx := (to_signed(-8, 18), to_signed(7, 18));
begin
  process (clk)
  begin
    if rising_edge(clk) then
      s0 <= (re => s0.im + 1, im => s0.re);
      s1 <= s0;
      s2 <= s1;
      s3 <= (re => s2.im, im => s2.re);
    end if;
  end process;
  process
  begin
    for i in 1 to CYC loop
      clk <= '1'; wait for 5 ns;
      clk <= '0'; wait for 5 ns;
    end loop;
    report "q=" & to_hstring(s3.re) & to_hstring(s3.im);
    wait;
  end process;
end;
