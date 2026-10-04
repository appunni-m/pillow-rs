//! PIL-compatible resize using two-pass separable interpolation.
//!
//! This implements PIL's exact two-pass approach:
//! 1. Horizontal pass: for each source row, compute weighted sum per output column,
//!    round to u8, store in intermediate image.
//! 2. Vertical pass: for each output column at each output row, compute weighted sum
//!    from intermediate rows, round to u8.
//!
//! PIL uses fixed-point arithmetic (PRECISION_BITS=22) but we use f64 with
//! intermediate rounding to match the two-pass quantization behavior.

use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, FromColor, ImageBuffer, Luma, Rgba};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

/// View little-endian F-mode sample bytes as native `f32` values when that is
/// safe, otherwise decode into owned samples. Select at most `sample_count`
/// words and ignore incomplete trailing bytes, matching the old
/// `chunks_exact(4).take(sample_count)` decoders.
#[must_use]
pub(crate) fn f32_samples_from_le_bytes(bytes: &[u8], sample_count: usize) -> Cow<'_, [f32]> {
    let sample_bytes = bytes.get(..sample_count.saturating_mul(4)).unwrap_or(bytes);
    #[cfg(target_endian = "little")]
    if let Ok(samples) = bytemuck::try_cast_slice::<u8, f32>(sample_bytes)
        && samples.len() == sample_count
    {
        return Cow::Borrowed(samples);
    }

    Cow::Owned(
        sample_bytes
            .chunks_exact(4)
            .take(sample_count)
            .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
            .collect(),
    )
}

/// View little-endian I-mode sample bytes as native `i32` values when the
/// storage is aligned; otherwise decode them without changing sample bits.
/// Select at most `sample_count` words and ignore incomplete trailing bytes,
/// matching the existing four-byte scalar-carrier readers.
#[must_use]
pub(crate) fn i32_samples_from_le_bytes(bytes: &[u8], sample_count: usize) -> Cow<'_, [i32]> {
    let sample_bytes = bytes.get(..sample_count.saturating_mul(4)).unwrap_or(bytes);
    #[cfg(target_endian = "little")]
    if let Ok(samples) = bytemuck::try_cast_slice::<u8, i32>(sample_bytes)
        && samples.len() == sample_count
    {
        return Cow::Borrowed(samples);
    }

    Cow::Owned(
        sample_bytes
            .chunks_exact(4)
            .take(sample_count)
            .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
            .collect(),
    )
}

#[cfg(test)]
mod f32_sample_view_tests {
    use super::f32_samples_from_le_bytes;
    use crate::raster::RgbaImage;
    use std::borrow::Cow;

    #[test]
    fn aligned_little_endian_scalar_carrier_is_borrowed() {
        let words = [1.25f32.to_bits(), (-0.0f32).to_bits()];
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        let image = RgbaImage::from_raw(2, 1, bytes).expect("two scalar samples");
        let raw = image.as_raw();
        let samples = f32_samples_from_le_bytes(raw, words.len());

        if cfg!(target_endian = "little")
            && (raw.as_ptr() as usize).is_multiple_of(std::mem::align_of::<f32>())
        {
            assert!(matches!(samples, Cow::Borrowed(_)));
        } else {
            assert!(matches!(samples, Cow::Owned(_)));
        }
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>(),
            words
        );
    }

    #[test]
    fn unaligned_little_endian_samples_decode_without_changing_word_bits() {
        let expected = [1.25f32.to_bits(), (-0.0f32).to_bits(), 0x7fc1_2345];
        let bytes = expected
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        let mut storage = vec![0; bytes.len() + std::mem::align_of::<f32>()];
        let base = storage.as_ptr() as usize;
        let offset = (0..std::mem::align_of::<f32>())
            .find(|offset| (base + offset) % std::mem::align_of::<f32>() != 0)
            .expect("an unaligned byte offset must exist");
        storage[offset..offset + bytes.len()].copy_from_slice(&bytes);

        let samples = f32_samples_from_le_bytes(&storage[offset..], expected.len());
        assert!(matches!(samples, Cow::Owned(_)));
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[cfg(test)]
mod i32_sample_view_tests {
    use super::i32_samples_from_le_bytes;
    use std::borrow::Cow;

    #[test]
    fn aligned_little_endian_integer_carrier_is_borrowed() {
        let expected = [i32::MIN, -123_456_789, -1, 0, 1, i32::MAX];
        let bytes = expected
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        let samples = i32_samples_from_le_bytes(&bytes, expected.len());

        if cfg!(target_endian = "little")
            && (bytes.as_ptr() as usize).is_multiple_of(std::mem::align_of::<i32>())
        {
            assert!(matches!(samples, Cow::Borrowed(_)));
        } else {
            assert!(matches!(samples, Cow::Owned(_)));
        }
        assert_eq!(samples.as_ref(), expected);
    }

    #[test]
    fn unaligned_integer_carrier_decodes_signed_words_and_ignores_trailing_bytes() {
        let expected = [i32::MIN, -1, 0, i32::MAX];
        let bytes = expected
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        let mut storage = vec![0; bytes.len() + std::mem::align_of::<i32>() + 1];
        let base = storage.as_ptr() as usize;
        let offset = (0..std::mem::align_of::<i32>())
            .find(|offset| (base + offset) % std::mem::align_of::<i32>() != 0)
            .expect("an unaligned byte offset must exist");
        storage[offset..offset + bytes.len()].copy_from_slice(&bytes);
        storage[offset + bytes.len()] = 0xa5;

        let samples = i32_samples_from_le_bytes(&storage[offset..], expected.len());
        assert!(matches!(samples, Cow::Owned(_)));
        assert_eq!(samples.as_ref(), expected);
    }
}

/// Evaluate the scalar sine used by Pillow's ARM64 resampler on WASM.
///
/// The native Pillow build uses the Apple ARM64 `libsystem_m` implementation
/// rather than a correctly-rounded libm.  WebAssembly's `f64::sin` follows
/// the target's bundled libm and therefore chooses a different last bit for a
/// small fraction of Lanczos coefficients.  Port the short/medium argument
/// paths of Apple's routine so coefficient generation has the same result
/// without crossing the runtime FFI boundary.  Resize kernels only call this
/// helper for finite arguments in the interval supported by the medium path.
#[inline]
pub(crate) fn pillow_sin_f64(value: f64) -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return value.sin();
    }

    #[cfg(target_arch = "wasm32")]
    {
        if value == 0.0 || !value.is_finite() {
            return value.sin();
        }

        // Constants are the exact bit patterns loaded by Apple's ARM64
        // sin() entry point (libsystem_m.dylib, _sin at offset 0x968).
        const PI_OVER_FOUR: f64 = f64::from_bits(0x3fe9_21fb_5444_2d18);
        const MEDIUM_BOUND: f64 = f64::from_bits(0x4120_0001_3be5_7a40);
        const TWO_OVER_PI: f64 = f64::from_bits(0x3fe4_5f30_6dc9_c883);
        const HALF_PI_HIGH: f64 = f64::from_bits(0x3ff9_21fb_5444_0000);
        const HALF_PI_LOW: f64 = f64::from_bits(0x3d86_8c23_4c4c_0000);
        const HALF_PI_TAIL: f64 = f64::from_bits(0x3b29_8a2e_0370_7345);

        // The coefficients below are loaded in pairs by the native routine.
        // Keeping their exact f64 encodings avoids a decimal-parser change in
        // the final coefficient bit.
        const SIN_C7: f64 = f64::from_bits(0x3de5_d8fd_1fd1_9ccd);
        const SIN_C6: f64 = f64::from_bits(0xbe5a_e5e5_a929_1f5d);
        const SIN_C5: f64 = f64::from_bits(0x3ec7_1de3_567d_48a1);
        const SIN_C4: f64 = f64::from_bits(0xbf2a_01a0_19bf_df03);
        const SIN_C3: f64 = f64::from_bits(0x3f81_1111_1110_f7d0);
        const SIN_C2: f64 = f64::from_bits(0xbfc5_5555_5555_5548);
        const COS_C7: f64 = f64::from_bits(0xbda8_fa49_a086_1a9b);
        const COS_C6: f64 = f64::from_bits(0x3e21_ee9d_7b4e_3f05);
        const COS_C5: f64 = f64::from_bits(0xbe92_7e4f_7eac_4bc6);
        const COS_C4: f64 = f64::from_bits(0x3efa_01a0_19c8_44f5);
        const COS_C3: f64 = f64::from_bits(0xbf56_c16c_16c1_4f91);
        const COS_C2: f64 = f64::from_bits(0x3fa5_5555_5555_554b);
        const COS_C1: f64 = f64::from_bits(0xbfe0_0000_0000_0000);

        #[inline]
        fn sin_polynomial(z: f64) -> f64 {
            let mut polynomial = SIN_C7.mul_add(z, SIN_C6);
            polynomial = polynomial.mul_add(z, SIN_C5);
            polynomial = polynomial.mul_add(z, SIN_C4);
            polynomial = polynomial.mul_add(z, SIN_C3);
            polynomial = polynomial.mul_add(z, SIN_C2);
            z * polynomial
        }

        #[inline]
        fn cos_polynomial(z: f64) -> f64 {
            let mut polynomial = COS_C7.mul_add(z, COS_C6);
            polynomial = polynomial.mul_add(z, COS_C5);
            polynomial = polynomial.mul_add(z, COS_C4);
            polynomial = polynomial.mul_add(z, COS_C3);
            polynomial = polynomial.mul_add(z, COS_C2);
            polynomial = polynomial.mul_add(z, COS_C1);
            z.mul_add(polynomial, 1.0)
        }

        let absolute = value.abs();
        if absolute <= PI_OVER_FOUR {
            let z = value * value;
            return value.mul_add(sin_polynomial(z), value);
        }
        if absolute >= MEDIUM_BOUND {
            return value.sin();
        }

        // ARM64 uses round-to-nearest-even (`frintn`) for the nearest
        // half-pi multiple.  The residual is kept as two f64 words and the
        // split is retained through the final polynomial add.
        let quotient = value * TWO_OVER_PI;
        let nearest = quotient.round_ties_even();
        let quadrant = nearest as i64;
        let high = nearest * HALF_PI_HIGH;
        let low = nearest * HALF_PI_LOW;
        let tail = nearest * HALF_PI_TAIL;
        let first = value - high;
        let residual_high = first - low;
        let compensation = (first - residual_high) - low;
        let residual_low = compensation - tail;

        let result = if quadrant & 1 == 0 {
            let residual = residual_high + residual_low;
            let z = residual * residual;
            let correction = sin_polynomial(z);
            // Keep the native two-word final grouping: fma(residual,
            // correction, residual_low) + residual_high.
            residual.mul_add(correction, residual_low) + residual_high
        } else {
            // Apple's cosine path uses the high/low split to retain the
            // residual square's leading bits before evaluating its polynomial.
            let z = residual_high * residual_low;
            let z = z + z;
            let z = residual_high.mul_add(residual_high, z);
            cos_polynomial(z)
        };
        if (quadrant >> 1) & 1 == 0 {
            result
        } else {
            -result
        }
    }
}

// ── Filter kernels ──

/// Box / Nearest-neighbor kernel.
fn kernel_box(x: f64) -> f64 {
    if x > -0.5 && x <= 0.5 { 1.0 } else { 0.0 }
}

/// Triangle (bilinear) kernel.
fn kernel_triangle(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 { 1.0 - a } else { 0.0 }
}

/// Catmull-Rom (bicubic) kernel.
fn kernel_catrom(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 {
        // Match Pillow's Resample.c Horner evaluation exactly.  Expanding
        // this polynomial with powi changes the last f64 bits for some
        // heterogeneous F samples, which becomes a visible f32 ULP after
        // ImagingResample stores the accumulated value.
        let t = 1.5f64.mul_add(a, -2.5);
        a.mul_add(t * a, 1.0)
    } else if a < 2.0 {
        let t = (a - 5.0).mul_add(a, 8.0);
        a.mul_add(t, -4.0) * -0.5
    } else {
        0.0
    }
}

/// Lanczos kernel with window `a`.
fn kernel_lanczos(x: f64, a: f64) -> f64 {
    if x.abs() >= a {
        return 0.0;
    }
    if x.abs() < 1e-10 {
        return 1.0;
    }
    let pix = std::f64::consts::PI * x;
    let sa = pillow_sin_f64(pix) / pix;
    // Pillow's src/libImaging/Resample.c forms x/a before multiplying by pi.
    // Reassociating this as pi*x/a changes a few wide cancellation rows by
    // one f64 ULP before the final f32 store.
    let scaled_pix = (x / a) * std::f64::consts::PI;
    let s = pillow_sin_f64(scaled_pix) / scaled_pix;
    sa * s
}

/// Hamming kernel.
fn kernel_hamming(x: f64) -> f64 {
    let x = x.abs();
    if x == 0.0 {
        // Resample.c has an exact-zero special case.  Values merely close to
        // zero still run through sin/cos; the float constants then leave a
        // visible residual (0.54f + 0.46f) in cancellation-sensitive F rows.
        return 1.0;
    }
    if x >= 1.0 {
        return 0.0;
    }
    // Pillow's Hamming resampler is a windowed sinc, not only the cosine
    // window. This mirrors the Hamming branch in Pillow's Resample.c and is
    // observable on downsampled impulses.
    let pix = std::f64::consts::PI * x;
    // Resample.c evaluates sin/cos together and contracts the Hamming
    // window's `0.46f * cos + 0.54f` expression.  Preserve both the
    // float-to-double constants and the fused operation; separating the
    // window terms changes cancellation rows by one f32 ULP.
    let (sin, cos) = pix.sin_cos();
    (sin / pix) * cos.mul_add(0.46_f32 as f64, 0.54_f32 as f64)
}

fn kernel_lanczos3(x: f64) -> f64 {
    kernel_lanczos(x, 3.0)
}

/// Choose kernel function and support based on filter type.
/// Returns the kernel and support used by the generic scalar resampler.
/// Backend adapters use this only to build the same scalar coefficient table;
/// pixel accumulation remains in the selected backend.
pub(crate) fn filter_from_resample(filter: ResampleFilter) -> (fn(f64) -> f64, f64) {
    match filter {
        ResampleFilter::Nearest => (kernel_box, 0.5),
        ResampleFilter::Bilinear => (kernel_triangle, 1.0),
        ResampleFilter::Bicubic => (kernel_catrom, 2.0),
        ResampleFilter::Lanczos => (kernel_lanczos3, 3.0),
        ResampleFilter::Box => (kernel_box, 0.5),
        ResampleFilter::Hamming => (kernel_hamming, 1.0),
    }
}

// Pillow 12.2.0's arm64 FLOAT32 horizontal resampler uses scalar FMA for
// rows with at most 15 taps, then switches to complete 16-tap vector blocks
// that round each product before the ordered additions; any tail remains
// scalar FMA. Its vertical path remains scalar FMA.
// Fractional-box F resizes use the same native resampler contract.
const F_RESIZE_VECTOR_WIDTH: usize = 16;

fn f_resize_accumulate(
    accumulator: &mut f64,
    weight: f64,
    sample: f32,
    separate_product_add: bool,
) {
    let sample = f64::from(sample);
    if separate_product_add {
        // Keep the product out of the following add. LLVM may otherwise
        // contract this expression back into an FMA, defeating the arm64
        // wide-row contract that Pillow uses after 15 taps.
        let product = std::hint::black_box(weight * sample);
        *accumulator += product;
    } else {
        *accumulator = weight.mul_add(sample, *accumulator);
    }
}

// ── Pixel access helpers ──

/// Get pixel as 4 f64 values (r, g, b, a). Grayscale replicates to RGB.
#[inline]
fn typed_pixel_as_rgba8<P>(pixel: &P) -> [f64; 4]
where
    Rgba<u8>: FromColor<P>,
{
    let mut converted = Rgba([0; 4]);
    converted.copy_from_color(pixel);
    converted.0.map(f64::from)
}

/// Convert one typed sample using the same channel conversion as `to_rgba8`,
/// without materializing an RGBA copy of the complete image for each sample.
fn pixel_at(img: &DynamicImage, x: u32, y: u32) -> [f64; 4] {
    match img {
        DynamicImage::ImageLuma8(g) => {
            let v = g.get_pixel(x, y)[0] as f64;
            [v, v, v, 255.0]
        }
        DynamicImage::ImageLumaA8(ga) => {
            let p = ga.get_pixel(x, y);
            let v = p[0] as f64;
            [v, v, v, p[1] as f64]
        }
        DynamicImage::ImageRgb8(rgb) => {
            let p = rgb.get_pixel(x, y);
            [p[0] as f64, p[1] as f64, p[2] as f64, 255.0]
        }
        DynamicImage::ImageRgba8(rgba) => {
            let p = rgba.get_pixel(x, y);
            [p[0] as f64, p[1] as f64, p[2] as f64, p[3] as f64]
        }
        DynamicImage::ImageLuma16(image) => {
            // The image model deliberately clips I;16 samples instead of
            // applying the generic normalized u16-to-u8 conversion.
            let value = image.get_pixel(x, y)[0].min(u16::from(u8::MAX)) as u8;
            [f64::from(value), f64::from(value), f64::from(value), 255.0]
        }
        DynamicImage::ImageLumaA16(image) => typed_pixel_as_rgba8(image.get_pixel(x, y)),
        DynamicImage::ImageRgb16(image) => typed_pixel_as_rgba8(image.get_pixel(x, y)),
        DynamicImage::ImageRgba16(image) => typed_pixel_as_rgba8(image.get_pixel(x, y)),
        DynamicImage::ImageRgb32F(image) => typed_pixel_as_rgba8(image.get_pixel(x, y)),
        DynamicImage::ImageRgba32F(image) => typed_pixel_as_rgba8(image.get_pixel(x, y)),
    }
}

/// Materialize typed multi-channel samples in Pillow's public 8-bit layout.
///
/// Pillow's RGB/LA/RGBA image core is byte-backed even when the source codec
/// decodes 16-bit or float samples. Convert into the matching logical layout
/// before byte resampling; treating the sample bytes as four unrelated
/// channels misaligns RGB and filters numeric sample encodings for every mode.
/// Keep RGB at three bytes and LA at two rather than widening both to RGBA.
fn typed_color_resize_bytes(img: &DynamicImage) -> Option<DynamicImage> {
    match img {
        DynamicImage::ImageLumaA16(_) => Some(DynamicImage::ImageLumaA8(img.to_luma_alpha8())),
        DynamicImage::ImageRgb16(_) | DynamicImage::ImageRgb32F(_) => {
            Some(DynamicImage::ImageRgb8(img.to_rgb8()))
        }
        DynamicImage::ImageRgba16(_) | DynamicImage::ImageRgba32F(_) => {
            Some(DynamicImage::ImageRgba8(img.to_rgba8()))
        }
        _ => None,
    }
}

/// Resize a native 16-bit grayscale image through PIL's nearest-neighbor path.
///
/// `I;16*` images carry unsigned 16-bit samples.  Keeping them in the native
/// buffer is required because converting through `to_rgba8()` changes both the
/// sample width and the bytes returned by `tobytes()`.
fn pil_resize_luma16_nearest(
    img: &ImageBuffer<Luma<u16>, Vec<u16>>,
    dst_w: u32,
    dst_h: u32,
    bounds: Option<(f64, f64, f64, f64)>,
) -> DynamicImage {
    let sw = img.width();
    let sh = img.height();
    let mut result = ImageBuffer::new(dst_w, dst_h);
    if sw == 0 || sh == 0 {
        return DynamicImage::ImageLuma16(result);
    }
    let (left, top, scale_x, scale_y) = match bounds {
        Some((left, top, right, bottom)) => (
            f64::from(left as f32),
            f64::from(top as f32),
            f64::from(right as f32 - left as f32) / f64::from(dst_w),
            f64::from(bottom as f32 - top as f32) / f64::from(dst_h),
        ),
        None => (0.0, 0.0, sw as f64 / dst_w as f64, sh as f64 / dst_h as f64),
    };

    let mut xintab = Vec::with_capacity(dst_w as usize);
    let mut xo = left + scale_x * 0.5;
    for _ in 0..dst_w {
        let xi = xo as u32;
        xintab.push(if xi >= sw { sw - 1 } else { xi });
        xo += scale_x;
    }

    let mut yo = top + scale_y * 0.5;
    for dy in 0..dst_h {
        let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 };
        for dx in 0..dst_w {
            let sx = xintab[dx as usize];
            result.put_pixel(dx, dy, *img.get_pixel(sx, sy));
        }
        yo += scale_y;
    }

    DynamicImage::ImageLuma16(result)
}

