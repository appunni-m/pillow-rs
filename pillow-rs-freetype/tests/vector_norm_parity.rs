//! TrueType vector normalization parity against a small C oracle.
//!
//! SPVFS/SFVFS and line-vector opcodes are cbox-sensitive: a one-unit
//! difference in the normalized 2.14 vector can become a one-unit outline bbox
//! failure.  This test keeps Rust's fixed normalization aligned with
//! FreeType's `Normalize` + `FT_Vector_NormLen` path.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(unused_crate_dependencies)]

use pillow_rs_freetype::fixed::ft_normalize_2dot14;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const ORACLE_SOURCE: &str = r#"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int ft_msb(uint32_t z) {
    int shift = 0;
    if (z >= (1UL << 16)) { z >>= 16; shift += 16; }
    if (z >= (1UL << 8))  { z >>= 8;  shift += 8;  }
    if (z >= (1UL << 4))  { z >>= 4;  shift += 4;  }
    if (z >= (1UL << 2))  { z >>= 2;  shift += 2;  }
    if (z >= (1UL << 1))  {           shift += 1;  }
    return shift;
}

static uint32_t ft_vector_norm_len(int32_t *vx, int32_t *vy) {
    int32_t x_ = *vx;
    int32_t y_ = *vy;
    int32_t b, z;
    uint32_t x, y, u, v, l;
    int sx = 1, sy = 1, shift;

    if (x_ < 0) { x = (uint32_t)(0 - (uint32_t)x_); sx = -1; }
    else        { x = (uint32_t)x_; }
    if (y_ < 0) { y = (uint32_t)(0 - (uint32_t)y_); sy = -1; }
    else        { y = (uint32_t)y_; }

    if (x == 0) {
        if (y > 0) *vy = sy * 0x10000;
        return y;
    } else if (y == 0) {
        if (x > 0) *vx = sx * 0x10000;
        return x;
    }

    l = x > y ? x + (y >> 1) : y + (x >> 1);
    shift = 31 - ft_msb(l);
    shift -= 15 + (l >= (0xAAAAAAAAUL >> shift));

    if (shift > 0) {
        x <<= shift;
        y <<= shift;
        l = x > y ? x + (y >> 1) : y + (x >> 1);
    } else {
        x >>= -shift;
        y >>= -shift;
        l >>= -shift;
    }

    b = 0x10000 - (int32_t)l;
    x_ = (int32_t)x;
    y_ = (int32_t)y;

    do {
        u = (uint32_t)(x_ + (x_ * b >> 16));
        v = (uint32_t)(y_ + (y_ * b >> 16));
        z = -(int32_t)(u * u + v * v) / 0x200;
        z = z * ((0x10000 + b) >> 8) / 0x10000;
        b += z;
    } while (z > 0);

    *vx = sx < 0 ? -(int32_t)u : (int32_t)u;
    *vy = sy < 0 ? -(int32_t)v : (int32_t)v;

    l = (uint32_t)(0x10000 + (int32_t)(u * x + v * y) / 0x10000);
    if (shift > 0)
        l = (l + (1 << (shift - 1))) >> shift;
    else
        l <<= -shift;
    return l;
}

static int normalize_2dot14(int32_t x, int32_t y, int32_t *out_x, int32_t *out_y) {
    if (x == 0 && y == 0)
        return 0;
    ft_vector_norm_len(&x, &y);
    *out_x = x / 4;
    *out_y = y / 4;
    return 1;
}

int main(int argc, char **argv) {
    if (argc != 4 || strcmp(argv[1], "norm") != 0)
        return 2;
    int32_t x = (int32_t)strtol(argv[2], 0, 10);
    int32_t y = (int32_t)strtol(argv[3], 0, 10);
    int32_t out_x = 0, out_y = 0;
    int ok = normalize_2dot14(x, y, &out_x, &out_y);
    printf("%d %d %d\n", ok, out_x, out_y);
    return 0;
}
"#;

static ORACLE_PATH: OnceLock<PathBuf> = OnceLock::new();

fn oracle_path() -> &'static PathBuf {
    ORACLE_PATH.get_or_init(|| {
        let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("vector_norm_c_oracle");
        fs::create_dir_all(&out_dir).expect("create vector norm oracle dir");
        let source = out_dir.join("ftecho_vector_norm.c");
        let binary = out_dir.join("ftecho_vector_norm");
        fs::write(&source, ORACLE_SOURCE).expect("write vector norm C oracle");

        let status = Command::new("cc")
            .arg("-std=c99")
            .arg("-O2")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-o")
            .arg(&binary)
            .arg(&source)
            .status()
            .expect("run cc for vector norm C oracle");
        assert!(status.success(), "compile vector norm C oracle");
        binary
    })
}

fn oracle_norm(x: i32, y: i32) -> Option<(i32, i32)> {
    let output = Command::new(oracle_path())
        .arg("norm")
        .arg(x.to_string())
        .arg(y.to_string())
        .output()
        .expect("run vector norm C oracle");
    assert!(
        output.status.success(),
        "vector norm C oracle failed for ({x}, {y}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = String::from_utf8(output.stdout).expect("vector norm oracle emitted UTF-8");
    let parts: Vec<i32> = out
        .split_whitespace()
        .map(|part| part.parse::<i32>().expect("integer oracle field"))
        .collect();
    assert_eq!(parts.len(), 3, "oracle emitted unexpected output: {out}");
    if parts[0] == 0 {
        None
    } else {
        Some((parts[1], parts[2]))
    }
}

#[test]
fn truetype_normalize_2dot14_matches_c_oracle() {
    let mut cases = vec![
        (0, 0),
        (0x4000, 0),
        (0, 0x4000),
        (-0x4000, 0),
        (0, -0x4000),
        (0x4000, 0x4000),
        (-0x4000, 0x4000),
        (0x2000, 0x4000),
        (0x4000, 0x2000),
        (1, 1),
        (-1, 1),
        (1, -1),
        (123, 456),
        (-123, 456),
        (123, -456),
        (16383, 1),
        (1, 16383),
        (32767, 32767),
        (-32768, 32767),
    ];

    for x in [-0x4000, -8192, -1024, -1, 1, 1024, 8192, 0x4000] {
        for y in [-0x4000, -8192, -1024, -1, 1, 1024, 8192, 0x4000] {
            cases.push((x, y));
        }
    }

    for (x, y) in cases {
        let rust = ft_normalize_2dot14(x, y);
        let c = oracle_norm(x, y);
        assert_eq!(rust, c, "Normalize({x}, {y})");
    }
}
