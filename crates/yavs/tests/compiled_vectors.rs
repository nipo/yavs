//! Logic vector operations in compiled code against the interpreter: `and`/`or`/`xor` and their
//! negations, `not`, `to_x01`, `to_ux01`, fills, slices (null ones too) and concatenations of
//! every length class (1 element, the plane block of 64, 65, several blocks), with every
//! std_ulogic code among the elements. Output and exit status are the same in every tier.
#![cfg(feature = "clift")]

mod common;
use common::tempdir;

use std::path::Path;
use std::process::{Command, Output};

const SRC: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity v is end;
architecture a of v is
  subtype slv is std_logic_vector;
  function mk(n : natural; k : natural) return slv is
    variable r : slv(n - 1 downto 0);
  begin
    for i in r'range loop
      r(i) := std_logic'val((i * k + i / 3 + k) mod 9);
    end loop;
    return r;
  end;
  function f_and(a, b : slv) return slv is begin return a and b; end;
  function f_or(a, b : slv) return slv is begin return a or b; end;
  function f_xor(a, b : slv) return slv is begin return a xor b; end;
  function f_nand(a, b : slv) return slv is begin return a nand b; end;
  function f_nor(a, b : slv) return slv is begin return a nor b; end;
  function f_xnor(a, b : slv) return slv is begin return a xnor b; end;
  function f_not(a : slv) return slv is begin return not a; end;
  function f_x01(a : slv) return slv is begin return to_x01(a); end;
  function f_ux01(a : slv) return slv is begin return to_ux01(a); end;
  function f_x01z(a : slv) return slv is begin return to_x01z(a); end;
  function fl(n : natural; c : std_logic) return slv is
    variable r : slv(n - 1 downto 0) := (others => c);
  begin
    return r;
  end;
  function part(a : slv; l, r : integer) return slv is
    variable s : slv(l downto r);
  begin
    s := a(l downto r);
    return s;
  end;
  function cat(a, b : slv) return slv is begin return a & '1' & b; end;
  function rep4(a : slv) return slv is begin return a & a & a & a; end;
  function mid(a : slv; l, r : integer) return natural is
  begin
    return a(l downto r)'length;
  end;
  -- Only 0, 1, L and H, for the unsigned arithmetic that has no metavalue to stop it.
  function mk01(n : natural; k : natural) return unsigned is
    variable r : unsigned(n - 1 downto 0);
    constant c : string(1 to 4) := "01LH";
  begin
    for i in r'range loop
      r(i) := std_logic'val(2 + ((i * k + i / 3) mod 4) + 0);
    end loop;
    return r;
  end;
  function u_add(a, b : unsigned) return unsigned is begin return a + b; end;
  function u_sub(a, b : unsigned) return unsigned is begin return a - b; end;
  function u_int(a : unsigned) return integer is begin return to_integer(a); end;
  function u_misc(a, b : unsigned; k : natural) return string is
    constant sa : signed(a'length - 1 downto 0) := signed(a);
    constant sb : signed(b'length - 1 downto 0) := signed(b);
  begin
    return boolean'image(a < b) & boolean'image(a >= b) & boolean'image(a = b) & boolean'image(a /= b)
           & boolean'image(sa < sb) & boolean'image(sa > sb) & boolean'image(sa <= sb)
           & boolean'image(a > k) & boolean'image(k <= b) & boolean'image(sa = k - 3)
           & " " & to_string(resize(a, k + 1)) & " " & to_string(resize(sa, k + 1))
           & " " & to_string(shift_left(a, k mod 5)) & " " & to_string(shift_right(a, k mod 4))
           & " " & to_string(shift_right(sa, 1 + k mod 3)) & " " & to_string(rotate_left(a, k)) & " " & to_string(rotate_right(a, k + 1))
           & " " & to_string(to_unsigned(k * 37, a'length)) & " " & to_string(to_signed(k * 11 - 17, a'length))
           & " " & to_string(a + k) & " " & to_string(k + b) & " " & to_string(sa - 1)
           & " " & to_string(not a) & " " & to_string(a xor a) & " " & to_string(a or a)
           & " " & integer'image(to_integer(sa(minimum(a'length, 31) - 1 downto 0)));
  end;
  procedure num_ops(n, m, k : natural) is
    variable a : unsigned(n - 1 downto 0);
    variable b : unsigned(m - 1 downto 0);
    variable w : unsigned(minimum(n, 31) - 1 downto 0);
  begin
    a := mk01(n, k);
    b := mk01(m, k + 1);
    w := a(w'range);
    report integer'image(n) & "," & integer'image(m) & " " & to_string(u_add(a, b)) & " " & to_string(u_sub(a, b))
           & " " & integer'image(u_int(w));
    -- A metavalue goes the general way (a warning, a result of unknowns).
    report u_misc(a, b, k);
    a(0) := 'X';
    report to_string(u_add(a, b)) & " " & integer'image(u_int(w));
  end;
  function eq_int(a, b : integer_vector) return boolean is begin return a = b; end;
  procedure all_ops(n, k : natural) is
    variable a, b : slv(n - 1 downto 0);
  begin
    a := mk(n, k);
    b := mk(n, k + 2);
    report integer'image(n) & " " & integer'image(k) & " " & to_string(a) & " " & to_string(b);
    report to_string(f_and(a, b)) & " " & to_string(f_or(a, b)) & " " & to_string(f_xor(a, b));
    report to_string(f_nand(a, b)) & " " & to_string(f_nor(a, b)) & " " & to_string(f_xnor(a, b));
    report to_string(f_not(a)) & " " & to_string(f_x01(a)) & " " & to_string(f_ux01(a)) & " " & to_string(f_x01z(b));
    report to_string(fl(n, 'Z')) & " " & to_string(fl(n, '-')) & " " & to_string(fl(n, '1'));
    report to_string(cat(a, b)) & " " & integer'image(cat(a, b)'length) & " " & to_string(rep4(a));
    report boolean'image(a = b) & boolean'image(a = a) & boolean'image(f_and(a, b) = f_and(b, a))
           & boolean'image(eq_int((1, n, 3), (1, n, k))) & boolean'image(eq_int((n, k), (n, k)));
    if n > 2 then
      report to_string(part(a, n - 1, 1)) & "|" & to_string(part(a, n - 2, 0)) & "|" & to_string(part(a, 0, 1));
      report integer'image(mid(a, n - 1, 1)) & " " & integer'image(mid(b, 1, n - 1));
    end if;
  end;
begin
  process
    constant lens : integer_vector := (1, 2, 7, 63, 64, 65, 100, 128, 129, 200);
  begin
    for li in lens'range loop
      for k in 1 to 3 loop
        all_ops(lens(li), k);
        num_ops(lens(li), lens((li + 3) mod lens'length), k);
        num_ops(lens(li), lens(li), k);
      end loop;
    end loop;
    wait;
  end process;
  -- Copies of a constant number of elements at every offset class, one block or two, overlapping.
  process
    variable x, y : slv(127 downto 0);
  begin
    y := mk(128, 5);
    x := mk(128, 7);
    x(70 downto 60) := y(10 downto 0);
    report to_string(x);
    x(127 downto 100) := x(63 downto 36);
    report to_string(x);
    x(30 downto 3) := x(33 downto 6);
    report to_string(x);
    x(64 downto 63) := y(1 downto 0);
    report to_string(x);
    x(20 downto 20) := y(100 downto 100);
    report to_string(x);
    x(127 downto 64) := y(63 downto 0);
    report to_string(x);
    x(40 downto 0) := y(127 downto 87);
    report to_string(x);
    wait;
  end process;
end;
"#;

fn yavs(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yavs"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run yavs")
}

fn run(dir: &Path, flags: &[&str]) -> (Option<i32>, String) {
    let mut args = vec!["run", "--std=2008", "--work=work:work"];
    args.extend_from_slice(flags);
    args.push("v");
    let o = yavs(dir, &args);
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
    )
}

