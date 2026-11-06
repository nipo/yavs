//! The decided path of every native `ieee` op: a one-word path in the interpreter (`lookup_fast`)
//! and computed in place by compiled code (`ieee_layout`, the generators' arms). Included by the
//! tests that hold each to it; a new op or a moved path changes [`row`] in the same commit.

// Each includer reads a part of it.
#![allow(dead_code)]

use yavs_ir::ieee::{Op, Syn, parse};

/// A one-word path in the interpreter (`lookup_fast` has an arm).
pub const FAST: bool = true;
pub const NO_FAST: bool = false;
/// Computed in place by compiled code (`ieee_layout` is `Some`).
pub const IN_PLACE: bool = true;
pub const CALL: bool = false;

pub struct Row {
    pub name: &'static str,
    pub fast: bool,
    pub in_place: bool,
}

macro_rules! rows {
    ($($name:literal: $pat:pat => $fast:expr, $in_place:expr;)*) => {
        /// The decision for `op`. No wildcard: a new op does not compile until it has a row.
        pub fn row(op: Op) -> Row {
            match op {
                $($pat => Row { name: $name, fast: $fast, in_place: $in_place },)*
            }
        }
        /// Every row, for the check that [`every_op`] reaches each.
        pub const NAMES: &[&str] = &[$($name),*];
    };
}

rows! {
    // Scalar: compiled code looks the result up in a table.
    "Gate": Op::Gate(_) => NO_FAST, CALL;
    "Not": Op::Not => NO_FAST, CALL;
    "Conv": Op::Conv(..) => FAST, CALL;
    "Truthy": Op::Truthy => FAST, CALL;
    "IsX": Op::IsX(_) => FAST, CALL;
    "Reduce": Op::Reduce(_) => FAST, CALL;
    "ToBitVector": Op::ToBitVector => FAST, CALL;
    // Compiled code tests the edge itself.
    "Edge": Op::Edge(_) => NO_FAST, CALL;
    "VecGate": Op::VecGate(_) => FAST, IN_PLACE;
    "VecNot": Op::VecNot => FAST, IN_PLACE;
    "NumGate": Op::NumGate(_) => FAST, IN_PLACE;
    "NumNot": Op::NumNot => FAST, IN_PLACE;
    "Neg": Op::Neg => FAST, IN_PLACE;
    "Abs": Op::Abs => FAST, IN_PLACE;
    "To01": Op::To01 => FAST, IN_PLACE;
    "Add": Op::Add { .. } => FAST, IN_PLACE;
    "Mul": Op::Mul { .. } => FAST, IN_PLACE;
    "Div": Op::Div { .. } => FAST, IN_PLACE;
    "Compare": Op::Compare { .. } => FAST, IN_PLACE;
    "QCompare": Op::QCompare { .. } => FAST, IN_PLACE;
    "MinMax": Op::MinMax { .. } => FAST, IN_PLACE;
    "Match": Op::Match => FAST, IN_PLACE;
    "ToInteger": Op::ToInteger { .. } => FAST, IN_PLACE;
    "ToUnsigned": Op::ToUnsigned => FAST, IN_PLACE;
    "ToSigned": Op::ToSigned => FAST, IN_PLACE;
    "Resize": Op::Resize { .. } => FAST, IN_PLACE;
    "Shift": Op::Shift(_) => FAST, IN_PLACE;
    "ShiftFn": Op::ShiftFn(_) => FAST, IN_PLACE;
    "Syn::Conv": Op::Syn(Syn::Conv { .. }) => FAST, IN_PLACE;
    "Syn::Shift": Op::Syn(Syn::Shift { .. }) => FAST, IN_PLACE;
    "Syn::ToInt": Op::Syn(Syn::ToInt { .. }) => FAST, IN_PLACE;
    "Syn::Oper": Op::Syn(Syn::Oper { .. }) => FAST, IN_PLACE;
    // The helpers the Synopsys bodies call after `Syn::Oper` gave the call back.
    "Syn::Add": Op::Syn(Syn::Add { .. }) => NO_FAST, CALL;
    "Syn::Mult": Op::Syn(Syn::Mult { .. }) => NO_FAST, CALL;
    "Syn::MakeBinary": Op::Syn(Syn::MakeBinary) => NO_FAST, CALL;
    "Syn::Less": Op::Syn(Syn::Less { .. }) => NO_FAST, CALL;
    // A few nanoseconds of `f64` work: no path worth having.
    "Math": Op::Math(_) => NO_FAST, CALL;
    // Compiled code has its own path for it (`uniform_fast`), outside `ieee_layout`.
    "Uniform": Op::Uniform => NO_FAST, CALL;
    "Str": Op::Str(_) => NO_FAST, CALL;
}

