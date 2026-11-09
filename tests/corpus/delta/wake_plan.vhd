-- Waiters on narrow signals whose waits change shape while the run goes on: resident waits on
-- the whole record, on fields, on one bit each; a process with two waits; one that switches
-- from one resident wait to another; one that starts waiting late; a wait with a timeout; a
-- postponed process; a process that ends; an edge. Each wake reports what had an event. The
-- waiters of `u` become resident after their second wake, then one switches to another wait,
-- one joins late and one ends, so what an event of `u` wakes changes three times.
-- `ignore=order`: processes of generates and with several waits report in the same cycle, an
-- order the LRM leaves open.
-- difftest: top=wake_plan ignore=order
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity wake_plan is
end entity;
architecture a of wake_plan is
  type rec is record
    a : std_ulogic;
    b : std_ulogic_vector(3 downto 0);
    c : std_ulogic;
    n : unsigned(7 downto 0);
  end record;
  signal r : rec := ('0', "0000", '0', x"00");
  signal v : std_ulogic_vector(15 downto 0) := (others => '0');
  signal nw : natural := 0;
  signal u : unsigned(5 downto 0) := (others => '0');
  function evs(signal s : rec) return string is
  begin
    return "ev=" & boolean'image(s.a'event) & boolean'image(s.b'event)
      & boolean'image(s.c'event) & boolean'image(s.n'event);
  end function;
begin
  whole : process (r)
  begin
    report "whole " & evs(r);
    nw <= nw + 1;
  end process;

  fa : process (r.a)
  begin
    report "fa a=" & std_ulogic'image(r.a);
  end process;

  fbc : process (r.b, r.c)
  begin
    report "fbc b=" & to_string(r.b) & " c=" & std_ulogic'image(r.c);
  end process;

  fn : process
  begin
    wait on r.n;
    report "fn n=" & integer'image(to_integer(r.n));
  end process;

  alt : process
  begin
    wait on r.a;
    report "alt a " & evs(r);
    wait on r.n;
    report "alt n " & evs(r);
  end process;

  sw : process
  begin
    for i in 1 to 3 loop
      wait on r.a;
      report "sw a " & integer'image(i);
    end loop;
    loop
      wait on r.c;
      report "sw c " & evs(r);
    end loop;
  end process;

  late : process
  begin
    wait for 22 ns;
    loop
      wait on r.b;
      report "late b=" & to_string(r.b);
    end loop;
  end process;

  tmo : process
  begin
    for i in 1 to 8 loop
      wait on r.c for 7 ns;
      report "tmo c'event=" & boolean'image(r.c'event);
    end loop;
    wait;
  end process;

  pp : postponed process (r.n)
  begin
    report "pp n=" & integer'image(to_integer(r.n));
  end process;

  ender : process
  begin
    wait on r.b;
    report "ender 1";
    wait on r.b;
    report "ender 2";
    wait;
  end process;

  edge : process (r.a)
  begin
    if rising_edge(r.a) then
      report "edge a";
    end if;
  end process;

  bits : for i in 0 to 15 generate
    bit_p : process (v(i))
    begin
      report "bit " & integer'image(i) & "=" & std_ulogic'image(v(i));
    end process;
  end generate;

  vall : process (v)
  begin
    report "vall " & to_hstring(v);
  end process;

  ua : process (u)
  begin
    report "ua " & to_hstring(u);
  end process;

  ub : process (u(0))
  begin
    report "ub " & std_ulogic'image(u(0));
  end process;

  uc : process (u(5 downto 3))
  begin
    report "uc " & to_string(u(5 downto 3));
  end process;

  usw : process
  begin
    for i in 1 to 3 loop
      wait on u(0);
      report "usw 0 " & integer'image(i);
    end loop;
    loop
      wait on u(5);
      report "usw 5 " & std_ulogic'image(u(5));
    end loop;
  end process;

  ulate : process
  begin
    wait for 32 ns;
    loop
      wait on u(1);
      report "ulate " & std_ulogic'image(u(1));
    end loop;
  end process;

  uend : process
  begin
    for i in 1 to 4 loop
      wait on u;
      report "uend " & integer'image(i);
    end loop;
    wait;
  end process;

  ustim : process
    type steps is array (natural range <>) of natural;
    constant seq : steps := (1, 3, 8, 9, 41, 40, 42, 10, 11, 9, 41);
  begin
    for i in seq'range loop
      wait for 5 ns;
      u <= to_unsigned(seq(i), 6);
    end loop;
    wait;
  end process;

  stim : process
  begin
    wait for 5 ns;
    r.a <= '1';
    wait for 5 ns;
    r.b <= "1010";
    v(3) <= '1';
    wait for 5 ns;
    r.c <= '1';
    r.n <= x"05";
    v <= x"00f8";
    wait for 5 ns;
    r <= ('0', "0101", '0', x"06");
    wait for 5 ns;
    r.a <= '1';
    r.b <= "0100";
    v(15) <= '1';
    wait for 5 ns;
    r.a <= '0';
    r.c <= '1';
    wait for 5 ns;
    r.n <= x"07";
    r.b <= "0100";
    v(0) <= '1';
    wait for 5 ns;
    r.b <= "1111";
    r.c <= '0';
    r.a <= '1';
    wait for 5 ns;
    r.n <= x"08";
    v <= x"0000";
    wait for 15 ns;
    assert nw = 10 report "whole woke " & integer'image(nw) & " times" severity failure;
    report "done";
    wait;
  end process;
end architecture;
