//! `cargo test` builds with `[profile.test]` of the workspace (opt-level 1, #1014). The checks must
//! stay on, or the tests would stop seeing overflows and broken invariants.
#![cfg(debug_assertions)]

use std::hint::black_box;

#[test]
#[should_panic(expected = "overflow")]
fn integer_overflow_panics() {
    let _ = black_box(u8::MAX) + black_box(1);
}

#[test]
#[should_panic(expected = "assertion")]
fn debug_assertions_run() {
    debug_assert!(black_box(false));
}
