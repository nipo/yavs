//! Argument classification per ABI. Pure Rust, so every ABI is
//! unit-tested on any host; only the host's is used for real calls.

use crate::{Arg, CType};

// The non-host variants are only built by the unit tests.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Abi {
    /// System V x86_64: rdi rsi rdx rcx r8 r9 and xmm0-7, independent
    /// counters, the rest on the stack in order in 8-byte slots.
    SysV64,
    /// AAPCS64 (Linux): x0-x7 and d0-d7, independent counters, the rest
    /// on the stack in order in 8-byte slots (C.16: arguments smaller than
    /// 8 bytes take a whole slot, value in the low bytes).
    Aapcs64,
    /// Apple arm64: registers as AAPCS64, but stack arguments are packed
    /// at their natural size and alignment instead of 8-byte slots. The
    /// caller extends integers narrower than 32 bits, which the 64-bit
    /// register image already does.
    Apple64,
    /// Win64: four unified slots by position (rcx/xmm0, rdx/xmm1,
    /// r8/xmm2, r9/xmm3), then 8-byte stack slots above the 32-byte
    /// shadow space.
    Win64,
}

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
pub(crate) const HOST: Abi = Abi::SysV64;
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
pub(crate) const HOST: Abi = Abi::Aapcs64;
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
pub(crate) const HOST: Abi = Abi::Apple64;
#[cfg(all(target_arch = "x86_64", target_os = "windows"))]
pub(crate) const HOST: Abi = Abi::Win64;
#[cfg(not(any(
    all(target_arch = "x86_64", target_os = "linux"),
    all(target_arch = "aarch64", target_os = "linux"),
    all(target_arch = "aarch64", target_os = "macos"),
    all(target_arch = "x86_64", target_os = "windows"),
)))]
compile_error!(
    "yavs-ffi supports x86_64 Linux (SysV), x86_64 Windows (Win64), aarch64 Linux \
     (AAPCS64) and aarch64 macOS (Apple arm64) only; foreign subprograms need a call \
     trampoline for this target"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Loc {
    Int(usize),
    Float(usize),
    /// Byte offset in the outgoing stack argument area.
    Stack(usize),
}

impl Abi {
    fn regs(self) -> (usize, usize) {
        match self {
            Abi::SysV64 => (6, 8),
            Abi::Aapcs64 | Abi::Apple64 => (8, 8),
            Abi::Win64 => (4, 4),
        }
    }

    /// Bytes `ty` takes on the stack, which is also its alignment there.
    fn stack_size(self, ty: CType) -> usize {
        match (self, ty) {
            (Abi::Apple64, CType::I8 | CType::U8) => 1,
            (Abi::Apple64, CType::I16 | CType::U16) => 2,
            (Abi::Apple64, CType::I32 | CType::U32 | CType::F32) => 4,
            _ => 8,
        }
    }
}

pub(crate) fn classify(abi: Abi, params: &[CType]) -> Vec<Loc> {
    let (nint, nfloat) = abi.regs();
    let (mut int, mut float, mut stack) = (0, 0, 0usize);
    let mut spill = |ty: CType| {
        let size = abi.stack_size(ty);
        let offset = stack.next_multiple_of(size);
        stack = offset + size;
        Loc::Stack(offset)
    };
    params
        .iter()
        .enumerate()
        .map(|(pos, &ty)| match abi {
            Abi::Win64 if pos < 4 => {
                if ty.is_float() {
                    Loc::Float(pos)
                } else {
                    Loc::Int(pos)
                }
            }
            Abi::Win64 => spill(ty),
            _ if ty.is_float() && float < nfloat => {
                float += 1;
                Loc::Float(float - 1)
            }
            _ if !ty.is_float() && int < nint => {
                int += 1;
                Loc::Int(int - 1)
            }
            _ => spill(ty),
        })
        .collect()
}

/// Register and stack images handed to the trampoline. The layout is
/// read by the asm in `sys`; the offsets are asserted there.
#[repr(C)]
pub(crate) struct Frame {
    pub int: [u64; 8],
    pub float: [u64; 8],
    pub stack: *const u64,
    pub nstack: usize,
    pub func: *const core::ffi::c_void,
    pub ret_int: u64,
    pub ret_float: u64,
    /// Float registers used (SysV passes it in `al`).
    pub nfloat: u64,
}

/// Fills `frame` and `stack` for `args`, which already match the
/// signature. `stack` holds the stack argument area as little-endian
/// 8-byte words; an argument never straddles two words since its size
/// is its alignment.
pub(crate) fn build(abi: Abi, args: &[Arg], frame: &mut Frame, stack: &mut Vec<u64>) {
    let types: Vec<CType> = args.iter().map(Arg::ctype).collect();
    for (arg, loc) in args.iter().zip(classify(abi, &types)) {
        match loc {
            Loc::Int(r) => frame.int[r] = arg.bits(),
            Loc::Float(r) => {
                frame.float[r] = arg.bits();
                frame.nfloat = frame.nfloat.max(r as u64 + 1);
            }
            Loc::Stack(offset) => {
                let size = abi.stack_size(arg.ctype());
                let (word, shift) = (offset / 8, offset % 8 * 8);
                let mask = u64::MAX >> (64 - 8 * size);
                if stack.len() <= word {
                    stack.resize(word + 1, 0);
                }
                stack[word] |= (arg.bits() & mask) << shift;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use CType::*;
    use Loc::*;

    #[test]
    fn sysv_independent_counters_and_spill() {
        let p = [I32, F64, I64, F32, Ptr, U8, I8, I16, F64];
        assert_eq!(
            classify(Abi::SysV64, &p),
            [
                Int(0),
                Float(0),
                Int(1),
                Float(1),
                Int(2),
                Int(3),
                Int(4),
                Int(5),
                Float(2)
            ]
        );
        let p = [I64; 8];
        assert_eq!(classify(Abi::SysV64, &p)[6..], [Stack(0), Stack(8)]);
        let mut p = vec![F64; 10];
        p.push(I32);
        let l = classify(Abi::SysV64, &p);
        assert_eq!(l[7..], [Float(7), Stack(0), Stack(8), Int(0)]);
    }

    #[test]
    fn aapcs64_eight_of_each() {
        let mut p = vec![I32; 9];
        p.extend([F32; 9]);
        p.push(I64);
        let l = classify(Abi::Aapcs64, &p);
        assert_eq!(l[7], Int(7));
        assert_eq!(l[8], Stack(0));
        assert_eq!(l[16], Float(7));
        assert_eq!(l[17..], [Stack(8), Stack(16)]);
    }

    #[test]
    fn apple64_packs_the_stack_at_natural_alignment() {
        let mut p = vec![I64; 8];
        p.extend([U8, I8, I16, I32, U8, U32, I64, U16, Ptr]);
        p.extend([F64; 8]);
        p.push(F32);
        let l = classify(Abi::Apple64, &p);
        assert_eq!(
            l[8..17],
            [
                Stack(0),
                Stack(1),
                Stack(2),
                Stack(4),
                Stack(8),
                Stack(12),
                Stack(16),
                Stack(24),
                Stack(32)
            ]
        );
        assert_eq!(l[24], Float(7));
        assert_eq!(l[25], Stack(40));
    }

    #[test]
    fn apple64_writes_low_bytes_in_place() {
        let mut p = vec![I64; 8];
        p.extend([I8, U16, I32]);
        let mut args: Vec<Arg> = (0..8).map(Arg::I64).collect();
        args.extend([Arg::I8(-1), Arg::U16(0xabcd), Arg::I32(-2)]);
        let mut frame = Frame {
            int: [0; 8],
            float: [0; 8],
            stack: std::ptr::null(),
            nstack: 0,
            func: std::ptr::null(),
            ret_int: 0,
            ret_float: 0,
            nfloat: 0,
        };
        let mut stack = Vec::new();
        build(Abi::Apple64, &args, &mut frame, &mut stack);
        assert_eq!(stack, [0xffff_fffe_abcd_00ff]);
    }

    #[test]
    fn win64_slots_by_position() {
        let p = [I32, F64, I64, F32, F64, I8];
        assert_eq!(
            classify(Abi::Win64, &p),
            [Int(0), Float(1), Int(2), Float(3), Stack(0), Stack(8)]
        );
        let p = [F64, F64, F64, F64, F64];
        assert_eq!(classify(Abi::Win64, &p)[4], Stack(0));
    }
}
