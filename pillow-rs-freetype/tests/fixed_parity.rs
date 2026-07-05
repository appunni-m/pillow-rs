//! Fixpoint parity regression tests: exhaustive spot-domain checks against a C oracle.
//!
//! The implementation under test is Rust. These tests compile a small C oracle
//! binary in the target temp directory and compare every value in the ranges
//! below against FreeType's fixed-point arithmetic semantics.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(unused_crate_dependencies)]

use fontdone::fixed::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const ORACLE_SOURCE: &str = r#"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int32_t neg_long(int32_t x) {
    return (int32_t)(0 - (uint32_t)x);
}

static int32_t ft_mul_div(int32_t a, int32_t b, int32_t c) {
    if (c == 0) {
        return 0x7fffffff;
    }
    uint64_t ua = a < 0 ? (uint64_t)-(int64_t)a : (uint64_t)a;
    uint64_t ub = b < 0 ? (uint64_t)-(int64_t)b : (uint64_t)b;
    uint64_t uc = c < 0 ? (uint64_t)-(int64_t)c : (uint64_t)c;
    uint64_t d = (ua * ub + (uc >> 1)) / uc;
    int32_t r = (int32_t)d;
    return ((a < 0) ^ (b < 0) ^ (c < 0)) ? neg_long(r) : r;
}

static int32_t ft_mul_fix(int32_t a, int32_t b) {
    int64_t ab = (int64_t)a * (int64_t)b;
    return (int32_t)((ab + 0x8000 + (ab >> 63)) >> 16);
}

static int32_t ft_div_fix(int32_t a, int32_t b) {
    if (b == 0) {
        return 0x7fffffff;
    }
    uint64_t ua = a < 0 ? (uint64_t)-(int64_t)a : (uint64_t)a;
    uint64_t ub = b < 0 ? (uint64_t)-(int64_t)b : (uint64_t)b;
    uint64_t q = ((ua << 16) + (ub >> 1)) / ub;
    int32_t r = (int32_t)q;
    return ((a < 0) ^ (b < 0)) ? neg_long(r) : r;
}

static int32_t ft_round_fix(int32_t a) {
    return (int32_t)((uint32_t)a + (uint32_t)(0x8000 - (a < 0))) & ~0xffff;
}

static int32_t ft_ceil_fix(int32_t a) {
    return (int32_t)((uint32_t)a + 0xffffu) & ~0xffff;
}

static int32_t ft_floor_fix(int32_t a) {
    return a & ~0xffff;
}

int main(int argc, char **argv) {
    if (argc < 3 || strcmp(argv[1], "fix") != 0) {
        return 2;
    }
    const char *op = argv[2];
    int32_t result = 0;
    if (strcmp(op, "mul_fix") == 0 && argc == 5) {
        result = ft_mul_fix((int32_t)strtol(argv[3], 0, 10), (int32_t)strtol(argv[4], 0, 10));
    } else if (strcmp(op, "div_fix") == 0 && argc == 5) {
        result = ft_div_fix((int32_t)strtol(argv[3], 0, 10), (int32_t)strtol(argv[4], 0, 10));
    } else if (strcmp(op, "mul_div") == 0 && argc == 6) {
        result = ft_mul_div((int32_t)strtol(argv[3], 0, 10), (int32_t)strtol(argv[4], 0, 10), (int32_t)strtol(argv[5], 0, 10));
    } else if (strcmp(op, "round_fix") == 0 && argc == 4) {
        result = ft_round_fix((int32_t)strtol(argv[3], 0, 10));
    } else if (strcmp(op, "ceil_fix") == 0 && argc == 4) {
        result = ft_ceil_fix((int32_t)strtol(argv[3], 0, 10));
    } else if (strcmp(op, "floor_fix") == 0 && argc == 4) {
        result = ft_floor_fix((int32_t)strtol(argv[3], 0, 10));
    } else {
        return 3;
    }
    printf("%d\n", result);
    return 0;
}
"#;

static ORACLE_PATH: OnceLock<PathBuf> = OnceLock::new();

fn oracle_path() -> &'static PathBuf {
    ORACLE_PATH.get_or_init(|| {
        let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fixed_parity_c_oracle");
        fs::create_dir_all(&out_dir).expect("create fixed parity oracle dir");
        let source = out_dir.join("ftecho_fixed.c");
        let binary = out_dir.join("ftecho_fixed");
        fs::write(&source, ORACLE_SOURCE).expect("write fixed parity C oracle");

        let status = Command::new("cc")
            .arg("-std=c99")
            .arg("-O2")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-o")
            .arg(&binary)
            .arg(&source)
            .status()
            .expect("run cc for fixed parity C oracle");
        assert!(status.success(), "compile fixed parity C oracle");

        binary
    })
}

fn oracle_c(op: &str, args: &[i32]) -> i32 {
    let mut cmd = Command::new(oracle_path());
    cmd.arg("fix").arg(op);
    for a in args {
        cmd.arg(a.to_string());
    }
    let output = cmd.output().expect("run fixed parity C oracle");
    assert!(
        output.status.success(),
        "fixed parity C oracle failed for {op} {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = String::from_utf8(output.stdout).expect("fixed parity oracle emitted UTF-8");
    out.trim()
        .parse::<i64>()
        .expect("fixed parity oracle emitted integer") as i32
}

macro_rules! check {
    ($rust:expr, $op:expr, $($args:expr),+) => {
        let c = oracle_c($op, &[$($args),+]);
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
