-- A testbench with one signal of each kind of type, for the console reference page.
library ieee;
use ieee.std_logic_1164.all;

entity types_tb is
end entity;

architecture sim of types_tb is
  type state_t is (idle, busy, done);
  type pair_t is record
    x   : integer;
    ok  : boolean;
    tag : std_logic;
  end record;
  type words_t is array (0 to 3) of std_logic_vector(7 downto 0);
  type node_t;
  type node_ptr is access node_t;
  type node_t is record
    value     : integer;
    next_node : node_ptr;
  end record;
  type tally_t is protected
    procedure add (n : integer);
    impure function total return integer;
  end protected;
  type tally_t is protected body
    variable sum : integer := 0;
    procedure add (n : integer) is
    begin
      sum := sum + n;
    end procedure;
    impure function total return integer is
    begin
      return sum;
    end function;
  end protected body;

  shared variable tally : tally_t;
  signal state : state_t := idle;
  signal pair  : pair_t := (0, false, '0');
  signal words : words_t := (others => x"00");
  signal ratio : real := 0.5;
  signal delay : time := 10 ns;
  signal count : integer range 0 to 255 := 0;
  signal bus8  : std_logic_vector(7 downto 0) := x"00";
begin
  build : process
    variable head, tail : node_ptr;
  begin
    head := new node_t'(value => 1, next_node => null);
    tail := new node_t'(value => 2, next_node => head);
    tally.add(tail.value + head.value);
    state <= busy;
    pair  <= (7, true, '1');
    words(2) <= x"A5";
    count <= 42;
    wait for 10 ns;
    state <= done;
    bus8 <= x"3C";
    wait for 10 ns;
    report "build finished";
    wait;
  end process;
end architecture;
