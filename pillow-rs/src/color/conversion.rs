use std::borrow::Cow;

use super::grayscale::cmyk_muldiv255_byte;
use crate::raster::{DynamicImage, RgbImage};

#[cfg(feature = "parallel")]
const CMYK_TO_RGB_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

/// Maps an RGB buffer to Pillow's default CMYK inverse: C=255-R, M=255-G,
/// Y=255-B, K=0.
pub fn rgb_to_cmyk_inverse(rgb: &crate::raster::RgbImage) -> crate::raster::RgbaImage {
    let (w, h) = rgb.dimensions();
    let mut out = crate::raster::RgbaImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
        *op = crate::raster::Rgba([
            255u8.wrapping_sub(ip[0]),
            255u8.wrapping_sub(ip[1]),
            255u8.wrapping_sub(ip[2]),
            0u8,
        ]);
    }
    out
}
// ── Non-standard mode conversions ──

/// Applies Pillow's `MULDIV255` integer scaling helper.
///
/// This is equivalent to `(a * b + 128) / 255` with Pillow-compatible rounding.
#[inline]
pub fn muldiv255(a: u32, b: u32) -> u32 {
    let t = a * b + 128;
    ((t >> 8) + t) >> 8
}

#[inline]
fn cmyk_to_rgb_row(source: &[u8], output: &mut [u8]) {
    for (rgb, cmyk) in output.chunks_exact_mut(3).zip(source.chunks_exact(4)) {
        let ink = u8::MAX - cmyk[3];
        // muldiv255(channel, ink) is in 0..=ink for byte channels, so the
        // subtraction is already the exact clipped Pillow result.
        rgb[0] = ink - cmyk_muldiv255_byte(cmyk[0], ink);
        rgb[1] = ink - cmyk_muldiv255_byte(cmyk[1], ink);
        rgb[2] = ink - cmyk_muldiv255_byte(cmyk[2], ink);
    }
}