/// Pillow's `Resample.c` uses a separate byte-oriented implementation for
/// `I;16*` images.  The source and destination images are native `u16`
/// buffers, but the C implementation reads and writes their two bytes using
/// the mode-dependent `bigendian` flag.  Keep that ABI-visible behavior at the
/// core boundary instead of feeding a 16-bit buffer into the u8 resampler.
pub(crate) fn luma16_resample_big_endian(mode: Option<&str>) -> bool {
    match mode {
        // Pillow's Resample.c deliberately selects the big-endian branch for
        // I;16N.  On little-endian hosts this is observable as the historical
        // native-mode byte ordering of the convolution path.
        Some("I;16N") => true,
        Some("I;16B") => true,
        _ => false,
    }
}

pub(crate) fn luma16_resample_read(sample: u16, big_endian: bool) -> u16 {
    let native_bytes = sample.to_ne_bytes();
    if big_endian {
        u16::from_be_bytes(native_bytes)
    } else {
        u16::from_le_bytes(native_bytes)
    }
}

fn clip_u8(value: i64) -> u8 {
    value.clamp(0, 255) as u8
}

pub(crate) fn luma16_resample_write(value: f64, big_endian: bool) -> u16 {
    let rounded = round_up(value) as i64;
    // Resample.c writes each byte through CLIP8 rather than clipping the
    // complete 16-bit result.  Keeping the byte-level operation matters for
    // negative-filter overshoot and for the exact I;16* output bytes.
    let low = clip_u8(rounded % 256);
    let high = clip_u8(rounded >> 8);
    let bytes = if big_endian { [high, low] } else { [low, high] };
    u16::from_ne_bytes(bytes)
}

fn horizontal_pass_luma16(
    src_row: &[u16],
    coeffs: &FilterCoeffsF64,
    out_w: u32,
    big_endian: bool,
    intermediate_row: &mut [u16],
) {
    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let mut acc = 0.0f64;
        for (cix, &weight) in coeffs.weights[ox].iter().enumerate() {
            let sx = (x0 + cix as i64) as usize;
            acc += luma16_resample_read(src_row[sx], big_endian) as f64 * weight;
        }
        intermediate_row[ox] = luma16_resample_write(acc, big_endian);
    }
}

fn vertical_pass_luma16(
    intermediate: &[u16],
    out_x: u32,
    out_w: u32,
    coeffs: &FilterCoeffsF64,
    out_y: usize,
    big_endian: bool,
) -> u16 {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return 0;
    }
    let mut acc = 0.0f64;
    for (cix, &weight) in coeffs.weights[out_y].iter().enumerate() {
        let sy = (y0 + cix as i64) as usize;
        let source = intermediate[sy * out_w as usize + out_x as usize];
        acc += luma16_resample_read(source, big_endian) as f64 * weight;
    }
    luma16_resample_write(acc, big_endian)
}

/// Resize `I;16*` through Pillow's native 16-bit two-pass resampler.
fn pil_resize_luma16(
    img: &ImageBuffer<Luma<u16>, Vec<u16>>,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
    bounds: Option<(f64, f64, f64, f64)>,
) -> DynamicImage {
    if matches!(filter, ResampleFilter::Nearest) {
        return pil_resize_luma16_nearest(img, dst_w, dst_h, bounds);
    }

    let (kernel, support) = filter_from_resample(filter);
    let (h_coeffs, v_coeffs) = match bounds {
        Some((left, top, right, bottom)) => (
            Arc::new(precompute_coeffs_f64_boxed(
                dst_w,
                img.width(),
                left,
                right,
                filter,
            )),
            Arc::new(precompute_coeffs_f64_boxed(
                dst_h,
                img.height(),
                top,
                bottom,
                filter,
            )),
        ),
        None => (
            precompute_coeffs_f64(dst_w, img.width(), kernel, support),
            precompute_coeffs_f64(dst_h, img.height(), kernel, support),
        ),
    };
    let big_endian = luma16_resample_big_endian(explicit_mode);

    let mut intermediate = ImageBuffer::<Luma<u16>, Vec<u16>>::new(img.height(), dst_w);
    {
        let intermediate_data: &mut [u16] = &mut *intermediate;
        for sy in 0..img.height() {
            let src_start = (sy * img.width()) as usize;
            let src_end = src_start + img.width() as usize;
            let intermediate_start = (sy * dst_w) as usize;
            let intermediate_end = intermediate_start + dst_w as usize;
            horizontal_pass_luma16(
                &img.as_raw()[src_start..src_end],
                &h_coeffs,
                dst_w,
                big_endian,
                &mut intermediate_data[intermediate_start..intermediate_end],
            );
        }
    }

    let mut output = ImageBuffer::<Luma<u16>, Vec<u16>>::new(dst_w, dst_h);
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let value = vertical_pass_luma16(
                intermediate.as_raw(),
                dx,
                dst_w,
                &v_coeffs,
                dy as usize,
                big_endian,
            );
            output.put_pixel(dx, dy, Luma([value]));
        }
    }

    DynamicImage::ImageLuma16(output)
}

/// PIL uses 22-bit fixed-point arithmetic (PRECISION_BITS=22) for weights
/// and intermediate accumulation. We match this exactly.
const PRECISION_BITS: u32 = 22;
const PRECISION: i64 = 1i64 << PRECISION_BITS; // 2^22
const HALF_PRECISION: i64 = 1i64 << (PRECISION_BITS - 1); // 2^21
const BOXED_ZERO_FAST_PATH_MAX_PIXELS: usize = 256 * 256;

/// Round a float to u8, matching PIL's fixed-point rounding:
///   `(int)(v + 0.5)` clamped to [0, 255]
fn pil_round(v: f64) -> u8 {
    let v = v + 0.5;
    if v <= 0.0 {
        0
    } else if v >= 256.0 {
        255
    } else {
        v as u8
    }
}

/// Convert a fixed-point sum to u8, matching PIL's:
///   `(UINT8)((sum + (1 << (PRECISION_BITS - 1))) >> PRECISION_BITS)`
fn fixed_point_to_u8(sum: i64) -> u8 {
    let v = (sum + HALF_PRECISION) >> PRECISION_BITS;
    if v <= 0 {
        0
    } else if v >= 255 {
        255
    } else {
        v as u8
    }
}

#[cfg(not(feature = "parallel"))]
#[inline]
fn fixed_point_to_u8_i32(sum: i32) -> u8 {
    let value = (sum + (1_i32 << (PRECISION_BITS - 1))) >> PRECISION_BITS;
    value.clamp(0, 255) as u8
}

// ── PIL-compatible pixel range and weight computation ──

/// Precompute filter coefficients for one dimension, using PIL's exact
/// fixed-point arithmetic (PRECISION_BITS=22).
///
/// For each output pixel, computes:
/// - xmin: first contributing source pixel index
/// - count: number of contributing source pixels
/// - weights: normalized filter weights in 22-bit fixed-point
///
/// This matches PIL's `precompute_coeffs`:
///   center = (xx + 0.5) * scale
///   xmin = (int)(center - support + 0.5)
///   xmax = (int)(center + support + 0.5)
///   weight = kernel((sx + 0.5 - center) * ss)  where ss = 1.0 / filterscale
pub(crate) struct FilterCoeffs {
    pub(crate) xmin: Vec<i64>,
    pub(crate) count: Vec<usize>,
    pub(crate) offsets: Vec<usize>,
    pub(crate) weights: Vec<i64>, // 22-bit fixed-point weights, flattened by output pixel
}

impl FilterCoeffs {
    #[inline]
    fn weights_for(&self, index: usize) -> &[i64] {
        let start = self.offsets[index];
        &self.weights[start..start + self.count[index]]
    }
}

/// Prove every ordered byte-sample partial sum, including Pillow's rounding
/// bias, fits in i32 before selecting the narrow HSV resize kernel.
#[cfg(not(feature = "parallel"))]
fn resize_u8_coefficients_fit_i32(coeffs: &FilterCoeffs) -> bool {
    if coeffs.count.len() != coeffs.offsets.len() {
        return false;
    }
    for index in 0..coeffs.count.len() {
        let Some(weights) = coeffs
            .offsets
            .get(index)
            .and_then(|&start| {
                start
                    .checked_add(*coeffs.count.get(index)?)
                    .map(|end| (start, end))
            })
            .and_then(|(start, end)| coeffs.weights.get(start..end))
        else {
            return false;
        };
        let Some(sum_abs) = weights.iter().try_fold(0u64, |sum, &weight| {
            i32::try_from(weight).ok()?;
            sum.checked_add(weight.unsigned_abs())
        }) else {
            return false;
        };
        let Some(bound) = sum_abs
            .checked_mul(255)
            .and_then(|bound| bound.checked_add(1 << (PRECISION_BITS - 1)))
        else {
            return false;
        };
        if bound > i32::MAX as u64 {
            return false;
        }
    }
    true
}

pub(crate) struct FilterCoeffsF64 {
    pub(crate) xmin: Vec<i64>,
    pub(crate) count: Vec<usize>,
    pub(crate) weights: Vec<Vec<f64>>, // double-precision weights (for I/F modes)
}

struct FilterCoeffsF64Entry {
    key: FilterCoeffsF64Key,
    coeffs: Arc<FilterCoeffsF64>,
    bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FilterCoeffsF64Key {
    input_size: u32,
    output_size: u32,
    kernel: usize,
    support_bits: u64,
}

struct FilterCoeffsF64Cache {
    entries: VecDeque<FilterCoeffsF64Entry>,
    retained_bytes: usize,
}

static FILTER_COEFF_F64_CACHE: OnceLock<Mutex<FilterCoeffsF64Cache>> = OnceLock::new();

fn filter_coeff_f64_cache() -> &'static Mutex<FilterCoeffsF64Cache> {
    FILTER_COEFF_F64_CACHE.get_or_init(|| {
        Mutex::new(FilterCoeffsF64Cache {
            entries: VecDeque::new(),
            retained_bytes: 0,
        })
    })
}

/// Stable cache identity for the ordinary (unboxed) resize geometry.
///
/// Boxed resizes carry floating-point crop coordinates and keep their separate
/// exact path.  The common `Image.resize` path has only integer dimensions and
/// a finite filter enum, so this key is complete without hashing function
/// pointers or floating-point values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FilterCoeffsKey {
    input_size: u32,
    output_size: u32,
    filter: u8,
}

struct FilterCoeffsEntry {
    key: FilterCoeffsKey,
    coeffs: Arc<FilterCoeffs>,
    bytes: usize,
}

struct FilterCoeffsCache {
    entries: VecDeque<FilterCoeffsEntry>,
    retained_bytes: usize,
}

const FILTER_COEFF_CACHE_CAPACITY: usize = 16;
const FILTER_COEFF_CACHE_BYTES: usize = 8 * 1024 * 1024;

static FILTER_COEFF_CACHE: OnceLock<Mutex<FilterCoeffsCache>> = OnceLock::new();

fn filter_coeff_cache() -> &'static Mutex<FilterCoeffsCache> {
    FILTER_COEFF_CACHE.get_or_init(|| {
        Mutex::new(FilterCoeffsCache {
            entries: VecDeque::new(),
            retained_bytes: 0,
        })
    })
}

fn filter_cache_id(filter: ResampleFilter) -> u8 {
    match filter {
        ResampleFilter::Nearest => 0,
        ResampleFilter::Bilinear => 1,
        ResampleFilter::Bicubic => 2,
        ResampleFilter::Lanczos => 3,
        ResampleFilter::Box => 4,
        ResampleFilter::Hamming => 5,
    }
}

fn filter_coeff_bytes(coeffs: &FilterCoeffs) -> usize {
    coeffs
        .xmin
        .len()
        .saturating_mul(std::mem::size_of::<i64>())
        .saturating_add(
            coeffs
                .count
                .len()
                .saturating_mul(std::mem::size_of::<usize>()),
        )
        .saturating_add(
            coeffs
                .offsets
                .len()
                .saturating_mul(std::mem::size_of::<usize>()),
        )
        .saturating_add(
            coeffs
                .weights
                .len()
                .saturating_mul(std::mem::size_of::<i64>()),
        )
}

fn filter_coeff_f64_bytes(coeffs: &FilterCoeffsF64) -> usize {
    coeffs
        .xmin
        .len()
        .saturating_mul(std::mem::size_of::<i64>())
        .saturating_add(
            coeffs
                .count
                .len()
                .saturating_mul(std::mem::size_of::<usize>()),
        )
        .saturating_add(
            coeffs
                .weights
                .iter()
                .map(|weights| weights.len().saturating_mul(std::mem::size_of::<f64>()))
                .sum::<usize>(),
        )
}

fn cache_filter_coeffs(key: FilterCoeffsKey, coeffs: Arc<FilterCoeffs>) -> Arc<FilterCoeffs> {
    let bytes = filter_coeff_bytes(&coeffs);
    if bytes > FILTER_COEFF_CACHE_BYTES {
        return coeffs;
    }

    let Ok(mut cache) = filter_coeff_cache().lock() else {
        return coeffs;
    };
    while cache.entries.len() >= FILTER_COEFF_CACHE_CAPACITY
        || cache.retained_bytes.saturating_add(bytes) > FILTER_COEFF_CACHE_BYTES
    {
        let Some(entry) = cache.entries.pop_back() else {
            break;
        };
        cache.retained_bytes = cache.retained_bytes.saturating_sub(entry.bytes);
    }
    cache.retained_bytes = cache.retained_bytes.saturating_add(bytes);
    cache.entries.push_front(FilterCoeffsEntry {
        key,
        coeffs: Arc::clone(&coeffs),
        bytes,
    });
    coeffs
}

fn cached_filter_coeffs(
    input_size: u32,
    output_size: u32,
    filter: ResampleFilter,
) -> Arc<FilterCoeffs> {
    let key = FilterCoeffsKey {
        input_size,
        output_size,
        filter: filter_cache_id(filter),
    };
    if let Ok(mut cache) = filter_coeff_cache().lock() {
        if let Some(index) = cache.entries.iter().position(|entry| entry.key == key) {
            let entry = cache
                .entries
                .remove(index)
                .expect("resize coefficient cache entry disappeared");
            let coeffs = Arc::clone(&entry.coeffs);
            cache.entries.push_front(entry);
            crate::compute::record_pipeline_resize_coeff_cache_hit();
            return coeffs;
        }
    }

    crate::compute::record_pipeline_resize_coeff_cache_miss();
    let (kernel, support) = filter_from_resample(filter);
    let coeffs = Arc::new(_precompute_coeffs_impl(
        output_size,
        input_size,
        input_size as f64 / output_size as f64,
        kernel,
        support,
    ));
    cache_filter_coeffs(key, coeffs)
}

fn cache_filter_coeffs_f64(
    key: FilterCoeffsF64Key,
    coeffs: Arc<FilterCoeffsF64>,
) -> Arc<FilterCoeffsF64> {
    let bytes = filter_coeff_f64_bytes(&coeffs);
    if bytes > FILTER_COEFF_CACHE_BYTES {
        return coeffs;
    }

    let Ok(mut cache) = filter_coeff_f64_cache().lock() else {
        return coeffs;
    };
    while cache.entries.len() >= FILTER_COEFF_CACHE_CAPACITY
        || cache.retained_bytes.saturating_add(bytes) > FILTER_COEFF_CACHE_BYTES
    {
        let Some(entry) = cache.entries.pop_back() else {
            break;
        };
        cache.retained_bytes = cache.retained_bytes.saturating_sub(entry.bytes);
    }
    cache.retained_bytes = cache.retained_bytes.saturating_add(bytes);
    cache.entries.push_front(FilterCoeffsF64Entry {
        key,
        coeffs: Arc::clone(&coeffs),
        bytes,
    });
    coeffs
}

fn cached_filter_coeffs_f64(
    input_size: u32,
    output_size: u32,
    kernel: fn(f64) -> f64,
    support: f64,
) -> Arc<FilterCoeffsF64> {
    let key = FilterCoeffsF64Key {
        input_size,
        output_size,
        kernel: kernel as usize,
        support_bits: support.to_bits(),
    };
    if let Ok(mut cache) = filter_coeff_f64_cache().lock() {
        if let Some(index) = cache.entries.iter().position(|entry| entry.key == key) {
            let entry = cache
                .entries
                .remove(index)
                .expect("f64 resize coefficient cache entry disappeared");
            let coeffs = Arc::clone(&entry.coeffs);
            cache.entries.push_front(entry);
            crate::compute::record_pipeline_resize_coeff_cache_hit();
            return coeffs;
        }
    }

    crate::compute::record_pipeline_resize_coeff_cache_miss();
    let coeffs = Arc::new(_precompute_coeffs_f64_impl(
        output_size,
        input_size,
        kernel,
        support,
    ));
    cache_filter_coeffs_f64(key, coeffs)
}

/// PIL's ROUND_UP: (int)((f) >= 0.0 ? (f) + 0.5 : (f) - 0.5)
pub(crate) fn round_up(f: f64) -> f64 {
    if f >= 0.0 {
        (f + 0.5).trunc()
    } else {
        (f - 0.5).trunc()
    }
}

/// Precompute f64 (double-precision) coefficients matching PIL's 32-bit image resample.
fn _precompute_coeffs_f64_impl(
    out_size: u32,
    in_size: u32,
    kernel: fn(f64) -> f64,
    support: f64,
) -> FilterCoeffsF64 {
    let scale = in_size as f64 / out_size as f64;
    let filterscale = scale.max(1.0);
    let ss = 1.0 / filterscale;
    let src_support = support * filterscale;
    let n = out_size as usize;
    let mut xmin = Vec::with_capacity(n);
    let mut count = Vec::with_capacity(n);
    let mut weights: Vec<Vec<f64>> = Vec::with_capacity(n);
    for ox in 0..n {
        let center = (ox as f64 + 0.5) * scale;
        let mut x0 = (center - src_support + 0.5).trunc() as i64;
        let mut x1 = (center + src_support + 0.5).trunc() as i64;
        if x0 < 0 {
            x0 = 0;
        }
        if x1 > in_size as i64 {
            x1 = in_size as i64;
        }
        let cnt = (x1 - x0) as usize;
        xmin.push(x0);
        count.push(cnt);
        if cnt == 0 {
            weights.push(Vec::new());
            continue;
        }
        let mut w: Vec<f64> = Vec::with_capacity(cnt);
        let mut wsum = 0.0;
        for ix in 0..cnt {
            let sx = x0 + ix as i64;
            // Resample.c evaluates `(x + xmin - center + 0.5)` with the
            // source index addition still integral.  Subtracting the center
            // before adding the half-pixel preserves its f64 rounding.
            let val = kernel((sx as f64 - center + 0.5) * ss);
            w.push(val);
            wsum += val;
        }
        if wsum != 0.0 {
            for val in &mut w {
                *val /= wsum;
            }
        }
        weights.push(w);
    }
    FilterCoeffsF64 {
        xmin,
        count,
        weights,
    }
}

pub(crate) fn precompute_coeffs_f64(
    out_size: u32,
    in_size: u32,
    kernel: fn(f64) -> f64,
    support: f64,
) -> Arc<FilterCoeffsF64> {
    cached_filter_coeffs_f64(in_size, out_size, kernel, support)
}

