-- difftest: top=file_in_generate_pkg std=2008 ref=hand
-- A package instance local to a generate has one file per iteration, and a package local to a
-- block has its own. Hand-written expected output: the first reference simulator fails on these
-- local packages and the second parses no package in a generate's declarative part, which
-- LRM 3.3.2 and 11.8 allow through the block declarative items.
use std.textio.all;
package figp_gen is
  generic (name : string);
  file f : text open write_mode is name;
end;

use std.textio.all;
entity file_in_generate_pkg is end;
architecture t of file_in_generate_pkg is
begin
  b : block
    package pk is
      file f : text open write_mode is "figp_b.txt";
      procedure put (s : string);
    end package;
    package body pk is
      procedure put (s : string) is
        variable l : line;
      begin
        write(l, s);
        writeline(f, l);
      end procedure;
    end package body;
  begin
    process
      variable l : line;
    begin
      pk.put("block package");
      file_close(pk.f);
      file_open(pk.f, "figp_b.txt", read_mode);
      readline(pk.f, l);
      report l.all;
      wait;
    end process;
  end block;

  p : for i in 1 to 2 generate
    package inst is new work.figp_gen generic map ("figp_" & integer'image(i) & ".txt");
  begin
    process
      variable l : line;
    begin
      wait for i * 1 ns;
      write(l, string'("package ") & integer'image(i));
      writeline(inst.f, l);
      file_close(inst.f);
      file_open(inst.f, "figp_" & integer'image(i) & ".txt", read_mode);
      readline(inst.f, l);
      report l.all;
      file_close(inst.f);
      wait;
    end process;
  end generate;
end;