/// Every op `parse` returns for keys spelled from the packages, designators and types it
/// matches, with one key each. A designator or type missing here shows as a row of [`NAMES`]
/// no op reaches.
pub fn every_op() -> Vec<(Op, String)> {
    const PKGS: [&str; 8] = [
        "std_logic_1164",
        "std_logic_misc",
        "numeric_std",
        "numeric_bit",
        "std_logic_arith",
        "std_logic_unsigned",
        "std_logic_signed",
        "math_real",
    ];
    const OPS: [&str; 34] = [
        "and", "nand", "or", "nor", "xor", "xnor", "not", "??", "-", "+", "*", "/", "abs", "rem",
        "mod", "?>", "?<", "?<=", "?>=", "?=", "?/=", ">", "<", "<=", ">=", "=", "/=", "**", "sll",
        "srl", "sla", "sra", "rol", "ror",
    ];
    const FNS: [&str; 61] = [
        "and_reduce",
        "nand_reduce",
        "or_reduce",
        "nor_reduce",
        "xor_reduce",
        "xnor_reduce",
        "to_x01",
        "to_x01z",
        "to_ux01",
        "to_01",
        "to_bitvector",
        "is_x",
        "rising_edge",
        "falling_edge",
        "maximum",
        "minimum",
        "std_match",
        "to_integer",
        "to_unsigned",
        "to_signed",
        "resize",
        "xsll",
        "xsrl",
        "xsra",
        "xrol",
        "xror",
        "shift_left",
        "shift_right",
        "rotate_left",
        "rotate_right",
        "plus",
        "minus",
        "unsigned_plus",
        "unsigned_minus",
        "mult",
        "make_binary",
        "is_less",
        "is_less_or_equal",
        "unsigned_is_less",
        "unsigned_is_less_or_equal",
        "conv_unsigned",
        "conv_signed",
        "conv_std_logic_vector",
        "ext",
        "sxt",
        "shl",
        "shr",
        "conv_integer",
        "uniform",
        "to_hstring",
        "to_ostring",
        "sqrt",
        "exp",
        "log",
        "log2",
        "log10",
        "sin",
        "cos",
        "ceil",
        "floor",
        "round",
    ];
    const MORE: [&str; 2] = ["trunc", "sign"];
    const TYPES: [&str; 11] = [
        "unsigned",
        "signed",
        "integer",
        "std_ulogic",
        "std_ulogic_vector",
        "std_logic_vector",
        "bit_vector",
        "bit",
        "boolean",
        "real",
        "string",
    ];
    let names = OPS
        .iter()
        .map(|n| format!("\"{n}\""))
        .chain(FNS.iter().chain(&MORE).map(|n| n.to_string()));
    let mut params: Vec<String> = TYPES.iter().map(|t| t.to_string()).collect();
    for a in TYPES {
        for b in TYPES {
            params.push(format!("{a},{b}"));
        }
    }
    // Three parameters: only the procedure `uniform` has them.
    for a in ["integer", "real"] {
        for b in ["integer", "real"] {
            for c in ["integer", "real"] {
                params.push(format!("{a},{b},{c}"));
            }
        }
    }
    let mut out: Vec<(Op, String)> = Vec::new();
    for name in names {
        for pkg in PKGS {
            for p in &params {
                for ret in TYPES.iter().chain(&[""]) {
                    let key = format!("{pkg}.{name}({p}){ret}");
                    if let Some(op) = parse(&key)
                        && !out.iter().any(|(o, _)| *o == op)
                    {
                        out.push((op, key));
                    }
                }
            }
        }
    }
    out
}

/// Fails when a row of [`NAMES`] has no op in [`every_op`].
pub fn assert_every_row_reached(ops: &[(Op, String)]) {
    for name in NAMES {
        assert!(
            ops.iter().any(|(op, _)| row(*op).name == *name),
            "no key of `every_op` reaches row `{name}`: add its designator or types there"
        );
    }
}