/// Precompute double-precision coefficients for a resize with a fractional
/// source box. Pillow receives these boundaries as `float` before computing
/// the f64 kernel centers, just as it does for the byte boxed-resample path.
pub(crate) fn precompute_coeffs_f64_boxed(
    out_size: u32,
    in_size: u32,
    box_start: f64,
    box_end: f64,
    filter: ResampleFilter,
) -> FilterCoeffsF64 {
    let (kernel, support) = filter_from_resample(filter);
    let box_start = box_start as f32 as f64;
    let box_end = box_end as f32 as f64;
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(out_size);
    let filterscale = scale.max(1.0);
    let src_support = support * filterscale;
    let source_size = i64::from(in_size);
    let mut xmin = Vec::with_capacity(out_size as usize);
    let mut count = Vec::with_capacity(out_size as usize);
    let mut weights = Vec::with_capacity(out_size as usize);
    for output in 0..out_size as usize {
        let center = box_start + (output as f64 + 0.5) * scale;
        let mut x0 = (center - src_support + 0.5).trunc() as i64;
        let mut x1 = (center + src_support + 0.5).trunc() as i64;
        x0 = x0.max(0);
        x1 = x1.min(source_size);
        let sample_count = (x1 - x0).max(0) as usize;
        xmin.push(x0);
        count.push(sample_count);
        let mut row_weights = Vec::with_capacity(sample_count);
        let mut sum = 0.0;
        let ss = 1.0 / filterscale;
        for tap in 0..sample_count {
            let value = kernel((x0 as f64 + tap as f64 - center + 0.5) * ss);
            row_weights.push(value);
            sum += value;
        }
        if sum != 0.0 {
            for value in &mut row_weights {
                *value /= sum;
            }
        }
        weights.push(row_weights);
    }
    FilterCoeffsF64 {
        xmin,
        count,
        weights,
    }
}

pub(crate) fn precompute_coeffs(
    out_size: u32,
    in_size: u32,
    filter: ResampleFilter,
) -> Arc<FilterCoeffs> {
    cached_filter_coeffs(in_size, out_size, filter)
}

/// Precompute coefficients for a box-based resize (PIL's box parameter).
/// The box is a (left, top, right, bottom) tuple in source coordinates,
/// mapping to the output size. All coordinates are in the source image
/// coordinate system (floating point).
pub(crate) fn precompute_coeffs_boxed(
    out_size: u32,
    in_size: u32,
    box_start: f64,
    box_end: f64,
    kernel: fn(f64) -> f64,
    support: f64,
) -> FilterCoeffs {
    // Pillow's ImagingResample ABI receives the box coordinates as `float`
    // before `Resample.c::precompute_coeffs` computes its double-precision
    // centers. Preserve that first-divergence conversion: keeping the
    // Python-provided f64 values changes fixed-point weights at boundaries.
    let box_start_f32 = box_start as f32;
    let box_end_f32 = box_end as f32;
    let box_start = box_start_f32 as f64;
    // Scale = (float)(box_end - box_start) / output_size, as in Pillow.
    let box_length = (box_end_f32 - box_start_f32) as f64;
    let scale = box_length / out_size as f64;
    let filterscale = scale.max(1.0);
    let src_support = support * filterscale;

    let n = out_size as usize;
    let mut xmin = Vec::with_capacity(n);
    let mut count = Vec::with_capacity(n);
    let mut offsets = Vec::with_capacity(n);
    let mut weights: Vec<i64> = Vec::with_capacity(n * support.ceil() as usize);

    for ox in 0..n {
        // PIL: center = box_start + (ox + 0.5) * scale
        let center = box_start + (ox as f64 + 0.5) * scale;

        let mut x0 = (center - src_support + 0.5).trunc() as i64;
        let mut x1 = (center + src_support + 0.5).trunc() as i64;

        // Clamp to image bounds
        if x0 < 0 {
            x0 = 0;
        }
        if x1 > in_size as i64 {
            x1 = in_size as i64;
        }

        let cnt = (x1 - x0) as usize;
        xmin.push(x0);
        count.push(cnt);
        offsets.push(weights.len());

        if cnt == 0 {
            continue;
        }

        // Compute f64 weights, then convert to fixed-point
        let ss = 1.0 / filterscale;
        let mut w_f64 = Vec::with_capacity(cnt);
        let mut wsum = 0.0;
        for ix in 0..cnt {
            let sx = x0 + ix as i64;
            let val = kernel((sx as f64 + 0.5 - center) * ss);
            w_f64.push(val);
            wsum += val;
        }

        if wsum != 0.0 {
            for wi in w_f64.iter_mut() {
                *wi /= wsum;
            }
        }

        weights.extend(w_f64.iter().map(|&w| {
            let scaled = w * PRECISION as f64;
            let rounded = if w >= 0.0 { scaled + 0.5 } else { scaled - 0.5 };
            rounded as i64
        }));
    }

    FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    }
}

/// Precompute box-resize coefficients for a public resampling filter.
///
/// The SIMD adapter uses the same Pillow-compatible coefficient builder as
/// the scalar resampler; keeping kernel selection here prevents a second
/// boxed-filter implementation from drifting at fixed-point boundaries.
pub(crate) fn precompute_coeffs_boxed_for_filter(
    out_size: u32,
    in_size: u32,
    box_start: f64,
    box_end: f64,
    filter: ResampleFilter,
) -> FilterCoeffs {
    let (kernel, support) = filter_from_resample(filter);
    precompute_coeffs_boxed(out_size, in_size, box_start, box_end, kernel, support)
}

/// Rebase coefficient source rows to the smallest span that can contribute.
///
/// Boxed resizes often read only a fraction of the source's vertical extent.
/// The horizontal pass is independent for each source row, so rows outside
/// the vertical coefficient ranges can be omitted exactly. The returned
/// `(first_row, row_count)` addresses the original source; `xmin` values are
/// shifted to address an intermediate containing only that range. Invalid or
/// empty tables are left untouched and return `None`.
pub(crate) fn compact_resize_coeffs_to_source_span(
    coeffs: &mut FilterCoeffs,
    source_rows: u32,
) -> Option<(u32, u32)> {
    if coeffs.xmin.len() != coeffs.count.len() {
        return None;
    }
    let mut first = i64::MAX;
    let mut end = i64::MIN;
    for (&xmin, &count) in coeffs.xmin.iter().zip(&coeffs.count) {
        if count == 0 {
            continue;
        }
        let row_end = xmin.checked_add(i64::try_from(count).ok()?)?;
        if xmin < 0 || row_end > i64::from(source_rows) {
            return None;
        }
        first = first.min(xmin);
        end = end.max(row_end);
    }
    if first == i64::MAX || end <= first {
        return None;
    }
    for (xmin, &count) in coeffs.xmin.iter_mut().zip(&coeffs.count) {
        if count > 0 {
            *xmin -= first;
        }
    }
    Some((u32::try_from(first).ok()?, u32::try_from(end - first).ok()?))
}

/// Internal implementation with explicit scale, called by pil_resize (double scale).
fn _precompute_coeffs_impl(
    out_size: u32,
    in_size: u32,
    scale: f64,
    kernel: fn(f64) -> f64,
    support: f64,
) -> FilterCoeffs {
    let filterscale = scale.max(1.0);
    let ss = 1.0 / filterscale;
    let src_support = support * filterscale;

    let n = out_size as usize;
    let mut xmin = Vec::with_capacity(n);
    let mut count = Vec::with_capacity(n);
    let mut offsets = Vec::with_capacity(n);
    let mut weights: Vec<i64> = Vec::with_capacity(n * support.ceil() as usize);

    for ox in 0..n {
        // PIL center = (ox + 0.5) * scale
        let center = (ox as f64 + 0.5) * scale;

        // PIL: xmin = (int)(center - support + 0.5), xmax = (int)(center + support + 0.5)
        let mut x0 = (center - src_support + 0.5).trunc() as i64;
        let mut x1 = (center + src_support + 0.5).trunc() as i64;

        // Clamp to image bounds
        if x0 < 0 {
            x0 = 0;
        }
        if x1 > in_size as i64 {
            x1 = in_size as i64;
        }

        let cnt = (x1 - x0) as usize;
        xmin.push(x0);
        count.push(cnt);
        offsets.push(weights.len());

        if cnt == 0 {
            continue;
        }

        // Compute f64 weights, then convert to fixed-point
        let mut w_f64 = Vec::with_capacity(cnt);
        let mut wsum = 0.0;
        for ix in 0..cnt {
            let sx = x0 + ix as i64;
            // PIL: kernel((sx + 0.5 - center) * ss)
            let val = kernel((sx as f64 + 0.5 - center) * ss);
            w_f64.push(val);
            wsum += val;
        }

        // Normalize weights. PIL's C code divides each weight by the sum
        // in-place on the double buffer: kk[offset + i] /= wsum.
        // This is subtly different from multiplication by 1/wsum due to
        // floating-point rounding (one ULP difference).
        if wsum != 0.0 {
            for wi in w_f64.iter_mut() {
                *wi /= wsum;
            }
        }

        // Convert to fixed-point: (int)(weight * (1 << PRECISION_BITS) + (weight >= 0 ? 0.5 : -0.5))
        // PIL uses different rounding for positive and negative weights.
        let w_fixed = w_f64.iter().map(|&w| {
            let scaled = w * PRECISION as f64;
            let rounded = if w >= 0.0 { scaled + 0.5 } else { scaled - 0.5 };
            rounded as i64
        });

        // NOTE: PIL does NOT adjust the fixed-point weights to sum exactly to
        // PRECISION. The normalizes weights are converted to fixed-point with
        // rounding (+0.5 for positive, -0.5 for negative) and used as-is.
        // Any small discrepancy from the ideal sum is absorbed by the
        // HALF_PRECISION bias added during accumulation.
        weights.extend(w_fixed);
    }

    FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    }
}

// ── Alpha premultiplication ──

pub(crate) fn premultiply_alpha(img: &DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgba8(rgba) => {
            let mut out = rgba.clone();
            for p in out.pixels_mut() {
                let a = p[3] as f64 / 255.0;
                p[0] = (p[0] as f64 * a + 0.5) as u8;
                p[1] = (p[1] as f64 * a + 0.5) as u8;
                p[2] = (p[2] as f64 * a + 0.5) as u8;
            }
            DynamicImage::ImageRgba8(out)
        }
        DynamicImage::ImageLumaA8(la) => {
            let mut out = la.clone();
            for p in out.pixels_mut() {
                let a = p[1] as f64 / 255.0;
                p[0] = (p[0] as f64 * a + 0.5) as u8;
            }
            DynamicImage::ImageLumaA8(out)
        }
        _ => img.clone(),
    }
}

pub(crate) fn unpremultiply_alpha(img: &DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgba8(rgba) => {
            let mut out = rgba.clone();
            for p in out.pixels_mut() {
                let alpha = u32::from(p[3]);
                if alpha > 0 {
                    // Pillow's RGBa -> RGBA conversion uses integer
                    // `(255 * value) / alpha` truncation. Keep the multiply
                    // before division: using `255.0 / alpha` and a floating
                    // multiply can land one ULP below an exact integer.
                    p[0] = (u32::from(p[0]) * 255 / alpha).min(255) as u8;
                    p[1] = (u32::from(p[1]) * 255 / alpha).min(255) as u8;
                    p[2] = (u32::from(p[2]) * 255 / alpha).min(255) as u8;
                }
            }
            DynamicImage::ImageRgba8(out)
        }
        DynamicImage::ImageLumaA8(la) => {
            let mut out = la.clone();
            for p in out.pixels_mut() {
                let alpha = u32::from(p[1]);
                if alpha > 0 {
                    // Match Pillow's La -> LA integer truncation and avoid
                    // reciprocal floating-point rounding at exact results.
                    p[0] = (u32::from(p[0]) * 255 / alpha).min(255) as u8;
                }
            }
            DynamicImage::ImageLumaA8(out)
        }
        _ => img.clone(),
    }
}

/// Resample one row of pixels into the output columns.
///
/// This is the horizontal pass: it computes one row of the intermediate image
/// from one row of the source image. Uses PIL's fixed-point arithmetic:
///   `result = (sum + (1 << (PRECISION_BITS - 1))) >> PRECISION_BITS`
fn horizontal_pass_row(
    src_row: &[u8],
    _src_w: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_w: u32,
    intermediate_row: &mut [u8],
) {
    // Native RGB has no alpha conversion or mode-dependent channel mapping.
    // Accumulate its interleaved channels together so each coefficient and
    // source pixel is visited once instead of repeating the tap walk 3 times.
    if channels == 3 {
        for ox in 0..out_w as usize {
            let x0 = coeffs.xmin[ox];
            let cnt = coeffs.count[ox];
            if cnt == 0 {
                continue;
            }
            let weights = coeffs.weights_for(ox);
            let (red_acc, green_acc, blue_acc) = if cnt == 4 {
                let source_index = x0 as usize * 3;
                let mut red_acc = i64::from(src_row[source_index]) * weights[0];
                let mut green_acc = i64::from(src_row[source_index + 1]) * weights[0];
                let mut blue_acc = i64::from(src_row[source_index + 2]) * weights[0];
                let source_index = source_index + 3;
                red_acc += i64::from(src_row[source_index]) * weights[1];
                green_acc += i64::from(src_row[source_index + 1]) * weights[1];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[1];
                let source_index = source_index + 3;
                red_acc += i64::from(src_row[source_index]) * weights[2];
                green_acc += i64::from(src_row[source_index + 1]) * weights[2];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[2];
                let source_index = source_index + 3;
                red_acc += i64::from(src_row[source_index]) * weights[3];
                green_acc += i64::from(src_row[source_index + 1]) * weights[3];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[3];
                (red_acc, green_acc, blue_acc)
            } else if cnt == 8 {
                let weights: &[i64; 8] = weights
                    .try_into()
                    .expect("an eight-tap coefficient span contains eight weights");
                let mut source_index = x0 as usize * 3;
                let mut red_acc = i64::from(src_row[source_index]) * weights[0];
                let mut green_acc = i64::from(src_row[source_index + 1]) * weights[0];
                let mut blue_acc = i64::from(src_row[source_index + 2]) * weights[0];
                source_index += 3;
                red_acc += i64::from(src_row[source_index]) * weights[1];
                green_acc += i64::from(src_row[source_index + 1]) * weights[1];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[1];
                source_index += 3;
                red_acc += i64::from(src_row[source_index]) * weights[2];
                green_acc += i64::from(src_row[source_index + 1]) * weights[2];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[2];
                source_index += 3;
                red_acc += i64::from(src_row[source_index]) * weights[3];
                green_acc += i64::from(src_row[source_index + 1]) * weights[3];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[3];
                source_index += 3;
                red_acc += i64::from(src_row[source_index]) * weights[4];
                green_acc += i64::from(src_row[source_index + 1]) * weights[4];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[4];
                source_index += 3;
                red_acc += i64::from(src_row[source_index]) * weights[5];
                green_acc += i64::from(src_row[source_index + 1]) * weights[5];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[5];
                source_index += 3;
                red_acc += i64::from(src_row[source_index]) * weights[6];
                green_acc += i64::from(src_row[source_index + 1]) * weights[6];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[6];
                source_index += 3;
                red_acc += i64::from(src_row[source_index]) * weights[7];
                green_acc += i64::from(src_row[source_index + 1]) * weights[7];
                blue_acc += i64::from(src_row[source_index + 2]) * weights[7];
                (red_acc, green_acc, blue_acc)
            } else {
                let mut red_acc = 0i64;
                let mut green_acc = 0i64;
                let mut blue_acc = 0i64;
                for (cix, &weight) in weights.iter().enumerate() {
                    let source_index = (x0 + cix as i64) as usize * 3;
                    red_acc += i64::from(src_row[source_index]) * weight;
                    green_acc += i64::from(src_row[source_index + 1]) * weight;
                    blue_acc += i64::from(src_row[source_index + 2]) * weight;
                }
                (red_acc, green_acc, blue_acc)
            };
            let destination_index = ox * 3;
            intermediate_row[destination_index] = fixed_point_to_u8(red_acc);
            intermediate_row[destination_index + 1] = fixed_point_to_u8(green_acc);
            intermediate_row[destination_index + 2] = fixed_point_to_u8(blue_acc);
        }
        return;
    }

    if channels == 4 {
        for ox in 0..out_w as usize {
            let x0 = coeffs.xmin[ox];
            let cnt = coeffs.count[ox];
            if cnt == 0 {
                continue;
            }
            let weights = coeffs.weights_for(ox);
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            let mut alpha_acc = 0i64;
            for (tap, &weight) in weights.iter().enumerate() {
                let source_start = (x0 + tap as i64) as usize * 4;
                red_acc += i64::from(src_row[source_start]) * weight;
                green_acc += i64::from(src_row[source_start + 1]) * weight;
                blue_acc += i64::from(src_row[source_start + 2]) * weight;
                alpha_acc += i64::from(src_row[source_start + 3]) * weight;
            }
            let destination_start = ox * 4;
            intermediate_row[destination_start] = fixed_point_to_u8(red_acc);
            intermediate_row[destination_start + 1] = fixed_point_to_u8(green_acc);
            intermediate_row[destination_start + 2] = fixed_point_to_u8(blue_acc);
            intermediate_row[destination_start + 3] = fixed_point_to_u8(alpha_acc);
        }
        return;
    }

    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let weights = coeffs.weights_for(ox);
        for c in 0..channels {
            let mut acc: i64 = 0;
            for (cix, &w) in weights.iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                acc += src_row[sx * channels + c] as i64 * w;
            }
            intermediate_row[ox * channels + c] = fixed_point_to_u8(acc);
        }
    }
}

#[inline]
fn premultiply_channel(value: u8, alpha: u8) -> u8 {
    ((value as u16 * alpha as u16 + 127) / 255) as u8
}

#[cfg(any(not(feature = "parallel"), test))]
fn premultiply_alpha_row(source: &[u8], channels: usize, output: &mut [u8]) {
    debug_assert!(matches!(channels, 2 | 4));
    debug_assert_eq!(source.len(), output.len());
    let alpha_channel = channels - 1;
    for (source_pixel, output_pixel) in source
        .chunks_exact(channels)
        .zip(output.chunks_exact_mut(channels))
    {
        let alpha = source_pixel[alpha_channel];
        for channel in 0..alpha_channel {
            output_pixel[channel] = premultiply_channel(source_pixel[channel], alpha);
        }
        output_pixel[alpha_channel] = alpha;
    }
}

/// Reference alpha-row kernel retained for the serial CPU path and focused
/// equivalence tests. Parallel CPU consumes each alpha tap directly per row.
fn horizontal_pass_row_alpha(
    src_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    out_w: u32,
    intermediate_row: &mut [u8],
) {
    debug_assert!(matches!(channels, 2 | 4));
    let alpha_channel = channels - 1;
    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let weights = coeffs.weights_for(ox);
        for c in 0..channels {
            let mut acc: i64 = 0;
            for (cix, &w) in weights.iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                let pixel_start = sx * channels;
                let source = src_row[pixel_start + c];
                let sample = if c == alpha_channel {
                    source
                } else {
                    premultiply_channel(source, src_row[pixel_start + alpha_channel])
                };
                acc += sample as i64 * w;
            }
            intermediate_row[ox * channels + c] = fixed_point_to_u8(acc);
        }
    }
}

