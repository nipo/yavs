//! Word-wise paths of the `numeric_std` and `std_logic_1164` natives for vectors of up to
//! [`MAX_BITS`] elements without a metavalue, where [`crate::ieee::small`] (one word) does not
//! apply. Operands are read from their planes into a fixed buffer, the operation runs on
//! words (carries across them) and the result block is written straight into temporary
//! memory: no `LogicVec`, no allocation. Each function returns `None` for anything it does not
//! take (a metavalue, a longer vector, a rotation, an error or a warning), and the native's
//! general path then words it. Part of the natives' one-word-or-wide fast path (`ieee::FastFn`).

use yavs_ir::DOPE_DIM_BYTES;
use yavs_ir::ieee::{ARRAY_REGS, Cmp, Gate};

use crate::exec::Machine;
use crate::ieee::{fit_bits, gate_bits, holds, low, put_down, put_small};

/// The words of an operand or a result buffer.
const WORDS: usize = 16;
/// The longest vector this module takes.
pub(crate) const MAX_BITS: usize = WORDS * 64;
/// A second operand starts at this cell (`ieee::A1`).
const A1: usize = ARRAY_REGS;

type W = [u64; WORDS];

/// An array operand: its elements as bits (element 0 in bit 0 of word 0; bits at and above `n` are 0).
pub(crate) struct Wide {
    pub(crate) n: usize,
    pub(crate) w: W,
}

impl Wide {
    fn bit(&self, i: usize) -> bool {
        self.w[i / 64] >> (i % 64) & 1 == 1
    }
}

/// The array operand at cell `i`, `1` to [`MAX_BITS`] elements, every one `0 1 L H`; with `pure`
/// none an `L` or `H` (operations that keep the elements). `None` for any other operand.
pub(crate) fn wide_of(m: &Machine, a: &[u64], i: usize, pure: bool) -> Option<Wide> {
    let &[data, left, right, d] = a.get(i..i + ARRAY_REGS)? else {
        return None;
    };
    let (left, right) = (left as i64, right as i64);
    let (lo, hi) = if d == 1 { (right, left) } else { (left, right) };
    let n = usize::try_from(hi.checked_sub(lo)?).ok()?.checked_add(1)?;
    if n > MAX_BITS {
        return None;
    }
    let nb = n.div_ceil(64);
    let b = m.rd(data, nb as u64 * 32).ok()?;
    let mut w = [0u64; WORDS];
    for (k, (c, o)) in b.as_chunks::<32>().0.iter().zip(&mut w).enumerate() {
        let valid = low(n - k * 64);
        let (p0, p1, p2, p3) = (le(&c[0..8]), le(&c[8..16]), le(&c[16..24]), le(&c[24..32]));
        // `0 L` and `1 H` are the codes with plane 1 set and plane 3 clear; plane 2 is the weak bit.
        if !p3 & p1 & valid != valid || (pure && p2 & valid != 0) {
            return None;
        }
        *o = p0 & valid;
    }
    Some(Wide { n, w })
}

#[inline(always)]
fn le(b: &[u8]) -> u64 {
    u64::from_le_bytes(b.try_into().unwrap_or([0; 8]))
}

/// How the result's bounds are written.
#[derive(Clone, Copy)]
pub(crate) enum Bounds {
    /// `(n-1 downto 0)`: `numeric_std`.
    Down,
    /// `(1 to n)`: `std_logic_1164`.
    Up1,
}

