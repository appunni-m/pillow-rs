//! GPU admission contracts and exact scalar parameter proofs.

use super::{registry, variant_key};
use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp, PixelMode};

/// Returns whether the GPU backend has an implementation for `op`.
#[cfg(feature = "gpu")]
pub fn gpu_supports(op: &PipelineOp) -> Result<bool, PilError> {
    // Merge aliases share a raster storage type with canonical modes, but
    // their target-specific tags are not all representable by the shader
    // output contract. LAB additionally needs an A/B +128 byte encoding, and
    // scalar/P targets need non-byte output handling; route those cases to
    // the exact CPU implementation before device work begins.
    if let PipelineOp::Merge { logical_mode, .. } = op
        && matches!(
            logical_mode.as_str(),
            "LAB" | "P" | "1" | "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N"
        )
    {
        return Ok(false);
    }
    // Filter exactness depends on the logical sample contract.  The GPU pool
    // performs the final image-aware check after lazy materialization, so let
    // contextual filters cross this operation-only registry gate even when
    // their byte-mode kernel is not admitted by the fixed-point proof.
    let contextual_filter = matches!(
        op,
        PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
    );
    if !gpu_shader_contract_is_supported(op) && !contextual_filter {
        return Ok(false);
    }
    Ok(registry()?
        .get(variant_key(op))
        .is_some_and(|e| e.gpu_shader.is_some()))
}

/// Return the fixed-point brightness parameter when the integer WGSL kernel
/// is bit-exact for every possible byte sample.
///
/// Pillow's byte implementation evaluates `sample * factor` as f64 and
/// truncates only after clamping. The shader uses `sample * factor_int / 1000`.
/// Checking all 256 samples here keeps the preflight scalar while allowing
/// arbitrary public factors whose quantized representation has the same byte
/// result. Values that would overflow the shader's u32 product remain on the
/// CPU path.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_brightness_factor_int(factor: f64) -> Option<u32> {
    if !factor.is_finite() {
        return None;
    }
    if factor <= 0.0 {
        return Some(0);
    }

    // The WGSL kernel multiplies a byte by this integer before dividing by
    // 1000. Keep the product within u32 for every byte value.
    let max_factor = f64::from(u32::MAX) / (255.0 * 1000.0);
    if factor > max_factor {
        return None;
    }
    let factor_int = (factor * 1000.0) as u32;
    for sample in 0..=255_u32 {
        let cpu = (f64::from(sample) * factor).clamp(0.0, 255.0) as u32;
        let shader = (sample * factor_int / 1000).min(255);
        if cpu != shader {
            return None;
        }
    }
    Some(factor_int)
}

/// Color and module blend use the same fused float32 byte arithmetic.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_color_saturation_factor_params(factor: f64) -> Option<u32> {
    gpu_blend_alpha_params(factor)
}

/// Return the fixed-point factor when the Sharpness WGSL blend is exact for
/// every possible blurred/original byte pair. The 3x3 smooth kernel is
/// represented with integer weights in the shader; this helper proves the
/// Pillow-compatible f32 fused-difference blend against its integer numerator
/// before dispatch.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_sharpness_factor_int(factor: f64) -> Option<u32> {
    if !factor.is_finite() || factor < 0.0 {
        return None;
    }
    let scaled = factor * 1000.0;
    if !scaled.is_finite() || scaled < 0.0 || scaled > f64::from(i32::MAX) {
        return None;
    }
    let factor_int = scaled as u32;
    if f64::from(factor_int) != scaled {
        return None;
    }
    // The WGSL numerator is `blurred * (1000 - factor) + original * factor`.
    // Bound both terms before admitting the operation so i32 arithmetic stays
    // defined for every byte value, including unsharp factors above 1.
    let max_factor = (i32::MAX as u32) / 255;
    if factor_int > max_factor || factor_int.saturating_add(1000) > max_factor {
        return None;
    }
    let factor_i32 = factor_int as i32;
    let alpha = factor as f32;
    for blurred in 0..=255_i32 {
        for original in 0..=255_i32 {
            let cpu = alpha
                .mul_add((original - blurred) as f32, blurred as f32)
                .clamp(0.0, 255.0) as u8;
            let shader = (blurred * (1000 - factor_i32) + original * factor_i32)
                .div_euclid(1000)
                .clamp(0, 255) as u8;
            if cpu != shader {
                return None;
            }
        }
    }
    Some(factor_int)
}

