// ── Forward DCT, IJG ISLOW (libjpeg-turbo 3.1.4.1 jfdctint.c) ─────────────
//
// 8×8 forward DCT in scaled fixed-point (CONST_BITS=13, PASS1_BITS=2).
// Input: 64 sample values already level-shifted (sample - 128) in natural
// order.  Output: DCT coefficients in natural order, scaled up by a factor
// of 8 (the factor of 8 is removed by the quantization step, see jcdctmgr.c).

const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;

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

#[inline(always)]
fn descale(x: i32, n: i32) -> i32 {
    // IJG DESCALE: round-to-nearest arithmetic right shift.
    (x + (1 << (n - 1))) >> n
}

/// Forward DCT on one 8×8 block.  `data` is natural-order samples in/out.
pub(crate) fn fdct_islow(data: &mut [i32; 64]) {
    // Pass 1: process rows.  Results scaled up by 2^PASS1_BITS.
    for ctr in 0..8 {
        let row = ctr * 8;
        let (tmp0, tmp7) = (data[row + 0] + data[row + 7], data[row + 0] - data[row + 7]);
        let (tmp1, tmp6) = (data[row + 1] + data[row + 6], data[row + 1] - data[row + 6]);
        let (tmp2, tmp5) = (data[row + 2] + data[row + 5], data[row + 2] - data[row + 5]);
        let (tmp3, tmp4) = (data[row + 3] + data[row + 4], data[row + 3] - data[row + 4]);

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        data[row + 0] = (tmp10 + tmp11) << PASS1_BITS;
        data[row + 4] = (tmp10 - tmp11) << PASS1_BITS;

        let z1 = (tmp12 + tmp13) * FIX_0_541196100;
        data[row + 2] = descale(z1 + tmp13 * FIX_0_765366865, CONST_BITS - PASS1_BITS);
        data[row + 6] = descale(z1 + tmp12 * (-FIX_1_847759065), CONST_BITS - PASS1_BITS);

        let z1 = tmp4 + tmp7;
        let z2 = tmp5 + tmp6;
        let z3 = tmp4 + tmp6;
        let z4 = tmp5 + tmp7;
        let z5 = (z3 + z4) * FIX_1_175875602;

        let t4 = tmp4 * FIX_0_298631336;
        let t5 = tmp5 * FIX_2_053119869;
        let t6 = tmp6 * FIX_3_072711026;
        let t7 = tmp7 * FIX_1_501321110;
        let z1 = z1 * (-FIX_0_899976223);
        let z2 = z2 * (-FIX_2_562915447);
        let z3 = z3 * (-FIX_1_961570560) + z5;
        let z4 = z4 * (-FIX_0_390180644) + z5;

        data[row + 7] = descale(t4 + z1 + z3, CONST_BITS - PASS1_BITS);
        data[row + 5] = descale(t5 + z2 + z4, CONST_BITS - PASS1_BITS);
        data[row + 3] = descale(t6 + z2 + z3, CONST_BITS - PASS1_BITS);
        data[row + 1] = descale(t7 + z1 + z4, CONST_BITS - PASS1_BITS);
    }

    // Pass 2: process columns.  Remove PASS1_BITS scaling; results scaled by 8.
    for ctr in 0..8 {
        let col = ctr;
        let (tmp0, tmp7) = (
            data[col + 0] + data[col + 56],
            data[col + 0] - data[col + 56],
        );
        let (tmp1, tmp6) = (
            data[col + 8] + data[col + 48],
            data[col + 8] - data[col + 48],
        );
        let (tmp2, tmp5) = (
            data[col + 16] + data[col + 40],
            data[col + 16] - data[col + 40],
        );
        let (tmp3, tmp4) = (
            data[col + 24] + data[col + 32],
            data[col + 24] - data[col + 32],
        );

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        data[col + 0] = descale(tmp10 + tmp11, PASS1_BITS);
        data[col + 32] = descale(tmp10 - tmp11, PASS1_BITS);

        let z1 = (tmp12 + tmp13) * FIX_0_541196100;
        data[col + 16] = descale(z1 + tmp13 * FIX_0_765366865, CONST_BITS + PASS1_BITS);
        data[col + 48] = descale(z1 + tmp12 * (-FIX_1_847759065), CONST_BITS + PASS1_BITS);

        let z1 = tmp4 + tmp7;
        let z2 = tmp5 + tmp6;
        let z3 = tmp4 + tmp6;
        let z4 = tmp5 + tmp7;
        let z5 = (z3 + z4) * FIX_1_175875602;

        let t4 = tmp4 * FIX_0_298631336;
        let t5 = tmp5 * FIX_2_053119869;
        let t6 = tmp6 * FIX_3_072711026;
        let t7 = tmp7 * FIX_1_501321110;
        let z1 = z1 * (-FIX_0_899976223);
        let z2 = z2 * (-FIX_2_562915447);
        let z3 = z3 * (-FIX_1_961570560) + z5;
        let z4 = z4 * (-FIX_0_390180644) + z5;

        data[col + 56] = descale(t4 + z1 + z3, CONST_BITS + PASS1_BITS);
        data[col + 40] = descale(t5 + z2 + z4, CONST_BITS + PASS1_BITS);
        data[col + 24] = descale(t6 + z2 + z3, CONST_BITS + PASS1_BITS);
        data[col + 8] = descale(t7 + z1 + z4, CONST_BITS + PASS1_BITS);
    }
}