/// The result `w` of `n` elements: a one-word block for `n <= 64`, else the fat pair, the dope
/// vector and the blocks of planes written straight into temporary memory.
pub(crate) fn emit(m: &mut Machine, w: &W, n: usize, bounds: Bounds) -> Option<u64> {
    let (left, right, down) = match bounds {
        Bounds::Down => (n as i64 - 1, 0, true),
        Bounds::Up1 => (1, n as i64, false),
    };
    if n <= 64 {
        return match bounds {
            Bounds::Down => put_down(m, n, w[0]),
            Bounds::Up1 => put_small(m, n, w[0], left, right, down),
        }
        .ok();
    }
    let size = n.div_ceil(64) as u64 * 32;
    let fat = m.temp_alloc(16 + DOPE_DIM_BYTES + size).ok()?;
    let b = m.wr(fat, 16 + DOPE_DIM_BYTES + size).ok()?;
    let (head, rest) = b.split_at_mut(16 + DOPE_DIM_BYTES as usize);
    let (dope, data) = (fat + 16, fat + 16 + DOPE_DIM_BYTES);
    for (d, v) in head.as_chunks_mut::<8>().0.iter_mut().zip([
        data,
        dope,
        left as u64,
        right as u64,
        u64::from(down),
    ]) {
        d.copy_from_slice(&v.to_le_bytes());
    }
    for (k, (c, x)) in rest.as_chunks_mut::<32>().0.iter_mut().zip(w).enumerate() {
        let mask = low(n - k * 64);
        for (d, v) in c
            .as_chunks_mut::<8>()
            .0
            .iter_mut()
            .zip([x & mask, mask, 0, 0])
        {
            d.copy_from_slice(&v.to_le_bytes());
        }
    }
    Some(fat)
}

/// Bits `lo..hi` of `w` set.
fn set_range(w: &mut W, lo: usize, hi: usize) {
    for (i, x) in w.iter_mut().enumerate().take(hi.div_ceil(64)).skip(lo / 64) {
        let (a, b) = (lo.max(i * 64) - i * 64, hi.min(i * 64 + 64) - i * 64);
        *x |= low(b) & !low(a);
    }
}

/// Bits at and above `n` cleared.
fn trim(w: &mut W, n: usize) {
    for (i, x) in w.iter_mut().enumerate() {
        *x &= if i * 64 >= n { 0 } else { low(n - i * 64) };
    }
}

/// The value of `x` of `n` elements as `to` elements, zero or sign extended.
fn extend(x: &Wide, to: usize, signed: bool) -> W {
    let mut w = x.w;
    if signed && x.bit(x.n - 1) {
        set_range(&mut w, x.n, to);
    }
    w
}

/// `x + y` or `x - y` over `n` elements, carries across the words.
fn add_words(x: &W, y: &W, n: usize, sub: bool) -> W {
    let mut out = [0u64; WORDS];
    let mut carry = u64::from(sub);
    for i in 0..n.div_ceil(64) {
        let b = if sub { !y[i] } else { y[i] };
        let (s, c1) = x[i].overflowing_add(b);
        let (s, c2) = s.overflowing_add(carry);
        out[i] = s;
        carry = u64::from(c1 | c2);
    }
    trim(&mut out, n);
    out
}

/// The integer `i` as `n` elements: its two's complement bits extended with its sign for a
/// `signed` operation, with zeros for an `unsigned` one (the operand is a NATURAL).
fn int_words(i: i64, n: usize, signed: bool) -> W {
    let mut w = [0u64; WORDS];
    w[0] = i as u64;
    if signed && i < 0 {
        set_range(&mut w, 64, n);
    }
    w
}

/// The operands of [`add`] (forms as there) extended to their common length `n`.
fn operands(m: &Machine, a: &[u64], signed: bool, form: u8) -> Option<(usize, W, W)> {
    Some(match form {
        0 => {
            let (l, r) = (wide_of(m, a, 0, false)?, wide_of(m, a, A1, false)?);
            let n = l.n.max(r.n);
            (n, extend(&l, n, signed), extend(&r, n, signed))
        }
        1 => {
            let l = wide_of(m, a, 0, false).filter(|l| l.n > 64)?;
            let i = int_operand(*a.get(A1)?, signed)?;
            (l.n, l.w, int_words(i, l.n, signed))
        }
        _ => {
            let r = wide_of(m, a, 1, false).filter(|r| r.n > 64)?;
            let i = int_operand(*a.first()?, signed)?;
            (r.n, int_words(i, r.n, signed), r.w)
        }
    })
}

