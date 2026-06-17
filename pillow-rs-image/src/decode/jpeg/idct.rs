// ── IDCT Constants (matching IJG jidctint.c) ──────────────────────────────

pub(crate) const CONST_BITS: i32 = 13;
pub(crate) const PASS1_BITS: i32 = 2;

// FIX(x) = (i32)(x * (1 << CONST_BITS) + 0.5)
pub(crate) const FIX_0_298631336: i32 = 2446;
pub(crate) const FIX_0_390180644: i32 = 3196;
pub(crate) const FIX_0_541196100: i32 = 4433;
pub(crate) const FIX_0_765366865: i32 = 6270;
pub(crate) const FIX_0_899976223: i32 = 7373;
pub(crate) const FIX_1_175875602: i32 = 9633;
pub(crate) const FIX_1_501321110: i32 = 12299;
pub(crate) const FIX_1_847759065: i32 = 15137;
pub(crate) const FIX_1_961570560: i32 = 16069;
pub(crate) const FIX_2_053119869: i32 = 16819;
pub(crate) const FIX_2_562915447: i32 = 20995;
pub(crate) const FIX_3_072711026: i32 = 25172;

pub(crate) const DCTSIZE: usize = 8;
pub(crate) const DCTSIZE2: usize = 64;

/// Full-precision multiply matching IJG's MULTIPLY macro (no premature descale).
/// Returns v * c at CONST_BITS (2^13) scale.
#[inline(always)]
pub(crate) fn mpy(v: i32, c: i32) -> i32 {
    (v as i64 * c as i64) as i32
}

#[inline(always)]
pub(crate) fn descale(x: i32, shift: i32) -> i32 {
    (x + (1 << (shift - 1))) >> shift
}

/// IJG-style range_limit: clamps (x + 128) to [0, 255].
#[inline(always)]
pub(crate) fn range_limit(x: i32) -> u8 {
    let x = x + 128;
    if x < 0 {
        0
    } else if x > 255 {
        255
    } else {
        x as u8
    }
}

// ── IJG jpeg_idct_islow — in-place on 8×8 block ─────────────────────────

pub(crate) fn jpeg_idct_islow(block: &mut [i32; DCTSIZE2], workspace: &mut [i32; DCTSIZE2]) {
    // Pass 1: columns
    for c in 0..DCTSIZE {
        let z2 = block[c + DCTSIZE * 2];
        let z3 = block[c + DCTSIZE * 6];
        let z1 = mpy(z2 + z3, FIX_0_541196100);
        let tmp2 = z1 + mpy(z3, -FIX_1_847759065);
        let tmp3 = z1 + mpy(z2, FIX_0_765366865);

        let z2 = block[c];
        let z3 = block[c + DCTSIZE * 4];
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        // Odd part — Figure 8
        let v0 = block[c + DCTSIZE * 7];
        let v1 = block[c + DCTSIZE * 5];
        let v2 = block[c + DCTSIZE * 3];
        let v3 = block[c + DCTSIZE];
        let z1 = v0 + v3;
        let z2 = v1 + v2;
        let z3 = v0 + v2;
        let z4 = v1 + v3;
        let z5 = mpy(z3 + z4, FIX_1_175875602);

        let t0 = mpy(v0, FIX_0_298631336);
        let t1 = mpy(v1, FIX_2_053119869);
        let t2 = mpy(v2, FIX_3_072711026);
        let t3 = mpy(v3, FIX_1_501321110);
        let z1 = mpy(z1, -FIX_0_899976223);
        let z2 = mpy(z2, -FIX_2_562915447);
        let z3 = mpy(z3, -FIX_1_961570560);
        let z4 = mpy(z4, -FIX_0_390180644);
        let z3 = z3 + z5;
        let z4 = z4 + z5;

        let o0 = t0 + z1 + z3;
        let o1 = t1 + z2 + z4;
        let o2 = t2 + z2 + z3;
        let o3 = t3 + z1 + z4;

        workspace[c] = descale(tmp10 + o3, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 7] = descale(tmp10 - o3, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE] = descale(tmp11 + o2, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 6] = descale(tmp11 - o2, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 2] = descale(tmp12 + o1, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 5] = descale(tmp12 - o1, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 3] = descale(tmp13 + o0, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 4] = descale(tmp13 - o0, CONST_BITS - PASS1_BITS);
    }

    // Pass 2: rows from workspace → block (in-place with range limiting)
    const FS: i32 = CONST_BITS + PASS1_BITS + 3;

    for r in 0..DCTSIZE {
        let row = r * DCTSIZE;
        let z2 = workspace[row + 2];
        let z3 = workspace[row + 6];
        let z1 = mpy(z2 + z3, FIX_0_541196100);
        let tmp2 = z1 + mpy(z3, -FIX_1_847759065);
        let tmp3 = z1 + mpy(z2, FIX_0_765366865);

        let z2 = workspace[row];
        let z3 = workspace[row + 4];
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        let v0 = workspace[row + 7];
        let v1 = workspace[row + 5];
        let v2 = workspace[row + 3];
        let v3 = workspace[row + 1];

        let z1 = v0 + v3;
        let z2 = v1 + v2;
        let z3 = v0 + v2;
        let z4 = v1 + v3;
        let z5 = mpy(z3 + z4, FIX_1_175875602);

        let t0 = mpy(v0, FIX_0_298631336);
        let t1 = mpy(v1, FIX_2_053119869);
        let t2 = mpy(v2, FIX_3_072711026);
        let t3 = mpy(v3, FIX_1_501321110);
        let z1 = mpy(z1, -FIX_0_899976223);
        let z2 = mpy(z2, -FIX_2_562915447);
        let z3 = mpy(z3, -FIX_1_961570560);
        let z4 = mpy(z4, -FIX_0_390180644);
        let z3 = z3 + z5;
        let z4 = z4 + z5;

        let o0 = t0 + z1 + z3;
        let o1 = t1 + z2 + z4;
        let o2 = t2 + z2 + z3;
        let o3 = t3 + z1 + z4;

        block[row] = range_limit(descale(tmp10 + o3, FS)) as i32;
        block[row + 7] = range_limit(descale(tmp10 - o3, FS)) as i32;
        block[row + 1] = range_limit(descale(tmp11 + o2, FS)) as i32;
        block[row + 6] = range_limit(descale(tmp11 - o2, FS)) as i32;
        block[row + 2] = range_limit(descale(tmp12 + o1, FS)) as i32;
        block[row + 5] = range_limit(descale(tmp12 - o1, FS)) as i32;
        block[row + 3] = range_limit(descale(tmp13 + o0, FS)) as i32;
        block[row + 4] = range_limit(descale(tmp13 - o0, FS)) as i32;
    }
}

