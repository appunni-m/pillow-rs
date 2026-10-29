use super::*;
use crate::raster::{DynamicImage, FromColor, Rgba};

const F_RESIZE_VECTOR_WIDTH: usize = 16;

pub(crate) trait F64MulAdd: Sync {
    fn mul_add(&self, weight: f64, sample: f64, accumulator: f64) -> f64;

    #[inline(always)]
    fn mul_add_finite(&self, weight: f64, sample: f64, accumulator: f64) -> f64 {
        self.mul_add(weight, sample, accumulator)
    }

    fn supports_finite_mul_add(&self) -> bool {
        false
    }
}

#[inline(always)]
fn f_resize_mul_add<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    fma: &F,
    weight: f64,
    sample: f64,
    accumulator: f64,
) -> f64 {
    if FINITE_OPERANDS {
        fma.mul_add_finite(weight, sample, accumulator)
    } else {
        fma.mul_add(weight, sample, accumulator)
    }
}

pub(crate) fn f_resize_samples_allow_finite_fma<F: F64MulAdd>(fma: &F, samples: &[f32]) -> bool {
    fma.supports_finite_mul_add() && samples.iter().all(|sample| sample.is_finite())
}

/// Check that every input to a finite-only FMA row is finite and that each
/// ordered f64 accumulator stays well within the finite range. F-mode samples
/// are f32, so a row's sum of absolute weighted sample bounds proves every
/// intermediate f64 accumulator finite.
pub(crate) fn f_resize_coefficients_allow_finite_fma(coefficients: &FilterCoeffsF64) -> bool {
    let max_accumulator = f64::MAX * 0.5;
    let max_sample = f64::from(f32::MAX);
    coefficients.weights.iter().all(|row| {
        let mut absolute_sum = 0.0f64;
        for &weight in row {
            if !weight.is_finite() {
                return false;
            }
            absolute_sum += weight.abs() * max_sample;
            if !absolute_sum.is_finite() || absolute_sum >= max_accumulator {
                return false;
            }
        }
        true
    })
}

pub(crate) struct PortableFma;

impl F64MulAdd for PortableFma {
    #[inline(always)]
    fn mul_add(&self, weight: f64, sample: f64, accumulator: f64) -> f64 {
        weight.mul_add(sample, accumulator)
    }
}

#[cfg(target_arch = "x86_64")]
pub(crate) struct X86FmaToken {
    _private: (),
}

#[cfg(target_arch = "x86_64")]
impl X86FmaToken {
    #[cfg(test)]
    pub(crate) fn detect() -> Option<Self> {
        std::is_x86_feature_detected!("fma").then_some(Self { _private: () })
    }

    pub(crate) fn from_context(context: &crate::compute::ExecutionContext) -> Option<Self> {
        context.uses_cpu_fma().then_some(Self { _private: () })
    }

    #[allow(unsafe_code)]
    #[inline(always)]
    fn mul_add_finite_operands(weight: f64, sample: f64, accumulator: f64) -> f64 {
        let mut result = accumulator;
        // SAFETY: callers have an `X86FmaToken` created by runtime feature
        // detection, so the current CPU supports the scalar FMA instruction.
        unsafe {
            std::arch::asm!(
                "vfmadd231sd {result}, {weight}, {sample}",
                result = inout(xmm_reg) result,
                weight = in(xmm_reg) weight,
                sample = in(xmm_reg) sample,
                options(pure, nomem, nostack, preserves_flags),
            );
        }
        result
    }
}

#[cfg(target_arch = "x86_64")]
impl F64MulAdd for X86FmaToken {
    #[allow(unsafe_code)]
    #[inline(always)]
    fn mul_add(&self, weight: f64, sample: f64, accumulator: f64) -> f64 {
        // libm and the hardware instruction can choose different NaN payload
        // bits for non-finite arithmetic. Preserve the portable reference's
        // exact float representation for those edge cases.
        if !weight.is_finite() || !sample.is_finite() || !accumulator.is_finite() {
            return weight.mul_add(sample, accumulator);
        }
        Self::mul_add_finite_operands(weight, sample, accumulator)
    }

    #[inline(always)]
    fn mul_add_finite(&self, weight: f64, sample: f64, accumulator: f64) -> f64 {
        Self::mul_add_finite_operands(weight, sample, accumulator)
    }

    #[inline(always)]
    fn supports_finite_mul_add(&self) -> bool {
        true
    }
}