/// `+` and `-` of `numeric_std` (`form` 0: two vectors, 1: vector and integer, 2: integer and
/// vector), where at least one vector is longer than 64 elements or the result is.
pub(crate) fn add(m: &mut Machine, a: &[u64], sub: bool, signed: bool, form: u8) -> Option<u64> {
    let (n, x, y) = operands(m, a, signed, form)?;
    let s = add_words(&x, &y, n, sub);
    emit(m, &s, n, Bounds::Down)
}

/// `maximum` / `minimum` of `numeric_std` (forms as in [`add`]): the larger or smaller operand
/// extended to the common length.
pub(crate) fn minmax(m: &mut Machine, a: &[u64], max: bool, signed: bool, form: u8) -> Option<u64> {
    let (n, x, y) = operands(m, a, signed, form)?;
    let less = if signed {
        cmp_words(&fill_top(x, n), &fill_top(y, n), n, true)
    } else {
        cmp_words(&x, &y, n, false)
    }
    .is_lt();
    emit(m, if less == max { &y } else { &x }, n, Bounds::Down)
}

/// The element count and plane blocks (32 bytes each) of the array operand at cell `i`, up to
/// [`MAX_BITS`] elements; `None` for a null or longer one.
fn planes<'a>(m: &'a Machine, a: &[u64], i: usize) -> Option<(usize, &'a [u8])> {
    let &[data, left, right, d] = a.get(i..i + ARRAY_REGS)? else {
        return None;
    };
    let (lo, hi) = if d == 1 { (right, left) } else { (left, right) };
    let n = usize::try_from((hi as i64).checked_sub(lo as i64)?)
        .ok()?
        .checked_add(1)?;
    (n <= MAX_BITS).then_some(())?;
    Some((n, m.rd(data, n.div_ceil(64) as u64 * 32).ok()?))
}

fn block(b: &[u8], k: usize) -> [u64; 4] {
    let c = &b[32 * k..32 * k + 32];
    [le(&c[0..8]), le(&c[8..16]), le(&c[16..24]), le(&c[24..32])]
}

/// `std_match` of two vectors of one length over 64 elements (up to [`MAX_BITS`]): an element
/// matches when either side is `-` or both are `0 1 L H` with the same value. Unequal lengths and
/// null arrays take the general path.
pub(crate) fn std_match(m: &Machine, a: &[u64]) -> Option<u64> {
    let ((n, p), (k, q)) = (planes(m, a, 0)?, planes(m, a, A1)?);
    if n != k {
        return None;
    }
    for b in 0..n.div_ceil(64) {
        let (x, y) = (block(p, b), block(q, b));
        let known = (x[1] & !x[3]) & (y[1] & !y[3]);
        let ok = x[3] | y[3] | known & !(x[0] ^ y[0]);
        if !ok & low(n - b * 64) != 0 {
            return Some(0);
        }
    }
    Some(1)
}

/// A block of an operand extended to the common length: the elements below `keep` as they are,
/// those in `fill` zeros (planes of `'0'`), or with `sign` (the masks of its sign element's planes)
/// copies of its sign element.
pub(crate) fn ext_block(x: [u64; 4], keep: u64, fill: u64, sign: Option<[u64; 4]>) -> [u64; 4] {
    let mut y = x.map(|w| w & keep);
    match sign {
        Some(s) => {
            for (w, s) in y.iter_mut().zip(s) {
                *w |= s & fill;
            }
        }
        None => y[1] |= fill,
    }
    y
}

/// The fold of the body of `?=` over one block of two operands of `valid` elements, as three
/// words: elements with a `U` (not against `-`), with an `X` (a non-`U` metavalue against
/// anything but `-`) and with two known different values. `-` matches anything.
pub(crate) fn qeq_block(p: [u64; 4], q: [u64; 4], valid: u64) -> [u64; 3] {
    let live = !(p[3] | q[3]) & valid;
    let (up, uq) = (!(p[0] | p[1] | p[2] | p[3]), !(q[0] | q[1] | q[2] | q[3]));
    let both = (p[1] & !p[3]) & (q[1] & !q[3]);
    [
        live & (up | uq),
        live & !(up | uq) & !both,
        live & both & (p[0] ^ q[0]),
    ]
}

