-- difftest: top=read_good_forms std=2008
-- The read, oread and hread forms with a GOOD parameter that no other test names: a value that
-- reads and one that does not, for each form.
use std.textio.all;
entity read_good_forms is end;
architecture t of read_good_forms is
begin
  process
    variable l : line;
    variable good : boolean;
    variable v_bit : bit;
    variable v_bv : bit_vector(3 downto 0);
    variable v_bool : boolean;
    variable v_char : character;
    variable v_real : real;
    variable v_str : string(1 to 3);
    variable v_time : time;
    procedure show (name : string; shown : string) is
    begin
      report name & " " & boolean'image(good) & " " & shown;
    end procedure;
  begin
    l := new string'("1 x");
    read(l, v_bit, good); show("bit", bit'image(v_bit));
    read(l, v_bit, good); show("bit bad", "");

    l := new string'(" 1010 xy");
    read(l, v_bv, good); show("bit_vector", to_string(v_bv));
    read(l, v_bv, good); show("bit_vector bad", "");

    l := new string'(" true 7");
    read(l, v_bool, good); show("boolean", boolean'image(v_bool));
    read(l, v_bool, good); show("boolean bad", "");

    l := new string'("q");
    read(l, v_char, good); show("character", character'image(v_char));
    read(l, v_char, good); show("character at end", "");

    l := new string'(" 2.5 x");
    read(l, v_real, good); show("real", real'image(v_real));
    read(l, v_real, good); show("real bad", "");

    l := new string'("abcd");
    read(l, v_str, good); show("string", v_str);
    read(l, v_str, good); show("string short", "");

    l := new string'(" 5 ns 7 xx");
    read(l, v_time, good); show("time", time'image(v_time));
    read(l, v_time, good); show("time bad", "");

    l := new string'(" 12 9");
    oread(l, v_bv(2 downto 0), good); show("oread", "");
    l := new string'(" 7 9");
    oread(l, v_bv(2 downto 0), good); show("oread", to_string(v_bv(2 downto 0)));
    oread(l, v_bv(2 downto 0), good); show("oread bad", "");

    l := new string'(" c g");
    hread(l, v_bv, good); show("hread", to_string(v_bv));
    hread(l, v_bv, good); show("hread bad", "");
    wait;
  end process;
end;
