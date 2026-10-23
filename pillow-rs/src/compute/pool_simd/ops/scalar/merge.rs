//! Scalar fallback for scalar image-band merge.

// ── Composite, Merge, Sharpen, Autocontrast ────────────────────────────────

/// Merge: combine single-channel bands into a multi-channel image.
///
/// Each band is a `&[u32]` of the same length as `pixels`. Band values are
/// extracted from the R byte of each u32 element.
///
/// Mode mapping:
/// - L (0): band[0] -> all channels, alpha = 0xFF.
/// - LA (1): band[0] -> R (luma), band[1] -> A. G = B = R.
/// - RGB (2): band[0] -> R, band[1] -> G, band[2] -> B, alpha = 0xFF.
/// - RGBA (3): band[0] -> R, band[1] -> G, band[2] -> B, band[3] -> A.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn merge(pixels: &mut [u32], mode: u32, bands: &[&[u32]]) {
    let n = pixels.len();
    match mode {
        0 => {
            for i in 0..n {
                let v = bands[0][i] & 0xFF;
                pixels[i] = v | (v << 8) | (v << 16) | 0xFF00_0000;
            }
        }
        1 => {
            for i in 0..n {
                let v = bands[0][i] & 0xFF;
                let a = (bands[1][i] & 0xFF) << 24;
                pixels[i] = v | (v << 8) | (v << 16) | a;
            }
        }
        2 => {
            for i in 0..n {
                let r = bands[0][i] & 0xFF;
                let g = (bands[1][i] & 0xFF) << 8;
                let b = (bands[2][i] & 0xFF) << 16;
                pixels[i] = r | g | b | 0xFF00_0000;
            }
        }
        3 => {
            for i in 0..n {
                let r = bands[0][i] & 0xFF;
                let g = (bands[1][i] & 0xFF) << 8;
                let b = (bands[2][i] & 0xFF) << 16;
                let a = (bands[3][i] & 0xFF) << 24;
                pixels[i] = r | g | b | a;
            }
        }
        _ => {}
    }
}