/// The `std_ulogic` code the body of `?=` returns for the words of [`qeq_block`] (ored over the
/// blocks): `U` before `X` before a mismatch, else a match (`0` U, `1` X, `2` '0', `3` '1').
pub(crate) fn qeq_code(w: [u64; 3]) -> u64 {
    if w[0] != 0 {
        0
    } else if w[1] != 0 {
        1
    } else if w[2] != 0 {
        2
    } else {
        3
    }
}

/// `?=` of `numeric_std` where a vector is longer than 64 elements (forms as in [`add`]): the
/// code of [`qeq_code`]. Any element value is taken; null arrays, an integer that does not fit
/// and more than [`MAX_BITS`] elements take the general path.
pub(crate) fn qeq(m: &Machine, a: &[u64], signed: bool, form: u8) -> Option<u64> {
    // The integer operand as `n`-element words (a vector over 64 elements always holds it).
    let int = |i: u64, n: usize| Some(int_words(int_operand(i, signed)?, n, signed));
    let (n, p, k, q, iw) = match form {
        0 => {
            let ((n, p), (k, q)) = (planes(m, a, 0)?, planes(m, a, A1)?);
            (n, p, k, q, None)
        }
        1 => {
            let (n, p) = planes(m, a, 0).filter(|v| v.0 > 64)?;
            (n, p, n, p, Some(int(*a.get(A1)?, n)?))
        }
        _ => {
            let (n, q) = planes(m, a, 1).filter(|v| v.0 > 64)?;
            (n, q, n, q, Some(int(*a.first()?, n)?))
        }
    };
    let size = n.max(k);
    // Per operand: the sign element's plane masks, for a `signed` extension.
    let sign = |x: &[u8], c: usize| {
        let s = block(x, (c - 1) / 64);
        s.map(|w| 0u64.wrapping_sub(w >> ((c - 1) % 64) & 1))
    };
    let ext = |x: &[u8], c: usize, b: usize, valid: u64| {
        let raw = if b * 64 < c { block(x, b) } else { [0; 4] };
        let keep = low(c.saturating_sub(b * 64));
        ext_block(raw, keep, valid & !keep, signed.then(|| sign(x, c)))
    };
    let mut acc = [0u64; 3];
    for b in 0..size.div_ceil(64) {
        let valid = low(size - b * 64);
        let (x, y) = match (form, iw) {
            (0, _) => (ext(p, n, b, valid), ext(q, k, b, valid)),
            (1, Some(w)) => (ext(p, n, b, valid), [w[b], valid, 0, 0]),
            (_, Some(w)) => ([w[b], valid, 0, 0], ext(q, k, b, valid)),
            _ => return None,
        };
        for (t, v) in acc.iter_mut().zip(qeq_block(x, y, valid)) {
            *t |= v;
        }
    }
    Some(qeq_code(acc))
}

/// The ordering relations `?<` ... of `numeric_std` (forms as in [`add`]) on an operand with a
/// metavalue other than `-`: the body returns `X` without a report. A `-`, a null array, an
/// integer that does not fit and more than [`MAX_BITS`] elements take the general path.
pub(crate) fn order_meta(m: &Machine, a: &[u64], signed: bool, form: u8) -> Option<u64> {
    let (i, ops) = match form {
        0 => (None, [planes(m, a, 0)?, planes(m, a, A1)?].to_vec()),
        1 => (Some(*a.get(A1)?), vec![planes(m, a, 0)?]),
        _ => (Some(*a.first()?), vec![planes(m, a, 1)?]),
    };
    if let Some(i) = i {
        let n = ops[0].0;
        let i = int_operand(i, signed)?;
        if n <= 64 {
            fit_bits(i, n, signed)?;
        }
    }
    let mut meta = false;
    for (n, p) in ops {
        for b in 0..n.div_ceil(64) {
            let (x, valid) = (block(p, b), low(n - b * 64));
            if x[3] & valid != 0 {
                return None;
            }
            meta |= !(x[1] & !x[3]) & valid != 0;
        }
    }
    meta.then_some(1)
}

