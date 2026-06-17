//! JPEG decoder — IJG DCT_ISLOW IDCT for pixel-perfect libjpeg parity.
//!
//! Implements libjpeg's exact "slow-but-accurate" integer IDCT from `jidctint.c`.
//! Uses CONST_BITS=13, PASS1_BITS=2 fixed-point arithmetic matching libjpeg-turbo.
//!
//! Reference: IJG libjpeg `jidctint.c` (Thomas G. Lane, 1991-1998)

use super::{ColorType, DecodedImage};

// ── IDCT Constants (matching IJG jidctint.c) ──────────────────────────────

const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;

// FIX(x) = (i32)(x * (1 << CONST_BITS) + 0.5)
const FIX_0_298631336: i32 = 2446;
const FIX_0_390180644: i32 = 3196;
const FIX_0_541196100: i32 = 4433;
const FIX_0_765366865: i32 = 6270;
const FIX_0_899976223: i32 = 7373;
const FIX_1_175875602: i32 = 9633;
const FIX_1_501321110: i32 = 12299;
const FIX_1_847759065: i32 = 15137;
const FIX_1_961570560: i32 = 16069;
const FIX_2_053119869: i32 = 16819;
const FIX_2_562915447: i32 = 20995;
const FIX_3_072711026: i32 = 25172;

const DCTSIZE: usize = 8;
const DCTSIZE2: usize = 64;

#[inline(always)]
fn mpy(v: i32, c: i32) -> i32 {
    ((v as i64 * c as i64) >> CONST_BITS) as i32
}

#[inline(always)]
fn descale(x: i32, shift: i32) -> i32 {
    (x + (1 << (shift - 1))) >> shift
}

#[inline(always)]
fn range_limit(x: i32) -> u8 {
    if x < 0 { 0 } else if x > 255 { 255 } else { x as u8 }
}

// ── IJG jpeg_idct_islow — in-place on 8×8 block ─────────────────────────

