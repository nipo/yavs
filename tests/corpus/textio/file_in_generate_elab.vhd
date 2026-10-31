-- difftest: top=file_in_generate_elab std=2008 ignore=stderr
-- Files of generates read while the region elaborates, passed as file parameters, nested, and
-- declared in instances that have no process at all. W2118 for the constant is yavs's alone.
use std.textio.all;
package fige_pkg is
  procedure put (file f : text; s : string);
end;
package body fige_pkg is
  procedure put (file f : text; s : string) is
    variable l : line;
  begin
    write(l, s);
    writeline(f, l);
  end procedure;
end;

use std.textio.all;
entity fige_quiet is
  generic (n : integer);
end;
architecture t of fige_quiet is
begin
  g : for i in 1 to 2 generate
    file f : text open write_mode is "fige_quiet" & integer'image(n * 10 + i) & ".txt";
  begin
  end generate;
end;

use std.textio.all;
use work.fige_pkg.all;
entity file_in_generate_elab is end;
architecture t of file_in_generate_elab is
  -- Written while the top architecture elaborates, read by the generates below.
  impure function make return integer is
    file f : text open write_mode is "fige_in.txt";
    variable l : line;
  begin
    write(l, string'("41"));
    writeline(f, l);
    file_close(f);
    return 1;
  end function;
  constant made : integer := make;
begin
  -- A constant of the iteration reads the iteration's file while the iteration elaborates.
  g : for i in 1 to 2 generate
    file f : text open read_mode is "fige_in.txt";
    impure function get return integer is
      variable l : line;
      variable v : integer;
    begin
      readline(f, l);
      read(l, v);
      return v;
    end function;
    constant c : integer := get + i;
  begin
    process
      file own : text open write_mode is "fige_own" & integer'image(i) & ".txt";
    begin
      wait for i * 1 ns;
      report "constant " & integer'image(c);
      assert endfile(f);
      wait;
    end process;
  end generate;

  -- Nested for generates: an outer file per outer iteration, an inner one per inner iteration.
  o : for i in 1 to 2 generate
    file fo : text open write_mode is "fige_o" & integer'image(i) & ".txt";
  begin
    n : for j in 1 to 2 generate
      file fi : text open write_mode is "fige_o" & integer'image(i) & "i" & integer'image(j) & ".txt";
    begin
      process
      begin
        wait for 10 ns + (i * 10 + j) * 1 ns;
        put(fo, "from " & integer'image(j));
        put(fi, "inner");
        file_close(fi);
        wait;
      end process;
    end generate;
    process
      variable l : line;
    begin
      wait for 100 ns + i * 1 ns;
      file_close(fo);
      file_open(fo, "fige_o" & integer'image(i) & ".txt", read_mode);
      while not endfile(fo) loop
        readline(fo, l);
        report "outer " & integer'image(i) & " " & l.all;
      end loop;
      wait;
    end process;
  end generate;

  -- A protected method writes the file of its iteration; the nested process is the first writer.
  t : for i in 1 to 2 generate
    file f : text open write_mode is "fige_t" & integer'image(i) & ".txt";
    type pt is protected
      procedure put (s : string);
    end protected;
    type pt is protected body
      procedure put (s : string) is
        variable l : line;
      begin
        write(l, s);
        writeline(f, l);
      end procedure;
    end protected body;
    shared variable sv : pt;
  begin
    process
      variable l : line;
    begin
      wait for 200 ns + i * 1 ns;
      sv.put("outer");
      file_close(f);
      file_open(f, "fige_t" & integer'image(i) & ".txt", read_mode);
      readline(f, l);
      report "protected " & integer'image(i) & " " & l.all;
      file_close(f);
      wait;
    end process;
    n : if true generate
    begin
      process
      begin
        sv.put("nested");
        wait;
      end process;
    end generate;
  end generate;

  -- An instance without a process still opens the files of its generates.
  q : entity work.fige_quiet generic map (7);
  r : entity work.fige_quiet generic map (8);

  process
    file f : text;
    variable st : file_open_status;
  begin
    wait for 300 ns;
    for i in 71 to 82 loop
      if i <= 72 or i >= 81 then
        file_open(st, f, "fige_quiet" & integer'image(i) & ".txt", read_mode);
        report "quiet " & integer'image(i) & " " & file_open_status'image(st);
        if st = open_ok then
          file_close(f);
        end if;
      end if;
    end loop;
    wait;
  end process;
end;
