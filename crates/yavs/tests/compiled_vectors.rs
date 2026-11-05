//! Logic vector operations in compiled code against the interpreter: `and`/`or`/`xor` and their
//! negations, `not`, `to_x01`, `to_ux01`, fills, slices (null ones too) and concatenations of
//! every length class (1 element, the plane block of 64, 65, several blocks), with every
//! std_ulogic code among the elements. Output and exit status are the same in every tier.
#![cfg(any(feature = "llvm", feature = "clift"))]

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
        #[cfg(feature = "llvm")]
        vec!["--tier=llvm"],
        #[cfg(feature = "llvm")]
        vec!["--tier=auto", "--tier-heat=1,0,0"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
        #[cfg(all(feature = "llvm", feature = "clift"))]
        vec!["--tier=auto3", "--tier-heat=1,0,0"],
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
        #[cfg(feature = "llvm")]
        vec!["--tier=llvm"],
        #[cfg(feature = "llvm")]
        vec!["--tier=auto", "--tier-heat=1,0,0"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
        #[cfg(all(feature = "llvm", feature = "clift"))]
        vec!["--tier=auto3", "--tier-heat=1,0,0"],
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
    assert!(want.1.lines().count() >= 70, "{}", want.1);
    for flags in [
        #[cfg(feature = "llvm")]
        vec!["--tier=llvm"],
        #[cfg(feature = "llvm")]
        vec!["--tier=auto", "--tier-heat=1,0,0"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift"],
        #[cfg(feature = "clift")]
        vec!["--tier=clift-auto", "--tier-heat=1,0,0"],
        #[cfg(all(feature = "llvm", feature = "clift"))]
        vec!["--tier=auto3", "--tier-heat=1,0,0"],
    ] {
        assert_eq!(go(&flags), want, "{flags:?}");
    }
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
