library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

library vunit_lib;
context vunit_lib.vunit_context;

entity tb_fifo is
  generic (runner_cfg : string);
end entity;

architecture sim of tb_fifo is
  constant DEPTH : positive := 4;

  signal clk   : std_ulogic := '0';
  signal rst   : std_ulogic := '1';
  signal wr    : std_ulogic := '0';
  signal rd    : std_ulogic := '0';
  signal din   : std_ulogic_vector(7 downto 0) := (others => '0');
  signal dout  : std_ulogic_vector(7 downto 0);
  signal empty : std_ulogic;
  signal full  : std_ulogic;

  function word(n : natural) return std_ulogic_vector is
  begin
    return std_ulogic_vector(to_unsigned((n * 37 + 1) mod 256, 8));
  end function;
begin
  clk <= not clk after 5 ns;

  dut : entity work.fifo
    generic map (DEPTH => DEPTH)
    port map (clk, rst, wr, din, rd, dout, empty, full);

  main : process
    procedure write_word(n : natural) is
    begin
      din <= word(n);
      wr <= '1';
      wait until falling_edge(clk);
      wr <= '0';
    end procedure;

    procedure read_word(n : natural) is
    begin
      rd <= '1';
      wait until falling_edge(clk);
      rd <= '0';
      check_equal(dout, word(n), "word " & integer'image(n));
    end procedure;
  begin
    test_runner_setup(runner, runner_cfg);
    wait for 22 ns;
    rst <= '0';
    wait until falling_edge(clk);

    while test_suite loop
      if run("fill_and_drain") then
        for i in 0 to DEPTH - 1 loop
          write_word(i);
        end loop;
        check_equal(full, '1', "full after DEPTH writes");
        for i in 0 to DEPTH - 1 loop
          read_word(i);
        end loop;
        check_equal(empty, '1', "empty after DEPTH reads");

      elsif run("write_to_full_is_ignored") then
        for i in 0 to DEPTH loop
          write_word(i);
        end loop;
        for i in 0 to DEPTH - 1 loop
          read_word(i);
        end loop;
        check_equal(empty, '1', "the extra word was not stored");

      -- Fails on purpose: debug it. The check expects the wrong word.
      elsif run("fails_on_purpose") then
        write_word(1);
        write_word(2);
        read_word(2);
      end if;
    end loop;

    test_runner_cleanup(runner);
  end process;
end architecture;