/// Resample one column into the output rows.
///
/// This is the vertical pass: it computes one column of the final image
/// from one column of the intermediate image. Uses PIL's fixed-point arithmetic.
/// Returns a single value per channel for this (x, y) position.
fn vertical_pass_col(
    intermediate: &[u8],
    _src_rows: u32,
    out_x: u32,
    out_w: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_y: usize,
) -> [u8; 4] {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return [0u8; 4];
    }
    let weights = coeffs.weights_for(out_y);
    let mut result = [0u8; 4];
    if channels == 3 {
        let x_index = out_x as usize * 3;
        let row_stride = out_w as usize * 3;
        let (red_acc, green_acc, blue_acc) = if cnt == 4 {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            let mut source_index = y0 as usize * row_stride + x_index;
            red_acc += i64::from(intermediate[source_index]) * weights[0];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[0];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[0];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[1];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[1];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[1];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[2];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[2];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[2];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[3];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[3];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[3];
            (red_acc, green_acc, blue_acc)
        } else if cnt == 8 {
            let weights: &[i64; 8] = weights
                .try_into()
                .expect("an eight-tap coefficient span contains eight weights");
            let mut source_index = y0 as usize * row_stride + x_index;
            let mut red_acc = i64::from(intermediate[source_index]) * weights[0];
            let mut green_acc = i64::from(intermediate[source_index + 1]) * weights[0];
            let mut blue_acc = i64::from(intermediate[source_index + 2]) * weights[0];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[1];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[1];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[1];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[2];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[2];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[2];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[3];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[3];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[3];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[4];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[4];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[4];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[5];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[5];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[5];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[6];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[6];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[6];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[7];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[7];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[7];
            (red_acc, green_acc, blue_acc)
        } else {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            for (cix, &weight) in weights.iter().enumerate() {
                let sy = (y0 + cix as i64) as usize;
                let source_index = sy * row_stride + x_index;
                red_acc += i64::from(intermediate[source_index]) * weight;
                green_acc += i64::from(intermediate[source_index + 1]) * weight;
                blue_acc += i64::from(intermediate[source_index + 2]) * weight;
            }
            (red_acc, green_acc, blue_acc)
        };
        result[0] = fixed_point_to_u8(red_acc);
        result[1] = fixed_point_to_u8(green_acc);
        result[2] = fixed_point_to_u8(blue_acc);
        return result;
    }

    if channels == 4 {
        let x_index = out_x as usize * 4;
        let row_stride = out_w as usize * 4;
        let mut red_acc = 0i64;
        let mut green_acc = 0i64;
        let mut blue_acc = 0i64;
        let mut alpha_acc = 0i64;
        for (tap, &weight) in weights.iter().enumerate() {
            let source_index = (y0 as usize + tap) * row_stride + x_index;
            red_acc += i64::from(intermediate[source_index]) * weight;
            green_acc += i64::from(intermediate[source_index + 1]) * weight;
            blue_acc += i64::from(intermediate[source_index + 2]) * weight;
            alpha_acc += i64::from(intermediate[source_index + 3]) * weight;
        }
        result[0] = fixed_point_to_u8(red_acc);
        result[1] = fixed_point_to_u8(green_acc);
        result[2] = fixed_point_to_u8(blue_acc);
        result[3] = fixed_point_to_u8(alpha_acc);
        return result;
    }

    for c in 0..channels {
        let mut acc: i64 = 0;
        for (cix, &w) in weights.iter().enumerate() {
            let sy = (y0 + cix as i64) as usize;
            let src_idx = (sy * out_w as usize + out_x as usize) * channels;
            acc += intermediate[src_idx + c] as i64 * w;
        }
        result[c] = fixed_point_to_u8(acc);
    }
    result
}

// A row-major intermediate makes the vertical pass revisit a different cache
// line for every source row and output column.  Above this size, one explicit
// transpose makes the samples for each output column contiguous.  Keep the
// small-image path unchanged: the extra allocation and copy are not amortized
// there, and preserving that path gives the benchmark a real crossover point.
const RESIZE_VERTICAL_TRANSPOSE_THRESHOLD: usize = 512 * 512;

#[inline]
fn should_transpose_vertical(source_rows: u32, output_width: u32, channels: usize) -> bool {
    matches!(channels, 1..=4)
        && source_rows > 1
        && output_width > 1
        && (source_rows as usize).saturating_mul(output_width as usize)
            >= RESIZE_VERTICAL_TRANSPOSE_THRESHOLD
}