/// An integer operand: a NATURAL one (`unsigned`) must not be negative.
fn int_operand(i: u64, signed: bool) -> Option<i64> {
    let i = i as i64;
    (signed || i >= 0).then_some(i)
}

/// The order of the numeric values of two `n`-element word strings.
fn cmp_words(x: &W, y: &W, n: usize, signed: bool) -> std::cmp::Ordering {
    let nb = n.div_ceil(64);
    for i in (0..nb).rev() {
        let (p, q) = (x[i], y[i]);
        if p != q {
            // The sign is in the top element: in the top word it is a signed comparison.
            return if signed && i == nb - 1 {
                // Sign-extended to the top word's end, so the word compares as `i64`.
                (p as i64).cmp(&(q as i64))
            } else {
                p.cmp(&q)
            };
        }
    }
    std::cmp::Ordering::Equal
}

/// The comparison `cmp` of `numeric_std` (forms as in [`add`]); `None` unless at least one vector
/// is longer than 64 elements.
pub(crate) fn compare(m: &Machine, a: &[u64], cmp: Cmp, signed: bool, form: u8) -> Option<u64> {
    let (n, x, y) = match form {
        0 => {
            let (l, r) = (wide_of(m, a, 0, false)?, wide_of(m, a, A1, false)?);
            let n = l.n.max(r.n);
            (n, extend(&l, n, signed), extend(&r, n, signed))
        }
        1 => {
            let l = wide_of(m, a, 0, false).filter(|l| l.n > 64)?;
            let i = int_operand(*a.get(A1)?, signed)?;
            (l.n, extend(&l, l.n, signed), int_words(i, l.n, signed))
        }
        _ => {
            let r = wide_of(m, a, 1, false).filter(|r| r.n > 64)?;
            let i = int_operand(*a.first()?, signed)?;
            (r.n, int_words(i, r.n, signed), extend(&r, r.n, signed))
        }
    };
    // A signed word string must be sign-extended to the end of its top word to compare as `i64`.
    let (x, y) = if signed {
        (fill_top(x, n), fill_top(y, n))
    } else {
        (x, y)
    };
    Some(u64::from(holds(cmp, cmp_words(&x, &y, n, signed))))
}

/// `w` of `n` elements with the sign element copied up to the end of its top word.
fn fill_top(mut w: W, n: usize) -> W {
    if w[(n - 1) / 64] >> ((n - 1) % 64) & 1 == 1 {
        set_range(&mut w, n, n.div_ceil(64) * 64);
    }
    w
}

/// `to_integer` of a vector longer than 64 elements whose value is an INTEGER within the
/// largest (`high`); anything else is the general path's.
pub(crate) fn to_integer(m: &Machine, a: &[u64], signed: bool, high: u64) -> Option<u64> {
    let x = wide_of(m, a, 0, false).filter(|x| x.n > 64)?;
    let nb = x.n.div_ceil(64);
    let neg = signed && x.bit(x.n - 1);
    let w = if signed { fill_top(x.w, x.n) } else { x.w };
    // The upper words are the sign (or zero), and the low word has the same sign bit.
    let ext = if neg { !0 } else { 0 };
    if w[1..nb].iter().any(|&u| u != ext) || (signed && (w[0] >> 63 == 1) != neg) {
        return None;
    }
    let mag = if neg { !w[0] } else { w[0] };
    (mag <= high).then(|| if neg { (-(mag as i64) - 1) as u64 } else { mag })
}

/// `to_unsigned` / `to_signed` of an integer to `size` elements, `size` from 65 to [`MAX_BITS`].
pub(crate) fn from_int(m: &mut Machine, arg: i64, size: usize, signed: bool) -> Option<u64> {
    if !(65..=MAX_BITS).contains(&size) {
        return None;
    }
    let w = int_words(int_operand(arg as u64, signed)?, size, signed);
    emit(m, &w, size, Bounds::Down)
}