/// Convert Pillow's f32 offset to the integer representation used by WGSL.
///
/// The shader ABI carries this value as an i32. Fractional, non-finite, and
/// out-of-range offsets therefore stay on the host path rather than being
/// silently truncated during parameter packing.
#[cfg(feature = "gpu")]
pub(crate) fn filter_offset_i32(offset: f32) -> Option<i32> {
    if !offset.is_finite() || offset.fract() != 0.0 {
        return None;
    }
    let offset_f64 = f64::from(offset);
    if offset_f64 < f64::from(i32::MIN) || offset_f64 > f64::from(i32::MAX) {
        return None;
    }
    Some(offset as i32)
}

/// Return whether the convolution WGSL path is safe for a finite normalized
/// kernel and an integer-representable offset.
///
/// Pillow evaluates the normalized coefficients and each row in `f32`, with
/// the middle product followed by fused multiply-adds. The WGSL byte path now
/// uses the same contraction order. The scalar preflight therefore needs to
/// prove only that all normalized coefficients and the full bounded byte
/// accumulation remain finite; the shader receives the host-computed `f32`
/// coefficient bits verbatim and performs the same operations on every byte
/// channel. This admits non-dyadic public kernels without substituting a
/// different fixed-point rounding contract.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_filter_kernel_is_exact(kernel: &[f32], scale: f32, offset: f32) -> bool {
    let Some(offset) = filter_offset_i32(offset) else {
        return false;
    };
    if !scale.is_finite() || scale == 0.0 {
        return false;
    }
    let denominator = scale;
    let normalized: Vec<f32> = kernel
        .iter()
        .map(|coefficient| *coefficient / denominator)
        .collect();
    if normalized
        .iter()
        .any(|coefficient| !coefficient.is_finite())
    {
        return false;
    }

    let coefficient_abs_sum: f64 = normalized
        .iter()
        .map(|coefficient| f64::from(coefficient.abs()))
        .sum();
    let maximum_abs = coefficient_abs_sum * 255.0 + f64::from(offset.abs()) + 0.5;
    maximum_abs.is_finite() && maximum_abs <= f64::from(f32::MAX)
}

/// Return an integer denominator for a convolution whose normalized f32
/// coefficients are exactly representable as small rational values.
///
/// The byte WGSL path can then evaluate the public truncating conversion with
/// integer arithmetic instead of allowing f32 accumulation to cross an
/// integer boundary.  The denominator is deliberately bounded: this helper
/// is a capability proof for the real shader contract, not a general-purpose
/// rational approximation of arbitrary user kernels.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_filter_rational_denominator(
    kernel: &[f32],
    scale: f32,
    offset: f32,
) -> Option<u32> {
    let offset = filter_offset_i32(offset)?;
    if !scale.is_finite() || scale == 0.0 {
        return None;
    }
    let normalized: Vec<f32> = kernel
        .iter()
        .map(|coefficient| *coefficient / scale)
        .collect();
    if normalized
        .iter()
        .any(|coefficient| !coefficient.is_finite())
    {
        return None;
    }

    for denominator in 1..=512_u32 {
        let denominator_f32 = denominator as f32;
        let mut coefficient_abs_sum = 0_i64;
        let mut exact = true;
        for coefficient in &normalized {
            let scaled = *coefficient * denominator_f32;
            let numerator = scaled.round();
            // Requiring the reconstruction to have the same f32 value keeps
            // the shader's integer numerator tied to the host's normalized
            // coefficient rather than accepting a loose approximation.
            if !scaled.is_finite()
                || (numerator / denominator_f32).to_bits() != coefficient.to_bits()
            {
                exact = false;
                break;
            }
            let Ok(numerator) = i64::try_from(numerator as i128) else {
                exact = false;
                break;
            };
            coefficient_abs_sum = coefficient_abs_sum.checked_add(numerator.abs())?;
        }
        if !exact {
            continue;
        }

        // The shader forms 2 * (sum + offset * denominator) + denominator
        // before truncating. Keep that intermediate inside signed i32.
        let maximum_abs = coefficient_abs_sum
            .checked_mul(255)?
            .checked_add(
                i64::from(offset)
                    .abs()
                    .checked_mul(i64::from(denominator))?,
            )?
            .checked_mul(2)?
            .checked_add(i64::from(denominator))?;
        if maximum_abs <= i64::from(i32::MAX) {
            return Some(denominator);
        }
    }
    None
}

/// The GPU applies a parameter-only lookup table, so zero and subnormal
/// divisors do not require device floating-point division or CPU pixel work.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_chops_affine_supported(scale: f64, offset: f64) -> bool {
    scale.is_finite() && offset.is_finite()
}

