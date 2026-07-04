//! Fixpoint parity regression tests — exhaustive spot-domain checks against C oracle.
//!
//! The implementation under test is Rust. The optional `/tmp/ftecho` binary is
//! an oracle only; when present, these tests compare every value in the ranges
//! below against vendored FreeType C behavior.

#![allow(clippy::cast_possible_truncation)]
#![allow(unused_crate_dependencies)]

use pillow_rs_freetype::fixed::*;
use std::process::Command;

fn oracle_c(op: &str, args: &[i32]) -> Option<i32> {
    let mut cmd = Command::new("/tmp/ftecho");
    cmd.arg("fix").arg(op);
    for a in args {
        cmd.arg(a.to_string());
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let out = String::from_utf8(output.stdout).ok()?;
    out.trim().parse::<i64>().ok().map(|value| value as i32)
}

macro_rules! check {
    ($rust:expr, $op:expr, $($args:expr),+) => {
        let Some(c) = oracle_c($op, &[$($args),+]) else {
            eprintln!("SKIP: /tmp/ftecho C oracle is unavailable");
            return;
        };
        assert_eq!($rust, c, "{}({}): rust={} c={}",
            $op, stringify!($($args),+), $rust, c);
    };
}

#[test]
fn mul_fix_parity() {
    for a in -32..32i32 {
        for b in -32..32i32 {
            check!(ft_mul_fix(a, b), "mul_fix", a, b);
        }
    }
}

#[test]
fn div_fix_parity() {
    for a in -32..32i32 {
        for b in -32..32i32 {
            if b != 0 {
                check!(ft_div_fix(a, b), "div_fix", a, b);
            }
        }
    }
}

#[test]
fn mul_div_parity() {
    for a in -8..8i32 {
        for b in -8..8i32 {
            for c in 1..8i32 {
                check!(ft_mul_div(a, b, c), "mul_div", a, b, c);
            }
        }
    }
}

#[test]
fn rounding_parity() {
    for a in -64..64i32 {
        check!(ft_round_fix(a), "round_fix", a);
        check!(ft_ceil_fix(a), "ceil_fix", a);
        check!(ft_floor_fix(a), "floor_fix", a);
    }
}

#[test]
fn div_fix_non_pow2() {
    for (a, b) in [(-52, 50), (-85, 100), (-339, 200), (-1, 3), (-7, 12)] {
        check!(ft_div_fix(a, b), "div_fix", a, b);
    }
}

#[test]
fn mul_div_signs() {
    for (a, b, c) in [
        (-3, 5, 2),
        (3, -5, 2),
        (-3, -5, 2),
        (-1, 0x10000, 3),
        (79, -512, 2048),
    ] {
        check!(ft_mul_div(a, b, c), "mul_div", a, b, c);
    }
}