/// Reorder the horizontal-pass result from `[source_y][output_x][channel]` to
/// `[output_x][source_y][channel]` for the cache-local vertical pass.
fn transpose_resize_intermediate(
    source: &[u8],
    source_rows: u32,
    output_width: u32,
    channels: usize,
) -> Vec<u8> {
    let source_row_stride = output_width as usize * channels;
    let destination_row_stride = source_rows as usize * channels;
    let mut destination = vec![0u8; source.len()];

    // RGB's three-byte pixels make the current column-at-a-time transpose
    // touch one source cache line per row and channel. Tile both dimensions so
    // each source tile is read in row-major order while the corresponding
    // transposed destination rows stay resident during their short write span.
    // Parallel CPU keeps its disjoint-column scheduling below.
    #[cfg(not(feature = "parallel"))]
    if channels == 3 {
        const TILE_WIDTH: usize = 32;
        const TILE_HEIGHT: usize = 32;
        let source_rows = source_rows as usize;
        let output_width = output_width as usize;
        for tile_y in (0..source_rows).step_by(TILE_HEIGHT) {
            let end_y = (tile_y + TILE_HEIGHT).min(source_rows);
            for tile_x in (0..output_width).step_by(TILE_WIDTH) {
                let end_x = (tile_x + TILE_WIDTH).min(output_width);
                for y in tile_y..end_y {
                    let source_row_start = y * source_row_stride;
                    for x in tile_x..end_x {
                        let source_start = source_row_start + x * 3;
                        let destination_start = x * destination_row_stride + y * 3;
                        destination[destination_start..destination_start + 3]
                            .copy_from_slice(&source[source_start..source_start + 3]);
                    }
                }
            }
        }
        return destination;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        &mut destination,
        destination_row_stride,
        output_width as usize,
        |_row_start, _row_end, x, row| {
            let x = x as usize;
            for y in 0..source_rows as usize {
                let source_start = y * source_row_stride + x * channels;
                let destination_start = y * channels;
                row[destination_start..destination_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for x in 0..output_width as usize {
        let row_start = x * destination_row_stride;
        let row = &mut destination[row_start..row_start + destination_row_stride];
        for y in 0..source_rows as usize {
            let source_start = y * source_row_stride + x * channels;
            let destination_start = y * channels;
            row[destination_start..destination_start + channels]
                .copy_from_slice(&source[source_start..source_start + channels]);
        }
    }

    destination
}

#[cfg(all(test, not(feature = "parallel")))]
mod resize_transpose_tests {
    use super::transpose_resize_intermediate;

    #[test]
    fn tiled_rgb_transpose_matches_reference_across_partial_tiles() {
        for (source_rows, output_width) in [(1u32, 1u32), (31, 33), (33, 31), (65, 67)] {
            let source = (0..source_rows as usize * output_width as usize * 3)
                .map(|index| (index.wrapping_mul(73).wrapping_add(index / 7 * 19)) as u8)
                .collect::<Vec<_>>();
            let mut expected = vec![0; source.len()];
            let source_stride = output_width as usize * 3;
            let destination_stride = source_rows as usize * 3;
            for x in 0..output_width as usize {
                for y in 0..source_rows as usize {
                    let source_start = y * source_stride + x * 3;
                    let destination_start = x * destination_stride + y * 3;
                    expected[destination_start..destination_start + 3]
                        .copy_from_slice(&source[source_start..source_start + 3]);
                }
            }

            assert_eq!(
                transpose_resize_intermediate(&source, source_rows, output_width, 3),
                expected,
                "source_rows={source_rows}, output_width={output_width}"
            );
        }
    }
}

#[inline]
fn vertical_pass_col_transposed(
    intermediate: &[u8],
    source_rows: u32,
    out_x: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_y: usize,
) -> [u8; 4] {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return [0u8; 4];
    }
    let weights = coeffs.weights_for(out_y);
    let column_start = out_x as usize * source_rows as usize * channels;
    let mut result = [0u8; 4];
    if channels == 3 {
        let column_start = out_x as usize * source_rows as usize * 3;
        let (red_acc, green_acc, blue_acc) = if cnt == 4 {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            let mut source_index = column_start + y0 as usize * 3;
            red_acc += i64::from(intermediate[source_index]) * weights[0];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[0];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[0];
            source_index += 3;
            red_acc += i64::from(intermediate[source_index]) * weights[1];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[1];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[1];
            source_index += 3;
            red_acc += i64::from(intermediate[source_index]) * weights[2];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[2];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[2];
            source_index += 3;
            red_acc += i64::from(intermediate[source_index]) * weights[3];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[3];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[3];
            (red_acc, green_acc, blue_acc)
        } else {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            for (cix, &weight) in weights.iter().enumerate() {
                let sy = (y0 + cix as i64) as usize;
                let source_index = column_start + sy * 3;
                red_acc += i64::from(intermediate[source_index]) * weight;
                green_acc += i64::from(intermediate[source_index + 1]) * weight;
                blue_acc += i64::from(intermediate[source_index + 2]) * weight;
            }
            (red_acc, green_acc, blue_acc)
        };
        result[0] = fixed_point_to_u8(red_acc);
        result[1] = fixed_point_to_u8(green_acc);
        result[2] = fixed_point_to_u8(blue_acc);
        return result;
    }

    if channels == 4 {
        let mut red_acc = 0i64;
        let mut green_acc = 0i64;
        let mut blue_acc = 0i64;
        let mut alpha_acc = 0i64;
        let mut source_index = column_start + y0 as usize * 4;
        for &weight in weights {
            red_acc += i64::from(intermediate[source_index]) * weight;
            green_acc += i64::from(intermediate[source_index + 1]) * weight;
            blue_acc += i64::from(intermediate[source_index + 2]) * weight;
            alpha_acc += i64::from(intermediate[source_index + 3]) * weight;
            source_index += 4;
        }
        result[0] = fixed_point_to_u8(red_acc);
        result[1] = fixed_point_to_u8(green_acc);
        result[2] = fixed_point_to_u8(blue_acc);
        result[3] = fixed_point_to_u8(alpha_acc);
        return result;
    }

    for c in 0..channels {
        let mut acc: i64 = 0;
        for (cix, &w) in weights.iter().enumerate() {
            let sy = (y0 + cix as i64) as usize;
            let src_idx = column_start + sy * channels;
            acc += intermediate[src_idx + c] as i64 * w;
        }
        result[c] = fixed_point_to_u8(acc);
    }
    result
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_row_hsv_i32(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    output_width: usize,
    output_row: &mut [u8],
) {
    for output_x in 0..output_width {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let mut red_sum = 0i32;
        let mut green_sum = 0i32;
        let mut blue_sum = 0i32;
        let mut source_start = coeffs.xmin[output_x] as usize * 3;
        if weights.len() == 4 {
            let weight = weights[0] as i32;
            red_sum = i32::from(source_row[source_start]) * weight;
            green_sum = i32::from(source_row[source_start + 1]) * weight;
            blue_sum = i32::from(source_row[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[1] as i32;
            red_sum += i32::from(source_row[source_start]) * weight;
            green_sum += i32::from(source_row[source_start + 1]) * weight;
            blue_sum += i32::from(source_row[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[2] as i32;
            red_sum += i32::from(source_row[source_start]) * weight;
            green_sum += i32::from(source_row[source_start + 1]) * weight;
            blue_sum += i32::from(source_row[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[3] as i32;
            red_sum += i32::from(source_row[source_start]) * weight;
            green_sum += i32::from(source_row[source_start + 1]) * weight;
            blue_sum += i32::from(source_row[source_start + 2]) * weight;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                let source_start = (coeffs.xmin[output_x] as usize + tap) * 3;
                let weight = weight as i32;
                red_sum += i32::from(source_row[source_start]) * weight;
                green_sum += i32::from(source_row[source_start + 1]) * weight;
                blue_sum += i32::from(source_row[source_start + 2]) * weight;
            }
        }
        let output_start = output_x * 3;
        output_row[output_start] = fixed_point_to_u8_i32(red_sum);
        output_row[output_start + 1] = fixed_point_to_u8_i32(green_sum);
        output_row[output_start + 2] = fixed_point_to_u8_i32(blue_sum);
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_col_hsv_i32(
    intermediate: &[u8],
    source_rows: usize,
    output_width: usize,
    output_x: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    transposed: bool,
) -> [u8; 3] {
    let weights = coeffs.weights_for(output_y);
    if weights.is_empty() {
        return [0; 3];
    }
    let first_source_y = coeffs.xmin[output_y] as usize;
    let (mut source_start, source_step) = if transposed {
        (output_x * source_rows * 3 + first_source_y * 3, 3)
    } else {
        (
            first_source_y * output_width * 3 + output_x * 3,
            output_width * 3,
        )
    };
    let mut red_sum = 0i32;
    let mut green_sum = 0i32;
    let mut blue_sum = 0i32;
    if weights.len() == 4 {
        let weight = weights[0] as i32;
        red_sum = i32::from(intermediate[source_start]) * weight;
        green_sum = i32::from(intermediate[source_start + 1]) * weight;
        blue_sum = i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[1] as i32;
        red_sum += i32::from(intermediate[source_start]) * weight;
        green_sum += i32::from(intermediate[source_start + 1]) * weight;
        blue_sum += i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[2] as i32;
        red_sum += i32::from(intermediate[source_start]) * weight;
        green_sum += i32::from(intermediate[source_start + 1]) * weight;
        blue_sum += i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[3] as i32;
        red_sum += i32::from(intermediate[source_start]) * weight;
        green_sum += i32::from(intermediate[source_start + 1]) * weight;
        blue_sum += i32::from(intermediate[source_start + 2]) * weight;
    } else {
        for &weight in weights {
            let weight = weight as i32;
            red_sum += i32::from(intermediate[source_start]) * weight;
            green_sum += i32::from(intermediate[source_start + 1]) * weight;
            blue_sum += i32::from(intermediate[source_start + 2]) * weight;
            source_start += source_step;
        }
    }
    [
        fixed_point_to_u8_i32(red_sum),
        fixed_point_to_u8_i32(green_sum),
        fixed_point_to_u8_i32(blue_sum),
    ]
}

#[cfg(not(feature = "parallel"))]
fn resize_coefficients_fit_source(coeffs: &FilterCoeffs, source_extent: usize) -> bool {
    if coeffs.xmin.len() != coeffs.count.len() || coeffs.xmin.len() != coeffs.offsets.len() {
        return false;
    }
    coeffs
        .xmin
        .iter()
        .zip(&coeffs.count)
        .all(|(&first, &count)| {
            usize::try_from(first)
                .ok()
                .and_then(|first| first.checked_add(count))
                .is_some_and(|end| end <= source_extent)
        })
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_luma_i32(source_row: &[u8], coeffs: &FilterCoeffs, output_row: &mut [u8]) {
    for (output_x, output) in output_row.iter_mut().enumerate() {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let mut sum = 0i32;
        let source_x = coeffs.xmin[output_x] as usize;
        if weights.len() == 4 {
            sum = i32::from(source_row[source_x]) * weights[0] as i32;
            sum += i32::from(source_row[source_x + 1]) * weights[1] as i32;
            sum += i32::from(source_row[source_x + 2]) * weights[2] as i32;
            sum += i32::from(source_row[source_x + 3]) * weights[3] as i32;
        } else if weights.len() == 5 {
            sum = i32::from(source_row[source_x]) * weights[0] as i32;
            sum += i32::from(source_row[source_x + 1]) * weights[1] as i32;
            sum += i32::from(source_row[source_x + 2]) * weights[2] as i32;
            sum += i32::from(source_row[source_x + 3]) * weights[3] as i32;
            sum += i32::from(source_row[source_x + 4]) * weights[4] as i32;
        } else if weights.len() == 6 {
            sum = i32::from(source_row[source_x]) * weights[0] as i32;
            sum += i32::from(source_row[source_x + 1]) * weights[1] as i32;
            sum += i32::from(source_row[source_x + 2]) * weights[2] as i32;
            sum += i32::from(source_row[source_x + 3]) * weights[3] as i32;
            sum += i32::from(source_row[source_x + 4]) * weights[4] as i32;
            sum += i32::from(source_row[source_x + 5]) * weights[5] as i32;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                sum += i32::from(source_row[source_x + tap]) * weight as i32;
            }
        }
        *output = fixed_point_to_u8_i32(sum);
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_luma_i32(
    intermediate: &[u8],
    source_height: usize,
    output_width: usize,
    coeffs: &FilterCoeffs,
    transposed: bool,
    output: &mut [u8],
) {
    for (output_y, output_row) in output.chunks_exact_mut(output_width).enumerate() {
        let weights = coeffs.weights_for(output_y);
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let source_y = coeffs.xmin[output_y] as usize;
        if transposed {
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let source_start = output_x * source_height + source_y;
                let mut sum = i32::from(intermediate[source_start]) * weights[0] as i32;
                for (tap, &weight) in weights.iter().enumerate().skip(1) {
                    sum += i32::from(intermediate[source_start + tap]) * weight as i32;
                }
                *pixel = fixed_point_to_u8_i32(sum);
            }
        } else if weights.len() == 4 {
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let row0 = source_y * output_width;
            let row1 = row0 + output_width;
            let row2 = row1 + output_width;
            let row3 = row2 + output_width;
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let mut sum = i32::from(intermediate[row0 + output_x]) * weight0;
                sum += i32::from(intermediate[row1 + output_x]) * weight1;
                sum += i32::from(intermediate[row2 + output_x]) * weight2;
                sum += i32::from(intermediate[row3 + output_x]) * weight3;
                *pixel = fixed_point_to_u8_i32(sum);
            }
        } else if weights.len() == 5 {
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let weight4 = weights[4] as i32;
            let row0 = source_y * output_width;
            let row1 = row0 + output_width;
            let row2 = row1 + output_width;
            let row3 = row2 + output_width;
            let row4 = row3 + output_width;
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let mut sum = i32::from(intermediate[row0 + output_x]) * weight0;
                sum += i32::from(intermediate[row1 + output_x]) * weight1;
                sum += i32::from(intermediate[row2 + output_x]) * weight2;
                sum += i32::from(intermediate[row3 + output_x]) * weight3;
                sum += i32::from(intermediate[row4 + output_x]) * weight4;
                *pixel = fixed_point_to_u8_i32(sum);
            }
        } else if weights.len() == 6 {
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let weight4 = weights[4] as i32;
            let weight5 = weights[5] as i32;
            let row0 = source_y * output_width;
            let row1 = row0 + output_width;
            let row2 = row1 + output_width;
            let row3 = row2 + output_width;
            let row4 = row3 + output_width;
            let row5 = row4 + output_width;
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let mut sum = i32::from(intermediate[row0 + output_x]) * weight0;
                sum += i32::from(intermediate[row1 + output_x]) * weight1;
                sum += i32::from(intermediate[row2 + output_x]) * weight2;
                sum += i32::from(intermediate[row3 + output_x]) * weight3;
                sum += i32::from(intermediate[row4 + output_x]) * weight4;
                sum += i32::from(intermediate[row5 + output_x]) * weight5;
                *pixel = fixed_point_to_u8_i32(sum);
            }
        } else {
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let source_start = source_y * output_width + output_x;
                let mut sum = i32::from(intermediate[source_start]) * weights[0] as i32;
                for (tap, &weight) in weights.iter().enumerate().skip(1) {
                    sum +=
                        i32::from(intermediate[source_start + tap * output_width]) * weight as i32;
                }
                *pixel = fixed_point_to_u8_i32(sum);
            }
        }
    }
}

#[cfg(not(feature = "parallel"))]
fn pil_resize_luma_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    let output_len = output_height.checked_mul(output_width)?;
    let mut output = vec![0; output_len];
    pil_resize_luma_i32_into(
        img,
        output_width,
        output_height,
        horizontal,
        vertical,
        &mut output,
    )?;
    Some(output)
}

#[cfg(not(feature = "parallel"))]
fn pil_resize_luma_i32_into(
    img: &DynamicImage,
    output_width: usize,
    output_height: usize,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
    output: &mut [u8],
) -> Option<()> {
    let DynamicImage::ImageLuma8(source) = img else {
        return None;
    };
    let source_width = usize::try_from(source.width()).ok()?;
    let source_height = usize::try_from(source.height()).ok()?;
    if horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !resize_coefficients_fit_source(horizontal, source_width)
        || !resize_coefficients_fit_source(vertical, source_height)
        || !resize_u8_coefficients_fit_i32(horizontal)
        || !resize_u8_coefficients_fit_i32(vertical)
    {
        return None;
    }
    let intermediate_len = source_height.checked_mul(output_width)?;
    let output_len = output_height.checked_mul(output_width)?;
    if source.as_raw().len() != source_width.checked_mul(source_height)?
        || output.len() != output_len
    {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_width)?;
        let output_start = source_y.checked_mul(output_width)?;
        horizontal_pass_luma_i32(
            &source.as_raw()[source_start..source_start + source_width],
            horizontal,
            &mut intermediate[output_start..output_start + output_width],
        );
    }

    vertical_pass_luma_i32(
        &intermediate,
        source_height,
        output_width,
        vertical,
        false,
        output,
    );
    Some(())
}

/// Resize native L bytes directly into a caller-owned full-width output
/// window. ImageOps.pad uses this to avoid materializing the contained image
/// before copying its rows into the final canvas.
#[cfg(not(feature = "parallel"))]
pub(crate) fn pil_resize_luma_i32_into_window(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> bool {
    let DynamicImage::ImageLuma8(_) = img else {
        return false;
    };
    if output_width == 0 || output_height == 0 || img.width() == 0 || img.height() == 0 {
        return false;
    }
    let horizontal = precompute_coeffs(output_width, img.width(), filter);
    let vertical = precompute_coeffs(output_height, img.height(), filter);
    pil_resize_luma_i32_into(
        img,
        output_width as usize,
        output_height as usize,
        &horizontal,
        &vertical,
        output,
    )
    .is_some()
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_la_i32(source_row: &[u8], coeffs: &FilterCoeffs, output_row: &mut [u8]) {
    for (output_x, output_pixel) in output_row.chunks_exact_mut(2).enumerate() {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let source_start = coeffs.xmin[output_x] as usize * 2;
        let (mut luma_sum, mut alpha_sum) = (0i32, 0i32);
        if weights.len() == 4 {
            luma_sum = i32::from(source_row[source_start]) * weights[0] as i32;
            alpha_sum = i32::from(source_row[source_start + 1]) * weights[0] as i32;
            luma_sum += i32::from(source_row[source_start + 2]) * weights[1] as i32;
            alpha_sum += i32::from(source_row[source_start + 3]) * weights[1] as i32;
            luma_sum += i32::from(source_row[source_start + 4]) * weights[2] as i32;
            alpha_sum += i32::from(source_row[source_start + 5]) * weights[2] as i32;
            luma_sum += i32::from(source_row[source_start + 6]) * weights[3] as i32;
            alpha_sum += i32::from(source_row[source_start + 7]) * weights[3] as i32;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                let sample = source_start + tap * 2;
                let weight = weight as i32;
                luma_sum += i32::from(source_row[sample]) * weight;
                alpha_sum += i32::from(source_row[sample + 1]) * weight;
            }
        }
        output_pixel[0] = fixed_point_to_u8_i32(luma_sum);
        output_pixel[1] = fixed_point_to_u8_i32(alpha_sum);
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_la_i32(
    intermediate: &[u8],
    output_width: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width * 2;
    for (output_y, output_row) in output.chunks_exact_mut(output_stride).enumerate() {
        let weights = coeffs.weights_for(output_y);
        if weights.is_empty() {
            continue;
        }
        let source_start = coeffs.xmin[output_y] as usize * output_stride;
        if weights.len() == 4 {
            let row1 = source_start + output_stride;
            let row2 = row1 + output_stride;
            let row3 = row2 + output_stride;
            let (weight0, weight1, weight2, weight3) = (
                weights[0] as i32,
                weights[1] as i32,
                weights[2] as i32,
                weights[3] as i32,
            );
            for (output_x, output_pixel) in output_row.chunks_exact_mut(2).enumerate() {
                let source_pixel = output_x * 2;
                let mut luma_sum = i32::from(intermediate[source_start + source_pixel]) * weight0;
                let mut alpha_sum =
                    i32::from(intermediate[source_start + source_pixel + 1]) * weight0;
                luma_sum += i32::from(intermediate[row1 + source_pixel]) * weight1;
                alpha_sum += i32::from(intermediate[row1 + source_pixel + 1]) * weight1;
                luma_sum += i32::from(intermediate[row2 + source_pixel]) * weight2;
                alpha_sum += i32::from(intermediate[row2 + source_pixel + 1]) * weight2;
                luma_sum += i32::from(intermediate[row3 + source_pixel]) * weight3;
                alpha_sum += i32::from(intermediate[row3 + source_pixel + 1]) * weight3;
                let alpha = fixed_point_to_u8_i32(alpha_sum);
                output_pixel[0] = unpremultiply_channel(fixed_point_to_u8_i32(luma_sum), alpha);
                output_pixel[1] = alpha;
            }
        } else {
            for (output_x, output_pixel) in output_row.chunks_exact_mut(2).enumerate() {
                let source_pixel = output_x * 2;
                let (mut luma_sum, mut alpha_sum) = (0i32, 0i32);
                for (tap, &weight) in weights.iter().enumerate() {
                    let sample = source_start + tap * output_stride + source_pixel;
                    let weight = weight as i32;
                    luma_sum += i32::from(intermediate[sample]) * weight;
                    alpha_sum += i32::from(intermediate[sample + 1]) * weight;
                }
                let alpha = fixed_point_to_u8_i32(alpha_sum);
                output_pixel[0] = unpremultiply_channel(fixed_point_to_u8_i32(luma_sum), alpha);
                output_pixel[1] = alpha;
            }
        }
    }
}

#[cfg(not(feature = "parallel"))]
fn pil_resize_la_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let DynamicImage::ImageLumaA8(source) = img else {
        return None;
    };
    let source_width = usize::try_from(source.width()).ok()?;
    let source_height = usize::try_from(source.height()).ok()?;
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    if horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !resize_coefficients_fit_source(horizontal, source_width)
        || !resize_coefficients_fit_source(vertical, source_height)
        || !resize_u8_coefficients_fit_i32(horizontal)
        || !resize_u8_coefficients_fit_i32(vertical)
    {
        return None;
    }

    let source_stride = source_width.checked_mul(2)?;
    let source_len = source_height.checked_mul(source_stride)?;
    let output_stride = output_width.checked_mul(2)?;
    let intermediate_len = source_height.checked_mul(output_stride)?;
    let output_len = output_height.checked_mul(output_stride)?;
    if source.as_raw().len() != source_len {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    let mut premultiplied_row = vec![0; source_stride];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let output_start = source_y.checked_mul(output_stride)?;
        for (source_pixel, output_pixel) in source.as_raw()
            [source_start..source_start + source_stride]
            .chunks_exact(2)
            .zip(premultiplied_row.chunks_exact_mut(2))
        {
            output_pixel[0] = premultiply_channel(source_pixel[0], source_pixel[1]);
            output_pixel[1] = source_pixel[1];
        }
        horizontal_pass_la_i32(
            &premultiplied_row,
            horizontal,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }

    let mut output = vec![0; output_len];
    vertical_pass_la_i32(&intermediate, output_width, vertical, &mut output);
    Some(output)
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_rgba_i32(source_row: &[u8], coeffs: &FilterCoeffs, output_row: &mut [u8]) {
    for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let mut sums = [0i32; 4];
        let mut source_start = coeffs.xmin[output_x] as usize * 4;
        for &weight in weights {
            let weight = weight as i32;
            for (channel, sum) in sums.iter_mut().enumerate() {
                *sum += i32::from(source_row[source_start + channel]) * weight;
            }
            source_start += 4;
        }
        for (output, sum) in output_pixel.iter_mut().zip(sums) {
            *output = fixed_point_to_u8_i32(sum);
        }
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_rgba_i32(
    intermediate: &[u8],
    output_width: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width * 4;
    for (output_y, output_row) in output.chunks_exact_mut(output_stride).enumerate() {
        let weights = coeffs.weights_for(output_y);
        if weights.is_empty() {
            continue;
        }
        let first_source_row = coeffs.xmin[output_y] as usize * output_stride;
        for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
            let source_start = first_source_row + output_x * 4;
            let mut sums = [0i32; 4];
            for (tap, &weight) in weights.iter().enumerate() {
                let pixel_start = source_start + tap * output_stride;
                let weight = weight as i32;
                for (channel, sum) in sums.iter_mut().enumerate() {
                    *sum += i32::from(intermediate[pixel_start + channel]) * weight;
                }
            }
            let alpha = fixed_point_to_u8_i32(sums[3]);
            output_pixel[0] = unpremultiply_channel(fixed_point_to_u8_i32(sums[0]), alpha);
            output_pixel[1] = unpremultiply_channel(fixed_point_to_u8_i32(sums[1]), alpha);
            output_pixel[2] = unpremultiply_channel(fixed_point_to_u8_i32(sums[2]), alpha);
            output_pixel[3] = alpha;
        }
    }
}

#[cfg(not(feature = "parallel"))]
fn pil_resize_rgba_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let source_width = usize::try_from(source.width()).ok()?;
    let source_height = usize::try_from(source.height()).ok()?;
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    if horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !resize_coefficients_fit_source(horizontal, source_width)
        || !resize_coefficients_fit_source(vertical, source_height)
        || !resize_u8_coefficients_fit_i32(horizontal)
        || !resize_u8_coefficients_fit_i32(vertical)
    {
        return None;
    }

    let source_stride = source_width.checked_mul(4)?;
    let output_stride = output_width.checked_mul(4)?;
    let intermediate_len = source_height.checked_mul(output_stride)?;
    let output_len = output_height.checked_mul(output_stride)?;
    if source.as_raw().len() != source_height.checked_mul(source_stride)? {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    let mut premultiplied_row = vec![0; source_stride];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let output_start = source_y.checked_mul(output_stride)?;
        for (source_pixel, output_pixel) in source.as_raw()
            [source_start..source_start + source_stride]
            .chunks_exact(4)
            .zip(premultiplied_row.chunks_exact_mut(4))
        {
            let alpha = source_pixel[3];
            output_pixel[0] = premultiply_channel(source_pixel[0], alpha);
            output_pixel[1] = premultiply_channel(source_pixel[1], alpha);
            output_pixel[2] = premultiply_channel(source_pixel[2], alpha);
            output_pixel[3] = alpha;
        }
        horizontal_pass_rgba_i32(
            &premultiplied_row,
            horizontal,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }

    let mut output = vec![0; output_len];
    vertical_pass_rgba_i32(&intermediate, output_width, vertical, &mut output);
    Some(output)
}

#[cfg(not(feature = "parallel"))]
fn pil_resize_hsv_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<DynamicImage> {
    let source_width = usize::try_from(img.width()).ok()?;
    let source_height = usize::try_from(img.height()).ok()?;
    let output_width_usize = usize::try_from(output_width).ok()?;
    let output_height_usize = usize::try_from(output_height).ok()?;
    let source_stride = source_width.checked_mul(3)?;
    let intermediate_stride = output_width_usize.checked_mul(3)?;
    let intermediate_len = source_height.checked_mul(intermediate_stride)?;
    let output_len = output_height_usize.checked_mul(intermediate_stride)?;
    let mut intermediate = vec![0u8; intermediate_len];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let source_row = img
            .as_bytes()
            .get(source_start..source_start.checked_add(source_stride)?)?;
        let output_start = source_y.checked_mul(intermediate_stride)?;
        let output_row =
            intermediate.get_mut(output_start..output_start.checked_add(intermediate_stride)?)?;
        horizontal_pass_row_hsv_i32(source_row, horizontal, output_width_usize, output_row);
    }

    let transposed = should_transpose_vertical(img.height(), output_width, 3);
    let transposed_intermediate = transposed
        .then(|| transpose_resize_intermediate(&intermediate, img.height(), output_width, 3));
    let vertical_source = transposed_intermediate.as_deref().unwrap_or(&intermediate);
    let mut output = vec![0u8; output_len];
    for output_y in 0..output_height_usize {
        let output_row_start = output_y.checked_mul(intermediate_stride)?;
        let output_row =
            output.get_mut(output_row_start..output_row_start.checked_add(intermediate_stride)?)?;
        for output_x in 0..output_width_usize {
            let pixel = vertical_pass_col_hsv_i32(
                vertical_source,
                source_height,
                output_width_usize,
                output_x,
                vertical,
                output_y,
                transposed,
            );
            let output_start = output_x * 3;
            output_row[output_start..output_start + 3].copy_from_slice(&pixel);
        }
    }
    Some(raw_to_dynamic_owned(output, output_width, output_height, 3))
}

/// Resize the four stored C/M/Y/K bytes with narrow, exact fixed-point sums.
/// CMYK shares an `ImageRgba8` carrier, but none of its channels are alpha.
#[cfg(not(feature = "parallel"))]
fn pil_resize_cmyk_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<DynamicImage> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let source_width = usize::try_from(source.width()).ok()?;
    let source_height = usize::try_from(source.height()).ok()?;
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    if source_width == 0
        || source_height == 0
        || output_width == 0
        || output_height == 0
        || horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !resize_coefficients_fit_source(horizontal, source_width)
        || !resize_coefficients_fit_source(vertical, source_height)
        || !resize_u8_coefficients_fit_i32(horizontal)
        || !resize_u8_coefficients_fit_i32(vertical)
    {
        return None;
    }

    let source_stride = source_width.checked_mul(4)?;
    let intermediate_stride = output_width.checked_mul(4)?;
    let intermediate_len = source_height.checked_mul(intermediate_stride)?;
    let output_len = output_height.checked_mul(intermediate_stride)?;
    if source.as_raw().len() != source_height.checked_mul(source_stride)? {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let source_row = source
            .as_raw()
            .get(source_start..source_start.checked_add(source_stride)?)?;
        let output_start = source_y.checked_mul(intermediate_stride)?;
        let output_row =
            intermediate.get_mut(output_start..output_start.checked_add(intermediate_stride)?)?;
        horizontal_pass_row_cmyk_i32(source_row, horizontal, output_width, output_row);
    }

    // CMYK vertical taps read four row-major streams. Keeping those streams
    // contiguous lets the output-x loop reuse cache lines and avoids a full
    // intermediate-frame transpose/copy.
    let vertical_source = &intermediate;
    let mut output = vec![0; output_len];
    for output_y in 0..output_height {
        let output_start = output_y.checked_mul(intermediate_stride)?;
        let output_row =
            output.get_mut(output_start..output_start.checked_add(intermediate_stride)?)?;
        let weights = vertical.weights_for(output_y);
        if weights.is_empty() {
            continue;
        }
        let first_source_y = usize::try_from(vertical.xmin[output_y]).ok()?;
        if weights.len() == 4 {
            // Bicubic uses four taps. Hoist row-invariant coefficients and
            // source offsets so the pixel loop only performs channel MACs.
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let first_source_row = first_source_y.checked_mul(intermediate_stride)?;
            for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
                let source_start = output_x * 4 + first_source_row;
                let (mut cyan, mut magenta, mut yellow, mut black) = (
                    i32::from(vertical_source[source_start]) * weight0,
                    i32::from(vertical_source[source_start + 1]) * weight0,
                    i32::from(vertical_source[source_start + 2]) * weight0,
                    i32::from(vertical_source[source_start + 3]) * weight0,
                );
                let source_start = source_start + intermediate_stride;
                cyan += i32::from(vertical_source[source_start]) * weight1;
                magenta += i32::from(vertical_source[source_start + 1]) * weight1;
                yellow += i32::from(vertical_source[source_start + 2]) * weight1;
                black += i32::from(vertical_source[source_start + 3]) * weight1;
                let source_start = source_start + intermediate_stride;
                cyan += i32::from(vertical_source[source_start]) * weight2;
                magenta += i32::from(vertical_source[source_start + 1]) * weight2;
                yellow += i32::from(vertical_source[source_start + 2]) * weight2;
                black += i32::from(vertical_source[source_start + 3]) * weight2;
                let source_start = source_start + intermediate_stride;
                cyan += i32::from(vertical_source[source_start]) * weight3;
                magenta += i32::from(vertical_source[source_start + 1]) * weight3;
                yellow += i32::from(vertical_source[source_start + 2]) * weight3;
                black += i32::from(vertical_source[source_start + 3]) * weight3;
                output_pixel[0] = fixed_point_to_u8_i32(cyan);
                output_pixel[1] = fixed_point_to_u8_i32(magenta);
                output_pixel[2] = fixed_point_to_u8_i32(yellow);
                output_pixel[3] = fixed_point_to_u8_i32(black);
            }
        } else {
            for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
                output_pixel.copy_from_slice(&vertical_pass_col_cmyk_i32(
                    vertical_source,
                    intermediate_stride,
                    first_source_y,
                    output_x,
                    weights,
                ));
            }
        }
    }

    Some(raw_to_dynamic_owned(
        output,
        output_width as u32,
        output_height as u32,
        4,
    ))
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_row_cmyk_i32(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    output_width: usize,
    output_row: &mut [u8],
) {
    for output_x in 0..output_width {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let mut cyan = 0i32;
        let mut magenta = 0i32;
        let mut yellow = 0i32;
        let mut black = 0i32;
        let mut source_start = coeffs.xmin[output_x] as usize * 4;
        if weights.len() == 4 {
            let weight = weights[0] as i32;
            cyan = i32::from(source_row[source_start]) * weight;
            magenta = i32::from(source_row[source_start + 1]) * weight;
            yellow = i32::from(source_row[source_start + 2]) * weight;
            black = i32::from(source_row[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[1] as i32;
            cyan += i32::from(source_row[source_start]) * weight;
            magenta += i32::from(source_row[source_start + 1]) * weight;
            yellow += i32::from(source_row[source_start + 2]) * weight;
            black += i32::from(source_row[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[2] as i32;
            cyan += i32::from(source_row[source_start]) * weight;
            magenta += i32::from(source_row[source_start + 1]) * weight;
            yellow += i32::from(source_row[source_start + 2]) * weight;
            black += i32::from(source_row[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[3] as i32;
            cyan += i32::from(source_row[source_start]) * weight;
            magenta += i32::from(source_row[source_start + 1]) * weight;
            yellow += i32::from(source_row[source_start + 2]) * weight;
            black += i32::from(source_row[source_start + 3]) * weight;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                let sample = coeffs.xmin[output_x] as usize + tap;
                let source_start = sample * 4;
                let weight = weight as i32;
                cyan += i32::from(source_row[source_start]) * weight;
                magenta += i32::from(source_row[source_start + 1]) * weight;
                yellow += i32::from(source_row[source_start + 2]) * weight;
                black += i32::from(source_row[source_start + 3]) * weight;
            }
        }
        let output_start = output_x * 4;
        output_row[output_start] = fixed_point_to_u8_i32(cyan);
        output_row[output_start + 1] = fixed_point_to_u8_i32(magenta);
        output_row[output_start + 2] = fixed_point_to_u8_i32(yellow);
        output_row[output_start + 3] = fixed_point_to_u8_i32(black);
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_col_cmyk_i32(
    intermediate: &[u8],
    source_stride: usize,
    first_source_y: usize,
    output_x: usize,
    weights: &[i64],
) -> [u8; 4] {
    let mut source_start = first_source_y * source_stride + output_x * 4;
    let (mut cyan, mut magenta, mut yellow, mut black) = (0i32, 0i32, 0i32, 0i32);
    for &weight in weights {
        let weight = weight as i32;
        cyan += i32::from(intermediate[source_start]) * weight;
        magenta += i32::from(intermediate[source_start + 1]) * weight;
        yellow += i32::from(intermediate[source_start + 2]) * weight;
        black += i32::from(intermediate[source_start + 3]) * weight;
        source_start += source_stride;
    }
    [
        fixed_point_to_u8_i32(cyan),
        fixed_point_to_u8_i32(magenta),
        fixed_point_to_u8_i32(yellow),
        fixed_point_to_u8_i32(black),
    ]
}

fn horizontal_pass_rows(
    work_bytes: &[u8],
    source_width: u32,
    source_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: u32,
    intermediate: &mut [u8],
) {
    let source_stride = source_width as usize * channels;
    let output_stride = output_width as usize * channels;
    if source_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        intermediate,
        output_stride,
        source_height as usize,
        |row_start, _row_end, y, row| {
            let source_start = y as usize * source_stride;
            horizontal_pass_row(
                &work_bytes[source_start..source_start + source_stride],
                source_width,
                channels,
                coeffs,
                output_width,
                &mut row[..output_stride],
            );
            debug_assert_eq!(row_start, y as usize * output_stride);
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..source_height as usize {
        let source_start = y * source_stride;
        let output_start = y * output_stride;
        horizontal_pass_row(
            &work_bytes[source_start..source_start + source_stride],
            source_width,
            channels,
            coeffs,
            output_width,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }
}

fn horizontal_pass_rows_alpha(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: u32,
    intermediate: &mut [u8],
) {
    let source_stride = source_width as usize * channels;
    let output_stride = output_width as usize * channels;
    if source_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        intermediate,
        output_stride,
        source_height as usize,
        |_row_start, _row_end, y, row| {
            let source_start = y as usize * source_stride;
            horizontal_pass_row_alpha(
                &source[source_start..source_start + source_stride],
                channels,
                coeffs,
                output_width,
                &mut row[..output_stride],
            );
        }
    );

    #[cfg(not(feature = "parallel"))]
    let mut premultiplied_row = vec![0u8; source_stride];

    #[cfg(not(feature = "parallel"))]
    for y in 0..source_height as usize {
        let source_start = y * source_stride;
        let output_start = y * output_stride;
        premultiply_alpha_row(
            &source[source_start..source_start + source_stride],
            channels,
            &mut premultiplied_row,
        );
        horizontal_pass_row(
            &premultiplied_row,
            source_width,
            channels,
            coeffs,
            output_width,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }
}

/// Boxed-resize variant that runs the horizontal pass only for source rows
/// referenced by the rebased vertical coefficient table.
fn horizontal_pass_boxed_rows(
    work_bytes: &[u8],
    source_width: u32,
    first_source_row: u32,
    source_row_count: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: u32,
    intermediate: &mut [u8],
    premultiplied_alpha: bool,
    parallel_pixel_threshold: usize,
) {
    let _ = parallel_pixel_threshold;
    let source_stride = source_width as usize * channels;
    let output_stride = output_width as usize * channels;
    if source_row_count == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    if (source_row_count as usize).saturating_mul(output_width as usize) < parallel_pixel_threshold
    {
        for row_index in 0..source_row_count as usize {
            let source_y = first_source_row as usize + row_index;
            let source_start = source_y * source_stride;
            let output_start = row_index * output_stride;
            let source_row = &work_bytes[source_start..source_start + source_stride];
            let output_row = &mut intermediate[output_start..output_start + output_stride];
            if premultiplied_alpha {
                horizontal_pass_row_alpha(source_row, channels, coeffs, output_width, output_row);
            } else {
                horizontal_pass_row(
                    source_row,
                    source_width,
                    channels,
                    coeffs,
                    output_width,
                    output_row,
                );
            }
        }
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        intermediate,
        output_stride,
        source_row_count as usize,
        |_row_start, _row_end, row_index, row| {
            let source_y = first_source_row as usize + row_index as usize;
            let source_start = source_y * source_stride;
            let source_row = &work_bytes[source_start..source_start + source_stride];
            if premultiplied_alpha {
                horizontal_pass_row_alpha(
                    source_row,
                    channels,
                    coeffs,
                    output_width,
                    &mut row[..output_stride],
                );
            } else {
                horizontal_pass_row(
                    source_row,
                    source_width,
                    channels,
                    coeffs,
                    output_width,
                    &mut row[..output_stride],
                );
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for row_index in 0..source_row_count as usize {
        let source_y = first_source_row as usize + row_index;
        let source_start = source_y * source_stride;
        let output_start = row_index * output_stride;
        let source_row = &work_bytes[source_start..source_start + source_stride];
        let output_row = &mut intermediate[output_start..output_start + output_stride];
        if premultiplied_alpha {
            horizontal_pass_row_alpha(source_row, channels, coeffs, output_width, output_row);
        } else {
            horizontal_pass_row(
                source_row,
                source_width,
                channels,
                coeffs,
                output_width,
                output_row,
            );
        }
    }
}

fn vertical_pass_rows(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
    parallel_pixel_threshold: usize,
) {
    let _ = parallel_pixel_threshold;
    let output_stride = output_width as usize * channels;
    if output_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    if (output_width as usize).saturating_mul(output_height as usize) < parallel_pixel_threshold {
        for y in 0..output_height as usize {
            let output_start = y * output_stride;
            let row = &mut output[output_start..output_start + output_stride];
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                row[start..start + channels].copy_from_slice(&value[..channels]);
            }
        }
        return;
    }

    if should_transpose_vertical(source_rows, output_width, channels) {
        let transposed =
            transpose_resize_intermediate(intermediate, source_rows, output_width, channels);
        vertical_pass_rows_transposed(
            &transposed,
            source_rows,
            output_width,
            output_height,
            channels,
            coeffs,
            output,
        );
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y as usize,
                );
                let start = dx as usize * channels;
                row[start..start + channels].copy_from_slice(&value[..channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value = vertical_pass_col(
                intermediate,
                source_rows,
                dx,
                output_width,
                channels,
                coeffs,
                y,
            );
            let start = dx as usize * channels;
            row[start..start + channels].copy_from_slice(&value[..channels]);
        }
    }
}

fn vertical_pass_rows_transposed(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width as usize * channels;

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            for dx in 0..output_width {
                let value = vertical_pass_col_transposed(
                    intermediate,
                    source_rows,
                    dx,
                    channels,
                    coeffs,
                    y as usize,
                );
                let start = dx as usize * channels;
                row[start..start + channels].copy_from_slice(&value[..channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value =
                vertical_pass_col_transposed(intermediate, source_rows, dx, channels, coeffs, y);
            let start = dx as usize * channels;
            row[start..start + channels].copy_from_slice(&value[..channels]);
        }
    }
}

#[inline]
pub(crate) fn unpremultiply_channel(value: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        return value;
    }
    // Both inputs are bytes, so Pillow's f64 expression has an exact
    // integer numerator. Integer division preserves its truncation; clamp
    // because Rust's float-to-u8 cast in the reference path saturates.
    (u16::from(value) * 255 / u16::from(alpha)).min(255) as u8
}

fn vertical_pass_rows_alpha(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
    parallel_pixel_threshold: usize,
) {
    let _ = parallel_pixel_threshold;
    debug_assert!(matches!(channels, 2 | 4));
    let output_stride = output_width as usize * channels;
    let alpha_channel = channels - 1;
    if output_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    if (output_width as usize).saturating_mul(output_height as usize) < parallel_pixel_threshold {
        for y in 0..output_height as usize {
            let output_start = y * output_stride;
            let row = &mut output[output_start..output_start + output_stride];
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                let alpha = value[alpha_channel];
                for c in 0..channels {
                    row[start + c] = if c == alpha_channel {
                        alpha
                    } else {
                        unpremultiply_channel(value[c], alpha)
                    };
                }
            }
        }
        return;
    }

    if should_transpose_vertical(source_rows, output_width, channels) {
        let transposed =
            transpose_resize_intermediate(intermediate, source_rows, output_width, channels);
        vertical_pass_rows_alpha_transposed(
            &transposed,
            source_rows,
            output_width,
            output_height,
            channels,
            coeffs,
            output,
        );
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                let alpha = value[alpha_channel];
                for c in 0..channels {
                    row[start + c] = if c == alpha_channel {
                        alpha
                    } else {
                        unpremultiply_channel(value[c], alpha)
                    };
                }
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value = vertical_pass_col(
                intermediate,
                source_rows,
                dx,
                output_width,
                channels,
                coeffs,
                y,
            );
            let start = dx as usize * channels;
            let alpha = value[alpha_channel];
            for c in 0..channels {
                row[start + c] = if c == alpha_channel {
                    alpha
                } else {
                    unpremultiply_channel(value[c], alpha)
                };
            }
        }
    }
}

fn vertical_pass_rows_alpha_transposed(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    debug_assert!(matches!(channels, 2 | 4));
    let output_stride = output_width as usize * channels;
    let alpha_channel = channels - 1;

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            for dx in 0..output_width {
                let value = vertical_pass_col_transposed(
                    intermediate,
                    source_rows,
                    dx,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                let alpha = value[alpha_channel];
                for c in 0..channels {
                    row[start + c] = if c == alpha_channel {
                        alpha
                    } else {
                        unpremultiply_channel(value[c], alpha)
                    };
                }
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value =
                vertical_pass_col_transposed(intermediate, source_rows, dx, channels, coeffs, y);
            let start = dx as usize * channels;
            let alpha = value[alpha_channel];
            for c in 0..channels {
                row[start + c] = if c == alpha_channel {
                    alpha
                } else {
                    unpremultiply_channel(value[c], alpha)
                };
            }
        }
    }
}

/// Preserve the original image's color mode.
pub(crate) fn pil_preserve_mode(original: &DynamicImage, result: DynamicImage) -> DynamicImage {
    let orig_color = original.color();
    let res_color = result.color();
    if orig_color == res_color {
        return result;
    }
    match orig_color {
        crate::raster::ColorType::L8 => DynamicImage::ImageLuma8(result.to_luma8()),
        crate::raster::ColorType::La8 => DynamicImage::ImageLumaA8(result.to_luma_alpha8()),
        crate::raster::ColorType::Rgb8 => DynamicImage::ImageRgb8(result.to_rgb8()),
        crate::raster::ColorType::Rgba8 => DynamicImage::ImageRgba8(result.to_rgba8()),
        _ => result,
    }
}

/// PIL-compatible resize using two-pass separable interpolation.
///
/// PIL's approach:
/// 1. Precompute horizontal and vertical filter coefficients
/// 2. Horizontal pass: for each source row, compute each output column's
///    weighted sum, round to u8, and store in intermediate image
/// 3. Vertical pass: for each output column at each output row, compute
///    weighted sum from the intermediate rows, round to u8
///
/// For RGBA and LA modes, premultiplies alpha before resizing (matching PIL's
/// RGBa/La internal handling).
pub fn pil_resize(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> DynamicImage {
    // Handle identity
    if (dst_w, dst_h) == (img.width(), img.height()) {
        return img.clone();
    }
    // Handle empty
    if dst_w == 0 || dst_h == 0 || img.width() == 0 || img.height() == 0 {
        return DynamicImage::new_rgba8(dst_w, dst_h);
    }

    // Pillow keeps I;16* samples in the native 16-bit resize path.  The
    // generic pixel accessor below is byte-oriented and would otherwise
    // convert this mode to RGBA8 before preserving only its mode label.
    if let DynamicImage::ImageLuma16(luma) = img {
        return pil_resize_luma16(luma, dst_w, dst_h, filter, explicit_mode, None);
    }

    if let Some(byte_mode) = typed_color_resize_bytes(img) {
        return pil_resize(&byte_mode, dst_w, dst_h, filter, None);
    }

    // Retain original image for final mode preservation
    let orig_img = img;

    // CMYK/F/I stored as RGBA8 but 4th channel is NOT alpha (K/float/int byte).
    // RGBa is already premultiplied storage, so PIL resamples its channels
    // directly rather than premultiplying them a second time.
    // PIL does NOT premultiply alpha for these modes.
    let is_cmyk = explicit_mode == Some("CMYK");
    let is_fi = explicit_mode == Some("F") || explicit_mode == Some("I");
    // Nearest-neighbour resizing copies one source sample directly.  Pillow's
    // ImagingScaleAffine path does not premultiply LA/RGBA for that filter;
    // doing so here would round a constant (173, 127) or (17, 83, 149, 127)
    // down by one during the unnecessary premultiply/unpremultiply cycle.
    let needs_alpha = !matches!(filter, ResampleFilter::Nearest)
        && !is_cmyk
        && !is_fi
        && !matches!(explicit_mode, Some("RGBa" | "RGBX" | "La" | "PA"))
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );
    let (sw, sh) = (img.width(), img.height());
    let (dw, dh) = (dst_w, dst_h);

    // Determine channel count
    let channels = match img.color() {
        crate::raster::ColorType::L8 => 1usize,
        crate::raster::ColorType::La8 => 2usize,
        crate::raster::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };

    // Pillow's ImagingResample and ImagingScaleAffine both preserve an
    // all-zero native byte image exactly: every weighted sample is zero and
    // the destination has no edge or alpha work to perform.  The generic
    // two-pass loops still build and walk both coefficient tables, which was
    // the first CPU divergence in the small ImageOps.contain/cover rows.
    // Keep this bounded to byte-backed layouts; typed F/I paths have their
    // own representation-preserving fast paths below.
    let native_byte_image = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_image && img.as_bytes().iter().all(|&value| value == 0) {
        let output_len = (dw as usize)
            .checked_mul(dh as usize)
            .and_then(|pixels| pixels.checked_mul(channels))
            .unwrap_or(0);
        let result = raw_to_dynamic_owned(vec![0; output_len], dw, dh, channels);
        return pil_preserve_mode(orig_img, result);
    }

    // PIL's _resize C code uses ImagingTransform with AFFINE for NEAREST filter
    // (single-pixel sampling), NOT the two-pass pipeline. Box and all other filters
    // go through ImagingResample (two-pass convolution).
    // The AFFINE formula is:
    //   xin = a[0] * (x + 0.5) + a[2]   (a[2] = box[0] = 0)
    //   ix = (int)floor(xin)
    // A tiny epsilon is subtracted because the C code computes the scale factor
    // using float then double promotion, causing exact-integer boundaries to
    // nudge down by ~1e-15.
    if matches!(filter, ResampleFilter::Nearest) {
        // PIL's NEAREST resize uses ImagingScaleAffine with cumulative f64 stepping.
        // From _imaging.c for NEAREST filter:
        //   a[0] = (double)(box[2] - box[0]) / xsize   (= sw / dw)
        //   a[2] = box[0]                                (= 0)
        // Then: xo = a[2] + a[0] * 0.5
        //       for each x: xin = (int)(xo); xo += a[0]
        let scale_x = sw as f64 / dw as f64;
        let scale_y = sh as f64 / dh as f64;
        let n = (dw * dh) as usize;
        let mut out_bytes: Vec<u8> = Vec::with_capacity(n * channels);
        // Precompute x-mapping table matching PIL's xintab approach
        let mut xintab: Vec<u32> = Vec::with_capacity(dw as usize);
        let mut xo = scale_x * 0.5;
        for _dx in 0..dw {
            let xi = xo as u32;
            xintab.push(if xi >= sw { sw - 1 } else { xi });
            xo += scale_x;
        }

        // Native byte images already have the exact sample layout required by
        // Pillow's affine nearest path. Copy complete source pixels directly
        // instead of expanding each one through `pixel_at`'s four-channel
        // f64 representation and then pushing bytes individually. This keeps
        // the coordinate calculation identical while making the common
        // byte-image path bandwidth-bound.
        if matches!(
            img,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        ) {
            let source = img.as_bytes();
            let mut out_bytes = vec![0u8; n * channels];
            let source_stride = sw as usize * channels;
            let destination_stride = dw as usize * channels;
            let mut yo = scale_y * 0.5;
            for dy in 0..dh as usize {
                let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 } as usize;
                let source_row = sy * source_stride;
                let destination_row = dy * destination_stride;
                if channels == 1 {
                    let destination =
                        &mut out_bytes[destination_row..destination_row + destination_stride];
                    for (destination_pixel, &sx) in destination.iter_mut().zip(&xintab) {
                        *destination_pixel = source[source_row + sx as usize];
                    }
                } else {
                    for (dx, &sx) in xintab.iter().enumerate() {
                        let source_start = source_row + sx as usize * channels;
                        let destination_start = destination_row + dx * channels;
                        out_bytes[destination_start..destination_start + channels]
                            .copy_from_slice(&source[source_start..source_start + channels]);
                    }
                }
                yo += scale_y;
            }
            let result = raw_to_dynamic_owned(out_bytes, dw, dh, channels);
            return pil_preserve_mode(orig_img, result);
        }
        // PIL also uses cumulative stepping for y: yo = a[4] * 0.5
        let mut yo = scale_y * 0.5;
        for _dy in 0..dh {
            let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 };
            for dx in 0..dw {
                let sx = xintab[dx as usize];
                let p = pixel_at(img, sx, sy);
                for c in 0..channels {
                    // `pixel_at` exposes grayscale-alpha pixels as RGBA
                    // (`[luma, luma, luma, alpha]`), while the resize buffer
                    // keeps their native two-byte `[luma, alpha]` layout.
                    // Select the alpha lane explicitly so PA/LA nearest
                    // resize does not copy luma into the alpha sample.
                    let rgba_channel = if channels == 2 && c == 1 { 3 } else { c };
                    out_bytes.push(pil_round(p[rgba_channel]));
                }
            }
            yo += scale_y;
        }
        let result = raw_to_dynamic_owned(out_bytes, dw, dh, channels);
        return pil_preserve_mode(orig_img, result);
    }

    // Precompute horizontal and vertical coefficients for two-pass pipeline
    let h_coeffs = precompute_coeffs(dw, sw, filter);
    let v_coeffs = precompute_coeffs(dh, sh, filter);

    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("L")) && matches!(img, DynamicImage::ImageLuma8(_)) {
        if let Some(output) = pil_resize_luma_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, raw_to_dynamic_owned(output, dw, dh, 1));
        }
    }

    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("LA")) && matches!(img, DynamicImage::ImageLumaA8(_)) {
        if let Some(output) = pil_resize_la_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, raw_to_dynamic_owned(output, dw, dh, 2));
        }
    }

    #[cfg(not(feature = "parallel"))]
    if needs_alpha && matches!(img, DynamicImage::ImageRgba8(_)) {
        if let Some(output) = pil_resize_rgba_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, raw_to_dynamic_owned(output, dw, dh, 4));
        }
    }

    #[cfg(not(feature = "parallel"))]
    if explicit_mode == Some("HSV")
        && matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(filter, ResampleFilter::Bicubic)
        && resize_u8_coefficients_fit_i32(&h_coeffs)
        && resize_u8_coefficients_fit_i32(&v_coeffs)
    {
        if let Some(result) = pil_resize_hsv_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, result);
        }
    }

    #[cfg(not(feature = "parallel"))]
    if is_cmyk && matches!(img, DynamicImage::ImageRgba8(_)) {
        if let Some(result) = pil_resize_cmyk_i32(img, dw, dh, &h_coeffs, &v_coeffs) {
            return pil_preserve_mode(orig_img, result);
        }
    }

    // Allocate intermediate image (sh rows × dw columns × channels)
    let mut intermediate = vec![0u8; (sh * dw) as usize * channels];

    // Horizontal pass: each source row is independent and can be written into
    // its own intermediate row.
    if needs_alpha {
        horizontal_pass_rows_alpha(
            img.as_bytes(),
            sw,
            sh,
            channels,
            &h_coeffs,
            dw,
            &mut intermediate,
        );
    } else {
        horizontal_pass_rows(
            img.as_bytes(),
            sw,
            sh,
            channels,
            &h_coeffs,
            dw,
            &mut intermediate,
        );
    }

    // Allocate output image
    let mut out_bytes = vec![0u8; (dw * dh) as usize * channels];

    // Vertical output rows are also independent once the intermediate image
    // exists, so they use the same disjoint-row write helper.
    if needs_alpha {
        vertical_pass_rows_alpha(
            &intermediate,
            sh,
            dw,
            dh,
            channels,
            &v_coeffs,
            &mut out_bytes,
            0,
        );
    } else {
        vertical_pass_rows(
            &intermediate,
            sh,
            dw,
            dh,
            channels,
            &v_coeffs,
            &mut out_bytes,
            0,
        );
    }

    // Build DynamicImage from bytes
    let result = raw_to_dynamic_owned(out_bytes, dw, dh, channels);

    pil_preserve_mode(orig_img, result)
}

/// Resize RGBX bytes into an existing, full-width output window.
///
/// ImageOps.pad can use this when contain preserves the destination width and
/// adds only top/bottom borders. The exact Pillow coefficient tables and
/// horizontal/vertical kernels remain unchanged; only the temporary resized
/// output allocation is removed. RGBX byte three is data, so this route must
/// never enable alpha premultiplication.
pub(crate) fn pil_resize_rgbx_into_window(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> bool {
    let DynamicImage::ImageRgba8(_) = img else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest)
        || dst_w == 0
        || dst_h == 0
        || img.width() == 0
        || img.height() == 0
    {
        return false;
    }
    let Ok(source_dims) = crate::checked_dims::CheckedDims::new(img.width(), img.height(), 4)
    else {
        return false;
    };
    let Ok(output_dims) = crate::checked_dims::CheckedDims::new(dst_w, dst_h, 4) else {
        return false;
    };
    if img.as_bytes().len() != source_dims.total_bytes()
        || output.len() != output_dims.total_bytes()
    {
        return false;
    }
    let Ok(source_height) = usize::try_from(img.height()) else {
        return false;
    };
    let Ok(output_width) = usize::try_from(dst_w) else {
        return false;
    };
    let Some(intermediate_len) = source_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };

    let horizontal = precompute_coeffs(dst_w, img.width(), filter);
    let vertical = precompute_coeffs(dst_h, img.height(), filter);
    let mut intermediate = vec![0u8; intermediate_len];
    horizontal_pass_rows(
        img.as_bytes(),
        img.width(),
        img.height(),
        4,
        &horizontal,
        dst_w,
        &mut intermediate,
    );
    vertical_pass_rows(
        &intermediate,
        img.height(),
        dst_w,
        dst_h,
        4,
        &vertical,
        output,
        0,
    );
    true
}

fn f_boxed_nearest_axis_maps_identity(
    source_size: u32,
    output_size: u32,
    box_start: f64,
    box_end: f64,
) -> bool {
    if source_size == 0
        || source_size != output_size
        || !box_start.is_finite()
        || !box_end.is_finite()
    {
        return false;
    }
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(output_size);
    if !scale.is_finite() {
        return false;
    }
    let last = i64::from(source_size - 1);
    let mut coordinate = box_start + scale * 0.5;
    for expected in 0..output_size {
        let selected = (coordinate.floor() as i64).clamp(0, last) as u32;
        if selected != expected {
            return false;
        }
        coordinate += scale;
    }
    true
}

/// Resize an F-mode image through a fractional source box.
///
/// F samples are IEEE-754 values packed four bytes at a time. They must be
/// decoded before filtered resampling; treating the bytes as four independent
/// image channels is not Pillow-compatible. The intermediate remains f32,
/// while each separable accumulation follows Pillow's f64-kernel/f32-store
/// order.
fn pil_resize_f_boxed(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> DynamicImage {
    // F-mode bytes are stored in the four-byte carrier used by ImageRgba8,
    // but they are scalar float samples, not RGBA channels. Borrow those
    // sample words directly; converting through `to_rgba8()` clones the
    // entire source before the float working set is built.
    let source_bytes = img.as_bytes();
    let (source_width, source_height) = (img.width(), img.height());
    let output_len = (dst_w as usize)
        .checked_mul(dst_h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .unwrap_or(0);
    if dst_w == 0 || dst_h == 0 || source_width == 0 || source_height == 0 {
        return DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(dst_w, dst_h, vec![0; output_len])
                .unwrap_or_else(|| crate::raster::RgbaImage::new(dst_w, dst_h)),
        );
    }
    let box_left = box_left as f32 as f64;
    let box_top = box_top as f32 as f64;
    let box_right = box_right as f32 as f64;
    let box_bottom = box_bottom as f32 as f64;
    let need_horizontal = dst_w != source_width || box_left != 0.0 || box_right != dst_w as f64;
    let need_vertical = dst_h != source_height || box_top != 0.0 || box_bottom != dst_h as f64;

    if matches!(filter, ResampleFilter::Nearest) {
        if source_width == dst_w
            && source_height == dst_h
            && f_boxed_nearest_axis_maps_identity(source_width, dst_w, box_left, box_right)
            && f_boxed_nearest_axis_maps_identity(source_height, dst_h, box_top, box_bottom)
        {
            // The fractional box can still select the same scalar sample at
            // every destination coordinate. A single owned copy then has the
            // same bytes as the general gather path and avoids its per-pixel
            // coordinate and output loops.
            return img.clone();
        }
        let scale_x = (box_right as f32 - box_left as f32) as f64 / f64::from(dst_w);
        let scale_y = (box_bottom as f32 - box_top as f32) as f64 / f64::from(dst_h);
        let last_x = i64::from(source_width - 1);
        let last_y = i64::from(source_height - 1);
        let mut output = Vec::with_capacity(output_len);
        // Pillow's nearest boxed F resize also reaches ImagingScaleAffine,
        // whose coordinates advance cumulatively from one pixel to the next.
        // Keep that recurrence so exact upsample boundaries select the same
        // source word as the native path.
        let mut source_y = box_top + scale_y * 0.5;
        for _ in 0..dst_h {
            let source_y_index = source_y.floor() as i64;
            let source_y_index = source_y_index.clamp(0, last_y) as usize;
            let mut source_x = box_left + scale_x * 0.5;
            for _ in 0..dst_w {
                let source_x_index = source_x.floor() as i64;
                let source_x_index = source_x_index.clamp(0, last_x) as usize;
                let source_start = (source_y_index * source_width as usize + source_x_index) * 4;
                output.extend_from_slice(&source_bytes[source_start..source_start + 4]);
                source_x += scale_x;
            }
            source_y += scale_y;
        }
        return raw_to_dynamic_owned(output, dst_w, dst_h, 4);
    }

    // Filtered resampling requires numeric samples and keeps Pillow's
    // f64-accumulate/f32-store order. Decode only for this branch; nearest
    // resampling copies the original four-byte F sample words above.
    let source: Vec<f32> = source_bytes
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();
    let horizontal = precompute_coeffs_f64_boxed(dst_w, source_width, box_left, box_right, filter);
    let vertical = precompute_coeffs_f64_boxed(dst_h, source_height, box_top, box_bottom, filter);
    let mut intermediate = vec![0.0f32; source_height as usize * dst_w as usize];
    if need_horizontal {
        for source_y in 0..source_height as usize {
            let source_start = source_y * source_width as usize;
            let intermediate_start = source_y * dst_w as usize;
            for output_x in 0..dst_w as usize {
                let x0 = horizontal.xmin[output_x];
                let mut sum = 0.0;
                let vector_product_count = (horizontal.weights[output_x].len()
                    / F_RESIZE_VECTOR_WIDTH)
                    * F_RESIZE_VECTOR_WIDTH;
                for (tap, &weight) in horizontal.weights[output_x].iter().enumerate() {
                    let source_x = (x0 + tap as i64) as usize;
                    f_resize_accumulate(
                        &mut sum,
                        weight,
                        source[source_start + source_x],
                        tap < vector_product_count,
                    );
                }
                intermediate[intermediate_start + output_x] =
                    if sum == 0.0 { 0.0 } else { sum as f32 };
            }
        }
    } else {
        intermediate.copy_from_slice(&source);
    }

    let output_floats = if need_vertical {
        let mut output_floats = vec![0.0f32; dst_w as usize * dst_h as usize];
        for output_y in 0..dst_h as usize {
            let y0 = vertical.xmin[output_y];
            for output_x in 0..dst_w as usize {
                let mut sum = 0.0;
                for (tap, &weight) in vertical.weights[output_y].iter().enumerate() {
                    let source_y = (y0 + tap as i64) as usize;
                    sum = weight.mul_add(
                        f64::from(intermediate[source_y * dst_w as usize + output_x]),
                        sum,
                    );
                }
                output_floats[output_y * dst_w as usize + output_x] =
                    if sum == 0.0 { 0.0 } else { sum as f32 };
            }
        }
        output_floats
    } else {
        intermediate
    };
    let output: Vec<u8> = output_floats
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    raw_to_dynamic_owned(output, dst_w, dst_h, 4)
}

/// Box-based resize: maps source region [box_left, box_right] × [box_top, box_bottom]
/// to the output (dst_w, dst_h). All box coordinates are in source pixel coordinates.
pub fn pil_resize_boxed(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> DynamicImage {
    pil_resize_boxed_with_parallel_pixel_threshold(
        img,
        dst_w,
        dst_h,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
        explicit_mode,
        0,
    )
}

/// Box-based resize with an operation-specific cutoff for Rayon row dispatch.
/// A zero threshold preserves the general resize scheduler behavior.
pub(crate) fn pil_resize_boxed_with_parallel_pixel_threshold(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
    parallel_pixel_threshold: usize,
) -> DynamicImage {
    let orig_img = img;
    // Pillow narrows the source box to float32 before `_resize` sees it. Keep
    // those values for the crop and pass-elision decisions below so integer
    // boundaries do not drift in Rust's f64 control plane.
    let box_left_f32 = box_left as f32;
    let box_top_f32 = box_top as f32;
    let box_right_f32 = box_right as f32;
    let box_bottom_f32 = box_bottom as f32;
    let finite_box = box_left_f32.is_finite()
        && box_top_f32.is_finite()
        && box_right_f32.is_finite()
        && box_bottom_f32.is_finite();
    if finite_box
        && dst_w == img.width()
        && dst_h == img.height()
        && box_left_f32 == 0.0
        && box_top_f32 == 0.0
        && box_right_f32 == img.width() as f32
        && box_bottom_f32 == img.height() as f32
    {
        // Image.resize returns self.copy() before mode conversion for a
        // complete source at its original size, including F and alpha modes.
        return img.clone();
    }
    let is_cmyk = explicit_mode == Some("CMYK");
    let is_fi = explicit_mode == Some("F") || explicit_mode == Some("I");
    let needs_alpha = !matches!(filter, ResampleFilter::Nearest)
        && !is_cmyk
        && !is_fi
        && !matches!(explicit_mode, Some("RGBa" | "RGBX" | "La" | "PA"))
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );

    // `_imaging.c::_resize` crops an integer-aligned box whose dimensions
    // already equal the target before selecting the resampler. This ordering
    // also matters for F: sending an exact crop through the f64 filter tails
    // would let NaN/Inf values leak into taps that Pillow never evaluates.
    let integer_crop = finite_box
        && box_left_f32.fract() == 0.0
        && box_top_f32.fract() == 0.0
        && box_right_f32 - box_left_f32 == dst_w as f32
        && box_bottom_f32 - box_top_f32 == dst_h as f32;
    if integer_crop && !needs_alpha {
        let left = box_left_f32 as u32;
        let top = box_top_f32 as u32;
        return img.crop_imm(left, top, dst_w, dst_h);
    }

    if let DynamicImage::ImageLuma16(luma) = img {
        // A two-byte typed sample cannot enter the generic four-byte pixel
        // transport. Reuse the native u16 passes with boxed coefficients.
        return pil_resize_luma16(
            luma,
            dst_w,
            dst_h,
            filter,
            explicit_mode,
            Some((box_left, box_top, box_right, box_bottom)),
        );
    }

    if explicit_mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return pil_resize_f_boxed(
            img, dst_w, dst_h, box_left, box_top, box_right, box_bottom, filter,
        );
    }
    if let Some(byte_mode) = typed_color_resize_bytes(img) {
        return pil_resize_boxed(
            &byte_mode, dst_w, dst_h, box_left, box_top, box_right, box_bottom, filter, None,
        );
    }
    let (kernel_fn, support) = filter_from_resample(filter);
    let (sw, sh) = (img.width(), img.height());

    let channels = match img.color() {
        crate::raster::ColorType::L8 => 1usize,
        crate::raster::ColorType::La8 => 2usize,
        crate::raster::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };

    // A bounded all-zero byte source remains zero through every normalized
    // Pillow resampling filter, including alpha premultiplication. Boxed
    // resizes otherwise rebuild two coefficient tables and walk both passes
    // even though every tap and destination sample is zero.
    let native_byte_image = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    let source_pixels = (sw as usize).checked_mul(sh as usize);
    if native_byte_image
        && source_pixels.is_some_and(|pixels| {
            pixels != 0
                && pixels <= BOXED_ZERO_FAST_PATH_MAX_PIXELS
                && img.as_bytes().iter().all(|&value| value == 0)
        })
    {
        if let Some(output_len) = (dst_w as usize)
            .checked_mul(dst_h as usize)
            .and_then(|pixels| pixels.checked_mul(channels))
        {
            return pil_preserve_mode(
                orig_img,
                raw_to_dynamic_owned(vec![0; output_len], dst_w, dst_h, channels),
            );
        }
    }

    // Pillow's boxed nearest path is an affine sample, not a one-tap box
    // convolution. The convolution-style coefficient builder can include
    // adjacent samples at a boundary and produce a value that Pillow never
    // emits (most visibly for indexed ImageOps.fit, but the same source
    // selection applies to every byte layout). Use the exact
    // ``int(box_start + (x + 0.5) * scale)`` mapping from ImagingTransform.
    if matches!(filter, ResampleFilter::Nearest) {
        if sw == 0 || sh == 0 {
            return pil_preserve_mode(
                orig_img,
                raw_to_dynamic_owned(
                    vec![0; (dst_w as usize) * (dst_h as usize) * channels],
                    dst_w,
                    dst_h,
                    channels,
                ),
            );
        }
        // The native transform path receives the same float box record as the
        // resampler; keep its scale calculation at that boundary too.
        let box_left_f32 = box_left as f32;
        let box_top_f32 = box_top as f32;
        let box_right_f32 = box_right as f32;
        let box_bottom_f32 = box_bottom as f32;
        let box_left = box_left_f32 as f64;
        let box_top = box_top_f32 as f64;
        let scale_x = ((box_right_f32 - box_left_f32) as f64) / dst_w as f64;
        let scale_y = ((box_bottom_f32 - box_top_f32) as f64) / dst_h as f64;
        let source = img.as_bytes();
        let mut out_bytes = Vec::with_capacity((dst_w * dst_h) as usize * channels);
        // `_resize` takes this path through ImagingScaleAffine. Its source
        // coordinate is initialized once and advanced with `xo += a[0]` /
        // `yo += a[4]` for every output pixel/row. Preserve that cumulative
        // floating-point rounding instead of recomputing `(index + 0.5) *
        // scale`; the difference is observable exactly at upsample boundaries
        // such as a 2x1 box expanded to seven pixels.
        let mut source_y = box_top + scale_y * 0.5;
        for _ in 0..dst_h {
            let sy = source_y.floor().clamp(0.0, (sh - 1) as f64) as usize;
            let mut source_x = box_left + scale_x * 0.5;
            for _ in 0..dst_w {
                let sx = source_x.floor().clamp(0.0, (sw - 1) as f64) as usize;
                let start = (sy * sw as usize + sx) * channels;
                out_bytes.extend_from_slice(&source[start..start + channels]);
                source_x += scale_x;
            }
            source_y += scale_y;
        }
        return pil_preserve_mode(
            orig_img,
            raw_to_dynamic_owned(out_bytes, dst_w, dst_h, channels),
        );
    }

    // Use box-parameter coefficients for both passes
    let h_coeffs = precompute_coeffs_boxed(dst_w, sw, box_left, box_right, kernel_fn, support);
    let mut v_coeffs = precompute_coeffs_boxed(dst_h, sh, box_top, box_bottom, kernel_fn, support);

    // Resample.c skips a pass when that axis already has the requested size
    // and covers the complete source extent. A boxed one-axis crop therefore
    // must resample only the changed axis; the unchanged axis retains source
    // samples (or premultiplied samples for LA/RGBA).
    let need_horizontal = dst_w != sw || box_left_f32 != 0.0 || box_right_f32 != dst_w as f32;
    let need_vertical = dst_h != sh || box_top_f32 != 0.0 || box_bottom_f32 != dst_h as f32;

    // The horizontal pass is independent per source row. Avoid processing
    // vertical rows that the rebased output table cannot read, which is a
    // substantial win when a small fractional box is enlarged.
    let (first_source_row, intermediate_rows) = if need_vertical {
        compact_resize_coeffs_to_source_span(&mut v_coeffs, sh).unwrap_or((0, sh))
    } else {
        (0, sh)
    };

    // Allocate only rows referenced by the vertical table.
    let mut intermediate = vec![0u8; (intermediate_rows * dst_w) as usize * channels];

    // Horizontal pass: each source row writes one independent intermediate row.
    if !need_horizontal {
        let source_stride = sw as usize * channels;
        let source_start = first_source_row as usize * source_stride;
        let source_end = source_start + intermediate.len();
        let source_rows = &img.as_bytes()[source_start..source_end];
        if needs_alpha {
            for (source_pixel, intermediate_pixel) in source_rows
                .chunks_exact(channels)
                .zip(intermediate.chunks_exact_mut(channels))
            {
                let alpha = source_pixel[channels - 1];
                for channel in 0..channels {
                    intermediate_pixel[channel] = if channel == channels - 1 {
                        alpha
                    } else {
                        premultiply_channel(source_pixel[channel], alpha)
                    };
                }
            }
        } else {
            intermediate.copy_from_slice(source_rows);
        }
    } else {
        horizontal_pass_boxed_rows(
            img.as_bytes(),
            sw,
            first_source_row,
            intermediate_rows,
            channels,
            &h_coeffs,
            dst_w,
            &mut intermediate,
            needs_alpha,
            parallel_pixel_threshold,
        );
    }

    // Allocate output image
    let mut out_bytes = vec![0u8; (dst_w * dst_h) as usize * channels];

    // Vertical output rows are independent after the horizontal pass.
    if !need_vertical {
        if needs_alpha {
            for (intermediate_pixel, output_pixel) in intermediate
                .chunks_exact(channels)
                .zip(out_bytes.chunks_exact_mut(channels))
            {
                let alpha = intermediate_pixel[channels - 1];
                for channel in 0..channels {
                    output_pixel[channel] = if channel == channels - 1 {
                        alpha
                    } else {
                        unpremultiply_channel(intermediate_pixel[channel], alpha)
                    };
                }
            }
        } else {
            let output_len = out_bytes.len();
            out_bytes.copy_from_slice(&intermediate[..output_len]);
        }
    } else if needs_alpha {
        vertical_pass_rows_alpha(
            &intermediate,
            intermediate_rows,
            dst_w,
            dst_h,
            channels,
            &v_coeffs,
            &mut out_bytes,
            parallel_pixel_threshold,
        );
    } else {
        vertical_pass_rows(
            &intermediate,
            intermediate_rows,
            dst_w,
            dst_h,
            channels,
            &v_coeffs,
            &mut out_bytes,
            parallel_pixel_threshold,
        );
    }

    let result = raw_to_dynamic_owned(out_bytes, dst_w, dst_h, channels);

    pil_preserve_mode(orig_img, result)
}

/// Move a completed native-byte result into its image without allocating
/// and copying the entire output a second time.
fn raw_to_dynamic_owned(bytes: Vec<u8>, w: u32, h: u32, channels: usize) -> DynamicImage {
    match channels {
        1 => DynamicImage::ImageLuma8(
            crate::raster::GrayImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::GrayImage::new(w, h)),
        ),
        2 => DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::GrayAlphaImage::new(w, h)),
        ),
        3 => DynamicImage::ImageRgb8(
            crate::raster::RgbImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::RgbImage::new(w, h)),
        ),
        _ => DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::RgbaImage::new(w, h)),
        ),
    }
}