/// Pack the complete affine numerator domain into 512 bytes of uniforms.
/// Add has sums 0..510; Subtract has differences -255..255. The last byte is
/// padding. This table depends only on scale/offset, never either image.
/// Host f32 division preserves Pillow rounding even when device division uses
/// an approximate reciprocal or flushes a subnormal divisor to zero.
#[cfg(feature = "gpu")]
pub(super) fn gpu_chops_affine_lut(scale: f64, offset: f64, subtract: bool) -> Vec<u32> {
    let scale = scale as f32;
    let offset = offset as f32;
    let mut packed = vec![0u32; 128];
    for index in 0..511 {
        let numerator = index as f32 - if subtract { 255.0 } else { 0.0 };
        let value = (numerator / scale + offset).clamp(0.0, 255.0) as u8;
        packed[index / 4] |= u32::from(value) << ((index % 4) * 8);
    }
    packed
}

/// Return the finite float alpha used by Pillow's ImagingBlend and the WGSL
/// fused interpolation. Matching their precision and evaluation order avoids
/// rescanning the complete byte-pair domain on every admission check.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_blend_alpha_params(alpha: f64) -> Option<u32> {
    if !alpha.is_finite() {
        return None;
    }
    let alpha_f32 = alpha as f32;
    if !alpha_f32.is_finite() {
        return None;
    }

    Some(alpha_f32.to_bits())
}

/// Returns whether the registered single-dispatch shader actually represents
/// the full operation contract.
///
/// A registry entry may retain its WGSL source for inspection, compilation,
/// and future multi-pass work while routing an operation to the universal CPU
/// implementation until the shader consumes every public parameter and
/// preserves the requested output mode. This distinction keeps GPU routing
/// honest without deleting reviewed shader assets or changing coverage scope.
#[cfg(feature = "gpu")]
fn gpu_geometry_shader_contract(op: &PipelineOp) -> Option<bool> {
    let supported = match op {
        // The bundled LCMS CLUT is uploaded as a read-only auxiliary buffer;
        // the shader reproduces its fixed-point tetrahedral interpolation.
        PipelineOp::ConvertLab { .. } => true,
        // ResizeBoxed is lowered to the exact separable resize shaders after
        // its float32 bounds have been converted into host-built coefficient
        // tables; the device never approximates the source box.
        PipelineOp::ResizeBoxed { .. } => true,
        // NEAREST is a single-dispatch relocation. The other filters expand
        // into the pool's exact horizontal/vertical fixed-point kernels; the
        // registry entry remains the public operation marker while the pool
        // resolves those filters to their private two-pass pipelines.
        PipelineOp::Resize { .. } => true,
        // Thumbnail lowers to the exact Resize plan in the GPU executor.
        // Fit retains a dedicated shader because its crop box can be
        // fractional; both operations are admitted after image-aware
        // geometry validation in the executor.
        PipelineOp::Thumbnail { .. } | PipelineOp::Fit { .. } => true,
        // Contain and Cover are scalar geometry wrappers around the exact
        // Resize contract. The GPU pool computes Pillow's aspect-ratio size
        // before replacing them with the internal resize plan; retaining the
        // public entries here lets valid byte-mode calls reach that plan while
        // invalid/typed cases still fail preflight before device work.
        PipelineOp::Contain { .. } | PipelineOp::Cover { .. } => true,
        // Pad is resolved by the GPU pool into the exact separable resize
        // followed by a device-side fill/copy placement pass. The public
        // registry entry remains the capability marker while the pool owns
        // the multi-dispatch implementation.
        PipelineOp::Pad { .. } => true,
        // Scale is normalized by the GPU pool to the exact Resize plan after
        // Pillow's ties-to-even output dimensions have been computed. The
        // retained public shader is not used for this path; admitting the
        // operation here lets valid byte-mode Scale nodes reach the same
        // nearest or fixed-point convolution kernels as Resize.
        PipelineOp::Scale { .. } => true,
        // Rotate lowers to the exact affine Transform/Transpose plans in the
        // GPU executor. Keep the public registry entry as the capability
        // marker so strict routing can validate the original operation before
        // lowering.
        PipelineOp::Rotate { .. } => true,
        // Affine Transform uses the scalar pixel-center/parameter decision
        // from the host and a mode-aware nearest/bilinear shader data path.
        // Perspective, Quad, Mesh, and unsupported filters are rejected by
        // the pool's image-aware preflight before this entry is dispatched.
        PipelineOp::Transform { .. } => true,
        // Reduce is implemented by the WGSL shader with Pillow's truncated
        // fixed-point reciprocal and premultiplied-alpha path, including
        // partial right/bottom blocks. Identity factors are handled as an
        // explicit zero-dispatch no-op by the GPU executor.
        PipelineOp::Reduce { .. } => true,
        _ => return None,
    };
    Some(supported)
}

