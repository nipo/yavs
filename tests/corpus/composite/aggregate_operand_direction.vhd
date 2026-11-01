-- difftest: top=aggregate_operand_direction std=2008
-- A named aggregate as an operand of a predefined operator is not in a constrained context, also
-- when the array type is constrained: it runs in the index subtype's direction, so its leftmost
-- element is the lowest index (lrm-checks L170, #1009). The contexts that do constrain it keep
-- its target's direction.
entity aggregate_operand_direction is end;
architecture t of aggregate_operand_direction is
  type down_t is array (3 downto 0) of integer;
  type up_t is array (0 to 3) of integer;
  type un_t is array (natural range <>) of integer;
  subtype dn_t is un_t(3 downto 0);
  type e_t is (e_a, e_b, e_c);
  type bye_t is array (e_c downto e_a) of bit;
  type bits_t is array (3 downto 0) of bit;
  type nest_t is array (1 downto 0) of down_t;
  type rec_t is record f : down_t; end record;
  type acc_t is access down_t;
  constant s1 : down_t := (3 => 3, 2 => 2, 1 => 1, 0 => 0);
  constant u1 : up_t := (0 => 0, 1 => 1, 2 => 2, 3 => 3);
  constant d1 : dn_t := (3 => 3, 2 => 2, 1 => 1, 0 => 0);
  constant bb : bits_t := "1100";
  constant ee : bye_t := "110";
  constant nest : nest_t := (others => s1);
  constant cr : rec_t := (f => (3 => 5, 2 => 0, 1 => 0, 0 => 0));
  constant ca : nest_t := (1 => (3 => 6, 2 => 0, 1 => 0, 0 => 0), 0 => (others => 0));
  signal sg : down_t := (3 => 2, 2 => 0, 1 => 0, 0 => 0);
  signal sw, sc, se : down_t;
  function fc (a : down_t) return integer is begin return a(3); end;
  function fu (a : un_t) return integer is begin return a(a'left); end;
  function fl (a : down_t) return integer is begin return a'left; end;
  function fr return down_t is begin return (3 => 4, 2 => 0, 1 => 0, 0 => 0); end;
  function fo return down_t is begin return (others => 11); end;
  function fd (a : down_t := (3 => 9, 2 => 0, 1 => 0, 0 => 0)) return integer is begin return a(3); end;
  procedure pp (x : in down_t) is begin report "proc " & integer'image(x(3)); end;
  procedure b (c : boolean; s : string) is begin report s & " " & boolean'image(c); end;
begin
  sw <= (3 => 1, 2 => 0, 1 => 0, 0 => 0);
  sc <= (3 => 7, 2 => 0, 1 => 0, 0 => 0) when true else (others => 0);
  with true select se <= (3 => 8, 2 => 0, 1 => 0, 0 => 0) when true, (others => 0) when false;
  process
    variable v : down_t;
    variable pa : acc_t;
  begin
    wait for 1 ns;
    b(s1 = (0 => 3, 1 => 2, 2 => 1, 3 => 0), "eq low first");
    b(s1 = (3 => 3, 2 => 2, 1 => 1, 0 => 0), "eq high first");
    b((0 => 3, 1 => 2, 2 => 1, 3 => 0) = s1, "eq on the left");
    b(s1 /= (3 => 3, 2 => 2, 1 => 1, 0 => 0), "ne");
    b(s1 < (3 => 3, 2 => 2, 1 => 1, 0 => 0), "lt");
    b(s1 <= (3 => 3, 2 => 2, 1 => 1, 0 => 0), "le");
    b(s1 > (0 => 0, 1 => 0, 2 => 0, 3 => 9), "gt");
    b(s1 >= (3 => 0, 2 => 0, 1 => 0, 0 => 9), "ge");
    b(s1 = (3, 2, 1, 0), "positional");
    b(u1 = (0 => 0, 1 => 1, 2 => 2, 3 => 3), "ascending type");
    b(u1 = (3 => 0, 2 => 1, 1 => 2, 0 => 3), "ascending type reversed");
    b(d1 = (3 => 3, 2 => 2, 1 => 1, 0 => 0), "subtype high first");
    b(d1 = (0 => 3, 1 => 2, 2 => 1, 3 => 0), "subtype low first");
    b(bb = (3 => '1', 2 => '1', 1 => '0', 0 => '0'), "bits high first");
    b(bb = (0 => '1', 1 => '1', 2 => '0', 3 => '0'), "bits low first");
    b(ee = (e_c => '1', e_b => '1', e_a => '0'), "enum index high first");
    b(ee = (e_a => '1', e_b => '1', e_c => '0'), "enum index low first");
    b(nest(0) = (3 => 3, 2 => 2, 1 => 1, 0 => 0), "element of an array");
    -- Constrained contexts: the target's direction.
    b(s1 = down_t'(3 => 3, 2 => 2, 1 => 1, 0 => 0), "qualified");
    report "function " & integer'image(fc((3 => 7, 2 => 0, 1 => 0, 0 => 0)));
    report "function left " & integer'image(fl((3 => 7, 2 => 0, 1 => 0, 0 => 0)));
    report "unconstrained formal " & integer'image(fu((3 => 7, 2 => 0, 1 => 0, 0 => 0)));
    report "default " & integer'image(fd);
    report "return " & integer'image(fr(3));
    report "return others " & integer'image(fo(3));
    pp((3 => 13, 2 => 0, 1 => 0, 0 => 0));
    pp(x => (others => 14));
    v := (3 => 7, 2 => 6, 1 => 5, 0 => 4);
    report "assign " & integer'image(v(3));
    v := (3 => 1, 2 => 2, 1 => 3, 0 => 4) when v(3) = 7 else (others => 0);
    report "conditional " & integer'image(v(3));
    pa := new down_t'(3 => 15, 2 => 0, 1 => 0, 0 => 0);
    report "allocator " & integer'image(pa.all(3));
    pa.all := (3 => 16, 2 => 0, 1 => 0, 0 => 0);
    report "dereference " & integer'image(pa.all(3));
    report "signal init " & integer'image(sg(3));
    report "waveform " & integer'image(sw(3));
    report "conditional waveform " & integer'image(sc(3));
    report "selected waveform " & integer'image(se(3));
    report "record field " & integer'image(cr.f(3));
    report "array element " & integer'image(ca(1)(3));
    wait;
  end process;
end;
