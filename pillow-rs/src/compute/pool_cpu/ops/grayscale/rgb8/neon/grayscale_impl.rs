//! Original AArch64 NEON RGB arithmetic and padded vector tail.

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn grayscale_rgb_luma_vector_neon(source: &[u8]) -> core::arch::aarch64::uint8x16_t {
    use core::arch::aarch64 as neon;

    debug_assert!(source.len() >= 48);
    macro_rules! luma_half {
        ($r:expr, $g:expr, $b:expr) => {{
            let r = neon::vmovl_u8($r);
            let g = neon::vmovl_u8($g);
            let b = neon::vmovl_u8($b);
            // 19595=77*256-117, 38470=150*256+70,
            // 7471=29*256+47. Wrapping u16 arithmetic on this biased
            // residual yields the exact rounded luma result.
            let base = neon::vaddq_u16(
                neon::vaddq_u16(neon::vmulq_n_u16(r, 77), neon::vmulq_n_u16(g, 150)),
                neon::vmulq_n_u16(b, 29),
            );
            let residual = neon::vaddq_u16(
                neon::vaddq_u16(
                    neon::vsubq_u16(neon::vmulq_n_u16(g, 70), neon::vmulq_n_u16(r, 117)),
                    neon::vmulq_n_u16(b, 47),
                ),
                neon::vdupq_n_u16(32768),
            );
            let carry = neon::vshrq_n_u16(residual, 8);
            let rounded = neon::vshrq_n_u16(neon::vaddq_u16(base, carry), 8);
            neon::vmovn_u16(rounded)
        }};
    }

    // SAFETY: execute passes an exact 48-byte source slice; the tail helper
    // passes an initialized 48-byte padded block. `vld3q_u8` reads exactly 48 initialized
    // bytes. The returned vector contains all sixteen computed luma values.
    unsafe {
        let rgb = neon::vld3q_u8(source.as_ptr());
        let low = luma_half!(
            neon::vget_low_u8(rgb.0),
            neon::vget_low_u8(rgb.1),
            neon::vget_low_u8(rgb.2)
        );
        let high = luma_half!(
            neon::vget_high_u8(rgb.0),
            neon::vget_high_u8(rgb.1),
            neon::vget_high_u8(rgb.2)
        );
        neon::vcombine_u8(low, high)
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn grayscale_rgb_block_neon(source: &[u8]) -> [u8; 16] {
    use core::arch::aarch64 as neon;

    let values = grayscale_rgb_luma_vector_neon(source);
    let mut output = [0u8; 16];
    // SAFETY: The local array has sixteen writable bytes and the NEON vector
    // contains exactly sixteen luma samples.
    unsafe { neon::vst1q_u8(output.as_mut_ptr(), values) };
    output
}

/// Store complete NEON RGB blocks directly into the final grayscale vector.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
pub(crate) fn execute(source: &[u8], pixels: usize) -> (Vec<u8>, u64) {
    use core::arch::aarch64 as neon;

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::<u8>::with_capacity(pixels);
    for pixel in (0..vector_pixels).step_by(16) {
        let source_start = pixel * 3;
        let values = grayscale_rgb_luma_vector_neon(&source[source_start..source_start + 48]);
        // SAFETY: `output` reserves at least `pixels` bytes, and each write
        // initializes sixteen bytes at a vector-aligned pixel offset below
        // `vector_pixels`. The source slice above proves the full 48-byte
        // interleaved block exists. `set_len` runs only after all full blocks
        // have been initialized; the partial block is appended afterward.
        unsafe { neon::vst1q_u8(output.as_mut_ptr().add(pixel), values) };
    }
    // SAFETY: Every byte in this vector prefix was initialized by the stores
    // above; `vector_pixels` never exceeds the reserved pixel capacity.
    unsafe { output.set_len(vector_pixels) };

    if vector_pixels < pixels {
        let source_start = vector_pixels * 3;
        let remainder = &source[source_start..];
        let mut padded = [0u8; 48];
        padded[..remainder.len()].copy_from_slice(remainder);
        let tail = grayscale_rgb_block_neon(&padded);
        output.extend_from_slice(&tail[..pixels - vector_pixels]);
    }

    (output, pixels.div_ceil(16) as u64)
}