fn gpu_pixel_shader_contract(op: &PipelineOp) -> Option<bool> {
    let supported = match op {
        // The current shader implements the byte/scalar targets whose output
        // is representable by the packed transport. It has no matrix or
        // dithering parameter path. Non-standard byte/scalar targets retain
        // their Pillow storage shape at this executor boundary.
        PipelineOp::Convert {
            mode,
            matrix,
            dither,
        } => {
            matrix.is_none()
                && dither.is_none()
                && matches!(
                    mode,
                    ColorMode::L
                        | ColorMode::LA
                        | ColorMode::RGB
                        | ColorMode::RGBA
                        | ColorMode::CMYK
                        | ColorMode::YCbCr
                        | ColorMode::HSV
                        | ColorMode::I
                        | ColorMode::F
                        | ColorMode::I16
                        | ColorMode::LaPremultiplied
                        | ColorMode::RgbPremultiplied
                        | ColorMode::Rgbx
                )
        }
        // Autocontrast and Equalize are admitted here because the GPU pool
        // expands them into their histogram/control/LUT/remap pass sequence.
        // The operation-level registry cannot inspect the source image, so
        // mode, mask, and dimensions remain contextual preflight checks.
        PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. } => true,
        // Contrast receives Pillow's image-wide midpoint from scalar control
        // code and performs the channel blend in the real WGSL data path.
        PipelineOp::Contrast { factor } => gpu_blend_alpha_params(*factor).is_some(),
        // Brightness uses a scalar-verified fixed-point representation. The
        // helper admits every factor whose 256 byte results match Pillow,
        // rather than restricting the real vector kernel to endpoints.
        PipelineOp::Brightness { factor } => gpu_brightness_factor_int(*factor).is_some(),
        // Color saturation uses an integer luma anchor followed by the same
        // fused float32 byte blend as the module operation.
        PipelineOp::ColorSaturation { factor } => {
            gpu_color_saturation_factor_params(*factor).is_some()
        }
        // Colorize uses the same integer floor-division LUT construction as
        // Pillow's ImageOps._lut path. The GPU packs that exact mapping into a
        // per-sample RGB lookup table before the image dispatch.
        PipelineOp::Colorize {
            mid,
            blackpoint,
            midpoint,
            whitepoint,
            ..
        } => {
            if mid.is_some() {
                blackpoint <= midpoint && midpoint <= whitepoint
            } else {
                blackpoint <= whitepoint
            }
        }
        // Sharpness uses integer 3x3 smooth weights and a scalar-proven
        // fixed-point blend in the active WGSL implementation.
        PipelineOp::Sharpness { factor } => gpu_sharpness_factor_int(*factor).is_some(),
        _ => return None,
    };
    Some(supported)
}

fn gpu_filters_shader_contract(op: &PipelineOp) -> Option<bool> {
    let supported = match op {
        // The convolution shaders are exact for bounded kernels whose
        // normalized coefficients are integers: byte products and sums stay
        // exactly representable in f32, including Pillow's +0.5 bias. The
        // scalar helper rejects fractional coefficients and large accumulators
        // until they have a backend-independent fixed-point contract.
        PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } => {
            gpu_filter_kernel_is_exact(kernel, *scale, *offset)
                || gpu_filter_rational_denominator(kernel, *scale, *offset).is_some()
        }
        PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } => {
            gpu_filter_kernel_is_exact(kernel, *scale, *offset)
                || gpu_filter_rational_denominator(kernel, *scale, *offset).is_some()
        }
        // The horizontal/vertical rolling shaders carry independent fixed-
        // point parameters, so tuple and fractional radii use the same real
        // GPU data path as the uniform integer form.
        PipelineOp::BoxBlurXY { .. } => true,
        _ => return None,
    };
    Some(supported)
}

