-- difftest: top=file_in_generate std=2008
-- Files declared in generates and blocks: one object per elaborated region, opened by the
-- declaration, written, read and closed per region; regions not elaborated open nothing.
use std.textio.all;
entity file_in_generate is
  generic (sel : integer := 2);
end;
architecture t of file_in_generate is
begin
  -- If generate: the chosen branch opens its file, the other one would fail if it did.
  g_if : if sel = 2 generate
    file f : text open write_mode is "fig_if.txt";
  begin
    process
      variable l : line;
    begin
      write(l, string'("in if"));
      writeline(f, l);
      file_close(f);
      file_open(f, "fig_if.txt", read_mode);
      readline(f, l);
      report "if read back " & l.all;
      assert endfile(f);
      file_close(f);
      wait;
    end process;
  else generate
    file f : text open read_mode is "fig_there_is_no_such_file.txt";
  begin
    process
      variable l : line;
    begin
      readline(f, l);
      report "else branch ran" severity failure;
      wait;
    end process;
  end generate;

  -- For generate: one file per iteration, named by the parameter; the nested generate's
  -- process and a subprogram of the iteration use the same object as the iteration's process.
  g_for : for i in 1 to 3 generate
    file f : text open write_mode is "fig_for" & integer'image(i) & ".txt";
    procedure put (constant s : in string) is
      variable l : line;
    begin
      write(l, s);
      writeline(f, l);
    end procedure;
  begin
    process
      variable l : line;
    begin
      wait for i * 1 ns;
      put("first " & integer'image(i));
      wait for 10 ns;
      file_close(f);
      file_open(f, "fig_for" & integer'image(i) & ".txt", read_mode);
      while not endfile(f) loop
        readline(f, l);
        report "for " & integer'image(i) & " read " & l.all;
      end loop;
      file_close(f);
      wait;
    end process;
    g_in : if true generate
    begin
      process
      begin
        wait for i * 1 ns + 5 ns;
        put("nested " & integer'image(i));
        wait;
      end process;
    end generate;
  end generate;

  -- Case generate.
  g_case : case sel generate
    when 1 =>
      file f : text open read_mode is "fig_there_is_no_such_file.txt";
    begin
    when 2 =>
      file f : text;
    begin
      process
        variable l : line;
        variable st : file_open_status;
      begin
        wait for 20 ns;
        file_open(st, f, "fig_there_is_no_such_file.txt", read_mode);
        report "case open missing " & file_open_status'image(st);
        file_open(st, f, "fig_case.txt", write_mode);
        report "case open " & file_open_status'image(st);
        write(l, string'("in case"));
        writeline(f, l);
        file_close(f);
        file_open(f, "fig_case.txt", read_mode);
        readline(f, l);
        report "case read back " & l.all;
        file_close(f);
        wait;
      end process;
    when others =>
    begin
  end generate;

  -- Block: the declaration opens the file, a generic of the block names it.
  b : block
    generic (name : string);
    generic map (name => "fig_block.txt");
    file f : text open write_mode is name;
  begin
    process
      variable l : line;
    begin
      wait for 25 ns;
      write(l, string'("in block"));
      writeline(f, l);
      file_close(f);
      file_open(f, name, read_mode);
      readline(f, l);
      report "block read back " & l.all;
      file_close(f);
      wait;
    end process;
  end block;

  -- No process in these regions: the declaration still creates the files at elaboration.
  g_quiet : for i in 1 to 2 generate
    file f : text open write_mode is "fig_quiet" & integer'image(i) & ".txt";
  begin
  end generate;
  b_quiet : block
    file f : text open write_mode is "fig_quiet3.txt";
  begin
  end block;

  process
    file f : text;
    variable st : file_open_status;
  begin
    wait for 30 ns;
    for i in 1 to 3 loop
      file_open(st, f, "fig_quiet" & integer'image(i) & ".txt", read_mode);
      report "quiet " & integer'image(i) & " " & file_open_status'image(st);
      assert st = open_ok;
      if st = open_ok then
        assert endfile(f);
        file_close(f);
      end if;
    end loop;
    wait;
  end process;
end;