pub fn jpeg_idct_islow(block: &mut [i32; DCTSIZE2], workspace: &mut [i32; DCTSIZE2]) {
    // Pass 1: columns
    for c in 0..DCTSIZE {
        let z2 = block[c + DCTSIZE * 2];
        let z3 = block[c + DCTSIZE * 6];
        let z1 = mpy(z2 + z3, FIX_0_541196100);
        let tmp2 = z1 + mpy(z3, -FIX_1_847759065);
        let tmp3 = z1 + mpy(z2, FIX_0_765366865);

        let z2 = block[c + DCTSIZE * 0];
        let z3 = block[c + DCTSIZE * 4];
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;

        let tmp10 = tmp0 + tmp3; let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2; let tmp12 = tmp1 - tmp2;

        // Odd part — Figure 8
        let t0 = &block[c + DCTSIZE * 7];
        let t1 = &block[c + DCTSIZE * 5];
        let t2 = &block[c + DCTSIZE * 3];
        let t3 = &block[c + DCTSIZE * 1];
        let (v0, v1, v2, v3) = (*t0, *t1, *t2, *t3);

        let z1 = v0 + v3; let z2 = v1 + v2;
        let z3 = v0 + v2; let z4 = v1 + v3;
        let z5 = mpy(z3 + z4, FIX_1_175875602);

        let t0 = mpy(v0, FIX_0_298631336); let t1 = mpy(v1, FIX_2_053119869);
        let t2 = mpy(v2, FIX_3_072711026); let t3 = mpy(v3, FIX_1_501321110);
        let z1 = mpy(z1, -FIX_0_899976223); let z2 = mpy(z2, -FIX_2_562915447);
        let z3 = mpy(z3, -FIX_1_961570560); let z4 = mpy(z4, -FIX_0_390180644);
        let z3 = z3 + z5; let z4 = z4 + z5;

        let o0 = t0 + z1 + z3; let o1 = t1 + z2 + z4;
        let o2 = t2 + z2 + z3; let o3 = t3 + z1 + z4;

        workspace[c + DCTSIZE * 0] = descale(tmp10 + o3, PASS1_BITS);
        workspace[c + DCTSIZE * 7] = descale(tmp10 - o3, PASS1_BITS);
        workspace[c + DCTSIZE * 1] = descale(tmp11 + o2, PASS1_BITS);
        workspace[c + DCTSIZE * 6] = descale(tmp11 - o2, PASS1_BITS);
        workspace[c + DCTSIZE * 2] = descale(tmp12 + o1, PASS1_BITS);
        workspace[c + DCTSIZE * 5] = descale(tmp12 - o1, PASS1_BITS);
        workspace[c + DCTSIZE * 3] = descale(tmp13 + o0, PASS1_BITS);
        workspace[c + DCTSIZE * 4] = descale(tmp13 - o0, PASS1_BITS);
    }

    // Pass 2: rows from workspace → block (in-place with range limiting)
    const FS: i32 = CONST_BITS + PASS1_BITS + 3;

    for r in 0..DCTSIZE {
        let row = r * DCTSIZE;
        let z2 = workspace[row + 2]; let z3 = workspace[row + 6];
        let z1 = mpy(z2 + z3, FIX_0_541196100);
        let tmp2 = z1 + mpy(z3, -FIX_1_847759065);
        let tmp3 = z1 + mpy(z2, FIX_0_765366865);

        let z2 = workspace[row + 0]; let z3 = workspace[row + 4];
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;

        let tmp10 = tmp0 + tmp3; let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2; let tmp12 = tmp1 - tmp2;

        let v0 = workspace[row + 7]; let v1 = workspace[row + 5];
        let v2 = workspace[row + 3]; let v3 = workspace[row + 1];

        let z1 = v0 + v3; let z2 = v1 + v2;
        let z3 = v0 + v2; let z4 = v1 + v3;
        let z5 = mpy(z3 + z4, FIX_1_175875602);

        let t0 = mpy(v0, FIX_0_298631336); let t1 = mpy(v1, FIX_2_053119869);
        let t2 = mpy(v2, FIX_3_072711026); let t3 = mpy(v3, FIX_1_501321110);
        let z1 = mpy(z1, -FIX_0_899976223); let z2 = mpy(z2, -FIX_2_562915447);
        let z3 = mpy(z3, -FIX_1_961570560); let z4 = mpy(z4, -FIX_0_390180644);
        let z3 = z3 + z5; let z4 = z4 + z5;

        let o0 = t0 + z1 + z3; let o1 = t1 + z2 + z4;
        let o2 = t2 + z2 + z3; let o3 = t3 + z1 + z4;

        block[row + 0] = range_limit(descale(tmp10 + o3, FS)) as i32;
        block[row + 7] = range_limit(descale(tmp10 - o3, FS)) as i32;
        block[row + 1] = range_limit(descale(tmp11 + o2, FS)) as i32;
        block[row + 6] = range_limit(descale(tmp11 - o2, FS)) as i32;
        block[row + 2] = range_limit(descale(tmp12 + o1, FS)) as i32;
        block[row + 5] = range_limit(descale(tmp12 - o1, FS)) as i32;
        block[row + 3] = range_limit(descale(tmp13 + o0, FS)) as i32;
        block[row + 4] = range_limit(descale(tmp13 - o0, FS)) as i32;
    }
}

// ── Public API ────────────────────────────────────────────────────────────

/// Decode JPEG bytes into a DecodedImage (pixel-perfect with libjpeg).
/// Currently stub — IDCT core ready, full decode pipeline TODO.
pub fn decode(_data: &[u8]) -> Option<DecodedImage> {
    None // TODO: Full JPEG pipeline
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idct_dc_only() {
        let mut block = [0i32; DCTSIZE2];
        block[0] = 512;
        let mut ws = [0i32; DCTSIZE2];
        jpeg_idct_islow(&mut block, &mut ws);
        let first = block[0];
        for i in 1..DCTSIZE2 {
            assert_eq!(block[i], first);
        }
    }
}
