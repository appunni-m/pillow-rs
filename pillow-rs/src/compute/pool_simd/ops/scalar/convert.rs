//! Scalar fallback for scalar mode conversion.

/// Convert: convert between color modes.
///
/// `target_mode`: 0=L, 1=LA, 2=RGB, 3=RGBA
///
/// Supported conversions:
/// - L(0) -> LA(1): add alpha=255
/// - L(0) -> RGB(2): replicate R to G,B
/// - L(0) -> RGBA(3): replicate R to G,B, add alpha=255
/// - LA(1) -> L(0): drop alpha, replicate R to G,B
/// - LA(1) -> RGB(2): replicate R to G,B, drop alpha
/// - LA(1) -> RGBA(3): replicate R to G,B, keep A
/// - RGB(2) -> L(0): BT.601 luma
/// - RGB(2) -> LA(1): BT.601 luma, add alpha=255
/// - RGB(2) -> RGBA(3): add alpha=255
/// - RGBA(3) -> L(0): BT.601 luma, drop alpha
/// - RGBA(3) -> LA(1): BT.601 luma, keep A
/// - RGBA(3) -> RGB(2): drop alpha
///
/// Returns `(Vec<u32>, w, h)` — same size as input.
/// Identity conversions (same mode) skip computation and only clamp alpha.
///
/// mode: 0=L, 1=LA, 2=RGB, 3=RGBA
#[inline]
pub fn convert(
    pixels: &[u32],
    w: u32,
    h: u32,
    mode: u32,
    target_mode: u32,
) -> (Vec<u32>, u32, u32) {
    let n = (w * h) as usize;
    let mut out = Vec::with_capacity(n);

    for &p in pixels.iter() {
        push_converted_pixel(&mut out, p, mode, target_mode);
    }

    (out, w, h)
}

#[inline(always)]
fn push_converted_pixel(out: &mut Vec<u32>, p: u32, mode: u32, target_mode: u32) {
    let r_src = p & 0xFF;
    let g_src = (p >> 8) & 0xFF;
    let b_src = (p >> 16) & 0xFF;
    let a_src = p >> 24;

    match (mode, target_mode) {
        // ── Same-mode identities ──
        (0, 0) | (1, 1) | (2, 2) | (3, 3) => {
            let out_a = if mode == 0 || mode == 2 { 0xFF } else { a_src };
            out.push(r_src | (g_src << 8) | (b_src << 16) | (out_a << 24));
        }

        // ── L(0) -> other modes ──
        (0, 1) => {
            // L->LA: add alpha=255
            out.push(r_src | (r_src << 8) | (r_src << 16) | (0xFF << 24));
        }
        (0, 2) | (0, 3) => {
            // L->RGB or L->RGBA: replicate R to G,B
            out.push(r_src | (r_src << 8) | (r_src << 16) | (0xFF << 24));
        }

        // ── LA(1) -> other modes ──
        (1, 0) | (1, 2) => {
            // LA->L or LA->RGB: drop alpha, replicate R to G,B
            out.push(r_src | (r_src << 8) | (r_src << 16) | (0xFF << 24));
        }
        (1, 3) => {
            // LA->RGBA: replicate R to G,B, keep A
            out.push(r_src | (r_src << 8) | (r_src << 16) | (a_src << 24));
        }

        // ── RGB(2) -> other modes ──
        (2, 0) | (2, 1) => {
            // RGB->L or RGB->LA: BT.601 luma
            let luma = ((299 * r_src + 587 * g_src + 114 * b_src + 500) / 1000).min(255);
            out.push(luma | (luma << 8) | (luma << 16) | (0xFF << 24));
        }
        (2, 3) => {
            // RGB->RGBA: add alpha=255
            out.push(r_src | (g_src << 8) | (b_src << 16) | (0xFF << 24));
        }

        // ── RGBA(3) -> other modes ──
        (3, 0) | (3, 1) => {
            // RGBA->L or RGBA->LA: BT.601 luma
            let luma = ((299 * r_src + 587 * g_src + 114 * b_src + 500) / 1000).min(255);
            let out_a = if target_mode == 1 { a_src } else { 0xFF };
            out.push(luma | (luma << 8) | (luma << 16) | (out_a << 24));
        }
        (3, 2) => {
            // RGBA->RGB: drop alpha
            out.push(r_src | (g_src << 8) | (b_src << 16) | (0xFF << 24));
        }

        // ── L/LA (0/1) -> CMYK (4): gray to K, C=M=Y=0 ──
        (0, 4) | (1, 4) => {
            let k = 255u32.saturating_sub(r_src) & 0xFF;
            out.push(k << 24);
        }

        // ── RGB/RGBA (2/3) -> CMYK (4): inverse, K=0 ──
        (2, 4) | (3, 4) => {
            out.push(
                (255u32.wrapping_sub(r_src) & 0xFF)
                    | ((255u32.wrapping_sub(g_src) & 0xFF) << 8)
                    | ((255u32.wrapping_sub(b_src) & 0xFF) << 16),
            );
        }

        // ── Unreachable (all combos covered above) ──
        _ => {
            let out_a = if target_mode == 0 || target_mode == 2 {
                0xFF
            } else {
                a_src
            };
            out.push(r_src | (g_src << 8) | (b_src << 16) | (out_a << 24));
        }
    }
}