fn gpu_chops_shader_contract(op: &PipelineOp) -> Option<bool> {
    let supported = match op {
        // Add/Subtract encode the complete sum/difference domain in a small
        // parameter-only table, preserving host f32 rounding on the GPU.
        PipelineOp::Add { scale, offset, .. } => gpu_chops_affine_supported(*scale, *offset),
        PipelineOp::Subtract { scale, offset, .. } => gpu_chops_affine_supported(*scale, *offset),
        // Blend narrows alpha to f32 and uses fused interpolation on both
        // host and device; admission needs only the finite-alpha contract.
        PipelineOp::BlendModule { alpha, .. } => gpu_blend_alpha_params(*alpha).is_some(),
        // Pillow's overlay family is implemented through exact 256x256 LUTs.
        // The integer WGSL formulas are the same LUT contract; alpha and CMYK
        // K are ordinary stored bands and are transformed just like RGB.
        PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. } => true,
        _ => return None,
    };
    Some(supported)
}

fn gpu_auxiliary_shader_contract(op: &PipelineOp) -> Option<bool> {
    let supported = match op {
        // Color3DLut uses the same signed 10.6 table preparation and 18.15
        // coordinate scales as the CPU implementation. The table is packed
        // into an auxiliary storage range by the GPU planner and the shader
        // performs the three fixed-point interpolations.
        PipelineOp::Color3DLut {
            size,
            table,
            channels,
            source_mode,
            target_mode,
        } => {
            (2..=65).contains(&size.0)
                && (2..=65).contains(&size.1)
                && (2..=65).contains(&size.2)
                && (*channels == 3 || *channels == 4)
                && table.len()
                    == size.0 as usize * size.1 as usize * size.2 as usize * *channels as usize
                && matches!(
                    source_mode,
                    PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK
                )
                && matches!(
                    target_mode,
                    PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK
                )
                && table.iter().all(|value| value.is_finite())
        }
        // Spread/Noise use exact scalar RNG control bytes uploaded as an
        // auxiliary image; the device shader only performs the deterministic
        // packed gather.
        PipelineOp::EffectSpread { .. } | PipelineOp::EffectNoise { .. } => true,
        // The composite shader carries image2's canvas dimensions and the
        // mask channel explicitly. The pool's contextual preflight below
        // still rejects mixed layouts and unsupported mask representations;
        // equal native byte layouts use the real three-input GPU path.
        PipelineOp::CompositeModule { .. } => true,
        // RemapPalette operates on raw L/P index bytes. The surrounding Image
        // pipeline carries the reordered palette metadata, so the shader only
        // needs to perform the exact inverse-index LUT lookup.
        PipelineOp::RemapPalette { .. } => true,
        // The packed shader transport represents ordinary byte modes plus the
        // raw P/PA and CMYK layouts used by Image.putalpha. P/PA are backed by
        // L/LA storage at this boundary, while CMYK is backed by RGBA storage
        // with K in byte three; put_alpha.wgsl handles both reinterpretations.
        PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => matches!(
            mode,
            PixelMode::L
                | PixelMode::LA
                | PixelMode::RGB
                | PixelMode::RGBA
                | PixelMode::P
                | PixelMode::PA
                | PixelMode::CMYK
        ),
        // PutData replaces raw logical bytes. The shader carries the logical
        // channel count and only packs those bytes for transport, so every
        // byte-backed Pillow mode is representable here; the pool separately
        // proves that its concrete source layout matches the logical mode.
        PipelineOp::PutData { mode, .. } => matches!(
            mode,
            PixelMode::L
                | PixelMode::LA
                | PixelMode::RGB
                | PixelMode::RGBA
                | PixelMode::P
                | PixelMode::PA
                | PixelMode::CMYK
                | PixelMode::Mode1
                | PixelMode::YCbCr
                | PixelMode::HSV
                | PixelMode::I
                | PixelMode::F
        ),
        // P/PA pipelines pass raw indexed samples as Luma8/LumaA8 at the
        // executor boundary. The GPU pool performs the logical-mode check
        // before dispatch, so both ordinary and palette-index writes use the
        // same packed byte kernel here.
        PipelineOp::PutPixel { .. } => true,
        _ => return None,
    };
    Some(supported)
}

pub(super) fn gpu_shader_contract_is_supported(op: &PipelineOp) -> bool {
    if let Some(supported) = gpu_geometry_shader_contract(op) {
        return supported;
    }
    if let Some(supported) = gpu_pixel_shader_contract(op) {
        return supported;
    }
    if let Some(supported) = gpu_filters_shader_contract(op) {
        return supported;
    }
    if let Some(supported) = gpu_chops_shader_contract(op) {
        return supported;
    }
    if let Some(supported) = gpu_auxiliary_shader_contract(op) {
        return supported;
    }
    true
}