// ── JPEG Utilities ────────────────────────────────────────────────────────

/// `jpeg_natural_order` maps zigzag index to natural (row-major) position.
pub(crate) const JPEG_NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Sign extension for DC/AC coefficient additional bits (Figure F.12).
#[inline(always)]
pub(crate) fn extend(value: u32, size: u8) -> i32 {
    if size == 0 {
        return 0;
    }
    let threshold = 1u32 << (size - 1);
    if value < threshold {
        (value as i32) - ((1i32 << size) - 1)
    } else {
        value as i32
    }
}

/// YCbCr -> RGB conversion matching libjpeg's jdcolor.c.
pub(super) struct YccColorConverter {
    cr_r_tab: [i32; 256],
    cb_b_tab: [i32; 256],
    cr_g_tab: [i32; 256],
    cb_g_tab: [i32; 256],
}

impl YccColorConverter {
    pub(crate) fn new() -> Self {
        let mut cr_r_tab = [0i32; 256];
        let mut cb_b_tab = [0i32; 256];
        let mut cr_g_tab = [0i32; 256];
        let mut cb_g_tab = [0i32; 256];

        for i in 0..256 {
            let x = i as i32 - 128;
            cr_r_tab[i] = ((91881i64 * x as i64 + 32768) >> 16) as i32;
            cb_b_tab[i] = ((116130i64 * x as i64 + 32768) >> 16) as i32;
            cr_g_tab[i] = (-46802i64 * x as i64) as i32;
            cb_g_tab[i] = ((-22554i64 * x as i64) + 32768) as i32;
        }

        YccColorConverter {
            cr_r_tab,
            cb_b_tab,
            cr_g_tab,
            cb_g_tab,
        }
    }

    #[inline(always)]
    pub(crate) fn ycc_to_rgb(&self, y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
        let y = y as i32;
        let r = y + self.cr_r_tab[cr as usize];
        let g = y + ((self.cb_g_tab[cb as usize] + self.cr_g_tab[cr as usize]) >> 16);
        let b = y + self.cb_b_tab[cb as usize];
        (
            r.clamp(0, 255) as u8,
            g.clamp(0, 255) as u8,
            b.clamp(0, 255) as u8,
        )
    }
}