#[inline(always)]
fn f_resize_accumulate<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    accumulator: &mut f64,
    weight: f64,
    sample: f32,
    separate_product_add: bool,
    fma: &F,
) {
    let sample = f64::from(sample);
    if separate_product_add {
        // Keep the product out of the following add. LLVM may otherwise
        // contract this expression back into an FMA, defeating the arm64
        // wide-row contract that Pillow uses after 15 taps.
        let product = std::hint::black_box(weight * sample);
        *accumulator += product;
    } else {
        *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(fma, weight, sample, *accumulator);
    }
}

#[inline(always)]
pub(super) fn f_resize_accumulate_boxed_row<
    const FINITE_OPERANDS: bool,
    F: F64MulAdd,
    const UNROLL_EIGHT: bool,
>(
    accumulator: &mut f64,
    source: &[f32],
    source_row_start: usize,
    source_width: usize,
    source_x: i64,
    weights: &[f64],
    fma: &F,
) {
    if UNROLL_EIGHT && weights.len() == 8 {
        let source_start = usize::try_from(source_x)
            .ok()
            .and_then(|x| source_row_start.checked_add(x));
        let source_end = source_start.and_then(|start| start.checked_add(8));
        if let (Some(start), Some(end)) = (source_start, source_end)
            && end <= source_row_start.saturating_add(source_width)
            && let Some(samples) = source.get(start..end)
        {
            *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weights[0],
                f64::from(samples[0]),
                *accumulator,
            );
            *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weights[1],
                f64::from(samples[1]),
                *accumulator,
            );
            *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weights[2],
                f64::from(samples[2]),
                *accumulator,
            );
            *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weights[3],
                f64::from(samples[3]),
                *accumulator,
            );
            *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weights[4],
                f64::from(samples[4]),
                *accumulator,
            );
            *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weights[5],
                f64::from(samples[5]),
                *accumulator,
            );
            *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weights[6],
                f64::from(samples[6]),
                *accumulator,
            );
            *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weights[7],
                f64::from(samples[7]),
                *accumulator,
            );
            return;
        }
    }

    let vector_product_count = (weights.len() / F_RESIZE_VECTOR_WIDTH) * F_RESIZE_VECTOR_WIDTH;
    for (tap, &weight) in weights.iter().enumerate() {
        let source_x = (source_x + tap as i64) as usize;
        f_resize_accumulate::<FINITE_OPERANDS, _>(
            accumulator,
            weight,
            source[source_row_start + source_x],
            tap < vector_product_count,
            fma,
        );
    }
}

pub(super) fn f_resize_boxed_horizontal_pass<
    const FINITE_OPERANDS: bool,
    F: F64MulAdd,
    const UNROLL_EIGHT: bool,
>(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    coefficients: &FilterCoeffsF64,
    intermediate: &mut [f32],
    fma: &F,
) {
    for source_y in 0..source_height {
        let source_start = source_y * source_width;
        let intermediate_start = source_y * output_width;
        for output_x in 0..output_width {
            let mut sum = 0.0;
            f_resize_accumulate_boxed_row::<FINITE_OPERANDS, _, UNROLL_EIGHT>(
                &mut sum,
                source,
                source_start,
                source_width,
                coefficients.xmin[output_x],
                &coefficients.weights[output_x],
                fma,
            );
            intermediate[intermediate_start + output_x] = if sum == 0.0 { 0.0 } else { sum as f32 };
        }
    }
}

pub(super) fn f_resize_boxed_vertical_pass<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    intermediate: &[f32],
    output_width: usize,
    output_height: usize,
    output_len: usize,
    coefficients: &FilterCoeffsF64,
    fma: &F,
) -> Vec<u8> {
    let mut output = Vec::with_capacity(output_len);
    let mut sums = vec![0.0f64; output_width];
    for output_y in 0..output_height {
        let y0 = coefficients.xmin[output_y];
        sums.fill(0.0);
        // Keep each column's tap accumulation ordered like Pillow while
        // visiting each source row contiguously across the output width.
        for (tap, &weight) in coefficients.weights[output_y].iter().enumerate() {
            let source_y = (y0 + tap as i64) as usize;
            let source_row = source_y * output_width;
            for output_x in 0..output_width {
                sums[output_x] = f_resize_mul_add::<FINITE_OPERANDS, _>(
                    fma,
                    weight,
                    f64::from(intermediate[source_row + output_x]),
                    sums[output_x],
                );
            }
        }
        for &sum in &sums {
            let value = if sum == 0.0 { 0.0 } else { sum as f32 };
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
    output
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
pub(super) fn pixel_at(img: &DynamicImage, x: u32, y: u32) -> [f64; 4] {
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
pub(super) fn typed_color_resize_bytes(img: &DynamicImage) -> Option<DynamicImage> {
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
