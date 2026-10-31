library ieee;
use ieee.std_logic_1164.all;

-- Small FIFO: a sub-instance to look at, with a memory array and a variable.
entity fifo is
  generic (DEPTH : positive := 4);
  port (
    clk, rst : in std_ulogic;
    wr       : in std_ulogic;
    din      : in std_ulogic_vector(7 downto 0);
    rd       : in std_ulogic;
    dout     : out std_ulogic_vector(7 downto 0);
    empty    : out std_ulogic;
    full     : out std_ulogic
  );
end entity;

architecture rtl of fifo is
  type mem_t is array (0 to DEPTH - 1) of std_ulogic_vector(7 downto 0);
  signal mem : mem_t := (others => (others => '0'));
  signal count : natural range 0 to DEPTH := 0;
begin
  empty <= '1' when count = 0 else '0';
  full <= '1' when count = DEPTH else '0';

  main : process (clk)
    variable wp, rp : natural range 0 to DEPTH - 1 := 0;
    variable next_count : natural range 0 to DEPTH := 0;
  begin
    if rising_edge(clk) then
      next_count := count;
      if rst = '1' then
        wp := 0;
        rp := 0;
        next_count := 0;
      else
        if wr = '1' and count < DEPTH then
          mem(wp) <= din;
          wp := (wp + 1) mod DEPTH;
          next_count := next_count + 1;
        end if;
        if rd = '1' and count > 0 then
          dout <= mem(rp);
          rp := (rp + 1) mod DEPTH;
          next_count := next_count - 1;
        end if;
      end if;
      count <= next_count;
    end if;
  end process;
end architecture;