#[cfg(test)]
mod alpha_row_precompute_tests {
    use super::{
        horizontal_pass_row, horizontal_pass_row_alpha, precompute_coeffs, premultiply_alpha_row,
        unpremultiply_channel,
    };
    use crate::pipeline::ResampleFilter;

    #[test]
    fn premultiplying_each_source_pixel_once_matches_each_alpha_tap() {
        for channels in [2usize, 4] {
            for source_width in [1u32, 7, 16, 33] {
                let source = (0..source_width as usize * channels)
                    .map(|index| {
                        let pixel = index / channels;
                        let channel = index % channels;
                        if channel == channels - 1 {
                            match pixel % 4 {
                                0 => 0,
                                1 => 255,
                                _ => (pixel * 47 + 13) as u8,
                            }
                        } else {
                            (pixel * 83 + channel * 61 + 19) as u8
                        }
                    })
                    .collect::<Vec<_>>();

                for output_width in [1u32, 3, 9, 17, 45] {
                    for filter in [
                        ResampleFilter::Bilinear,
                        ResampleFilter::Bicubic,
                        ResampleFilter::Lanczos,
                    ] {
                        let coeffs = precompute_coeffs(output_width, source_width, filter);
                        let output_len = output_width as usize * channels;
                        let mut expected = vec![0; output_len];
                        horizontal_pass_row_alpha(
                            &source,
                            channels,
                            &coeffs,
                            output_width,
                            &mut expected,
                        );

                        let mut premultiplied = vec![0; source.len()];
                        premultiply_alpha_row(&source, channels, &mut premultiplied);
                        let mut actual = vec![0; output_len];
                        horizontal_pass_row(
                            &premultiplied,
                            source_width,
                            channels,
                            &coeffs,
                            output_width,
                            &mut actual,
                        );
                        assert_eq!(
                            actual, expected,
                            "channels={channels}, {source_width}x1 -> {output_width}x1, {filter:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn integer_unpremultiplication_matches_pillow_float_for_every_byte_pair() {
        for alpha in 0..=u8::MAX {
            for value in 0..=u8::MAX {
                let expected = if alpha == 0 {
                    value
                } else {
                    (f64::from(value) * 255.0 / f64::from(alpha)) as u8
                };
                assert_eq!(
                    unpremultiply_channel(value, alpha),
                    expected,
                    "value={value}, alpha={alpha}"
                );
            }
        }
    }
}

#[cfg(test)]
mod typed_nearest_tests {
    use super::{pil_resize, pixel_at};
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, ImageBuffer, Rgb, Rgba};

    fn typed_images() -> Vec<DynamicImage> {
        vec![
            DynamicImage::ImageLuma16(
                ImageBuffer::from_raw(3, 2, vec![0, 1, 32_768, 65_535, 257, 65_407])
                    .expect("L16 samples"),
            ),
            DynamicImage::ImageLumaA16(
                ImageBuffer::from_raw(
                    3,
                    2,
                    vec![
                        0, 65_535, 1, 32_768, 32_768, 1, 65_535, 0, 257, 65_407, 129, 65_406,
                    ],
                )
                .expect("LA16 samples"),
            ),
            DynamicImage::ImageRgb16(
                ImageBuffer::from_raw(
                    3,
                    2,
                    vec![
                        0, 1, 65_535, 129, 32_768, 32_767, 257, 65_406, 65_535, 1, 0, 32_768,
                        65_407, 257, 32_768, 32_767, 128, 1,
                    ],
                )
                .expect("RGB16 samples"),
            ),
            DynamicImage::ImageRgba16(
                ImageBuffer::from_raw(
                    3,
                    2,
                    vec![
                        0, 1, 65_535, 32_768, 129, 32_768, 32_767, 65_535, 257, 65_406, 65_535, 0,
                        65_535, 1, 0, 257, 65_407, 257, 32_768, 32_767, 128, 1, 65_407, 65_535,
                    ],
                )
                .expect("RGBA16 samples"),
            ),
            DynamicImage::ImageRgb32F(
                ImageBuffer::<Rgb<f32>, Vec<f32>>::from_raw(
                    3,
                    2,
                    vec![
                        0.0,
                        0.5,
                        1.0,
                        -1.0,
                        0.25,
                        2.0,
                        f32::NAN,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        0.1,
                        0.9,
                        1.1,
                        -0.0,
                        0.001,
                        0.999,
                        0.75,
                        0.33,
                        0.66,
                    ],
                )
                .expect("RGB32F samples"),
            ),
            DynamicImage::ImageRgba32F(
                ImageBuffer::<Rgba<f32>, Vec<f32>>::from_raw(
                    3,
                    2,
                    vec![
                        0.0,
                        0.5,
                        1.0,
                        1.0,
                        -1.0,
                        0.25,
                        2.0,
                        0.5,
                        f32::NAN,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        0.0,
                        0.1,
                        0.9,
                        1.1,
                        0.25,
                        -0.0,
                        0.001,
                        0.999,
                        0.75,
                        0.75,
                        0.33,
                        0.66,
                        0.5,
                    ],
                )
                .expect("RGBA32F samples"),
            ),
        ]
    }

    fn typed_public_byte_mode(image: &DynamicImage) -> DynamicImage {
        match image {
            DynamicImage::ImageLumaA16(_) => DynamicImage::ImageLumaA8(image.to_luma_alpha8()),
            DynamicImage::ImageRgb16(_) | DynamicImage::ImageRgb32F(_) => {
                DynamicImage::ImageRgb8(image.to_rgb8())
            }
            DynamicImage::ImageRgba16(_) | DynamicImage::ImageRgba32F(_) => {
                DynamicImage::ImageRgba8(image.to_rgba8())
            }
            _ => unreachable!("only multi-channel typed inputs use this conversion"),
        }
    }

    #[test]
    fn typed_nearest_resize_keeps_the_logical_byte_mode_layout() {
        let destination_width = 4;
        let destination_height = 3;
        for image in typed_images() {
            let rgba = image.to_rgba8();
            for y in 0..image.height() {
                for x in 0..image.width() {
                    let expected = rgba.get_pixel(x, y).0.map(f64::from);
                    assert_eq!(pixel_at(&image, x, y), expected);
                }
            }

            // L16 has its own native nearest path. The other typed layouts
            // exercise pixel_at from the generic affine-nearest fallback.
            if matches!(image, DynamicImage::ImageLuma16(_)) {
                continue;
            }
            let byte_mode = typed_public_byte_mode(&image);
            let expected = pil_resize(
                &byte_mode,
                destination_width,
                destination_height,
                ResampleFilter::Nearest,
                None,
            );
            let resized = pil_resize(
                &image,
                destination_width,
                destination_height,
                ResampleFilter::Nearest,
                None,
            );
            assert_eq!(resized.color(), expected.color());
            assert_eq!(resized.as_bytes(), expected.as_bytes());
        }
    }

    #[test]
    fn typed_filtered_resize_uses_the_logical_byte_mode_layout() {
        let destination = (4, 3);
        let box_bounds = (0.25, 0.5, 2.75, 1.5);
        for image in typed_images().into_iter().skip(1) {
            let byte_mode = typed_public_byte_mode(&image);
            let normal_expected = pil_resize(
                &byte_mode,
                destination.0,
                destination.1,
                ResampleFilter::Bicubic,
                None,
            );
            let normal_actual = pil_resize(
                &image,
                destination.0,
                destination.1,
                ResampleFilter::Bicubic,
                None,
            );
            assert_eq!(
                normal_actual.color(),
                normal_expected.color(),
                "standard resize layout for {:?}",
                image.color()
            );
            assert_eq!(
                normal_actual.as_bytes(),
                normal_expected.as_bytes(),
                "standard resize samples for {:?}",
                image.color()
            );

            let boxed_expected = super::pil_resize_boxed(
                &byte_mode,
                destination.0,
                destination.1,
                box_bounds.0,
                box_bounds.1,
                box_bounds.2,
                box_bounds.3,
                ResampleFilter::Bicubic,
                None,
            );
            let boxed_actual = super::pil_resize_boxed(
                &image,
                destination.0,
                destination.1,
                box_bounds.0,
                box_bounds.1,
                box_bounds.2,
                box_bounds.3,
                ResampleFilter::Bicubic,
                None,
            );
            assert_eq!(
                boxed_actual.color(),
                boxed_expected.color(),
                "boxed resize layout for {:?}",
                image.color()
            );
            assert_eq!(
                boxed_actual.as_bytes(),
                boxed_expected.as_bytes(),
                "boxed resize samples for {:?}",
                image.color()
            );
        }
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod narrow_u8_resize_tests {
    use super::{
        FilterCoeffs, pil_resize, pil_resize_la_i32, pil_resize_luma_i32, precompute_coeffs,
        resize_u8_coefficients_fit_i32,
    };
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage};

    #[test]
    fn hsv_narrow_cpu_resize_matches_wide_three_channel_resize() {
        for (width, height, output_width, output_height) in
            [(5, 3, 7, 5), (37, 17, 53, 23), (67, 29, 31, 41)]
        {
            let horizontal = precompute_coeffs(output_width, width, ResampleFilter::Bicubic);
            let vertical = precompute_coeffs(output_height, height, ResampleFilter::Bicubic);
            assert!(resize_u8_coefficients_fit_i32(&horizontal));
            assert!(resize_u8_coefficients_fit_i32(&vertical));

            for pattern in 0..4u32 {
                let bytes = (0..width * height * 3)
                    .map(|index| match pattern {
                        0 => 0,
                        1 => 255,
                        2 if (index + index / (width * 3)) % 2 == 0 => 0,
                        2 => 255,
                        _ => index
                            .wrapping_mul(97)
                            .wrapping_add(index / (width * 3) * 53)
                            as u8,
                    })
                    .collect::<Vec<_>>();
                let image = DynamicImage::ImageRgb8(
                    RgbImage::from_raw(width, height, bytes).expect("HSV test image shape"),
                );
                let hsv = pil_resize(
                    &image,
                    output_width,
                    output_height,
                    ResampleFilter::Bicubic,
                    Some("HSV"),
                );
                let wide = pil_resize(
                    &image,
                    output_width,
                    output_height,
                    ResampleFilter::Bicubic,
                    Some("RGB"),
                );
                assert!(matches!(hsv, DynamicImage::ImageRgb8(_)));
                assert_eq!(
                    hsv.as_bytes(),
                    wide.as_bytes(),
                    "{width}x{height} pattern {pattern}"
                );
            }
        }
    }

    #[test]
    fn hsv_i32_accumulator_guard_fails_closed_at_the_overflow_boundary() {
        let safe = FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![8_413_280],
        };
        let unsafe_sum = FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![8_413_281],
        };
        let unsafe_weight = FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![i64::MAX],
        };
        assert!(resize_u8_coefficients_fit_i32(&safe));
        assert!(!resize_u8_coefficients_fit_i32(&unsafe_sum));
        assert!(!resize_u8_coefficients_fit_i32(&unsafe_weight));
    }

    #[test]
    fn luma_narrow_cpu_resize_matches_wide_one_channel_reference() {
        for (width, height, output_width, output_height) in [
            (1, 7, 9, 3),
            (5, 3, 7, 5),
            (37, 17, 53, 23),
            (73, 41, 13, 9),
        ] {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = precompute_coeffs(output_width, width, filter);
                let vertical = precompute_coeffs(output_height, height, filter);
                assert!(resize_u8_coefficients_fit_i32(&horizontal));
                assert!(resize_u8_coefficients_fit_i32(&vertical));

                for pattern in 0..4u32 {
                    let bytes = (0..width * height)
                        .map(|index| match pattern {
                            0 => 0,
                            1 => 255,
                            2 if (index + index / width) % 2 == 0 => 0,
                            2 => 255,
                            _ => index
                                .wrapping_mul(73)
                                .wrapping_add(index / width * 19)
                                .wrapping_add(31) as u8,
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageLuma8(
                        GrayImage::from_raw(width, height, bytes).expect("L test image shape"),
                    );
                    let specialized = pil_resize_luma_i32(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    )
                    .expect("safe L coefficients select the narrow path");

                    // A different logical-mode hint leaves the L storage at
                    // one byte per sample but bypasses the L-only candidate.
                    let reference =
                        pil_resize(&image, output_width, output_height, filter, Some("RGB"));
                    assert_eq!(
                        specialized,
                        reference.as_bytes(),
                        "{width}x{height}->{output_width}x{output_height} {filter:?} pattern {pattern}"
                    );
                }
            }
        }
    }

    #[test]
    fn la_narrow_cpu_resize_matches_wide_two_channel_reference() {
        let small_shapes = [
            (1, 7, 9, 3),
            (5, 3, 7, 5),
            (37, 17, 53, 23),
            (73, 41, 13, 9),
        ];
        for (width, height, output_width, output_height) in small_shapes {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = precompute_coeffs(output_width, width, filter);
                let vertical = precompute_coeffs(output_height, height, filter);
                assert!(resize_u8_coefficients_fit_i32(&horizontal));
                assert!(resize_u8_coefficients_fit_i32(&vertical));

                for pattern in 0..4u32 {
                    let bytes = (0..width * height)
                        .flat_map(|index| {
                            let luma = index
                                .wrapping_mul(73)
                                .wrapping_add(index / width * 19)
                                .wrapping_add(31) as u8;
                            let alpha = match pattern {
                                0 => 0,
                                1 => 255,
                                2 if (index + index / width) % 2 == 0 => 0,
                                2 => 255,
                                _ => index
                                    .wrapping_mul(47)
                                    .wrapping_add(index / width * 61)
                                    .wrapping_add(7) as u8,
                            };
                            [luma, alpha]
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageLumaA8(
                        GrayAlphaImage::from_raw(width, height, bytes)
                            .expect("LA test image shape"),
                    );
                    let specialized = pil_resize_la_i32(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    )
                    .expect("safe LA coefficients select the narrow path");
                    let public = pil_resize(&image, output_width, output_height, filter, None);
                    let wide = pil_resize(&image, output_width, output_height, filter, Some("RGB"));
                    assert_eq!(specialized, wide.as_bytes());
                    assert_eq!(public.as_bytes(), wide.as_bytes());
                    assert!(matches!(public, DynamicImage::ImageLumaA8(_)));
                }
            }
        }

        // The generic path transposes at this size; the direct LA kernel
        // intentionally reads its row-major intermediate without that copy.
        let (width, height, output_width, output_height) = (641, 481, 853, 641);
        let bytes = (0u32..width * height)
            .flat_map(|index| {
                [
                    index.wrapping_mul(73).wrapping_add(23) as u8,
                    index.wrapping_mul(29).wrapping_add(11) as u8,
                ]
            })
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, bytes).expect("large LA test image shape"),
        );
        let filter = ResampleFilter::Bicubic;
        let horizontal = precompute_coeffs(output_width, width, filter);
        let vertical = precompute_coeffs(output_height, height, filter);
        let specialized =
            pil_resize_la_i32(&image, output_width, output_height, &horizontal, &vertical)
                .expect("safe LA coefficients select the narrow path");
        let wide = pil_resize(&image, output_width, output_height, filter, Some("RGB"));
        assert_eq!(specialized, wide.as_bytes());
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod rgba_i32_resize_tests {
    use super::{
        horizontal_pass_rows_alpha, pil_resize, pil_resize_rgba_i32, precompute_coeffs,
        resize_u8_coefficients_fit_i32, vertical_pass_rows_alpha,
    };
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, RgbaImage};

    fn wide_rgba_reference(
        image: &DynamicImage,
        output_width: u32,
        output_height: u32,
        horizontal: &super::FilterCoeffs,
        vertical: &super::FilterCoeffs,
    ) -> Vec<u8> {
        let source_height = image.height() as usize;
        let intermediate_stride = output_width as usize * 4;
        let mut intermediate = vec![0; source_height * intermediate_stride];
        horizontal_pass_rows_alpha(
            image.as_bytes(),
            image.width(),
            image.height(),
            4,
            horizontal,
            output_width,
            &mut intermediate,
        );
        let mut output = vec![0; output_height as usize * intermediate_stride];
        vertical_pass_rows_alpha(
            &intermediate,
            image.height(),
            output_width,
            output_height,
            4,
            vertical,
            &mut output,
            0,
        );
        output
    }

    #[test]
    fn rgba_narrow_cpu_resize_matches_wide_premultiplied_reference() {
        for (width, height, output_width, output_height) in
            [(5, 3, 7, 5), (37, 17, 53, 23), (73, 41, 13, 9)]
        {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = precompute_coeffs(output_width, width, filter);
                let vertical = precompute_coeffs(output_height, height, filter);
                assert!(resize_u8_coefficients_fit_i32(&horizontal));
                assert!(resize_u8_coefficients_fit_i32(&vertical));

                for pattern in 0..4u32 {
                    let bytes = (0..width * height)
                        .flat_map(|index| {
                            let alpha = match pattern {
                                0 => 0,
                                1 => 255,
                                2 if (index + index / width) % 2 == 0 => 0,
                                2 => 255,
                                _ => index
                                    .wrapping_mul(47)
                                    .wrapping_add(index / width * 61)
                                    .wrapping_add(7) as u8,
                            };
                            [
                                index.wrapping_mul(73).wrapping_add(31) as u8,
                                index.wrapping_mul(29).wrapping_add(19) as u8,
                                index.wrapping_mul(97).wrapping_add(11) as u8,
                                alpha,
                            ]
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageRgba8(
                        RgbaImage::from_raw(width, height, bytes).expect("RGBA test image shape"),
                    );
                    let specialized = pil_resize_rgba_i32(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    )
                    .expect("safe RGBA coefficients select the narrow path");
                    let reference = wide_rgba_reference(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    );
                    assert_eq!(specialized, reference);

                    let public = pil_resize(&image, output_width, output_height, filter, None);
                    assert_eq!(public.as_bytes(), reference);
                    assert!(matches!(public, DynamicImage::ImageRgba8(_)));
                }
            }
        }
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod cmyk_i32_resize_tests {
    use super::{
        horizontal_pass_rows, pil_resize, pil_resize_cmyk_i32, precompute_coeffs,
        resize_u8_coefficients_fit_i32, vertical_pass_rows,
    };
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, RgbaImage};

    fn wide_cmyk_reference(
        image: &DynamicImage,
        output_width: u32,
        output_height: u32,
        filter: ResampleFilter,
    ) -> Vec<u8> {
        let horizontal = precompute_coeffs(output_width, image.width(), filter);
        let vertical = precompute_coeffs(output_height, image.height(), filter);
        let intermediate_len = image.height() as usize * output_width as usize * 4;
        let mut intermediate = vec![0; intermediate_len];
        horizontal_pass_rows(
            image.as_bytes(),
            image.width(),
            image.height(),
            4,
            &horizontal,
            output_width,
            &mut intermediate,
        );
        let mut output = vec![0; output_width as usize * output_height as usize * 4];
        vertical_pass_rows(
            &intermediate,
            image.height(),
            output_width,
            output_height,
            4,
            &vertical,
            &mut output,
            0,
        );
        output
    }

    #[test]
    fn cmyk_narrow_resize_matches_wide_channel_arithmetic() {
        for (source_width, source_height, output_width, output_height) in
            [(7, 5, 11, 9), (11, 13, 5, 7), (1, 7, 9, 4), (9, 1, 4, 9)]
        {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = precompute_coeffs(output_width, source_width, filter);
                let vertical = precompute_coeffs(output_height, source_height, filter);
                assert!(resize_u8_coefficients_fit_i32(&horizontal));
                assert!(resize_u8_coefficients_fit_i32(&vertical));

                for pattern in 0..3u32 {
                    let samples = (0..source_width * source_height)
                        .flat_map(|pixel| {
                            let values = match pattern {
                                0 => [
                                    pixel.wrapping_mul(73).wrapping_add(19),
                                    pixel.wrapping_mul(41).wrapping_add(53),
                                    pixel.wrapping_mul(29).wrapping_add(97),
                                    pixel.wrapping_mul(11).wrapping_add(151),
                                ],
                                1 if pixel % 2 == 0 => [0; 4],
                                1 => [u32::MAX; 4],
                                _ => [0, u32::MAX, pixel * 17, pixel * 31],
                            };
                            values.map(|value| value as u8)
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageRgba8(
                        RgbaImage::from_raw(source_width, source_height, samples)
                            .expect("CMYK carrier dimensions are valid"),
                    );
                    let expected = wide_cmyk_reference(&image, output_width, output_height, filter);
                    let narrow = pil_resize_cmyk_i32(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    )
                    .expect("safe CMYK coefficients select the narrow path");
                    assert_eq!(narrow.as_bytes(), expected);
                    let actual =
                        pil_resize(&image, output_width, output_height, filter, Some("CMYK"));
                    assert_eq!(actual.as_bytes(), expected);
                    assert_eq!(actual.as_bytes(), narrow.as_bytes());
                    assert!(matches!(actual, DynamicImage::ImageRgba8(_)));
                }
            }
        }
    }

    #[test]
    fn cmyk_narrow_row_major_vertical_matches_transposed_wide_path() {
        let (source_width, source_height, output_width, output_height) =
            (1024u32, 768u32, 1365u32, 1024u32);
        let samples = (0..source_width * source_height)
            .flat_map(|pixel| {
                [
                    pixel.wrapping_mul(73).wrapping_add(19) as u8,
                    pixel.wrapping_mul(41).wrapping_add(53) as u8,
                    pixel.wrapping_mul(29).wrapping_add(97) as u8,
                    pixel.wrapping_mul(11).wrapping_add(151) as u8,
                ]
            })
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(source_width, source_height, samples)
                .expect("CMYK carrier dimensions are valid"),
        );
        let horizontal = precompute_coeffs(output_width, source_width, ResampleFilter::Bicubic);
        let vertical = precompute_coeffs(output_height, source_height, ResampleFilter::Bicubic);
        let expected =
            wide_cmyk_reference(&image, output_width, output_height, ResampleFilter::Bicubic);
        let actual =
            pil_resize_cmyk_i32(&image, output_width, output_height, &horizontal, &vertical)
                .expect("safe CMYK coefficients select the narrow path");
        assert_eq!(actual.as_bytes(), expected);
    }
}