/// Converts CMYK storage to RGB using Pillow-compatible integer arithmetic.
///
/// The input is an RGBA buffer interpreted as `C`, `M`, `Y`, `K`. The returned
/// image is `RGB8` with the same dimensions.
pub fn cmyk_to_rgb(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let cmyk = match img {
        // CMYK already occupies this exact four-byte layout. Borrow it so the
        // conversion allocates only its RGB result instead of cloning a full
        // RGBA-shaped intermediate.
        DynamicImage::ImageRgba8(image) => Cow::Borrowed(image.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let mut out = RgbImage::new(w, h);
    if w == 0 || h == 0 {
        return DynamicImage::ImageRgb8(out);
    }
    let source_stride = w as usize * 4;
    let output_stride = w as usize * 3;
    #[cfg(feature = "parallel")]
    if w as usize * h as usize >= CMYK_TO_RGB_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            out.as_mut(),
            output_stride,
            h as usize,
            |_row_start, _row_end, y, row| {
                let source_start = y as usize * source_stride;
                cmyk_to_rgb_row(&cmyk[source_start..source_start + source_stride], row);
            }
        );
        return DynamicImage::ImageRgb8(out);
    }
    for (source, output) in cmyk
        .chunks_exact(source_stride)
        .zip(out.as_mut().chunks_exact_mut(output_stride))
    {
        cmyk_to_rgb_row(source, output);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts HSV storage to RGB using Pillow's integer-domain algorithm.
///
/// HSV is stored as an RGB buffer interpreted as `H`, `S`, `V`, all in the
/// `0..=255` byte range. The returned image is `RGB8` with the same dimensions.
///
/// PIL's formula (from C source):
/// ```text
///   fs = s / 255.0    (normalized saturation)
///   h = h * 6.0 / 255.0  (0-6 sector mapping)
///   j = floor(h); f = h - j
///   p = v * (1.0 - fs)     ← v is 0-255, so p is 0-255
///   q = v * (1.0 - fs * f)
///   t = v * (1.0 - fs * (1.0 - f))
///   Then round all to nearest integer, CLIP8
/// ```
pub fn hsv_to_rgb(img: &DynamicImage) -> DynamicImage {
    use std::sync::OnceLock;

    static FACTORS: OnceLock<Vec<[f32; 3]>> = OnceLock::new();
    let factors = FACTORS.get_or_init(|| {
        let mut table = Vec::with_capacity(256 * 256);
        for h_in in 0..=255u8 {
            let h = h_in as f32 * 6.0 / 255.0;
            let j = h.floor() as i32;
            let f = h - h.floor();
            for s_in in 0..=255u8 {
                let fs = s_in as f32 / 255.0;
                let p = 1.0 - fs;
                let q = 1.0 - fs * f;
                let t = 1.0 - fs * (1.0 - f);
                table.push(match j % 6 {
                    0 => [1.0, t, p],
                    1 => [q, 1.0, p],
                    2 => [p, 1.0, t],
                    3 => [p, q, 1.0],
                    4 => [t, p, 1.0],
                    _ => [1.0, p, q],
                });
            }
        }
        table
    });

    let (w, h) = (img.width(), img.height());
    let rgb = match img {
        DynamicImage::ImageRgb8(rgb) => Cow::Borrowed(rgb.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgb8().into_raw()),
    };
    let mut out = Vec::with_capacity(rgb.len());
    for ip in rgb.chunks_exact(3) {
        let v = ip[2] as f32;
        let factor = factors[ip[0] as usize * 256 + ip[1] as usize];
        let r = (v * factor[0]).round().clamp(0.0, 255.0) as u8;
        let g = (v * factor[1]).round().clamp(0.0, 255.0) as u8;
        let b = (v * factor[2]).round().clamp(0.0, 255.0) as u8;
        out.extend_from_slice(&[r, g, b]);
    }
    DynamicImage::ImageRgb8(
        RgbImage::from_raw(w, h, out)
            .expect("HSV-to-RGB output preserves the source's RGB dimensions"),
    )
}

/// Converts RGB to HSV using Pillow's `rgb2hsv` precision behavior.
///
/// Input and output are `RGB8` buffers. Output channels are `H`, `S`, `V`, each
/// in `0..=255`.
///
/// PIL uses float (f32) for intermediate values (rc, gc, bc, cr, h), only
/// promoting to double when combining with `2.0`/`4.0`/`6.0`/`255.0` literals.
/// The result is stored back to f32, and the final `(int)(h * 255.0)` truncates.
/// This function matches PIL pixel-for-pixel by mimicking the exact precision
/// flow: f32 divisions → f64 promotions for constant arithmetic → f32 storage.
pub fn rgb_to_hsv(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let rgb = match img {
        // Native RGB bytes are already the converter's input layout. Borrow
        // them instead of cloning the full image through `to_rgb8`.
        DynamicImage::ImageRgb8(rgb) => Cow::Borrowed(rgb.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgb8().into_raw()),
    };
    let mut out = crate::raster::RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.chunks_exact(3)) {
        let r = ip[0];
        let g = ip[1];
        let b = ip[2];
        let maxc = r.max(g.max(b));
        let minc = r.min(g.min(b));
        let v = maxc;
        let (uh, us) = if minc == maxc {
            (0u8, 0u8)
        } else {
            // All intermediate computations in f32, matching PIL's `float` type
            let cr = (maxc - minc) as f32;
            let rc = (maxc as f32 - r as f32) / cr;
            let gc = (maxc as f32 - g as f32) / cr;
            let bc = (maxc as f32 - b as f32) / cr;

            // Compute h_val in f64 (PIL promotes to double due to 2.0/4.0 literals),
            // then immediately cast back to f32 for storage in `h`.
            let h_float: f32 = if r == maxc {
                (bc as f64 - gc as f64) as f32
            } else if g == maxc {
                (2.0_f64 + rc as f64 - bc as f64) as f32
            } else {
                (4.0_f64 + gc as f64 - rc as f64) as f32
            };

            // PIL: h = fmod((h / 6.0 + 1.0), 1.0)
            // h is float (f32), 6.0/1.0 are double literals → promotion to double.
            // The fmod result is stored back to float (f32).
            let h_double = h_float as f64;
            let h_fmod = ((h_double / 6.0) + 1.0) % 1.0;
            let h_stored = h_fmod as f32; // PIL stores h back to float here

            // PIL: uh = (int)(h * 255.0) — float promoted to double for multiplication
            // with 255.0 (double literal), then truncated
            let s_float = cr / maxc as f32; // s = cr / maxc in f32
            let us = ((s_float as f64) * 255.0) as u8;
            let uh = ((h_stored as f64) * 255.0) as u8;

            (uh, us)
        };
        *op = crate::raster::Rgb([uh, us, v]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts RGB to YCbCr using Pillow's lookup-table BT.601 conversion.
///
/// The returned image is `RGB8` storage interpreted as `Y`, `Cb`, `Cr`.
/// Uses precomputed tables matching PIL's ConvertYCbCr.c with SCALE=6.
/// Table formula: `table[i] = (int)(i * coeff * 64 + 0.5)`.
pub fn rgb_to_ycbcr(img: &DynamicImage) -> DynamicImage {
    use std::sync::OnceLock;

    fn make_table(coeff: f64) -> [i32; 256] {
        let mut t = [0i32; 256];
        for i in 0..256 {
            t[i] = (i as f64 * coeff * 64.0 + 0.5) as i32;
        }
        t
    }

    static Y_R: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_G: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_R: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_B: OnceLock<[i32; 256]> = OnceLock::new();

    let y_r = Y_R.get_or_init(|| make_table(0.299));
    let y_g = Y_G.get_or_init(|| make_table(0.587));
    let y_b = Y_B.get_or_init(|| make_table(0.114));
    let cb_r = CB_R.get_or_init(|| make_table(-0.16874));
    let cb_g = CB_G.get_or_init(|| make_table(-0.33126));
    let cb_b = CB_B.get_or_init(|| make_table(0.5));
    let cr_g = CR_G.get_or_init(|| make_table(-0.41869));
    let cr_b = CR_B.get_or_init(|| make_table(-0.08131));

    let rgb = match img {
        DynamicImage::ImageRgb8(rgb) => Cow::Borrowed(rgb.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgb8().into_raw()),
    };
    let (w, h) = (img.width(), img.height());
    let mut out = Vec::with_capacity(rgb.len());
    for ip in rgb.chunks_exact(3) {
        let r = ip[0] as usize;
        let g = ip[1] as usize;
        let b = ip[2] as usize;

        let y = ((y_r[r] + y_g[g] + y_b[b]) >> 6) as u8;
        let cb = (((cb_r[r] + cb_g[g] + cb_b[b]) >> 6) + 128) as u8;
        let cr = (((cb_b[r] + cr_g[g] + cr_b[b]) >> 6) + 128) as u8; // Cr_R = Cb_B

        out.extend_from_slice(&[y, cb, cr]);
    }
    DynamicImage::ImageRgb8(
        crate::raster::RgbImage::from_raw(w, h, out)
            .expect("YCbCr output preserves the source's RGB dimensions"),
    )
}

/// Converts YCbCr storage to RGB using Pillow's lookup-table BT.601 conversion.
///
/// The input is `RGB8` storage interpreted as `Y`, `Cb`, `Cr`; the returned
/// image is `RGB8`.
/// PIL uses precomputed tables with 6-bit fixed-point (SCALE=6, multiply by 64):
/// ```text
///   R = Y + (R_Cr[Cr] >> 6)
///   G = Y + ((G_Cb[Cb] + G_Cr[Cr]) >> 6)
///   B = Y + (B_Cb[Cb] >> 6)
/// ```
/// Tables extracted directly from PIL's _imaging C extension.
pub fn ycbcr_to_rgb(img: &DynamicImage) -> DynamicImage {
    const R_CR: [i32; 256] = [
        -11484, -11394, -11305, -11215, -11125, -11036, -10946, -10856, -10766, -10677, -10587,
        -10497, -10407, -10318, -10228, -10138, -10049, -9959, -9869, -9779, -9690, -9600, -9510,
        -9420, -9331, -9241, -9151, -9062, -8972, -8882, -8792, -8703, -8613, -8523, -8433, -8344,
        -8254, -8164, -8075, -7985, -7895, -7805, -7716, -7626, -7536, -7446, -7357, -7267, -7177,
        -7088, -6998, -6908, -6818, -6729, -6639, -6549, -6459, -6370, -6280, -6190, -6101, -6011,
        -5921, -5831, -5742, -5652, -5562, -5472, -5383, -5293, -5203, -5113, -5024, -4934, -4844,
        -4755, -4665, -4575, -4485, -4396, -4306, -4216, -4126, -4037, -3947, -3857, -3768, -3678,
        -3588, -3498, -3409, -3319, -3229, -3139, -3050, -2960, -2870, -2781, -2691, -2601, -2511,
        -2422, -2332, -2242, -2152, -2063, -1973, -1883, -1794, -1704, -1614, -1524, -1435, -1345,
        -1255, -1165, -1076, -986, -896, -807, -717, -627, -537, -448, -358, -268, -178, -89, 0,
        90, 179, 269, 359, 449, 538, 628, 718, 808, 897, 987, 1077, 1166, 1256, 1346, 1436, 1525,
        1615, 1705, 1795, 1884, 1974, 2064, 2153, 2243, 2333, 2423, 2512, 2602, 2692, 2782, 2871,
        2961, 3051, 3140, 3230, 3320, 3410, 3499, 3589, 3679, 3769, 3858, 3948, 4038, 4127, 4217,
        4307, 4397, 4486, 4576, 4666, 4756, 4845, 4935, 5025, 5114, 5204, 5294, 5384, 5473, 5563,
        5653, 5743, 5832, 5922, 6012, 6102, 6191, 6281, 6371, 6460, 6550, 6640, 6730, 6819, 6909,
        6999, 7089, 7178, 7268, 7358, 7447, 7537, 7627, 7717, 7806, 7896, 7986, 8076, 8165, 8255,
        8345, 8434, 8524, 8614, 8704, 8793, 8883, 8973, 9063, 9152, 9242, 9332, 9421, 9511, 9601,
        9691, 9780, 9870, 9960, 10050, 10139, 10229, 10319, 10408, 10498, 10588, 10678, 10767,
        10857, 10947, 11037, 11126, 11216, 11306, 11395,
    ];
    const G_CB: [i32; 256] = [
        2819, 2797, 2775, 2753, 2731, 2709, 2687, 2665, 2643, 2621, 2599, 2577, 2555, 2533, 2511,
        2489, 2467, 2445, 2423, 2401, 2379, 2357, 2335, 2313, 2291, 2269, 2247, 2225, 2202, 2180,
        2158, 2136, 2114, 2092, 2070, 2048, 2026, 2004, 1982, 1960, 1938, 1916, 1894, 1872, 1850,
        1828, 1806, 1784, 1762, 1740, 1718, 1696, 1674, 1652, 1630, 1608, 1586, 1564, 1542, 1520,
        1498, 1476, 1454, 1432, 1410, 1388, 1366, 1344, 1321, 1299, 1277, 1255, 1233, 1211, 1189,
        1167, 1145, 1123, 1101, 1079, 1057, 1035, 1013, 991, 969, 947, 925, 903, 881, 859, 837,
        815, 793, 771, 749, 727, 705, 683, 661, 639, 617, 595, 573, 551, 529, 507, 485, 463, 440,
        418, 396, 374, 352, 330, 308, 286, 264, 242, 220, 198, 176, 154, 132, 110, 88, 66, 44, 22,
        0, -21, -43, -65, -87, -109, -131, -153, -175, -197, -219, -241, -263, -285, -307, -329,
        -351, -373, -395, -417, -439, -462, -484, -506, -528, -550, -572, -594, -616, -638, -660,
        -682, -704, -726, -748, -770, -792, -814, -836, -858, -880, -902, -924, -946, -968, -990,
        -1012, -1034, -1056, -1078, -1100, -1122, -1144, -1166, -1188, -1210, -1232, -1254, -1276,
        -1298, -1320, -1343, -1365, -1387, -1409, -1431, -1453, -1475, -1497, -1519, -1541, -1563,
        -1585, -1607, -1629, -1651, -1673, -1695, -1717, -1739, -1761, -1783, -1805, -1827, -1849,
        -1871, -1893, -1915, -1937, -1959, -1981, -2003, -2025, -2047, -2069, -2091, -2113, -2135,
        -2157, -2179, -2201, -2224, -2246, -2268, -2290, -2312, -2334, -2356, -2378, -2400, -2422,
        -2444, -2466, -2488, -2510, -2532, -2554, -2576, -2598, -2620, -2642, -2664, -2686, -2708,
        -2730, -2752, -2774, -2796,
    ];
    const G_CR: [i32; 256] = [
        5850, 5805, 5759, 5713, 5667, 5622, 5576, 5530, 5485, 5439, 5393, 5347, 5302, 5256, 5210,
        5165, 5119, 5073, 5028, 4982, 4936, 4890, 4845, 4799, 4753, 4708, 4662, 4616, 4570, 4525,
        4479, 4433, 4388, 4342, 4296, 4251, 4205, 4159, 4113, 4068, 4022, 3976, 3931, 3885, 3839,
        3794, 3748, 3702, 3656, 3611, 3565, 3519, 3474, 3428, 3382, 3336, 3291, 3245, 3199, 3154,
        3108, 3062, 3017, 2971, 2925, 2879, 2834, 2788, 2742, 2697, 2651, 2605, 2559, 2514, 2468,
        2422, 2377, 2331, 2285, 2240, 2194, 2148, 2102, 2057, 2011, 1965, 1920, 1874, 1828, 1782,
        1737, 1691, 1645, 1600, 1554, 1508, 1463, 1417, 1371, 1325, 1280, 1234, 1188, 1143, 1097,
        1051, 1006, 960, 914, 868, 823, 777, 731, 686, 640, 594, 548, 503, 457, 411, 366, 320, 274,
        229, 183, 137, 91, 46, 0, -45, -90, -136, -182, -228, -273, -319, -365, -410, -456, -502,
        -547, -593, -639, -685, -730, -776, -822, -867, -913, -959, -1005, -1050, -1096, -1142,
        -1187, -1233, -1279, -1324, -1370, -1416, -1462, -1507, -1553, -1599, -1644, -1690, -1736,
        -1781, -1827, -1873, -1919, -1964, -2010, -2056, -2101, -2147, -2193, -2239, -2284, -2330,
        -2376, -2421, -2467, -2513, -2558, -2604, -2650, -2696, -2741, -2787, -2833, -2878, -2924,
        -2970, -3016, -3061, -3107, -3153, -3198, -3244, -3290, -3335, -3381, -3427, -3473, -3518,
        -3564, -3610, -3655, -3701, -3747, -3793, -3838, -3884, -3930, -3975, -4021, -4067, -4112,
        -4158, -4204, -4250, -4295, -4341, -4387, -4432, -4478, -4524, -4569, -4615, -4661, -4707,
        -4752, -4798, -4844, -4889, -4935, -4981, -5027, -5072, -5118, -5164, -5209, -5255, -5301,
        -5346, -5392, -5438, -5484, -5529, -5575, -5621, -5666, -5712, -5758, -5804,
    ];
    const B_CB: [i32; 256] = [
        -14515, -14402, -14288, -14175, -14062, -13948, -13835, -13721, -13608, -13495, -13381,
        -13268, -13154, -13041, -12928, -12814, -12701, -12587, -12474, -12360, -12247, -12134,
        -12020, -11907, -11793, -11680, -11567, -11453, -11340, -11226, -11113, -11000, -10886,
        -10773, -10659, -10546, -10433, -10319, -10206, -10092, -9979, -9865, -9752, -9639, -9525,
        -9412, -9298, -9185, -9072, -8958, -8845, -8731, -8618, -8505, -8391, -8278, -8164, -8051,
        -7938, -7824, -7711, -7597, -7484, -7371, -7257, -7144, -7030, -6917, -6803, -6690, -6577,
        -6463, -6350, -6236, -6123, -6010, -5896, -5783, -5669, -5556, -5443, -5329, -5216, -5102,
        -4989, -4876, -4762, -4649, -4535, -4422, -4309, -4195, -4082, -3968, -3855, -3741, -3628,
        -3515, -3401, -3288, -3174, -3061, -2948, -2834, -2721, -2607, -2494, -2381, -2267, -2154,
        -2040, -1927, -1814, -1700, -1587, -1473, -1360, -1246, -1133, -1020, -906, -793, -679,
        -566, -453, -339, -226, -112, 0, 113, 227, 340, 454, 567, 680, 794, 907, 1021, 1134, 1247,
        1361, 1474, 1588, 1701, 1815, 1928, 2041, 2155, 2268, 2382, 2495, 2608, 2722, 2835, 2949,
        3062, 3175, 3289, 3402, 3516, 3629, 3742, 3856, 3969, 4083, 4196, 4310, 4423, 4536, 4650,
        4763, 4877, 4990, 5103, 5217, 5330, 5444, 5557, 5670, 5784, 5897, 6011, 6124, 6237, 6351,
        6464, 6578, 6691, 6804, 6918, 7031, 7145, 7258, 7372, 7485, 7598, 7712, 7825, 7939, 8052,
        8165, 8279, 8392, 8506, 8619, 8732, 8846, 8959, 9073, 9186, 9299, 9413, 9526, 9640, 9753,
        9866, 9980, 10093, 10207, 10320, 10434, 10547, 10660, 10774, 10887, 11001, 11114, 11227,
        11341, 11454, 11568, 11681, 11794, 11908, 12021, 12135, 12248, 12361, 12475, 12588, 12702,
        12815, 12929, 13042, 13155, 13269, 13382, 13496, 13609, 13722, 13836, 13949, 14063, 14176,
        14289, 14403,
    ];

    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
        let y_ = ip[0] as i32;
        let cb = ip[1] as i32;
        let cr = ip[2] as i32;

        // PIL: right-shift is arithmetic (sign-extending) for negative values
        let r = y_ + (R_CR[cr as usize] >> 6);
        let g = y_ + ((G_CB[cb as usize] + G_CR[cr as usize]) >> 6);
        let b = y_ + (B_CB[cb as usize] >> 6);

        *op = crate::raster::Rgb([
            r.clamp(0, 255) as u8,
            g.clamp(0, 255) as u8,
            b.clamp(0, 255) as u8,
        ]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts Pillow `"I"` storage to RGB.
///
/// The input is RGBA storage interpreted as little-endian `i32` pixels. Each
/// value is clamped to `0..=255` and broadcast to `R=G=B`.
pub fn i_to_rgb(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let rgba = match img {
        // I mode is already stored as four little-endian bytes per sample.
        // Borrow that carrier instead of cloning the full frame through the
        // convenience accessor.
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.chunks_exact(4)) {
        // I mode packs int32 as RGBA bytes (little-endian)
        let val = i32::from_le_bytes([ip[0], ip[1], ip[2], ip[3]]);
        let clamped = val.clamp(0, 255) as u8;
        *op = crate::raster::Rgb([clamped, clamped, clamped]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts Pillow `"F"` storage to RGB.
///
/// The input is RGBA storage interpreted as little-endian `f32` pixels. Each
/// value is clamped to `0..=255`, truncated to a byte, and broadcast to `R=G=B`.
pub fn f_to_rgb(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let rgba = match img {
        // F mode is already stored as four little-endian bytes per sample.
        // Borrow that carrier instead of cloning it through `to_rgba8`.
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.chunks_exact(4)) {
        let val = f32::from_le_bytes([ip[0], ip[1], ip[2], ip[3]]);
        // PIL: F→X casts float to int via truncation
        let clamped = val.clamp(0.0, 255.0) as u8;
        *op = crate::raster::Rgb([clamped, clamped, clamped]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts Pillow `I` storage to the intermediate `L` image used by
/// `Image.convert` when the destination has no direct integer converter.
///
/// This is the `i2l` path in Pillow's `Convert.c`: values at or below zero
/// become zero, values at or above 255 become 255, and interior values are
/// truncated rather than rounded.
pub fn i_to_l(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let rgba = match img {
        // I mode is already stored as four little-endian bytes per sample.
        // Borrow that scalar carrier rather than cloning the whole frame.
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    // I storage is a packed little-endian i32 stream. When its byte buffer is
    // naturally aligned, view it as words so the hot loop avoids rebuilding
    // each value from four indexed bytes. Preserve the byte-wise path for
    // unusual alignment or malformed internal buffers.
    let output = match bytemuck::try_cast_slice::<u8, i32>(rgba.as_ref()) {
        Ok(values) => values
            .iter()
            .map(|&value| i32::from_le(value).clamp(0, 255) as u8)
            .collect(),
        Err(_) => rgba
            .chunks_exact(4)
            .map(|ip| {
                let value = i32::from_le_bytes([ip[0], ip[1], ip[2], ip[3]]);
                value.clamp(0, 255) as u8
            })
            .collect(),
    };
    DynamicImage::ImageLuma8(
        crate::raster::GrayImage::from_raw(w, h, output)
            .expect("I to L output matches the source dimensions"),
    )
}

/// Converts Pillow `F` storage to the intermediate `L` image used by
/// `Image.convert` when the destination has no direct float converter.
///
/// This is the `f2l` path in Pillow's `Convert.c`, including truncation of
/// interior floating-point samples.
pub fn f_to_l(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let rgba = match img {
        // F mode is already stored as four little-endian bytes per sample.
        // Borrow that scalar carrier rather than cloning the whole frame.
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let output = rgba
        .chunks_exact(4)
        .map(|ip| {
            let value = f32::from_le_bytes([ip[0], ip[1], ip[2], ip[3]]);
            value.clamp(0.0, 255.0) as u8
        })
        .collect();
    DynamicImage::ImageLuma8(
        crate::raster::GrayImage::from_raw(w, h, output)
            .expect("F to L output matches the source dimensions"),
    )
}

/// Converts Pillow `I` storage to `F` storage using the direct `i2f` path.
pub fn i_to_f(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let rgba = match img {
        // I mode is already stored as four little-endian bytes per sample.
        // Borrow that scalar carrier rather than cloning the whole frame.
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let mut out = crate::raster::RgbaImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.chunks_exact(4)) {
        let value = i32::from_le_bytes([ip[0], ip[1], ip[2], ip[3]]) as f32;
        *op = crate::raster::Rgba(value.to_le_bytes());
    }
    DynamicImage::ImageRgba8(out)
}

/// Converts Pillow `F` storage to `I` storage using the direct `f2i` path.
pub fn f_to_i(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let rgba = match img {
        // F mode is already stored as four little-endian bytes per sample.
        // Borrow that scalar carrier rather than cloning the whole frame.
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let mut out = crate::raster::RgbaImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.chunks_exact(4)) {
        let value = f32::from_le_bytes([ip[0], ip[1], ip[2], ip[3]]) as i32;
        *op = crate::raster::Rgba(value.to_le_bytes());
    }
    DynamicImage::ImageRgba8(out)
}

/// Converts a luma image to YCbCr through Pillow's direct `l2ycbcr` path.
pub fn luma_to_ycbcr(img: &DynamicImage) -> DynamicImage {
    let luma = img.to_luma8();
    let (w, h) = luma.dimensions();
    let mut out = RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(luma.pixels()) {
        *op = crate::raster::Rgb([ip[0], 128, 128]);
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts `P` palette-index storage to RGB.
///
/// `img` is interpreted as one index byte per pixel. A missing palette or a
/// missing entry maps to black, matching Pillow's empty/default `P` palette.
pub fn p_to_rgb(img: &DynamicImage, palette: Option<&[u8]>) -> DynamicImage {
    let luma = img.to_luma8();
    let (w, h) = luma.dimensions();
    let mut out = crate::raster::RgbImage::new(w, h);
    let palette = palette.unwrap_or(&[]);
    for (op, ip) in out.pixels_mut().zip(luma.pixels()) {
        let idx = ip[0] as usize;
        if idx * 3 + 2 < palette.len() {
            op[0] = palette[idx * 3];
            op[1] = palette[idx * 3 + 1];
            op[2] = palette[idx * 3 + 2];
        }
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts `PA` palette-index-plus-alpha storage to RGB.
///
/// PA has no implicit grayscale fallback: when no palette is attached, Pillow
/// maps every index to black and preserves the alpha only for alpha-bearing
/// destinations. RGB conversion itself ignores the per-pixel alpha byte.
pub fn pa_to_rgb(img: &DynamicImage, palette: Option<&[u8]>) -> DynamicImage {
    let indices = img.to_luma8();
    let (w, h) = indices.dimensions();
    let mut out = crate::raster::RgbImage::new(w, h);
    let palette = palette.unwrap_or(&[]);
    for (op, ip) in out.pixels_mut().zip(indices.pixels()) {
        let index = usize::from(ip[0]);
        let base = index * 3;
        if base + 2 < palette.len() {
            op[0] = palette[base];
            op[1] = palette[base + 1];
            op[2] = palette[base + 2];
        }
    }
    DynamicImage::ImageRgb8(out)
}

/// Converts a non-standard Pillow mode into a standard RGB-family image.
///
/// `palette` supplies `P` mode RGB triples when `src_mode == "P"`.
///
/// # Returns
///
/// `Some(image)` for modes that need reinterpretation (`CMYK`, `HSV`, `YCbCr`,
/// `I`, `F`, `P`, `PA`) and `None` for modes that are already standard.
pub fn convert_from_nonstandard(
    src_mode: &str,
    img: &DynamicImage,
    palette: Option<&[u8]>,
) -> Option<DynamicImage> {
    match src_mode {
        "CMYK" => Some(cmyk_to_rgb(img)),
        "HSV" => Some(hsv_to_rgb(img)),
        "YCbCr" => Some(ycbcr_to_rgb(img)),
        "I" => Some(i_to_rgb(img)),
        "F" => Some(f_to_rgb(img)),
        "P" => Some(p_to_rgb(img, palette)),
        "PA" => Some(pa_to_rgb(img, palette)),
        _ => None,
    }
}