/// `resize` of a vector to `new` elements where the argument or the result is longer than 64.
pub(crate) fn resize(m: &mut Machine, a: &[u64], signed: bool) -> Option<u64> {
    let x = wide_of(m, a, 0, true)?;
    let new = usize::try_from(*a.get(A1)? as i64).ok()?;
    if !(1..=MAX_BITS).contains(&new) || (x.n <= 64 && new <= 64) {
        return None;
    }
    let keep = x.n.min(new) - 1;
    let mut w = x.w;
    // The top element of the result is the argument's sign element (signed), the low
    // `keep` elements the argument's lowest.
    if signed {
        trim(&mut w, keep);
        if x.bit(x.n - 1) {
            set_range(&mut w, keep, new);
        }
    } else {
        trim(&mut w, new);
    }
    emit(m, &w, new, Bounds::Down)
}

/// `sll`/`srl` (`K` 0, 1) of a vector longer than 64 elements.
pub(crate) fn shift(m: &mut Machine, a: &[u64], k: u8) -> Option<u64> {
    let x = wide_of(m, a, 0, true).filter(|x| x.n > 64)?;
    let count = usize::try_from(*a.get(A1)? as i64).ok()?;
    let n = x.n;
    let mut out = [0u64; WORDS];
    if count < n {
        let (ws, bs) = (count / 64, count % 64);
        let at = |i: usize| x.w.get(i).copied().unwrap_or(0);
        for (i, o) in out.iter_mut().enumerate().take(n.div_ceil(64)) {
            *o = if k == 0 {
                // Left: word `i - ws`, with the top bits of the word below it.
                let lo = i.checked_sub(ws);
                let v = lo.map_or(0, |j| at(j) << bs);
                let c = if bs > 0 {
                    lo.and_then(|j| j.checked_sub(1))
                        .map_or(0, |j| at(j) >> (64 - bs))
                } else {
                    0
                };
                v | c
            } else {
                let v = at(i + ws) >> bs;
                let c = if bs > 0 {
                    at(i + ws + 1) << (64 - bs)
                } else {
                    0
                };
                v | c
            };
        }
        trim(&mut out, n);
    }
    emit(m, &out, n, Bounds::Down)
}

/// Unary `-` of a `signed` over 64 elements: `0 - x`, carries across the words.
pub(crate) fn neg(m: &mut Machine, a: &[u64]) -> Option<u64> {
    let x = wide_of(m, a, 0, false)?;
    let out = add_words(&[0; WORDS], &x.w, x.n, true);
    emit(m, &out, x.n, Bounds::Down)
}

/// `abs` of a `signed` over 64 elements: `0 - x` when the top element is `1`, else `x`.
pub(crate) fn abs(m: &mut Machine, a: &[u64]) -> Option<u64> {
    let x = wide_of(m, a, 0, false)?;
    let out = if x.bit(x.n - 1) {
        add_words(&[0; WORDS], &x.w, x.n, true)
    } else {
        x.w
    };
    emit(m, &out, x.n, Bounds::Down)
}

/// The gate `g` of two vectors of one length (`bounds` as the native writes them), or of one
/// vector for `not` (`y` is `None`), where an operand is longer than 64 elements.
pub(crate) fn gate(m: &mut Machine, a: &[u64], g: Option<Gate>, bounds: Bounds) -> Option<u64> {
    let x = wide_of(m, a, 0, false)?;
    let mut out = [0u64; WORDS];
    match g {
        Some(g) => {
            let y = wide_of(m, a, A1, false)?;
            if x.n != y.n || x.n <= 64 {
                return None;
            }
            // (Words past `n` are trimmed below.)
            for ((o, &p), &q) in out.iter_mut().zip(&x.w).zip(&y.w) {
                *o = gate_bits(g, p, q);
            }
        }
        None => {
            if x.n <= 64 {
                return None;
            }
            for (o, &p) in out.iter_mut().zip(&x.w) {
                *o = !p;
            }
        }
    }
    trim(&mut out, x.n);
    emit(m, &out, x.n, bounds)
}