#[test]
fn vector_operations_in_compiled_code_match_the_interpreter() {
    let dir = tempdir("compiled_vectors");
    std::fs::write(dir.join("v.vhd"), SRC).expect("write");
    let a = yavs(
        &dir,
        &["analyze", "--std=2008", "--work=work:work", "v.vhd"],
    );
    assert!(a.status.success(), "{a:?}");
    let want = run(&dir, &["--tier=interp"]);
    assert_eq!(want.0, Some(0), "{want:?}");
    assert!(want.1.lines().count() >= 300, "{}", want.1);
    for flags in [
        #[cfg(feature = "clift")]
        vec!["--tier=clift"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
    ] {
        assert_eq!(run(&dir, &flags), want, "{flags:?}");
    }
}

/// `to_01` of every length class (null, 1, 63 to 65, 70 elements), ascending and descending
/// with a lower bound other than zero, with and without metavalues, every `xmap`, as
/// `std_logic_vector`, `unsigned` and `signed`, in a function (native frame) and in a process
/// (frame mode).
const TO01: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity z is end;
architecture a of z is
  function img(v : std_logic_vector) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(v);
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return std_logic_vector is
    variable r : std_logic_vector(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  function up(n : natural; k : natural; meta : boolean) return std_logic_vector is
    variable r : std_logic_vector(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    return r;
  end function;
  function all_forms(v : std_logic_vector; x : std_logic) return string is
  begin
    return img(to_01(v, x)) & " " & img(to_01(v)) & " " &
           img(std_logic_vector(to_01(unsigned(v), x))) & " " &
           img(std_logic_vector(to_01(signed(v)))) & " " &
           img(std_logic_vector(to_01(unsigned(v)) + 1));
  end function;
begin
  process
    constant lens : integer_vector := (0, 1, 2, 7, 8, 9, 31, 32, 33, 63, 64, 65, 70);
    constant xs : string := "UX01ZWLH-";
    variable n : natural := 0;
    variable w : std_logic_vector(7 downto 0) := "10LH0101";
    variable m : std_logic_vector(3 downto 0) := "10X1";
  begin
    for i in lens'range loop
      for k in 1 to 3 loop
        for meta in boolean'range loop
          for j in xs'range loop
            report integer'image(lens(i)) & "/" & integer'image(k) & "/" & boolean'image(meta) &
                   ": " & all_forms(down(lens(i), k, meta), std_logic'val(j - 1)) &
                   " | " & all_forms(up(lens(i), k, meta), std_logic'val(j - 1));
          end loop;
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      w := std_logic_vector(unsigned(w) + k);
      n := n + to_integer(unsigned(to_01(w, 'X')));
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(to_01(w)) & " " & img(to_01(m, '1')) & " " &
               img(to_01(m)) & " " & img(to_01(w(5 downto 2), 'X'));
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

fn run_z(dir: &Path, flags: &[&str]) -> (Option<i32>, String) {
    let mut args = vec!["run", "--std=2008", "--work=work:work"];
    args.extend_from_slice(flags);
    args.push("z");
    let o = yavs(dir, &args);
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
    )
}

#[test]
fn to_01_in_compiled_code_matches_the_interpreter() {
    let dir = tempdir("compiled_to01");
    std::fs::write(dir.join("z.vhd"), TO01).expect("write");
    let a = yavs(
        &dir,
        &["analyze", "--std=2008", "--work=work:work", "z.vhd"],
    );
    assert!(a.status.success(), "{a:?}");
    let want = run_z(&dir, &["--tier=interp"]);
    assert_eq!(want.0, Some(0), "{want:?}");
    assert!(want.1.lines().count() >= 700, "{}", want.1);
    for flags in [
        #[cfg(feature = "clift")]
        vec!["--tier=clift"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
    ] {
        assert_eq!(run_z(&dir, &flags), want, "{flags:?}");
    }
}

/// Unary `-` of a `signed` of every length class (null, 1, 63 to 65, 70 elements), ascending and
/// descending with a lower bound other than zero, with and without metavalues, the most negative
/// value, in a function (native frame) and in a process (frame mode).
const NEG: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  function img(v : signed) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(std_logic_vector(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return signed is
    variable r : signed(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  function up(n : natural; k : natural; meta : boolean) return signed is
    variable r : signed(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    return r;
  end function;
  function minv(n : natural) return signed is
    variable r : signed(n - 1 downto 0) := (others => '0');
  begin
    if n > 0 then r(n - 1) := '1'; end if;
    return r;
  end function;
  function neg(v : signed) return signed is begin return -v; end function;
begin
  process
    constant lens : integer_vector := (0, 1, 2, 7, 8, 9, 31, 32, 33, 63, 64, 65, 70);
    variable n : integer := 0;
    variable w : signed(15 downto 0) := x"8001";
    variable m : signed(3 downto 0) := "10X1";
  begin
    for i in lens'range loop
      for k in 1 to 3 loop
        for meta in boolean'range loop
          report integer'image(lens(i)) & "/" & integer'image(k) & "/" & boolean'image(meta) &
                 ": " & img(neg(down(lens(i), k, meta))) & " " & img(neg(up(lens(i), k, meta))) &
                 " " & img(neg(neg(down(lens(i), k, meta)))) & " " & img(neg(minv(lens(i)))) &
                 " " & img(-minv(lens(i)));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      w := -w + k;
      n := n + to_integer(-w);
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(-w) & " " & img(-m) & " " & img(-w(5 downto 2));
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

/// Runs `src` (entity `y`) on every compiled tier of this build and compares with `--tier=interp`.
fn all_tiers_match(name: &str, src: &str) {
    tiers_match(name, src, 70);
}

/// `all_tiers_match` for a run of at least `min` lines of output.
fn tiers_match(name: &str, src: &str, min: usize) {
    let dir = tempdir(name);
    std::fs::write(dir.join("y.vhd"), src).expect("write");
    let a = yavs(
        &dir,
        &["analyze", "--std=2008", "--work=work:work", "y.vhd"],
    );
    assert!(a.status.success(), "{a:?}");
    let go = |flags: &[&str]| {
        let mut args = vec!["run", "--std=2008", "--work=work:work"];
        args.extend_from_slice(flags);
        args.push("y");
        let o = yavs(&dir, &args);
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    };
    let want = go(&["--tier=interp"]);
    assert_eq!(want.0, Some(0), "{want:?}");
    assert!(want.1.lines().count() >= min, "{}", want.1);
    for flags in [
        #[cfg(feature = "clift")]
        vec!["--tier=clift"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
    ] {
        assert_eq!(go(&flags), want, "{flags:?}");
    }
}

/// Whole reads of records of logic vectors, which compiled code reads leaf by leaf in place: a
/// record pack read through an `in` port (a leaf that starts inside a plane word), a leaf that
/// crosses a plane word (the general path), leaves of 1 and 64 elements, metavalues, a read
/// after a drive in the same activation, a forced and released record; before and after the
/// review lets the record images go.
const RECORD_READS: &str = r#"library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
package rp is
  type pair is record
    re, im : signed(17 downto 0);
  end record;
  type wide is record
    a, b : std_logic_vector(39 downto 0);
  end record;
  type edge is record
    x : std_logic_vector(0 downto 0);
    y : unsigned(63 downto 0);
  end record;
end;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use work.rp.all;
entity sub is
  port (i : in pair; o : out pair);
end;
architecture a of sub is
begin
  process (i) is
    variable v : pair;
  begin
    v := i;
    o <= (re => v.im, im => v.re);
  end process;
end;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use work.rp.all;
entity y is
end;
architecture a of y is
  signal p0, p1, p2 : pair := (to_signed(3, 18), to_signed(-4, 18));
  signal q : pair;
  signal w0, w1 : wide := (others => (others => '0'));
  signal e0, e1 : edge := ("0", (others => '1'));
  function h (p : pair) return string is
  begin
    return to_hstring(p.re) & ":" & to_hstring(p.im);
  end;
begin
  u : entity work.sub port map (i => p1, o => q);
  process is
    variable v, s : pair;
    variable w : wide;
    variable e : edge;
  begin
    for k in 1 to 1300 loop
      wait for 1 ns;
      v := p0;
      p0 <= (re => v.im + k, im => v.re - 1);
      p1 <= p0;
      -- Read again after the drive: still the old value.
      s := p0;
      p2 <= s;
      w := w0;
      w1 <= w;
      w0 <= (a => std_logic_vector(to_unsigned(k * 7, 36)) & "XZUW", b => std_logic_vector(to_unsigned(k * 13 + 2 ** 20, 40)));
      e := e0;
      e1 <= e;
      e0 <= (x => std_logic_vector(to_unsigned(k mod 2, 1)), y => e.y - k);
      if k = 30 or k = 1250 then
        p2 <= force (to_signed(7, 18), to_signed(8, 18));
      elsif k = 40 or k = 1260 then
        p2 <= release;
      end if;
      -- Past the review of the record images (1 024 copies) the reads are in place.
      if k mod 13 = 0 or k > 1240 then
        report integer'image(k) & " " & h(v) & " " & h(s) & " " & h(p1) & " " & h(p2) & " " & h(q) & " "
        & to_hstring(w.b) & " " & to_string(w1.a) & " " & to_hstring(e.x) & to_hstring(e.y) & " "
        & to_hstring(e1.y);
      end if;
    end loop;
    report "done";
    wait;
  end process;
end;
"#;

#[test]
fn whole_record_reads_in_compiled_code_match_the_interpreter() {
    all_tiers_match("compiled_record_reads", RECORD_READS);
}

#[test]
fn unary_minus_in_compiled_code_matches_the_interpreter() {
    all_tiers_match("compiled_neg", NEG);
}

/// `abs` of a `signed`: the same operands as [`NEG`], in a function and in a process, with a
/// non-negative, a negative and the most negative operand at every length class.
const ABS: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  function img(v : signed) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(std_logic_vector(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean; top : std_logic) return signed is
    variable r : signed(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    if n > 0 and not meta then r(n + 4) := top; end if;
    return r;
  end function;
  function up(n : natural; k : natural; meta : boolean; top : std_logic) return signed is
    variable r : signed(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    if n > 0 and not meta then r(3) := top; end if;
    return r;
  end function;
  function minv(n : natural) return signed is
    variable r : signed(n - 1 downto 0) := (others => '0');
  begin
    if n > 0 then r(n - 1) := '1'; end if;
    return r;
  end function;
  function ab(v : signed) return signed is begin return abs v; end function;
begin
  process
    constant lens : integer_vector := (0, 1, 2, 7, 8, 9, 31, 32, 33, 63, 64, 65, 70);
    variable n : integer := 0;
    variable w : signed(15 downto 0) := x"8001";
    variable m : signed(3 downto 0) := "10X1";
  begin
    for i in lens'range loop
      for k in 1 to 3 loop
        for meta in boolean'range loop
          report integer'image(lens(i)) & "/" & integer'image(k) & "/" & boolean'image(meta) &
                 ": " & img(ab(down(lens(i), k, meta, '0'))) & " " & img(ab(down(lens(i), k, meta, '1'))) &
                 " " & img(ab(up(lens(i), k, meta, '0'))) & " " & img(ab(up(lens(i), k, meta, '1'))) &
                 " " & img(ab(ab(down(lens(i), k, meta, '1')))) & " " & img(ab(minv(lens(i)))) &
                 " " & img(abs minv(lens(i)));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      w := -w + k;
      n := n + to_integer(abs w);
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(abs w) & " " & img(abs m) & " " & img(abs w(5 downto 2));
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn abs_in_compiled_code_matches_the_interpreter() {
    all_tiers_match("compiled_abs", ABS);
}

/// `*` of two vectors, `unsigned` and `signed`, around the 64 element limit of the product
/// (`l + r`): in a function and in a process, ascending and descending operands with non-zero
/// lower bounds, extreme values, metavalues, null operands and a counted loop.
const MUL: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  function img(v : unsigned) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(std_logic_vector(v));
  end function;
  function img(v : signed) return string is
  begin
    return img(unsigned(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  function up(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    return r;
  end function;
  function ext(n : natural; top : std_logic; rest : std_logic) return unsigned is
    variable r : unsigned(n - 1 downto 0) := (others => rest);
  begin
    if n > 0 then r(n - 1) := top; end if;
    return r;
  end function;
  function mu(x, y : unsigned) return unsigned is begin return x * y; end function;
  function ms(x, y : signed) return signed is begin return x * y; end function;
begin
  process
    constant la : integer_vector := (1, 1, 2, 7, 8, 32, 33, 31, 32, 63, 64, 1, 0, 5, 3, 40, 9);
    constant lb : integer_vector := (1, 63, 2, 8, 9, 32, 31, 33, 33, 1, 1, 64, 5, 0, 4, 40, 70);
    variable n : integer := 0;
    variable w : signed(15 downto 0) := x"8001";
    variable u : unsigned(11 downto 0) := x"801";
    variable p : signed(15 downto 0);
    variable q : unsigned(15 downto 0);
  begin
    for i in la'range loop
      for k in 1 to 3 loop
        for meta in boolean'range loop
          report integer'image(la(i)) & "*" & integer'image(lb(i)) & "/" & integer'image(k) & "/" &
                 boolean'image(meta) & ": " &
                 img(mu(down(la(i), k, meta), up(lb(i), k + 1, meta))) & " " &
                 img(mu(up(la(i), k, meta), down(lb(i), k + 1, meta))) & " " &
                 img(ms(signed(down(la(i), k, meta)), signed(up(lb(i), k + 1, meta)))) & " " &
                 img(ms(signed(up(la(i), k, meta)), signed(down(lb(i), k + 1, meta)))) & " " &
                 img(ms(signed(ext(la(i), '1', '0')), signed(ext(lb(i), '1', '1')))) & " " &
                 img(ms(signed(ext(la(i), '1', '0')), signed(ext(lb(i), '1', '0')))) & " " &
                 img(ms(signed(ext(la(i), '1', '1')), signed(ext(lb(i), '1', '1')))) & " " &
                 img(mu(ext(la(i), '1', '1'), ext(lb(i), '1', '1')));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      p := w(7 downto 0) * to_signed(k mod 100 - 50, 8);
      q := u(7 downto 0) * to_unsigned(k mod 200, 8);
      n := n + to_integer(p) + to_integer(q);
      w := p + k;
      u := q(11 downto 0) + k;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(p) & " " & img(q) & " " & img(w(5 downto 2) * w(11 downto 8));
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn mul_in_compiled_code_matches_the_interpreter() {
    all_tiers_match("compiled_mul", MUL);
}

/// `*` of a vector and an integer, both orders, `unsigned` (`natural`) and `signed`: the integer
/// is converted at the vector's length, so it fits, just fits or just does not (the truncation
/// warning comes from the callback), zero, the most negative value, lengths around 32 (the product
/// is `2 n` elements), metavalues, weak bits, null, non-zero lower bounds in both directions, in
/// functions, in a process and in a counted loop.
const MULINT: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  function img(v : unsigned) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(std_logic_vector(v));
  end function;
  function img(v : signed) return string is
  begin
    return img(unsigned(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  function up(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    return r;
  end function;
  -- Operands for a vector of n elements: they fit, just fit or just do not.
  function ui(n : natural; k : natural) return natural is
  begin
    case k mod 8 is
      when 0 => return 0;
      when 1 => return 1;
      when 2 => return 255;
      when 3 => return integer'high;
      when 4 => if n < 31 then return 2 ** n - 1; else return 12345; end if;
      when 5 => if n < 31 then return 2 ** n; else return 99; end if;
      when 6 => if n < 31 then return 2 ** n + 1; else return 7; end if;
      when others => return 65535;
    end case;
  end function;
  function si(n : natural; k : natural) return integer is
  begin
    case k mod 10 is
      when 0 => return 0;
      when 1 => return -1;
      when 2 => return integer'low;
      when 3 => return integer'high;
      when 4 => if n < 32 and n > 0 then return 2 ** (n - 1) - 1; else return 77; end if;
      when 5 => if n < 32 and n > 0 then return 2 ** (n - 1); else return 78; end if;
      when 6 => if n < 32 and n > 0 then return -(2 ** (n - 1)); else return -79; end if;
      when 7 => if n < 32 and n > 0 then return -(2 ** (n - 1)) - 1; else return -80; end if;
      when 8 => return -1000;
      when others => return 1000;
    end case;
  end function;
  function ux(x : unsigned; i : natural) return unsigned is begin return x * i; end function;
  function xu(i : natural; x : unsigned) return unsigned is begin return i * x; end function;
  function sx(x : signed; i : integer) return signed is begin return x * i; end function;
  function xs(i : integer; x : signed) return signed is begin return i * x; end function;
begin
  process
    constant la : integer_vector := (1, 2, 31, 32, 33, 64, 0, 5, 16, 30, 3);
    variable n : integer := 0;
    variable w : signed(15 downto 0) := x"8001";
    variable u : unsigned(11 downto 0) := x"801";
    variable p : signed(15 downto 0);
    variable q : unsigned(15 downto 0);
    variable z : unsigned(7 downto 0);
  begin
    for i in la'range loop
      for k in 0 to 9 loop
        for meta in boolean'range loop
          report integer'image(la(i)) & "/" & integer'image(k) & "/" & boolean'image(meta) & ": " &
                 img(ux(down(la(i), k, meta), ui(la(i), k))) & " " &
                 img(xu(ui(la(i), k), up(la(i), k + 1, meta))) & " " &
                 img(sx(signed(down(la(i), k + 2, meta)), si(la(i), k))) & " " &
                 img(xs(si(la(i), k), signed(up(la(i), k + 3, meta)))) & " " &
                 img(down(la(i), k, meta) * ui(la(i), k + 1)) & " " &
                 img(ui(la(i), k + 2) * up(la(i), k, meta)) & " " &
                 img(signed(up(la(i), k, meta)) * si(la(i), k + 1)) & " " &
                 img(si(la(i), k + 2) * signed(down(la(i), k, meta)));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      p := w(7 downto 0) * (k mod 100 - 50);
      q := u(7 downto 0) * (k mod 200);
      z := u(3 downto 0) * (k mod 40);
      n := n + to_integer(p) + to_integer(q) + to_integer(z) + to_integer((k - 150) * w(14 downto 8));
      w := p + k;
      u := q(11 downto 0) + k;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(p) & " " & img(q) & " " & img(z) & " " & img(w(5 downto 2) * 3);
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn mul_integer_in_compiled_code_matches_the_interpreter() {
    all_tiers_match("compiled_mul_integer", MULINT);
}

/// The matching operators `?<` ... `?/=`, `maximum`, `minimum` and `std_match`, vectors and
/// integers, `unsigned` and `signed`: lengths around 64 and unequal, metavalues, `-`, weak bits,
/// null, integers that fit, just fit and just do not, non-zero lower bounds in both directions,
/// in functions, in a process and in a counted loop.
const MATCHES: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  function img(v : unsigned) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(std_logic_vector(v));
  end function;
  function img(v : signed) return string is
  begin
    return img(unsigned(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  function up(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    return r;
  end function;
  -- Operands for a vector of n elements: they fit, just fit or just do not.
  function ui(n : natural; k : natural) return natural is
  begin
    case k mod 8 is
      when 0 => return 0;
      when 1 => return 1;
      when 2 => return 255;
      when 3 => return integer'high;
      when 4 => if n < 31 then return 2 ** n - 1; else return 12345; end if;
      when 5 => if n < 31 then return 2 ** n; else return 99; end if;
      when 6 => if n < 31 then return 2 ** n + 1; else return 7; end if;
      when others => return 65535;
    end case;
  end function;
  function si(n : natural; k : natural) return integer is
  begin
    case k mod 10 is
      when 0 => return 0;
      when 1 => return -1;
      when 2 => return integer'low;
      when 3 => return integer'high;
      when 4 => if n < 32 and n > 0 then return 2 ** (n - 1) - 1; else return 77; end if;
      when 5 => if n < 32 and n > 0 then return 2 ** (n - 1); else return 78; end if;
      when 6 => if n < 32 and n > 0 then return -(2 ** (n - 1)); else return -79; end if;
      when 7 => if n < 32 and n > 0 then return -(2 ** (n - 1)) - 1; else return -80; end if;
      when 8 => return -1000;
      when others => return 1000;
    end case;
  end function;
  -- The ordering relations report an error for a `-`, which ends the run with status 1.
  function dash(x : std_ulogic_vector) return boolean is
  begin
    for i in x'range loop
      if x(i) = '-' then return true; end if;
    end loop;
    return false;
  end function;
  function qs(x, y : unsigned) return string is
  begin
    if dash(std_ulogic_vector(x)) or dash(std_ulogic_vector(y)) then
      return std_ulogic'image(x ?= y) & std_ulogic'image(x ?/= y) &
             boolean'image(std_match(x, y)) & img(maximum(x, y)) & img(minimum(x, y));
    end if;
    return std_ulogic'image(x ?< y) & std_ulogic'image(x ?<= y) & std_ulogic'image(x ?> y) &
           std_ulogic'image(x ?>= y) & std_ulogic'image(x ?= y) & std_ulogic'image(x ?/= y) &
           boolean'image(std_match(x, y)) & boolean'image(std_match(std_logic_vector(x), std_logic_vector(y))) &
           img(maximum(x, y)) & img(minimum(x, y));
  end function;
  function qg(x, y : signed) return string is
  begin
    if dash(std_ulogic_vector(x)) or dash(std_ulogic_vector(y)) then
      return std_ulogic'image(x ?= y) & std_ulogic'image(x ?/= y) &
             boolean'image(std_match(x, y)) & img(maximum(x, y)) & img(minimum(x, y));
    end if;
    return std_ulogic'image(x ?< y) & std_ulogic'image(x ?<= y) & std_ulogic'image(x ?> y) &
           std_ulogic'image(x ?>= y) & std_ulogic'image(x ?= y) & std_ulogic'image(x ?/= y) &
           boolean'image(std_match(x, y)) &
           img(maximum(x, y)) & img(minimum(x, y));
  end function;
  function qu(x : unsigned; i : natural) return string is
  begin
    if dash(std_ulogic_vector(x)) then
      return std_ulogic'image(x ?= i) & std_ulogic'image(i ?/= x) &
             img(maximum(x, i)) & img(minimum(i, x));
    end if;
    return std_ulogic'image(x ?< i) & std_ulogic'image(i ?<= x) & std_ulogic'image(x ?> i) &
           std_ulogic'image(i ?>= x) & std_ulogic'image(x ?= i) & std_ulogic'image(i ?/= x) &
           img(maximum(x, i)) & img(minimum(i, x));
  end function;
  function qi(x : signed; i : integer) return string is
  begin
    if dash(std_ulogic_vector(x)) then
      return std_ulogic'image(x ?= i) & std_ulogic'image(i ?/= x) &
             img(maximum(x, i)) & img(minimum(i, x));
    end if;
    return std_ulogic'image(x ?< i) & std_ulogic'image(i ?<= x) & std_ulogic'image(x ?> i) &
           std_ulogic'image(i ?>= x) & std_ulogic'image(x ?= i) & std_ulogic'image(i ?/= x) &
           img(maximum(x, i)) & img(minimum(i, x));
  end function;
begin
  process
    constant la : integer_vector := (1, 1, 2, 7, 8, 32, 33, 63, 64, 64, 65, 1, 0, 5, 3, 70, 9);
    constant lb : integer_vector := (1, 64, 2, 8, 9, 32, 31, 64, 63, 64, 65, 64, 5, 0, 4, 3, 70);
    variable n : integer := 0;
    variable w : signed(15 downto 0) := x"8001";
    variable u : unsigned(11 downto 0) := x"801";
    variable p : signed(15 downto 0);
    variable q : unsigned(15 downto 0);
  begin
    for i in la'range loop
      for k in 1 to 3 loop
        for meta in boolean'range loop
          report integer'image(la(i)) & "/" & integer'image(lb(i)) & "/" & integer'image(k) & "/" &
                 boolean'image(meta) & ": " &
                 qs(down(la(i), k, meta), up(lb(i), k + 1, meta)) & " " &
                 qs(up(la(i), k, meta), down(lb(i), k + 1, meta)) & " " &
                 qg(signed(down(la(i), k, meta)), signed(up(lb(i), k + 1, meta))) & " " &
                 qg(signed(up(la(i), k, meta)), signed(down(lb(i), k + 1, meta))) & " " &
                 qu(down(la(i), k + 2, meta), ui(la(i), k)) & " " &
                 qu(up(la(i), k, meta), ui(la(i), k + 1)) & " " &
                 qi(signed(down(la(i), k, meta)), si(la(i), k)) & " " &
                 qi(signed(up(la(i), k + 3, meta)), si(la(i), k + 1));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      p := maximum(w(7 downto 0) * to_signed(k mod 100 - 50, 8), minimum(w, k - 150));
      q := minimum(u(7 downto 0) * to_unsigned(k mod 200, 8), maximum(u, k));
      if (p ?< w) = '1' then n := n + 1; end if;
      if (q ?>= u) = '1' then n := n + 2; end if;
      if (p ?= k - 100) = '1' then n := n + 4; end if;
      if std_match(q, u & "0000") then n := n + 8; end if;
      n := n + to_integer(p) + to_integer(q);
      w := p + k;
      u := q(11 downto 0) + k;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(p) & " " & img(q) & " " & qs(q, u & "0000");
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn matching_operators_in_compiled_code_match_the_interpreter() {
    all_tiers_match("compiled_match", MATCHES);
}

/// `*` of two vectors with 65 to 1025 result elements, `unsigned` and `signed` (the word-wise
/// native, reached through the callback of compiled code), over the limit of 1024, extremes, weak
/// bits, metavalues, null, non-zero lower bounds in both directions, and a counted loop.
#[test]
fn wide_mul_in_compiled_code_matches_the_interpreter() {
    let head = MUL.split_once("begin\n  process").expect("head").0;
    let src = format!(
        "{head}{}",
        r#"begin
  process
    constant la : integer_vector := (33, 33, 32, 64, 65, 1, 100, 129, 512, 513, 2, 0, 5);
    constant lb : integer_vector := (32, 33, 33, 64, 1, 65, 28, 5, 512, 512, 1025, 70, 0);
    variable n : integer := 0;
    variable w : signed(39 downto 0) := x"8000000001";
    variable u : unsigned(39 downto 0) := x"8000000001";
    variable p : signed(79 downto 0);
    variable q : unsigned(79 downto 0);
  begin
    for i in la'range loop
      for k in 1 to 3 loop
        for meta in boolean'range loop
          report integer'image(la(i)) & "*" & integer'image(lb(i)) & "/" & integer'image(k) & "/" &
                 boolean'image(meta) & ": " &
                 img(mu(down(la(i), k, meta), up(lb(i), k + 1, meta))) & " " &
                 img(mu(up(la(i), k, meta), down(lb(i), k + 1, meta))) & " " &
                 img(ms(signed(down(la(i), k, meta)), signed(up(lb(i), k + 1, meta)))) & " " &
                 img(ms(signed(ext(la(i), '1', '0')), signed(ext(lb(i), '1', '1')))) & " " &
                 img(ms(signed(ext(la(i), '1', '1')), signed(ext(lb(i), '1', '1')))) & " " &
                 img(ms(signed(ext(la(i), '1', '0')), signed(ext(lb(i), '1', '0')))) & " " &
                 img(mu(ext(la(i), '1', '1'), ext(lb(i), '1', '1')));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      p := w * to_signed(k - 150, 40);
      q := u * to_unsigned(k * 1000003, 40);
      n := (n + to_integer(p(20 downto 0)) + to_integer(q(20 downto 0))) mod 1000003;
      w := p(60 downto 21) + k;
      u := q(70 downto 31) + k;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(p) & " " & img(q);
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#
    );
    all_tiers_match("compiled_wide_mul", &src);
}

/// `vector * integer` and `integer * vector` where the product has over 64 elements: the integer
/// fits, just fits or just does not (the truncation warning), lengths 33 to 64 (the integer is
/// converted at the vector's length) and over 64.
#[test]
fn wide_mul_integer_in_compiled_code_matches_the_interpreter() {
    let (head, tail) = MULINT.split_once("begin\n  process").expect("head");
    let body = tail
        .split_once("    for k in 0 to 300 loop")
        .expect("loop")
        .0;
    let body = body.replace(
        "(1, 2, 31, 32, 33, 64, 0, 5, 16, 30, 3)",
        "(33, 34, 40, 64, 65, 66, 100, 128, 513, 3)",
    );
    let src = format!(
        "{head}begin\n  process{body}{}",
        r#"    for k in 0 to 300 loop
      q := u * (k mod 200);
      p := w * (k mod 100 - 50);
      n := (n + to_integer(q(20 downto 0)) + to_integer(p(20 downto 0)) + to_integer(w(14 downto 8) * (k mod 50))) mod 1000003;
      u := q(39 downto 0) + k;
      w := p(39 downto 0) + k;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(q) & " " & img(p);
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#
    );
    let src = src
        .replace(
            "signed(15 downto 0) := x\"8001\"",
            "signed(39 downto 0) := x\"0000008001\"",
        )
        .replace(
            "unsigned(11 downto 0) := x\"801\"",
            "unsigned(39 downto 0) := x\"0000000801\"",
        )
        .replace(
            "variable p : signed(15 downto 0)",
            "variable p : signed(79 downto 0)",
        )
        .replace(
            "variable q : unsigned(15 downto 0)",
            "variable q : unsigned(79 downto 0)",
        );
    all_tiers_match("compiled_wide_mul_integer", &src);
}

/// `rotate_left`, `rotate_right`, `rol` and `ror` of 65 to 1025 elements by 0, 1, a word, `n - 1`,
/// `n`, more than `n`, the largest NATURAL and (for the operators) negative counts, weak bits,
/// metavalues, non-zero lower bounds in both directions, and a counted loop.
#[test]
fn wide_rotate_in_compiled_code_matches_the_interpreter() {
    let head = MUL.split_once("begin\n  process").expect("head").0;
    let src = format!(
        "{head}{}",
        r#"begin
  process
    constant la : integer_vector := (65, 66, 100, 128, 129, 192, 513, 1024, 1025, 64, 3);
    constant cs : integer_vector := (0, 1, 2, 63, 64, 65, 127, 1000, 2147483647);
    variable n : integer := 0;
    variable r : unsigned(99 downto 0) := (0 => '1', 70 => '1', others => '0');
    variable g : signed(129 downto 0) := (0 => '1', 129 => '1', others => '0');
  begin
    for i in la'range loop
      for j in cs'range loop
        for meta in boolean'range loop
          report integer'image(la(i)) & "/" & integer'image(cs(j)) & "/" & boolean'image(meta) & ": " &
                 img(rotate_left(down(la(i), j, meta), cs(j))) & " " &
                 img(rotate_right(up(la(i), j + 1, meta), cs(j))) & " " &
                 img(rotate_left(signed(down(la(i), j, false)), la(i) - 1)) & " " &
                 img(rotate_right(signed(up(la(i), j, false)), la(i))) & " " &
                 img(rotate_left(down(la(i), j + 2, false), la(i) + 1)) & " " &
                 img(down(la(i), j, false) rol (-cs(j))) & " " &
                 img(up(la(i), j, false) ror cs(j)) & " " &
                 img(signed(up(la(i), j, false)) rol (-cs(j))) & " " &
                 img(signed(down(la(i), j, false)) ror (-cs(j)));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      r := rotate_left(r, k mod 7 + 1) + k;
      g := rotate_right(g, k mod 70) - k;
      n := (n + to_integer(r(20 downto 0)) + to_integer(g(20 downto 0))) mod 1000003;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(r) & " " & img(g);
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#
    );
    all_tiers_match("compiled_wide_rotate", &src);
}

/// `/`, `rem` and `mod` of two vectors, `unsigned` and `signed`: lengths around and above the 64
/// element limit, every sign combination and the most negative value, weak bits, metavalues, null,
/// non-zero lower bounds in both directions, in functions, in a process and in a counted loop.
const DIV: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  function img(v : unsigned) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(std_logic_vector(v));
  end function;
  function img(v : signed) return string is
  begin
    return img(unsigned(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  -- A divisor: never zero (the body's error ends the run).
  function up(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    if n > 0 then r(n + 2) := '1'; end if;
    return r;
  end function;
  function ext(n : natural; top : std_logic; rest : std_logic) return unsigned is
    variable r : unsigned(n - 1 downto 0) := (others => rest);
  begin
    if n > 0 then r(n - 1) := top; end if;
    return r;
  end function;
  -- A divisor: never zero.
  function dext(n : natural; top : std_logic; rest : std_logic) return unsigned is
    variable r : unsigned(n - 1 downto 0) := ext(n, top, rest);
  begin
    if n > 0 then r(0) := '1'; end if;
    return r;
  end function;
  function du(x, y : unsigned) return string is
  begin
    return img(x / y) & " " & img(x rem y) & " " & img(x mod y);
  end function;
  function ds(x, y : signed) return string is
  begin
    return img(x / y) & " " & img(x rem y) & " " & img(x mod y);
  end function;
begin
  process
    constant la : integer_vector := (1, 1, 2, 7, 8, 32, 33, 31, 63, 64, 64, 1, 65, 1, 65, 70, 3, 128, 0, 5, 3);
    constant lb : integer_vector := (1, 63, 2, 8, 9, 32, 31, 33, 64, 64, 1, 64, 1, 65, 65, 3, 70, 128, 5, 0, 4);
    variable n : integer := 0;
    variable w : signed(15 downto 0) := x"8001";
    variable u : unsigned(11 downto 0) := x"801";
    variable p : signed(15 downto 0);
    variable q : unsigned(15 downto 0);
  begin
    for i in la'range loop
      for k in 1 to 3 loop
        for meta in boolean'range loop
          report integer'image(la(i)) & "/" & integer'image(lb(i)) & "/" & integer'image(k) & "/" &
                 boolean'image(meta) & ": " &
                 du(down(la(i), k, meta), up(lb(i), k + 1, meta)) & " " &
                 du(up(la(i), k, meta), up(lb(i), k + 1, meta)) & " " &
                 ds(signed(down(la(i), k, meta)), signed(up(lb(i), k + 1, meta))) & " " &
                 ds(signed(up(la(i), k, meta)), signed(up(lb(i), k + 1, meta))) & " " &
                 ds(signed(ext(la(i), '1', '0')), signed(dext(lb(i), '1', '1'))) & " " &
                 ds(signed(ext(la(i), '1', '0')), signed(dext(lb(i), '0', '1'))) & " " &
                 ds(signed(ext(la(i), '0', '1')), signed(dext(lb(i), '1', '1'))) & " " &
                 ds(signed(ext(la(i), '1', '1')), signed(dext(lb(i), '1', '0'))) & " " &
                 du(ext(la(i), '1', '1'), dext(lb(i), '1', '1')) & " " &
                 du(ext(la(i), '0', '0'), dext(lb(i), '0', '1'));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      p := (w(11 downto 4) & w(7 downto 0)) / to_signed(k mod 100 - 50 + 51, 8) + w(15 downto 0) rem to_signed(k mod 61 + 1, 8);
      q := resize((u(7 downto 0) & u(7 downto 0)) mod to_unsigned(k mod 200 + 1, 8), 16);
      n := n + to_integer(p) + to_integer(q);
      w := p + k + (w mod to_signed(k mod 7 + 1, 4));
      u := q(11 downto 0) + k;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(p) & " " & img(q) & " " & ds(w(11 downto 4), w(5 downto 2) & "1");
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn div_in_compiled_code_matches_the_interpreter() {
    all_tiers_match("compiled_div", DIV);
}

/// `/`, `rem` and `mod` of a vector and an integer, both orders: the integer fits, just fits or
/// just does not (the body's results and warnings come from the callback), the most negative value,
/// lengths around and above 64, weak bits, metavalues, null, non-zero lower bounds, loops.
const DIVINT: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  function img(v : unsigned) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(std_logic_vector(v));
  end function;
  function img(v : signed) return string is
  begin
    return img(unsigned(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  -- A divisor: never zero (the body's error ends the run).
  function up(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    if n > 0 then r(n + 2) := '1'; end if;
    return r;
  end function;
  -- Integers for a vector of n elements: they fit, just fit or just do not; never zero.
  function ui(n : natural; k : natural) return natural is
  begin
    case k mod 8 is
      when 0 => return 3;
      when 1 => return 1;
      when 2 => return 255;
      when 3 => return integer'high;
      when 4 => if n < 31 and n > 0 then return 2 ** n - 1; else return 12345; end if;
      when 5 => if n < 31 then return 2 ** n; else return 99; end if;
      when 6 => if n < 31 then return 2 ** n + 1; else return 7; end if;
      when others => return 65535;
    end case;
  end function;
  function si(n : natural; k : natural) return integer is
  begin
    case k mod 10 is
      when 0 => return 3;
      when 1 => return -1;
      when 2 => return integer'low;
      when 3 => return integer'high;
      when 4 => if n < 32 and n > 1 then return 2 ** (n - 1) - 1; else return 77; end if;
      when 5 => if n < 32 and n > 0 then return 2 ** (n - 1); else return 78; end if;
      when 6 => if n < 32 and n > 0 then return -(2 ** (n - 1)); else return -79; end if;
      when 7 => if n < 32 and n > 0 then return -(2 ** (n - 1)) - 1; else return -80; end if;
      when 8 => return -1000;
      when others => return 1000;
    end case;
  end function;
  function vi(x : unsigned; i : natural) return string is
  begin
    return img(x / i) & " " & img(x rem i) & " " & img(x mod i);
  end function;
  -- `natural rem unsigned` is a length error (the body has no RESIZE) for an integer wider than x.
  function iv(i : natural; x : unsigned) return string is
  begin
    if x'length >= 31 or i < 2 ** x'length then
      return img(i / x) & " " & img(i rem x) & " " & img(i mod x);
    end if;
    return img(i / x) & " " & img(i mod x);
  end function;
  function vs(x : signed; i : integer) return string is
  begin
    return img(x / i) & " " & img(x rem i) & " " & img(x mod i);
  end function;
  function sv(i : integer; x : signed) return string is
  begin
    return img(i / x) & " " & img(i rem x) & " " & img(i mod x);
  end function;
begin
  process
    constant la : integer_vector := (1, 2, 31, 32, 33, 64, 65, 70, 128, 0, 5, 16, 30, 3);
    variable n : integer := 0;
    variable w : signed(15 downto 0) := x"8001";
    variable u : unsigned(11 downto 0) := x"801";
    variable p : signed(15 downto 0);
    variable q : unsigned(15 downto 0);
    variable z : unsigned(7 downto 0);
  begin
    for i in la'range loop
      for k in 0 to 9 loop
        for meta in boolean'range loop
          report integer'image(la(i)) & "/" & integer'image(k) & "/" & boolean'image(meta) & ": " &
                 vi(down(la(i), k, meta), ui(la(i), k)) & " " &
                 iv(ui(la(i), k), up(la(i), k + 1, meta)) & " " &
                 vs(signed(down(la(i), k + 2, meta)), si(la(i), k)) & " " &
                 sv(si(la(i), k), signed(up(la(i), k + 3, meta))) & " " &
                 vi(up(la(i), k, meta), ui(la(i), k + 1)) & " " &
                 iv(ui(la(i), k + 2), up(la(i), k, meta)) & " " &
                 vs(signed(up(la(i), k, meta)), si(la(i), k + 1)) & " " &
                 sv(si(la(i), k + 2), signed(up(la(i), k + 4, meta)));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      p := w(7 downto 0) / (k mod 100 - 50 + 51) + (w mod (k mod 9 + 1)) + ((k - 150) rem (w(14 downto 8) & "1"));
      q := resize(u(7 downto 0) mod (k mod 200 + 1), 16) + ((k mod 77) / ("1" & u(5 downto 0)));
      z := resize(u(3 downto 0) rem (k mod 40 + 1), 8);
      n := n + to_integer(p) + to_integer(q) + to_integer(z);
      w := p + k;
      u := q(11 downto 0) + k;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(p) & " " & img(q) & " " & img(z) & " " & img(w(5 downto 2) mod 3);
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn div_integer_in_compiled_code_matches_the_interpreter() {
    all_tiers_match("compiled_div_integer", DIVINT);
}

/// `/`, `rem` and `mod` of 4 and 8 element vectors and integers around and beyond their range
/// (`x mod 16` of a 4 element `unsigned`), both orders, every value of the 4 element vectors: the
/// integer wider than the vector, whose results and truncation warnings are the body's.
const DIVSMALL: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  type ints_t is array (natural range <>) of integer;
  constant ints : ints_t := (-1000, -33, -17, -9, -8, -5, -4, -3, -2, -1, 1, 2, 3, 4, 5, 7, 8, 9, 15,
                             16, 17, 31, 32, 100, 255, 256, 1000);
  function img(v : unsigned) return string is
  begin
    return to_string(std_logic_vector(v));
  end function;
  function img(v : signed) return string is
  begin
    return to_string(std_logic_vector(v));
  end function;
  function vi(x : unsigned; i : natural) return string is
  begin
    return img(x / i) & " " & img(x rem i) & " " & img(x mod i);
  end function;
  -- `natural rem unsigned` is a length error for an integer wider than x (the body has no RESIZE).
  function iv(i : natural; x : unsigned) return string is
  begin
    if x = 0 then return "-"; end if;
    if i < 2 ** x'length then
      return img(i / x) & " " & img(i rem x) & " " & img(i mod x);
    end if;
    return img(i / x) & " " & img(i mod x);
  end function;
  function vs(x : signed; i : integer) return string is
  begin
    return img(x / i) & " " & img(x rem i) & " " & img(x mod i);
  end function;
  function sv(i : integer; x : signed) return string is
  begin
    if x = 0 then return "-"; end if;
    return img(i / x) & " " & img(i rem x) & " " & img(i mod x);
  end function;
begin
  process
    variable n : integer := 0;
  begin
    for v in 0 to 15 loop
      for t in ints'range loop
        n := n + t;
        report integer'image(v) & "," & integer'image(ints(t)) & ": " &
               vs(to_signed(v - 8, 4), ints(t)) & " | " & sv(ints(t), to_signed(v - 8, 4)) & " | " &
               vs(to_signed(v * 17 - 128, 8), ints(t)) & " | " & sv(ints(t), to_signed(v * 17 - 128, 8)) &
               " | " & vi(to_unsigned(v, 4), abs ints(t)) & " | " & iv(abs ints(t), to_unsigned(v, 4)) &
               " | " & vi(to_unsigned(v * 17, 8), abs ints(t)) & " | " & iv(abs ints(t), to_unsigned(v * 17, 8));
      end loop;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn div_small_in_compiled_code_matches_the_interpreter() {
    all_tiers_match("compiled_div_small", DIVSMALL);
}

/// A zero divisor after a compiled loop is the body's error report and the run ends there, in
/// every tier alike (a vector, the integer 0, a vector of weak zeros, 16 and 70 elements).
const DIVZERO: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  function zero(n : natural; c : std_logic) return unsigned is
    variable r : unsigned(n - 1 downto 0) := (others => c);
  begin
    return r;
  end function;
  function dv(x : unsigned; k : natural; c : std_logic) return unsigned is
  begin
    case k mod 3 is
      when 0 => return x / zero(x'length, c);
      when 1 => return x rem zero(x'length, c);
      when others => return x mod zero(x'length, c);
    end case;
  end function;
  function dsv(x : signed; k : natural; c : std_logic) return signed is
  begin
    case k mod 3 is
      when 0 => return x / signed(zero(x'length, c));
      when 1 => return x rem signed(zero(x'length, c));
      when others => return x mod signed(zero(x'length, c));
    end case;
  end function;
  function di(x : unsigned; k : natural; c : integer) return unsigned is
  begin
    case k mod 3 is
      when 0 => return x / c;
      when 1 => return x rem c;
      when others => return x mod c;
    end case;
  end function;
begin
  process
    variable x : unsigned(15 downto 0) := x"8001";
    variable n : integer := 0;
    variable stop : boolean := false;
  begin
    for k in 0 to 120 loop
      x := dv(x, k, '1') + k;
      n := n + to_integer(x(7 downto 0));
      if k mod 40 = 0 then report integer'image(n); end if;
    end loop;
    report "zero divisors " & integer'image(n);
    report @STMT@;
    report "not reached";
    wait;
  end process;
end;
"#;

/// Runs `src` (entity `y`, which ends in a fatal error) on every compiled tier and compares the
/// output, the error report and the exit status with `--tier=interp`.
fn all_tiers_fail(name: &str, src: &str) {
    let dir = tempdir(name);
    std::fs::write(dir.join("y.vhd"), src).expect("write");
    let a = yavs(
        &dir,
        &["analyze", "--std=2008", "--work=work:work", "y.vhd"],
    );
    assert!(a.status.success(), "{a:?}");
    let go = |flags: &[&str]| {
        let mut args = vec!["run", "--std=2008", "--work=work:work"];
        args.extend_from_slice(flags);
        args.push("y");
        let o = yavs(&dir, &args);
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let want = go(&["--tier=interp"]);
    assert_ne!(want.0, Some(0), "{want:?}");
    assert!(want.1.contains("zero divisors"), "{want:?}");
    assert!(
        want.1.contains("by zero") || want.2.contains("by zero"),
        "{want:?}"
    );
    for flags in [
        #[cfg(feature = "clift")]
        vec!["--tier=clift"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
    ] {
        assert_eq!(go(&flags), want, "{flags:?}");
    }
}

#[test]
fn div_by_zero_in_compiled_code_matches_the_interpreter() {
    for (i, stmt) in [
        "to_string(dv(x, 0, '0'))",
        "to_string(dv(x, 1, '0'))",
        "to_string(dv(x, 2, 'L'))",
        "to_string(dv(resize(x, 70), 0, '0'))",
        "to_string(dv(resize(x, 64), 2, '0'))",
        "to_string(dsv(signed(x), 0, '0'))",
        "to_string(dsv(signed(x), 1, 'L'))",
        "to_string(dsv(signed(resize(x, 1)), 2, '0'))",
        "to_string(di(x, 0, 0))",
        "to_string(di(x, 1, 0))",
        "to_string(di(x, 2, 0))",
    ]
    .iter()
    .enumerate()
    {
        all_tiers_fail(
            &format!("compiled_div_zero{i}"),
            &DIVZERO.replace("@STMT@", stmt),
        );
    }
}

/// The operators and conversions of the Synopsys packages (`std_logic_arith` with
/// `std_logic_unsigned`): every pair of operand kinds and result type, integers that the packages
/// truncate (the most negative value, 2 ** n, -1000), weak bits, metavalues (the body warns and
/// gives `X`s; the compiled guard hands those calls to the body), lengths around 32 and 64 (a
/// mixed pair and a product reach 65), bounds that do not start at zero in both directions.
const SYN: &str = r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.std_logic_arith.all;
use ieee.std_logic_unsigned.all;
entity y is end;
architecture a of y is
  function img(v : std_logic_vector) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(v);
  end function;
  function img(v : unsigned) return string is
  begin
    return img(std_logic_vector(v));
  end function;
  function img(v : signed) return string is
  begin
    return img(std_logic_vector(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  function up(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    return r;
  end function;
  function si(n : natural; k : natural) return integer is
  begin
    case k mod 10 is
      when 0 => return 0;
      when 1 => return -1;
      when 2 => return integer'low;
      when 3 => return integer'high;
      when 4 => if n < 31 then return 2 ** n - 1; else return 77; end if;
      when 5 => if n < 31 then return 2 ** n; else return 78; end if;
      when 6 => if n < 31 then return -(2 ** n); else return -79; end if;
      when 7 => if n < 31 then return -(2 ** n) - 1; else return -80; end if;
      when 8 => return -1000;
      when others => return 1000;
    end case;
  end function;
  -- Without INTEGER'LOW: the unsigned conversion of it fails in the body.
  function sj(n : natural; k : natural) return integer is
  begin
    if k mod 10 = 2 then return -2 ** 20; else return si(n, k); end if;
  end function;
  function ops(x, y : unsigned) return string is
    constant sx : signed(x'length - 1 downto 0) := signed(x);
    constant sy : signed(y'length - 1 downto 0) := signed(y);
  begin
    return img(unsigned'(x + y)) & " " & img(unsigned'(x - y)) & " " & img(signed'(sx + sy)) & " " &
           img(signed'(sx - sy)) & " " & img(signed'(x + sy)) & " " & img(signed'(sx - y)) & " " &
           img(signed'(sx + y)) & " " & img(signed'(x - sy));
  end function;
  function muls(x, y : unsigned) return string is
    constant sx : signed(x'length - 1 downto 0) := signed(x);
    constant sy : signed(y'length - 1 downto 0) := signed(y);
  begin
    return img(unsigned'(x * y)) & " " & img(signed'(sx * sy)) & " " & img(signed'(x * sy)) & " " &
           img(signed'(sx * y));
  end function;
  function cmps(x, y : unsigned) return string is
    constant sx : signed(x'length - 1 downto 0) := signed(x);
    constant sy : signed(y'length - 1 downto 0) := signed(y);
  begin
    return boolean'image(x < y) & boolean'image(x <= y) & boolean'image(x > y) &
           boolean'image(x >= y) & boolean'image(x = y) & boolean'image(x /= y) & " " &
           boolean'image(sx < sy) & boolean'image(sx <= sy) & boolean'image(sx > sy) &
           boolean'image(sx >= sy) & boolean'image(sx = sy) & boolean'image(sx /= sy) & " " &
           boolean'image(x < sy) & boolean'image(x >= sy) & boolean'image(x = sy) & " " &
           boolean'image(sx > y) & boolean'image(sx <= y) & boolean'image(sx /= y);
  end function;
  function intops(x : unsigned; i : integer) return string is
    constant sx : signed(x'length - 1 downto 0) := signed(x);
  begin
    return img(unsigned'(x + i)) & " " & img(unsigned'(i + x)) & " " & img(unsigned'(x - i)) & " " &
           img(unsigned'(i - x)) & " " & img(signed'(sx + i)) & " " & img(signed'(i + sx)) & " " &
           img(signed'(sx - i)) & " " & img(signed'(i - sx));
  end function;
  function intcmps(x : unsigned; i : integer) return string is
    constant sx : signed(x'length - 1 downto 0) := signed(x);
  begin
    return boolean'image(x < i) & boolean'image(x <= i) & boolean'image(x > i) &
           boolean'image(x >= i) & boolean'image(x = i) & boolean'image(x /= i) & " " &
           boolean'image(i < x) & boolean'image(i >= x) & boolean'image(i = x) & " " &
           boolean'image(sx < i) & boolean'image(sx <= i) & boolean'image(sx > i) &
           boolean'image(sx >= i) & boolean'image(sx = i) & boolean'image(sx /= i) & " " &
           boolean'image(i > sx) & boolean'image(i <= sx) & boolean'image(i /= sx);
  end function;
  function lvops(a, b : std_logic_vector; i : integer) return string is
  begin
    return img(a + b) & " " & img(a - b) & " " & img(a * b) & " " & img(a + i) & " " &
           img(i + a) & " " & img(a - i) & " " & img(i - a) & " " &
           boolean'image(a < b) & boolean'image(a >= b) & boolean'image(a = b) &
           boolean'image(a /= b) & boolean'image(a < i) & boolean'image(i <= a) &
           boolean'image(a = i) & boolean'image(i /= a);
  end function;
  function convs(x : unsigned; i : integer; n : natural; k : natural) return string is
    constant sx : signed(x'length - 1 downto 0) := signed(x);
    constant lx : std_logic_vector(x'length - 1 downto 0) := std_logic_vector(x);
    constant s : natural := 1 + (k * 7 + n) mod 70;
    constant t : natural := 1 + (k * 3) mod 5;
  begin
    return img(conv_unsigned(x, s)) & " " & img(conv_signed(x, s)) & " " &
           img(conv_unsigned(sx, s)) & " " & img(conv_signed(sx, s)) & " " &
           img(conv_std_logic_vector(x, s)) & " " & img(conv_std_logic_vector(sx, t)) & " " &
           img(conv_unsigned(i, s)) & " " & img(conv_signed(i, s)) & " " &
           img(conv_std_logic_vector(i, s)) & " " & img(conv_unsigned(i, t)) & " " &
           img(ext(lx, s)) & " " & img(sxt(lx, s)) & " " & img(ext(lx, t)) & " " & img(sxt(lx, t));
  end function;
  function shifts(x : unsigned; c : unsigned) return string is
    constant sx : signed(x'length - 1 downto 0) := signed(x);
    constant lx : std_logic_vector(x'length - 1 downto 0) := std_logic_vector(x);
    constant lc : std_logic_vector(c'length - 1 downto 0) := std_logic_vector(c);
  begin
    return img(shl(x, c)) & " " & img(shr(x, c)) & " " & img(shl(sx, c)) & " " & img(shr(sx, c)) & " " &
           img(shl(lx, lc)) & " " & img(shr(lx, lc));
  end function;
  function ints(x : unsigned) return string is
    constant sx : signed(x'length - 1 downto 0) := signed(x);
    constant lx : std_logic_vector(x'length - 1 downto 0) := std_logic_vector(x);
  begin
    return integer'image(conv_integer(x)) & " " & integer'image(conv_integer(sx)) & " " &
           integer'image(conv_integer(lx));
  end function;
begin
  process
    constant la : integer_vector := (1, 2, 31, 32, 33, 64, 5, 16, 30, 3, 63, 65);
    variable n : integer := 0;
    variable w : unsigned(15 downto 0) := x"8001";
    variable u : std_logic_vector(11 downto 0) := x"801";
    variable p : unsigned(15 downto 0);
    variable q : std_logic_vector(11 downto 0);
    variable c : unsigned(7 downto 0);
  begin
    for i in la'range loop
      for k in 0 to 9 loop
        for meta in boolean'range loop
          report integer'image(la(i)) & "/" & integer'image(k) & "/" & boolean'image(meta) & ": " &
                 ops(down(la(i), k, meta), up(la(i), k + 1, meta)) & " | " &
                 ops(up(la(i), k + 2, meta), down(1 + (la(i) + k) mod 33, k, meta)) & " | " &
                 cmps(down(la(i), k, meta), up(la(i), k + 3, meta)) & " | " &
                 cmps(up(1 + (la(i) + k) mod 5, k, meta), down(la(i), k + 1, meta)) & " | " &
                 intops(down(la(i), k + 4, meta), si(la(i), k)) & " | " &
                 intcmps(up(la(i), k, meta), si(la(i), k + 1));
          if la(i) + (la(i) + k) mod 33 < 130 then
            report muls(down(la(i), k, meta), up(1 + (la(i) + k) mod 33, k + 1, meta));
          end if;
          report lvops(std_logic_vector(down(la(i), k, meta)), std_logic_vector(up(la(i), k + 1, meta)), si(la(i), k)) & " | " &
                 lvops(std_logic_vector(up(la(i), k, meta)), std_logic_vector(down(la(i), k + 2, meta)), si(la(i), k + 1)) &
                 " | " & convs(down(la(i), k, meta), sj(la(i), k), la(i), k) &
                 " | " & convs(up(la(i), k, meta), sj(la(i), k + 1), la(i), k + 2);
          report shifts(down(la(i), k, meta), up(1 + (k * 5) mod 6, k + 1, false)) & " | " &
                 shifts(up(la(i), k + 4, meta), down(1 + (la(i) + k) mod 7, k + 2, meta)) & " | " &
                 shifts(down(la(i), k + 1, false), conv_unsigned(k * k, 1 + (k * 3) mod 9));
          if la(i) < 31 then
            report ints(down(la(i), k, meta)) & " " & ints(up(la(i), k + 1, meta));
          end if;
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      p := w(7 downto 0) * unsigned(w(12 downto 5)) + (k mod 100);
      q := u + std_logic_vector(p(11 downto 0)) - k;
      c := conv_unsigned(k * 37, 8) + conv_unsigned(w(3 downto 0), 8);
      n := n + conv_integer(p(15 downto 8)) + conv_integer(q(10 downto 0)) + conv_integer(c);
      if p(7 downto 0) < unsigned(u(7 downto 0)) then n := n + 1; end if;
      if q > k then n := n + 3; end if;
      w := p + k;
      u := q + 5;
      if k mod 60 = 0 then
        report integer'image(n) & " " & img(p) & " " & img(q) & " " & img(c);
      end if;
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn synopsys_operators_in_compiled_code_match_the_interpreter() {
    all_tiers_match("compiled_syn", SYN);
}

/// Guard conditions on signal attributes (`'event`, `'stable`, edge functions, a port, an external
/// name, a guarded bus): GUARD's initial value, which the kernel takes from the guard process, and
/// the guarded assignments it controls.
#[test]
fn guards_on_signal_attributes_match_the_interpreter() {
    let src = include_str!("../../../tests/corpus/delta/guard_signal_attributes.vhd")
        .replace("guard_signal_attributes", "y");
    all_tiers_match("compiled_guard_attributes", &src);
}

/// Guards evaluated again when a signal of the condition is active with no event (`s <= s;`), on
/// a whole signal, an element, a record field and a port, beside drives compiled code may skip.
#[test]
fn guards_on_activity_match_the_interpreter() {
    let src = include_str!("../../../tests/corpus/delta/guard_activity.vhd")
        .replace("guard_activity", "y");
    all_tiers_match("compiled_guard_activity", &src);
}

/// Guards that read a signal of an instance elaborated after them, through an external name, and
/// a signal beside a part no evaluation reaches, each on transactions that keep the value.
#[test]
fn late_guard_reads_match_the_interpreter() {
    let src = include_str!("../../../tests/corpus/delta/guard_activity_late.vhd")
        .replace("guard_activity_late", "y");
    tiers_match("compiled_guard_activity_late", &src, 16);
}

/// `+` and `-` with a `std_ulogic` operand and the unary `+`, `-` and `abs` of the Synopsys
/// packages (`std_logic_arith`, and `std_logic_signed` or `std_logic_unsigned` on
/// `std_logic_vector`): every bit value, weak bits, metavalues (the guard hands those calls to the
/// body), lengths around 64, bounds that do not start at zero in both directions.
fn syn_bit_src(pkg: &str) -> String {
    let lv = if pkg == "signed" {
        "img(lv + b) & img(b - lv) & img(lv - b) & img(b + lv) & img(+ lv) & img(- lv) & img(abs lv)"
    } else {
        "img(lv + b) & img(b - lv) & img(lv - b) & img(b + lv) & img(+ lv)"
    };
    r#"library ieee;
use ieee.std_logic_1164.all;
use ieee.std_logic_arith.all;
use ieee.std_logic_PKG.all;
entity y is end;
architecture a of y is
  function img(v : std_logic_vector) return string is
  begin
    return integer'image(v'left) & "," & integer'image(v'right) & "," &
           boolean'image(v'ascending) & ":" & to_string(v) & " ";
  end function;
  function img(v : unsigned) return string is
  begin
    return img(std_logic_vector(v));
  end function;
  function img(v : signed) return string is
  begin
    return img(std_logic_vector(v));
  end function;
  function pick(i : natural; meta : boolean) return std_logic is
  begin
    if meta and i mod 7 = 3 then return std_logic'val(i mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i * k, meta); end loop;
    return r;
  end function;
  function up(n : natural; k : natural; meta : boolean) return unsigned is
    variable r : unsigned(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(i * k + 1, meta); end loop;
    return r;
  end function;
  function bits(x : unsigned; b : std_ulogic) return string is
    constant lv : std_logic_vector(x'range) := std_logic_vector(x);
  begin
    return img(unsigned'(x + b)) & img(unsigned'(b + x)) & img(unsigned'(x - b)) &
           img(unsigned'(b - x)) & img(signed'(signed(x) + b)) & img(signed'(b + signed(x))) &
           img(signed'(signed(x) - b)) & img(signed'(b - signed(x))) &
           img(std_logic_vector'(x + b)) & img(std_logic_vector'(b - signed(x))) & LV;
  end function;
  function unas(x : unsigned) return string is
  begin
    return img(unsigned'(+ x)) & img(signed'(+ signed(x))) & img(signed'(- signed(x))) &
           img(signed'(abs signed(x))) & img(std_logic_vector'(+ x)) &
           img(std_logic_vector'(- signed(x))) & img(std_logic_vector'(abs signed(x)));
  end function;
begin
  process
    constant la : integer_vector := (1, 2, 31, 32, 33, 63, 64, 65, 5, 16);
    variable n : integer := 0;
    variable w : unsigned(15 downto 0) := x"8001";
    variable v : std_logic_vector(0 to 11) := x"801";
    variable b : std_ulogic;
  begin
    for i in la'range loop
      for k in 0 to 9 loop
        for meta in boolean'range loop
          b := std_ulogic'val((k + i) mod 9);
          report bits(down(la(i), k, meta), b) & "| " & unas(up(la(i), k + 1, meta));
          report bits(up(la(i), k + 2, meta), b) & "| " & unas(down(la(i), k, meta));
        end loop;
      end loop;
    end loop;
    for k in 0 to 300 loop
      b := std_ulogic'val(2 + k mod 2);
      w := w + b - unsigned(signed'(abs signed(w(7 downto 0)))) +
           unsigned(signed'(- signed(w(15 downto 8))));
      v := (v - b) + (+ v);
      if k mod 30 = 0 then
        report img(w) & img(v);
      end if;
    end loop;
    report "done " & img(w) & img(v);
    wait;
  end process;
end;
"#
    .replace("PKG", pkg)
    .replace("LV", lv)
}

#[test]
fn synopsys_bit_and_unary_operators_in_compiled_code_match_the_interpreter() {
    all_tiers_match("compiled_syn_bit_s", &syn_bit_src("signed"));
    all_tiers_match("compiled_syn_bit_u", &syn_bit_src("unsigned"));
}

/// The predefined `?=` and `?/=` of `std_logic_vector` and `std_logic`: every element value at
/// 1 to 70 elements (one word and the native above 64), a mismatch with `U` or `X` elsewhere,
/// both directions, non-zero bounds, null operands, and a loop that compiled code runs.
const PREDEFINED_MATCH: &str = r#"library ieee;
use ieee.std_logic_1164.all;
entity y is end;
architecture a of y is
  function pick(i : natural; k : natural) return std_logic is
  begin
    if k > 0 and i mod k = 0 then return std_logic'val((i / k + k) mod 9); end if;
    case (i * 5 + i / 3) mod 4 is
      when 0 => return '0';
      when 1 => return '1';
      when 2 => return 'L';
      when others => return 'H';
    end case;
  end function;
  function down(n : natural; k : natural) return std_logic_vector is
    variable r : std_logic_vector(n + 4 downto 5);
  begin
    for i in r'range loop r(i) := pick(i - 5, k); end loop;
    return r;
  end function;
  function up(n : natural; k : natural) return std_logic_vector is
    variable r : std_logic_vector(3 to n + 2);
  begin
    for i in r'range loop r(i) := pick(n + 2 - i, k); end loop;
    return r;
  end function;
  function m(x, y : std_logic_vector) return string is
  begin
    return std_ulogic'image(x ?= y) & std_ulogic'image(x ?/= y);
  end function;
begin
  process
    constant ks : integer_vector := (0, 1, 3, 7, 13, 64);
    variable n : integer := 0;
    variable v, w : std_logic_vector(31 downto 0);
    variable s : string(1 to 81);
  begin
    for len in 0 to 70 loop
      for i in ks'range loop
        report integer'image(len) & "/" & integer'image(ks(i)) & ": " &
               m(down(len, ks(i)), up(len, ks(i))) & m(down(len, 0), up(len, ks(i))) &
               m(up(len, ks(i)), down(len, 0)) & m(down(len, ks(i)), down(len, ks(i)));
      end loop;
    end loop;
    report m("U0", "01") & m("X1", "0-") & m("UX", "-1") & m("", "");
    for a in std_logic loop
      for b in std_logic loop
        s(std_logic'pos(a) * 9 + std_logic'pos(b) + 1) := std_ulogic'image(a ?= b)(2);
      end loop;
      report std_ulogic'image(a) & " " & s(std_logic'pos(a) * 9 + 1 to std_logic'pos(a) * 9 + 9) &
             " " & std_ulogic'image(a ?/= '1') & std_ulogic'image('-' ?/= a);
    end loop;
    v := x"0000_0001";
    for k in 0 to 2000 loop
      w := v xor std_logic_vector'(31 downto 8 => '0') & std_logic_vector'(down(8, k mod 5));
      n := (n * 7 + std_ulogic'pos(v ?= w) * 3 + std_ulogic'pos(v ?/= w)) mod 1000003;
      v := v(30 downto 0) & v(31);
    end loop;
    report "done " & integer'image(n);
    wait;
  end process;
end;
"#;

#[test]
fn predefined_match_in_compiled_code_matches_the_interpreter() {
    all_tiers_match("compiled_pmatch", PREDEFINED_MATCH);
}

/// Elements of arrays of logic arrays in subprograms: the operands and targets of native
/// operations on elements of unconstrained parameters (ascending, descending, non-zero bounds)
/// and of locals sized from them, 12-bit elements that cross a plane block, nested calls, a
/// target that is also an operand, parameters that alias each other, locals left at their
/// default (`U`, `X`, `'0'`), called from several processes. Results of more than 64 elements
/// are driven whole, into part of a wider signal and after a delay; a local of characters
/// starts at its subtype's `'a'`.
const ELEMS: &str = r#"library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
package ep is
  type t_arr is array (natural range <>) of signed(11 downto 0);
  type x01v is array (natural range <>) of X01;
  type x_arr is array (natural range <>) of x01v(4 downto 0);
  type b_arr is array (natural range <>) of bit_vector(4 downto 0);
  subtype lc is character range 'a' to 'z';
  type lcv is array (natural range <>) of lc;
  function lcs (n : positive) return string;
  function reduce (src : t_arr) return t_arr;
  function pairs (src : t_arr) return t_arr;
  function partial (src : t_arr; n : natural) return t_arr;
  function xdef (n : positive) return x_arr;
  function bdef (n : positive) return b_arr;
  procedure acc (a : inout t_arr; b : in t_arr);
  function fixed (a : t_arr) return t_arr;
  function img (a : t_arr) return string;
  function str (v : x01v) return string;
end;

package body ep is
  -- One stage of an adder tree on a 0-based operand.
  function reduce (src : t_arr) return t_arr is
    variable r : t_arr(0 to (src'length + 1) / 2 - 1);
  begin
    for k in r'range loop
      if 2 * k + 1 <= src'high then
        r(k) := src(2 * k) + src(2 * k + 1);
      else
        r(k) := src(2 * k);
      end if;
    end loop;
    return r;
  end;
  -- Pairs in the operand's own order, any bounds; the target is an operand too.
  function pairs (src : t_arr) return t_arr is
    variable r : t_arr((src'length + 1) / 2 - 1 downto 0);
    variable k : integer := r'high;
    variable odd : boolean := false;
  begin
    for i in src'range loop
      if not odd then
        r(k) := src(i);
      else
        r(k) := r(k) - src(i);
        k := k - 1;
      end if;
      odd := not odd;
    end loop;
    return r;
  end;
  -- Every third element stays at its default.
  function partial (src : t_arr; n : natural) return t_arr is
    variable r : t_arr(src'range);
  begin
    for i in src'range loop
      if i mod 3 /= n then
        r(i) := src(i) xor src(src'left);
      end if;
    end loop;
    return r;
  end;
  function lcs (n : positive) return string is
    variable v : lcv(1 to n);
    variable s : string(1 to n);
  begin
    v(n) := 'z';
    for i in v'range loop
      s(i) := v(i);
    end loop;
    return s;
  end;
  function xdef (n : positive) return x_arr is
    variable r : x_arr(1 to n);
  begin
    r(1) := "10X01";
    return r;
  end;
  function bdef (n : positive) return b_arr is
    variable r : b_arr(n downto 1);
  begin
    r(n) := "10101";
    return r;
  end;
  procedure acc (a : inout t_arr; b : in t_arr) is
  begin
    for i in a'range loop
      a(i) := a(i) + b(b'low + (i - a'low) mod b'length);
    end loop;
  end;
  -- Elements at constant positions of a local of static bounds, one across a plane block.
  function fixed (a : t_arr) return t_arr is
    variable r : t_arr(0 to 5);
  begin
    r(5) := a(a'left);
    r(0) := r(5) + 1;
    r(2) := r(0);
    return r;
  end;
  function img (a : t_arr) return string is
    variable s : string(1 to 4 * a'length);
    variable p : positive := 1;
  begin
    for i in a'range loop
      s(p to p + 3) := to_hstring(a(i)) & " ";
      p := p + 4;
    end loop;
    return s;
  end;
  function str (v : x01v) return string is
    variable s : string(1 to v'length);
    variable p : positive := 1;
  begin
    for i in v'range loop
      s(p) := std_ulogic'image(v(i))(2);
      p := p + 1;
    end loop;
    return s;
  end;
end;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use work.ep.all;
entity y is
end;
architecture a of y is
  signal t : t_arr(0 to 12) := (others => (others => '0'));
  signal d : t_arr(9 downto 3) := (others => (others => '0'));
  signal u : t_arr(5 to 17) := (others => (others => '0'));
  signal l1 : t_arr(0 to 6) := (others => (others => '0'));
  signal l2 : t_arr(0 to 3) := (others => (others => '0'));
  signal wide : t_arr(0 to 13) := (others => (others => '1'));
  signal late : t_arr(0 to 6) := (others => (others => '0'));
begin
  process is
  begin
    for c in 1 to 120 loop
      for k in t'range loop
        t(k) <= t(k) + to_signed(37 * k + c, 12);
      end loop;
      for k in d'range loop
        d(k) <= d(k) - to_signed(5 * k + 3 * c, 12);
      end loop;
      for k in u'range loop
        u(k) <= u(k) + to_signed(11 * k - c, 12);
      end loop;
      wait for 1 ns;
    end loop;
    wait;
  end process;

  process (t) is
  begin
    l1 <= reduce(t);
    l2 <= reduce(l1);
    wide(3 to 9) <= reduce(t);
    late <= transport reduce(t) after 2 ns;
    report "t " & img(l2) & img(reduce(reduce(t))) & img(pairs(d)) & img(pairs(u))
      & img(partial(u, 1)) & img(fixed(d));
  end process;

  process is
    variable v : t_arr(4 to 11) := (others => to_signed(1, 12));
    variable w : t_arr(7 downto 0);
  begin
    for c in 1 to 100 loop
      wait for 1 ns;
      v := pairs(v & t(0 to 7));
      acc(v, v);
      w := partial(t(0 to 7), c mod 3);
      acc(w, l1);
      report "v " & img(v) & img(w) & img(wide) & img(late) & img(partial(d, c mod 3)) & str(xdef(3)(1))
        & str(xdef(3)(2)) & to_string(bdef(2)(1)) & to_string(bdef(2)(2));
      -- The defaults of the element subtypes' left bounds.
      assert str(xdef(3)(2)) = "XXXXX" and bdef(2)(1) = "00000"
        and to_string(partial(w, 0)(0)) = "UUUUUUUUUUUU" and lcs(c mod 4 + 1) = (1 to c mod 4 => 'a') & "z"
        report "default" severity failure;
    end loop;
    wait;
  end process;
end;
"#;

#[test]
fn elements_of_array_operands_in_subprograms_match_the_interpreter() {
    all_tiers_match("compiled_elem_operands", ELEMS);
    let dir = tempdir("compiled_elem_operands_threads");
    std::fs::write(dir.join("y.vhd"), ELEMS).expect("write");
    let a = yavs(
        &dir,
        &["analyze", "--std=2008", "--work=work:work", "y.vhd"],
    );
    assert!(a.status.success(), "{a:?}");
    let go = |flags: &[&str]| {
        let mut args = vec!["run", "--std=2008", "--work=work:work"];
        args.extend_from_slice(flags);
        args.push("y");
        let o = yavs(&dir, &args);
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    };
    let want = go(&["--tier=interp"]);
    for flags in [
        #[cfg(feature = "clift")]
        vec!["--tier=clift", "--threads=4"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0", "--threads=4"],
    ] {
        assert_eq!(go(&flags), want, "{flags:?}");
    }
}

/// Procedures with signal parameters of array types (#1121), whose element reads are references
/// sliced from the formal and whose drives go through the caller's driver: whole signals, a slice
/// and descending actuals, an individually associated constrained formal (a reference of several
/// parts), ports, delayed and transport drives, integer elements, elements across a plane word
/// and wider than 64, `'event` and `'last_value` of an element, one package procedure from two
/// processes, a resolved signal driven through it by two processes, a procedure that waits,
/// forces on the actuals. The runs on four threads pool every batch (`YAVS_POOL_ALL`).
const SIGPARAMS: &str = r#"library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
package sp is
  type t_arr is array (natural range <>) of signed(15 downto 0);
  subtype t_arr4 is t_arr(0 to 3);
  type i_arr is array (natural range <>) of integer;
  type r_arr is array (natural range <>) of std_logic_vector(7 downto 0);
  type t18 is array (natural range <>) of signed(17 downto 0);
  type tw is array (natural range <>) of unsigned(79 downto 0);
  procedure halve (signal src : in t_arr; signal dst : out t_arr);
  procedure put (signal r : out r_arr; v : integer; act : boolean);
end;

package body sp is
  procedure halve (signal src : in t_arr; signal dst : out t_arr) is
  begin
    for k in dst'range loop
      dst(k) <= shift_right(src(src'low + k - dst'low), 1);
    end loop;
  end;

  procedure put (signal r : out r_arr; v : integer; act : boolean) is
  begin
    for k in r'range loop
      if act then
        r(k) <= std_logic_vector(to_unsigned((v + k) mod 256, 8));
      else
        r(k) <= (others => 'Z');
      end if;
    end loop;
  end;
end;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use work.sp.all;
entity sub is
  port (clk : in std_logic; pi : in t_arr; po : out t_arr);
end;
architecture a of sub is
begin
  process (clk) is
    procedure inc (signal s : in t_arr; signal d : out t_arr) is
    begin
      for k in s'range loop
        d(k) <= s(k) + 1;
      end loop;
    end;
  begin
    if rising_edge(clk) then
      inc(pi, po);
    end if;
  end process;
end;

library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use work.sp.all;
entity y is
end;
architecture a of y is
  signal clk : std_logic := '0';
  signal t : t_arr(0 to 12) := (others => (others => '0'));
  signal l1 : t_arr(0 to 6) := (others => (others => '0'));
  signal l2 : t_arr(0 to 3) := (others => (others => '0'));
  signal m : t_arr(0 to 7) := (others => (others => '0'));
  signal dn : t_arr(7 downto 0) := (others => (others => '0'));
  signal dm : t_arr(3 downto 0) := (others => (others => '0'));
  signal q4 : t_arr(0 to 3) := (others => (others => '0'));
  signal dl : t_arr(0 to 3) := (others => (others => '0'));
  signal pq : t_arr(0 to 3);
  signal h1, h2 : t_arr(0 to 2) := (others => (others => '0'));
  signal wq : t_arr(0 to 1) := (others => (others => '0'));
  signal ia, ib : i_arr(0 to 4) := (others => 0);
  signal rs : r_arr(0 to 3);
  signal ev : integer := 0;
  signal a18, b18 : t18(0 to 9) := (others => (others => '0'));
  signal aw, bw : tw(0 to 2) := (others => (others => '0'));
  signal done : boolean := false;

  function sum (a : t_arr) return signed is
    variable s : signed(23 downto 0) := (others => '0');
  begin
    for k in a'range loop
      s := s + a(k);
    end loop;
    return s;
  end;
begin
  clk <= not clk after 5 ns when not done;
  u : entity work.sub port map (clk => clk, pi => l2, po => pq);

  main : process (clk) is
    -- One adder-tree stage: element reads of the formal, its bounds, drives of its elements.
    procedure reduce (signal src : in t_arr; signal dst : out t_arr) is
    begin
      for k in dst'range loop
        if 2 * k + 1 <= src'high then
          dst(k) <= src(2 * k) + src(2 * k + 1);
        else
          dst(k) <= src(2 * k);
        end if;
      end loop;
    end;
    -- Bounds of a slice or a descending actual: positions relative to the formal.
    procedure mix (signal src : in t_arr; signal dst : out t_arr) is
    begin
      for k in 0 to dst'length - 1 loop
        dst(dst'left - k * (dst'left - dst'right) / (dst'length - 1)) <= src(src'right) - src(src'left + k * (src'right - src'left) / (src'length - 1));
      end loop;
      dst(dst'right) <= src(src'left);
    end;
    -- A constrained formal, individually associated below: a reference of several parts.
    procedure take4 (signal src : in t_arr4; signal dst : out t_arr) is
    begin
      for k in src'range loop
        dst(k) <= src(k) - k;
      end loop;
    end;
    -- Delays and transport through the formal.
    procedure later (signal src : in t_arr; signal dst : out t_arr) is
    begin
      dst(0) <= transport src(1) after 1 ns;
      dst(1) <= src(2) after 2 ns;
      dst(2 to 3) <= src(3 to 4);
    end;
    -- Integer elements.
    procedure iadd (signal a : in i_arr; signal b : out i_arr; n : integer) is
    begin
      for k in a'range loop
        b(k) <= a(k) + n * k;
      end loop;
    end;
    -- Elements that cross a plane word.
    procedure add18 (signal a : in t18; signal b : out t18) is
    begin
      for k in a'range loop
        b(k) <= a(k) + a((k + 3) mod a'length);
      end loop;
    end;
    -- Elements wider than 64.
    procedure wide (signal a : in tw; signal b : out tw) is
    begin
      for k in a'range loop
        b(k) <= a(k) + a(a'high - k) + 1;
      end loop;
    end;
    variable n : integer := 0;
  begin
    if rising_edge(clk) then
      n := n + 1;
      for k in a18'range loop
        a18(k) <= a18(k) + to_signed(k * 4099 + 7, 18);
      end loop;
      for k in aw'range loop
        aw(k) <= aw(k) + shift_left(to_unsigned(k * 977 + 3, 80), 50);
      end loop;
      add18(a18, b18);
      wide(aw, bw);
      for k in t'range loop
        t(k) <= t(k) + to_signed(7 + 3 * k, 16);
      end loop;
      reduce(t, l1);
      reduce(l1, l2);
      mix(t(2 to 9), m);
      mix(m, dn);
      mix(dn(6 downto 3), dm);
      take4(src(0) => t(5), src(1) => t(1), src(2 to 3) => t(10 to 11), dst => q4);
      later(t, dl);
      iadd(ia, ib, n);
      ia <= (n, n * 2, -n, n mod 7, 3);
    end if;
  end process;

  -- Attributes of the formal's elements in a process woken by the actual.
  wt : process (t) is
    procedure watch (signal src : in t_arr; signal e : out integer) is
    begin
      if src(0)'event then
        e <= to_integer(src(0)(3 downto 0)) + to_integer(src(1)'last_value(3 downto 0));
      end if;
    end;
  begin
    watch(t, ev);
  end process;

  -- The same package procedure from two processes, each with its own signals.
  p1 : process (clk) is
  begin
    if rising_edge(clk) then
      halve(l1(0 to 2), h1);
    end if;
  end process;
  p2 : process (clk) is
  begin
    if falling_edge(clk) then
      halve(l1(4 to 6), h2);
    end if;
  end process;

  -- A resolved signal driven by two processes through one procedure.
  r1 : process (clk) is
    variable n : integer := 0;
  begin
    if rising_edge(clk) then
      n := n + 1;
      put(rs, n, n mod 3 /= 0);
    end if;
  end process;
  r2 : process (clk) is
    variable n : integer := 0;
  begin
    if rising_edge(clk) then
      n := n + 1;
      put(rs, 100 + n, n mod 3 = 0);
    end if;
  end process;

  -- A procedure that waits (no native frame).
  w : process is
    procedure follow (signal s : in t_arr; signal d : out t_arr) is
    begin
      wait on s;
      d(0) <= s(0);
      d(1) <= s(s'high);
    end;
  begin
    follow(l2, wq);
  end process;

  -- Forces on an actual the procedures read.
  f : process is
  begin
    wait for 203 ns;
    t(3) <= force to_signed(1000, 16);
    wait for 100 ns;
    t(3) <= release;
    wait for 200 ns;
    l1 <= force (others => to_signed(-5, 16));
    wait for 50 ns;
    l1 <= release;
    wait;
  end process;

  rep : process is
  begin
    for c in 1 to 90 loop
      wait until rising_edge(clk);
      wait for 1 ns;
      report integer'image(c) & " " & to_hstring(sum(l1)) & " " & to_hstring(sum(l2)) & " "
        & to_hstring(sum(m)) & " " & to_hstring(sum(dn)) & " " & to_hstring(sum(dm)) & " "
        & to_hstring(sum(q4)) & " " & to_hstring(sum(dl)) & " " & to_hstring(sum(pq)) & " "
        & to_hstring(sum(h1)) & " " & to_hstring(sum(h2)) & " " & to_hstring(sum(wq)) & " "
        & integer'image(ib(0) + ib(1) + ib(2) + ib(3) + ib(4)) & " " & integer'image(ev) & " "
        & to_string(rs(0)) & to_string(rs(3)) & " " & to_hstring(b18(0) + b18(4) + b18(9)) & " "
        & to_hstring(bw(0) xor bw(1) xor bw(2));
    end loop;
    done <= true;
    wait;
  end process;
end;
"#;

#[test]
fn signal_parameters_in_compiled_code_match_the_interpreter() {
    all_tiers_match("compiled_sigparams", SIGPARAMS);
    let dir = tempdir("compiled_sigparams_threads");
    std::fs::write(dir.join("y.vhd"), SIGPARAMS).expect("write");
    let a = yavs(
        &dir,
        &["analyze", "--std=2008", "--work=work:work", "y.vhd"],
    );
    assert!(a.status.success(), "{a:?}");
    let go = |tier: &str, threads: &str| {
        let o = Command::new(env!("CARGO_BIN_EXE_yavs"))
            .args(["run", "--std=2008", "--work=work:work", tier])
            .args(["--threads", threads, "y"])
            .env("YAVS_POOL_ALL", "1")
            .current_dir(&dir)
            .output()
            .expect("run yavs");
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    };
    let want = go("--tier=interp", "1");
    assert_eq!(go("--tier=interp", "4"), want);
    #[cfg(feature = "clift")]
    assert_eq!(go("--tier=clift", "4"), want);
}

/// The fixed cost of a call of a function on arrays (#1122): results of an unconstrained subtype
/// with the local's own bounds (ascending, descending, not from 0), a constrained result, targets
/// of either direction with bounds of the constant pool, local arrays sized at run time, whole
/// signal operands of 65 to 272 logic elements, one that starts inside a plane block, metavalues
/// among the elements.
const CALLS: &str = r#"library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
use std.textio.all;
entity y is end;
architecture a of y is
  type t_arr is array (natural range <>) of signed(15 downto 0);
  subtype t4 is t_arr(0 to 3);
  type v100 is array (natural range <>) of std_logic_vector(99 downto 0);
  function pick (src : t_arr; lo : natural) return t_arr is
    variable r : t_arr(lo to lo + (src'length + 1) / 2 - 1);
  begin
    for k in r'range loop
      r(k) := src(src'low + 2 * (k - lo)) - k;
    end loop;
    return r;
  end;
  function down (src : t_arr) return t_arr is
    variable r : t_arr(src'length - 1 downto 0);
  begin
    for k in r'range loop
      r(k) := src(src'high - k) + 1;
    end loop;
    return r;
  end;
  function four (src : t_arr) return t4 is
    variable c : t_arr(0 to src'length - 1) := src;
  begin
    return c(0 to 3);
  end;
  function img (a : t_arr) return string is
    variable s : line;
  begin
    write(s, integer'image(a'left) & ":" & integer'image(a'right) & " ");
    for k in a'range loop
      write(s, to_hstring(a(k)));
    end loop;
    return s.all;
  end;
  function fold (v : std_logic_vector) return std_logic_vector is
    variable r : std_logic_vector(15 downto 0) := (others => '0');
  begin
    for k in 0 to v'length - 1 loop
      r(k mod 16) := r(k mod 16) xor v(v'low + k);
    end loop;
    return r;
  end;
  signal t : t_arr(0 to 12) := (others => (others => '0'));
  signal m : t_arr(0 to 16) := (others => (others => '0'));
  signal l1 : t_arr(6 downto 0) := (others => (others => '0'));
  signal l2 : t_arr(0 to 3) := (others => (others => '0'));
  signal l3 : t_arr(4 to 12) := (others => (others => '0'));
  signal w65 : std_logic_vector(64 downto 0) := (others => '0');
  signal w128 : std_logic_vector(127 downto 0) := (others => '1');
  signal w256 : std_logic_vector(255 downto 0) := (others => '0');
  signal w257 : std_logic_vector(256 downto 0) := (others => '0');
  signal a2 : v100(0 to 2) := (others => (others => '0'));
begin
  process is
    variable u : t4;
  begin
    for k in 1 to 90 loop
      wait for 1 ns;
      for i in t'range loop
        t(i) <= t(i) + to_signed(7 * i + k, 16);
      end loop;
      for i in m'range loop
        m(i) <= m(i) - to_signed(3 * i + k, 16);
      end loop;
      if k mod 7 = 0 then
        t(3) <= "01XZ01UW01-H01L0";
      end if;
      l1 <= pick(t, 0);
      l2 <= four(down(l1));
      l3 <= pick(m, 4);
      u := four(down(m));
      w65 <= w65(63 downto 0) & w65(64) xor std_logic_vector(to_unsigned(k, 65));
      w128 <= w128(0) & w128(127 downto 1);
      w256 <= std_logic_vector(unsigned(w256) + k * 977);
      w257 <= w257(255 downto 0) & not w257(256);
      a2(1) <= std_logic_vector(to_unsigned(k * 31, 100));
      a2(2) <= a2(1);
      if k = 40 then
        w128(70) <= 'X';
        w256(255) <= 'Z';
      end if;
      report integer'image(k) & " " & img(l1) & " " & img(l2) & " " & img(l3) & " " & img(u) & " "
        & img(down(t)) & " " & to_hstring(fold(w65)) & to_hstring(fold(w128)) & " "
        & to_hstring(fold(w256)) & to_hstring(fold(w257)) & " " & to_hstring(fold(a2(1)))
        & to_hstring(fold(a2(2)));
    end loop;
    report "done";
    wait;
  end process;
end;
"#;

#[test]
fn calls_of_functions_on_arrays_match_the_interpreter() {
    all_tiers_match("compiled_calls", CALLS);
    let dir = tempdir("compiled_calls_threads");
    std::fs::write(dir.join("y.vhd"), CALLS).expect("write");
    let a = yavs(
        &dir,
        &["analyze", "--std=2008", "--work=work:work", "y.vhd"],
    );
    assert!(a.status.success(), "{a:?}");
    let go = |flags: &[&str]| {
        let mut args = vec!["run", "--std=2008", "--work=work:work"];
        args.extend_from_slice(flags);
        args.push("y");
        let o = yavs(&dir, &args);
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    };
    let want = go(&["--tier=interp"]);
    assert_eq!(go(&["--tier=interp", "--threads=4"]), want);
    #[cfg(feature = "clift")]
    assert_eq!(go(&["--tier=clift", "--threads=4"]), want);
}

/// A result of the wrong length for a constrained result subtype (LRM 10.13) and for the target
/// of a signal assignment (LRM 10.5.2.1) fails the same way in every tier.
#[test]
fn length_checks_of_call_results_fail_alike_on_every_tier() {
    let body = |stmt: &str| {
        format!(
            r#"library ieee; use ieee.std_logic_1164.all; use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  type t_arr is array (natural range <>) of signed(15 downto 0);
  subtype t4 is t_arr(0 to 3);
  function four (src : t_arr) return t4 is
  begin
    return src(src'low to src'low + 2);
  end;
  function half (src : t_arr) return t_arr is
    variable r : t_arr(0 to src'length / 2 - 1);
  begin
    return r;
  end;
  signal t : t_arr(0 to 12) := (others => (others => '0'));
  signal l1 : t_arr(6 downto 0);
  signal l4 : t4;
begin
  process is
  begin
    for k in 1 to 3 loop
      wait for 1 ns;
      report "k=" & integer'image(k);
      if k = 3 then
        {stmt}
      end if;
    end loop;
    wait;
  end process;
end;
"#
        )
    };
    for (name, stmt, what) in [
        (
            "compiled_call_result_len",
            "l4 <= four(t);",
            "length 3 does not match the result of four",
        ),
        (
            "compiled_call_target_len",
            "l1 <= half(t);",
            "length 6 does not match signal L1",
        ),
    ] {
        let dir = tempdir(name);
        std::fs::write(dir.join("y.vhd"), body(stmt)).expect("write");
        let a = yavs(
            &dir,
            &["analyze", "--std=2008", "--work=work:work", "y.vhd"],
        );
        assert!(a.status.success(), "{a:?}");
        let go = |flags: &[&str]| {
            let mut args = vec!["run", "--std=2008", "--work=work:work"];
            args.extend_from_slice(flags);
            args.push("y");
            let o = yavs(&dir, &args);
            (
                o.status.code(),
                String::from_utf8_lossy(&o.stdout).into_owned(),
                String::from_utf8_lossy(&o.stderr).into_owned(),
            )
        };
        let want = go(&["--tier=interp"]);
        assert_ne!(want.0, Some(0), "{name}: {want:?}");
        assert!(want.1.contains("k=3"), "{name}: {want:?}");
        assert!(want.2.contains(what), "{name}: {want:?}");
        for flags in [
            #[cfg(feature = "clift")]
            vec!["--tier=clift"],
            #[cfg(feature = "clift")]
            vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
        ] {
            assert_eq!(go(&flags), want, "{name} {flags:?}");
        }
    }
}

/// Whole values from functions driven at once (no delay: the kernel takes the value where it
/// is, else the waveform is made): targets wider than 64 elements, of exactly 64, of 1, an array
/// of vectors, a slice, values that stay the same, metavalues, a resolved signal of two
/// drivers, an integer array, and the same with a delay, inertial and transport.
#[test]
fn whole_values_driven_from_functions_match() {
    all_tiers_match(
        "compiled_fused_copy",
        r#"
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity y is end;
architecture a of y is
  type t_arr is array (natural range <>) of signed(15 downto 0);
  type i_arr is array (0 to 3) of integer;
  signal clk : std_logic := '0';
  signal wide : unsigned(111 downto 0) := (others => '0');
  signal w64 : unsigned(63 downto 0) := (others => '0');
  signal w16 : std_logic_vector(15 downto 0) := (others => '0');
  signal w1 : std_logic_vector(0 downto 0) := "0";
  signal arr : t_arr(0 to 6) := (others => (others => '0'));
  signal sl : unsigned(31 downto 0) := (others => '0');
  signal held : unsigned(47 downto 0) := (others => '0');
  signal res : std_logic_vector(15 downto 0);
  signal ia : i_arr := (others => 0);
  signal dl, tr : unsigned(79 downto 0) := (others => '0');
  signal dn : std_logic_vector(15 downto 0) := (others => '0');
  function mix(v : unsigned; k : natural) return unsigned is
    variable r : unsigned(v'range);
  begin
    r := rotate_left(v, 3) xor to_unsigned(k * 7 + 1, v'length);
    return r;
  end function;
  function meta(k : natural) return std_logic_vector is
    variable r : std_logic_vector(15 downto 0) := std_logic_vector(to_unsigned(k * 37, 16));
  begin
    if k mod 5 = 0 then r(3) := 'X'; r(9) := 'U'; end if;
    if k mod 7 = 0 then r(0) := 'Z'; end if;
    return r;
  end function;
  function bit1(k : natural) return std_logic_vector is
  begin
    if k mod 3 = 0 then return "1"; elsif k mod 3 = 1 then return "X"; else return "0"; end if;
  end function;
  function step(s : t_arr; k : natural) return t_arr is
    variable r : t_arr(0 to 6);
  begin
    for i in r'range loop r(i) := s(i) + to_signed(i * 3 + k, 16); end loop;
    return r;
  end function;
  function inc(a : i_arr; k : natural) return i_arr is
    variable r : i_arr;
  begin
    for i in r'range loop r(i) := a(i) + i + k; end loop;
    return r;
  end function;
  function keep(k : natural) return unsigned is
  begin
    return to_unsigned(k / 8, 48);
  end function;
begin
  clk <= not clk after 5 ns when now < 400 ns;
  process (clk) is
    variable k : natural := 0;
  begin
    if rising_edge(clk) then
      k := k + 1;
      wide <= mix(wide, k);
      w64 <= mix(w64, k);
      w16 <= meta(k);
      w1 <= bit1(k);
      arr <= step(arr, k);
      sl(23 downto 8) <= mix(sl(23 downto 8), k);
      held <= keep(k);
      res <= meta(k + 1);
      ia <= inc(ia, k);
      dl <= mix(dl, k) after 2 ns;
      tr <= transport mix(tr, k) after 7 ns;
      dn <= meta(k + 2) after 0 ns;
    end if;
  end process;
  res <= (others => 'Z'), x"00F0" after 101 ns, (others => 'L') after 203 ns;
  process (clk) is
  begin
    if falling_edge(clk) then
      report to_hstring(wide) & " " & to_hstring(w64) & " " & to_hstring(w16) & " " &
             to_string(w1) & " " & to_hstring(sl) & " " & to_hstring(held);
      report to_hstring(arr(0)) & to_hstring(arr(6)) & " " & to_string(res) & " " &
             integer'image(ia(0)) & "," & integer'image(ia(3)) & " " & to_hstring(dl) & " " &
             to_hstring(tr) & " " & to_string(dn);
    end if;
  end process;
end;
"#,
    );
}
