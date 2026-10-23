//! Scalar fallback for scalar palette remapping and evaluation operations.

/// Remap palette indices with Pillow's inverse destination map.
///
/// Each source pixel's R byte is an old palette index. `inverse_map[old]`
/// contains its new index; entries omitted from the destination map are zero.
///
/// Returns a Vec<u32> of packed RGBA pixels (same length as input).
/// Mode-aware: G/B set to looked-up values for RGB/RGBA (mode >= 2), mirrored
/// from R for L/LA. Alpha forced to 0xFF for non-alpha modes.
/// mode: 0=L (P-mode encoding), 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn remap_palette(pixels: &[u32], mode: u32, inverse_map: &[u8; 256]) -> Vec<u32> {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);
    let mut out = Vec::with_capacity(pixels.len());

    for &p in pixels.iter() {
        let old_r = (p & 0xFF) as usize;
        let old_g = ((p >> 8) & 0xFF) as usize;
        let old_b = ((p >> 16) & 0xFF) as usize;
        let r = u32::from(inverse_map[old_r]);
        let g = u32::from(inverse_map[old_g]);
        let b = u32::from(inverse_map[old_b]);

        let out_g = if has_gb { g } else { r };
        let out_b = if has_gb { b } else { r };
        let out_a = if has_a { p & 0xFF00_0000 } else { 0xFF00_0000 };

        out.push(r | (out_g << 8) | (out_b << 16) | out_a);
    }

    out
}

/// Apply per-channel lookup table to pixels.
///
/// LUT has 256 entries per channel (256*4 = 1024 bytes):
///   [0..256)   -> R/L channel
///   [256..512) -> G channel
///   [512..768) -> B channel
///   [768..1024) -> A channel
///
/// For L mode, only the first 256 entries are used and G/B mirror the L
/// output.  LA uses the first segment for L and the second segment for A;
/// Pillow's public `Image.point` contract supplies one table per band.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn eval(pixels: &mut [u32], mode: u32, lut: &[u8; 1024]) {
    let has_gb = super::mode_has_green_blue(mode);
    let has_a = super::mode_has_alpha(mode);

    for p in pixels.iter_mut() {
        let r = (*p & 0xFF) as usize;
        let g = ((*p >> 8) & 0xFF) as usize;
        let b = ((*p >> 16) & 0xFF) as usize;
        let a = ((*p >> 24) & 0xFF) as usize;

        if has_gb {
            // RGB/RGBA: per-channel LUT segments
            let out_r = lut[r] as u32;
            let out_g = lut[256 + g] as u32;
            let out_b = lut[512 + b] as u32;
            let out_a = if has_a {
                (lut[768 + a] as u32) << 24
            } else {
                0xFF00_0000
            };
            *p = out_r | (out_g << 8) | (out_b << 16) | out_a;
        } else {
            // L: first segment; LA: first segment for luma and second for
            // alpha. G = B = R in the packed representation.
            let out_r = lut[r] as u32;
            let out_a = if has_a {
                let alpha_offset = if mode == 1 { 256 } else { 768 };
                (lut[alpha_offset + a] as u32) << 24
            } else {
                0xFF00_0000
            };
            *p = out_r | (out_r << 8) | (out_r << 16) | out_a;
        }
    }
}
