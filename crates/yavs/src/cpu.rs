//! The start-up check of a build that assumes x86-64-v3 (`-C target-cpu=x86-64-v3`, the release
//! archives for x86_64; docs/decisions.md "x86-64-v3 baseline"). Such a binary dies with an illegal
//! instruction on an older CPU, so `main` reads CPUID first and says why. The check is compiled in
//! only when AVX2 is a compile-time feature; the decoding below is plain code, tested on every build.
//!
//! `is_x86_feature_detected!` cannot do this job: it answers true without asking the CPU for a
//! feature that is enabled at compile time. Only `__cpuid` (safe to call) is used.

use core::arch::x86_64::{__cpuid, __cpuid_count};

/// The CPUID words the check reads.
#[derive(Clone, Copy)]
struct Words {
    /// Leaf 1, ECX.
    l1_ecx: u32,
    /// Leaf 7 sub-leaf 0, EBX (0 when the CPU has no leaf 7).
    l7_ebx: u32,
    /// Leaf 0x8000_0001, ECX (0 when the CPU has no such leaf).
    ext1_ecx: u32,
}

/// A word of [`Words`].
#[derive(Clone, Copy)]
enum Word {
    L1Ecx,
    L7Ebx,
    Ext1Ecx,
}

impl Words {
    fn get(&self, w: Word) -> u32 {
        match w {
            Word::L1Ecx => self.l1_ecx,
            Word::L7Ebx => self.l7_ebx,
            Word::Ext1Ecx => self.ext1_ecx,
        }
    }
}

/// What x86-64-v3 adds over the baseline that the compiler may use, as (name, word, bit). The
/// order is the order of the message.
const NEEDED: [(&str, Word, u32); 9] = [
    ("AVX", Word::L1Ecx, 28),
    // ponytail: OSXSAVE stands for "the OS saves the AVX registers"; XGETBV would confirm it but
    // needs `unsafe`. An OS that sets OSXSAVE and still hides AVX state is not seen.
    ("OSXSAVE", Word::L1Ecx, 27),
    ("FMA", Word::L1Ecx, 12),
    ("F16C", Word::L1Ecx, 29),
    ("MOVBE", Word::L1Ecx, 22),
    ("AVX2", Word::L7Ebx, 5),
    ("BMI1", Word::L7Ebx, 3),
    ("BMI2", Word::L7Ebx, 8),
    ("LZCNT", Word::Ext1Ecx, 5),
];

/// Bit `i` is set when `NEEDED[i]` is missing in `w`.
fn missing(w: &Words) -> u16 {
    let mut m = 0;
    for (i, (_, word, bit)) in NEEDED.iter().enumerate() {
        // black_box: without it the compiler vectorizes this loop with AVX2, which is the
        // instruction the check exists to keep from running on a CPU without it.
        m |= std::hint::black_box(u16::from(w.get(*word) >> bit & 1 == 0) << i);
    }
    m
}

/// The words of this CPU. A leaf above the CPU's maximum is not read: on some CPUs it returns the
/// data of the highest leaf instead of zeros.
fn read() -> Words {
    let l1_ecx = __cpuid(1).ecx;
    let l7_ebx = if __cpuid(0).eax >= 7 {
        __cpuid_count(7, 0).ebx
    } else {
        0
    };
    let ext1_ecx = if __cpuid(0x8000_0000).eax >= 0x8000_0001 {
        __cpuid(0x8000_0001).ecx
    } else {
        0
    };
    Words {
        l1_ecx,
        l7_ebx,
        ext1_ecx,
    }
}

/// On a CPU without x86-64-v3: print why and the way out on stderr and return the exit status.
/// Plain `write_all` calls of fixed text: no formatting and no diagnostic renderer, since both
/// are compiled for v3 too.
#[inline(never)]
#[cfg_attr(not(target_feature = "avx2"), allow(dead_code))]
pub(crate) fn check() -> Option<std::process::ExitCode> {
    use std::io::Write;
    let m = missing(&read());
    if m == 0 {
        return None;
    }
    let mut e = std::io::stderr();
    let _ = e.write_all(
        b"error: this build of yavs needs a CPU with x86-64-v3 (AVX2, BMI1, BMI2, FMA, F16C, LZCNT, MOVBE), and this CPU lacks:",
    );
    let mut sep: &[u8] = b" ";
    for (i, (name, _, _)) in NEEDED.iter().enumerate() {
        if m >> i & 1 != 0 {
            let _ = e.write_all(sep);
            sep = b", ";
            let _ = e.write_all(name.as_bytes());
        }
    }
    let _ = e.write_all(
        b"\nhelp: build yavs from source, which runs on any x86-64 CPU: cargo build --release -p yavs --features clift (https://yavs.readthedocs.io/en/latest/getting-started.html#installation)\n",
    );
    Some(std::process::ExitCode::from(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real values: a Haswell (leaf 1 ECX 0x7ffafbff-like, AVX2 and BMI in leaf 7) and a Nehalem
    // (SSE4.2 only), reduced to the bits the check reads.
    const HASWELL: Words = Words {
        l1_ecx: 1 << 28 | 1 << 27 | 1 << 12 | 1 << 29 | 1 << 22,
        l7_ebx: 1 << 5 | 1 << 3 | 1 << 8,
        ext1_ecx: 1 << 5,
    };
    const NEHALEM: Words = Words {
        l1_ecx: 1 << 20 | 1 << 23,
        l7_ebx: 0,
        ext1_ecx: 1 << 0,
    };

    fn names(m: u16) -> Vec<&'static str> {
        (0..NEEDED.len())
            .filter(|i| m >> i & 1 != 0)
            .map(|i| NEEDED[i].0)
            .collect()
    }

    #[test]
    fn v3_cpu_is_accepted() {
        assert_eq!(missing(&HASWELL), 0);
    }

    #[test]
    fn old_cpu_names_every_missing_feature() {
        assert_eq!(
            names(missing(&NEHALEM)),
            [
                "AVX", "OSXSAVE", "FMA", "F16C", "MOVBE", "AVX2", "BMI1", "BMI2", "LZCNT"
            ]
        );
    }

    #[test]
    fn one_missing_feature_is_named_alone() {
        let w = Words {
            l7_ebx: HASWELL.l7_ebx & !(1 << 5),
            ..HASWELL
        };
        assert_eq!(names(missing(&w)), ["AVX2"]);
    }

    /// Whatever CPU runs the tests, the decoding of its own CPUID must not panic; a v3 build of
    /// the tests (the release smoke) also proves the CPU passes.
    #[test]
    fn this_cpu_decodes() {
        let m = missing(&read());
        if cfg!(target_feature = "avx2") {
            assert_eq!(m, 0, "missing: {:?}", names(m));
        }
    }
}
