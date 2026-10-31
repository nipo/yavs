-- What the tests of the debugger need besides tb.vhd: a shared variable, processes without a label,
-- and with SPIN a process that reports and then never suspends.
entity more_tb is
  generic (SPIN : boolean := false);
end entity;

architecture a of more_tb is
  type tally_t is protected
    procedure add(x : integer);
  end protected;
  type tally_t is protected body
    variable total : integer := 0;
    procedure add(x : integer) is
    begin
      total := total + x;
    end procedure;
  end protected body;
  shared variable tally : tally_t;
  signal clk : bit := '0';
begin
  clk <= not clk after 5 ns when now < 20 ns;

  process
  begin
    tally.add(3);
    if SPIN then
      report "before the loop";
      loop
        null;
      end loop;
    end if;
    wait for 10 ns;
    report "added";
    wait;
  end process;
end architecture;
