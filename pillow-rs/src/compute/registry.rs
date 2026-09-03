//! Unified operation registry for compute backends.
//!
//! This module maps each [`crate::pipeline::PipelineOp`] variant to
//! the backend implementations that can execute it. CPU operations are normal
//! Rust functions, GPU operations point at embedded WGSL shader sources, and
//! SIMD operations use vectorized adapters where present.
//!
//! Performance: `variant_key()` returns a `&'static str` for O(1) HashMap lookup.
//! No allocations on the hot path.
//!
//! # Internal Contract
//!
//! The registry is public for backend wiring and tests. It is not a stable
//! end-user extension mechanism. A registry entry is valid only when its key,
//! [`crate::pipeline::PipelineOp`] variant, backend implementation, and optional shader metadata
//! all describe the same operation.
//!
//! # Dispatch Invariants
//!
//! - CPU is the universal fallback for materialized pipelines.
//! - `variant_key` must return the key used during registration.
//! - GPU shader names and embedded shader sources must stay aligned.
//! - Registry keys are dispatch keys, not public operation names.
//! - Specialized shader variants may map from one higher-level
//!   [`crate::pipeline::PipelineOp`] variant.

use crate::compute::pool_simd::ops::adapters;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
#[cfg(feature = "gpu")]
use crate::pipeline::{ColorMode, PixelMode, ResampleFilter, TransposeMethod};
use crate::raster::DynamicImage;
use std::collections::HashMap;
use std::sync::OnceLock;

// ── Op function types ────────────────────────────────────────────────────────────

/// CPU operation function signature used by registry entries.
pub type CpuOpFn =
    fn(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> Result<DynamicImage, PilError>;

/// SIMD operation function — same signature as CPU but vectorized internally.
pub type SimdOpFn =
    fn(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> Result<DynamicImage, PilError>;

// ── OpEntry — one entry per operation, all backends ──────────────────────────────

/// Backend implementations registered for one pipeline operation.
pub struct OpEntry {
    /// CPU implementation, present for operations supported by scalar fallback.
    pub cpu_fn: Option<CpuOpFn>,
    /// Embedded WGSL shader file name for GPU execution.
    #[cfg(feature = "gpu")]
    pub gpu_shader: Option<&'static str>,
    /// Embedded WGSL shader source for GPU execution.
    #[cfg(feature = "gpu")]
    pub gpu_source: Option<&'static str>,
    /// SIMD implementation, present when a vectorized adapter exists.
    pub simd_fn: Option<SimdOpFn>,
}

impl OpEntry {
    /// Creates a registry entry with only a CPU implementation.
    #[cfg_attr(feature = "gpu", allow(dead_code))]
    pub const fn cpu_only(f: CpuOpFn) -> Self {
        OpEntry {
            cpu_fn: Some(f),
            simd_fn: None,
            #[cfg(feature = "gpu")]
            gpu_shader: None,
            #[cfg(feature = "gpu")]
            gpu_source: None,
        }
    }

    /// Creates a registry entry with no executable backend implementation.
    ///
    /// This is used for descriptors that are retained only when the optional
    /// GPU feature is enabled. A no-GPU build keeps the operation key visible
    /// to the CPU/SIMD registry without embedding shader assets.
    #[cfg(not(feature = "gpu"))]
    pub const fn empty() -> Self {
        OpEntry {
            cpu_fn: None,
            simd_fn: None,
        }
    }
}

// ── Registration macros — one per backend ────────────────────────────────────────

/// Create an OpEntry with CPU function + GPU shader (wgpu — native + WASM WebGPU).
#[cfg(feature = "gpu")]
macro_rules! gpu_entry {
    ($f:expr, $shader:literal) => {
        $crate::compute::registry::OpEntry {
            cpu_fn: Some($f as $crate::compute::registry::CpuOpFn),
            gpu_shader: Some($shader),
            gpu_source: Some(include_str!(concat!("pool_gpu/shaders/", $shader))),
            simd_fn: None,
        }
    };
}

/// Create the CPU-only equivalent when the optional GPU backend is disabled.
#[cfg(not(feature = "gpu"))]
macro_rules! gpu_entry {
    ($f:expr, $shader:literal) => {
        $crate::compute::registry::OpEntry::cpu_only($f as $crate::compute::registry::CpuOpFn)
    };
}

/// Create an OpEntry that retains a shader contract without a CPU executor.
///
/// Some deprecated descriptors remain in the operation metadata so backend
/// contract tests can inspect their shader sources, while their supported
/// public constructors materialize eagerly and never dispatch these entries.
#[cfg(feature = "gpu")]
macro_rules! gpu_only_entry {
    ($shader:literal) => {
        $crate::compute::registry::OpEntry {
            cpu_fn: None,
            gpu_shader: Some($shader),
            gpu_source: Some(include_str!(concat!("pool_gpu/shaders/", $shader))),
            simd_fn: None,
        }
    };
}

/// Omit shader-only descriptors entirely from no-GPU builds.
#[cfg(not(feature = "gpu"))]
macro_rules! gpu_only_entry {
    ($shader:literal) => {
        $crate::compute::registry::OpEntry::empty()
    };
}

static REGISTRY: OnceLock<Result<HashMap<&'static str, OpEntry>, PilError>> = OnceLock::new();

/// Returns the global operation registry keyed by [`variant_key`].
pub fn registry() -> Result<&'static HashMap<&'static str, OpEntry>, PilError> {
    match REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        register_all(&mut m).map(|()| m)
    }) {
        Ok(registry) => Ok(registry),
        Err(error) => Err(error.clone()),
    }
}

// ── Lookup helpers ────────────────────────────────────────────────────────────

/// Returns the registry key for a pipeline operation variant.
///
/// The key must match the string used during operation registration. It is kept
/// static so backend support checks avoid allocation.
pub fn variant_key(op: &PipelineOp) -> &'static str {
    match op {
        PipelineOp::Resize { .. } => "Resize",
        PipelineOp::Crop { .. } => "Crop",
        PipelineOp::Rotate { .. } => "Rotate",
        PipelineOp::Transpose { .. } => "Transpose",
        PipelineOp::Thumbnail { .. } => "Thumbnail",
        PipelineOp::Reduce { .. } => "Reduce",
        PipelineOp::Convert { .. } => "Convert",
        PipelineOp::Quantize { .. } => "Quantize",
        PipelineOp::RemapPalette { .. } => "RemapPalette",
        PipelineOp::Filter3x3 { .. } => "Filter3x3",
        PipelineOp::Filter5x5 { .. } => "Filter5x5",
        PipelineOp::GaussianBlur { .. } => "GaussianBlur",
        PipelineOp::BoxBlur { .. } => "BoxBlur",
        PipelineOp::BoxBlurXY { .. } => "BoxBlur",
        PipelineOp::MedianFilter { .. } => "MedianFilter",
        PipelineOp::MaxFilter { .. } => "MaxFilter",
        PipelineOp::MinFilter { .. } => "MinFilter",
        PipelineOp::RankFilter { .. } => "RankFilter",
        PipelineOp::Autocontrast { .. } => "Autocontrast",
        PipelineOp::Equalize => "Equalize",
        PipelineOp::Invert => "Invert",
        PipelineOp::Flip => "Flip",
        PipelineOp::Mirror => "Mirror",
        PipelineOp::Posterize { .. } => "Posterize",
        PipelineOp::Solarize { .. } => "Solarize",
        PipelineOp::Grayscale => "Grayscale",
        PipelineOp::Colorize { .. } => "Colorize",
        PipelineOp::Contain { .. } => "Contain",
        PipelineOp::Cover { .. } => "Cover",
        PipelineOp::Fit { .. } => "Fit",
        PipelineOp::Pad { .. } => "Pad",
        PipelineOp::Scale { .. } => "Scale",
        PipelineOp::Expand { .. } => "Expand",
        PipelineOp::CropBorder { .. } => "CropBorder",
        PipelineOp::Add { .. } => "Add",
        PipelineOp::Subtract { .. } => "Subtract",
        PipelineOp::Multiply { .. } => "Multiply",
        PipelineOp::Screen { .. } => "Screen",
        PipelineOp::Darker { .. } => "Darker",
        PipelineOp::Lighter { .. } => "Lighter",
        PipelineOp::Difference { .. } => "Difference",
        PipelineOp::Overlay { .. } => "Overlay",
        PipelineOp::HardLight { .. } => "HardLight",
        PipelineOp::SoftLight { .. } => "SoftLight",
        PipelineOp::AddModulo { .. } => "AddModulo",
        PipelineOp::SubtractModulo { .. } => "SubtractModulo",
        PipelineOp::LogicalAnd { .. } => "LogicalAnd",
        PipelineOp::LogicalOr { .. } => "LogicalOr",
        PipelineOp::LogicalXor { .. } => "LogicalXor",
        PipelineOp::Constant { .. } => "Constant",
        PipelineOp::Offset { .. } => "Offset",
        PipelineOp::Duplicate => "Duplicate",
        PipelineOp::InvertChops => "InvertChops",
        PipelineOp::Brightness { .. } => "Brightness",
        PipelineOp::Contrast { .. } => "Contrast",
        PipelineOp::ColorSaturation { .. } => "ColorSaturation",
        PipelineOp::Sharpness { .. } => "Sharpness",
        PipelineOp::EffectSpread { .. } => "EffectSpread",
        PipelineOp::Paste { .. } => "Paste",
        PipelineOp::AlphaComposite { .. } => "AlphaComposite",
        PipelineOp::Merge { .. } => "Merge",
        PipelineOp::BlendModule { .. } => "BlendModule",
        PipelineOp::CompositeModule { .. } => "CompositeModule",
        PipelineOp::Eval { .. } => "Eval",
        PipelineOp::EffectNoise { .. } => "EffectNoise",
        PipelineOp::PointOp { .. } => "PointOp",
        PipelineOp::Transform { .. } => "Transform",
        PipelineOp::Color3DLut { .. } => "Color3DLut",
        PipelineOp::PutPixel { .. } => "PutPixel",
        PipelineOp::PutData { .. } => "PutData",
        PipelineOp::PutAlpha { .. } => "PutAlpha",
        PipelineOp::PutAlphaData { .. } => "PutAlphaData",
        PipelineOp::ExtractBand { .. } => "ExtractBand",
        PipelineOp::LinearGradient { .. } => "LinearGradient",
        PipelineOp::RadialGradient { .. } => "RadialGradient",
        PipelineOp::EffectMandelbrot { .. } => "EffectMandelbrot",
        PipelineOp::DrawLine { .. } => "DrawLine",
        PipelineOp::DrawRectangle { .. } => "DrawRectangle",
        PipelineOp::DrawRoundedRect { .. } => "DrawRoundedRect",
        PipelineOp::DrawEllipse { .. } => "DrawEllipse",
        PipelineOp::DrawCircle { .. } => "DrawCircle",
        PipelineOp::DrawPolygon { .. } => "DrawPolygon",
        PipelineOp::DrawArc { .. } => "DrawArc",
        PipelineOp::DrawChord { .. } => "DrawChord",
        PipelineOp::DrawPieslice { .. } => "DrawPieslice",
        PipelineOp::DrawPoint { .. } => "DrawPoint",
    }
}

/// Returns whether the CPU backend has an implementation for `op`.
pub fn cpu_supports(op: &PipelineOp) -> Result<bool, PilError> {
    Ok(registry()?
        .get(variant_key(op))
        .is_some_and(|e| e.cpu_fn.is_some()))
}

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

/// Return the fixed-point factor when the Color enhancement WGSL kernel is
/// exact for every possible byte sample and grayscale value.  Pillow computes
/// the grayscale anchor with integer BT.601 arithmetic, then evaluates the
/// channel blend in f64 and truncates after clamping.  The shader uses the
/// same integer luma and a factor scaled by 1000; proving the two byte
/// contracts in scalar preflight avoids admitting a neighboring rounding
/// result as a GPU implementation.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_color_saturation_factor_int(factor: f64) -> Option<u32> {
    if !factor.is_finite() || factor < 0.0 {
        return None;
    }
    let scaled = factor * 1000.0;
    if !scaled.is_finite() || scaled > f64::from(i32::MAX) || scaled < 0.0 {
        return None;
    }
    let factor_int = scaled as u32;
    // The WGSL affine expression multiplies a byte by the factor numerator
    // in i32.  Bound the numerator before the proof so every intermediate is
    // representable on the device as well as in this host calculation.
    if factor_int > (i32::MAX as u32) / 255 {
        return None;
    }
    if f64::from(factor_int) != scaled {
        return None;
    }
    let factor_int = factor_int as i32;
    for luma in 0..=255_i32 {
        for channel in 0..=255_i32 {
            let cpu =
                (f64::from(luma) + factor * f64::from(channel - luma)).clamp(0.0, 255.0) as u8;
            let shader = (luma * (1000 - factor_int) + channel * factor_int)
                .div_euclid(1000)
                .clamp(0, 255) as u8;
            if cpu != shader {
                return None;
            }
        }
    }
    Some(factor_int as u32)
}

/// Return the fixed-point factor when the Sharpness WGSL blend is exact for
/// every possible blurred/original byte pair. The 3x3 smooth kernel is
/// represented with integer weights in the shader; this helper proves the
/// remaining f64 blend against its integer numerator before dispatch.
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
    for blurred in 0..=255_i32 {
        for original in 0..=255_i32 {
            let cpu = (f64::from(blurred) * (1.0 - factor) + f64::from(original) * factor)
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

/// Return the fixed-point factor when the Contrast WGSL blend is exact for
/// every possible image midpoint and byte sample. Pillow computes the
/// midpoint from the source image in scalar control code, then evaluates the
/// per-channel blend in f64. The shader receives that midpoint and uses the
/// same integer numerator; proving the complete byte domain here prevents a
/// f32 or signed-division rounding difference from being presented as GPU
/// parity.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_contrast_factor_int(factor: f64) -> Option<u32> {
    if !factor.is_finite() {
        return None;
    }
    let scaled = factor * 1000.0;
    if !scaled.is_finite() || scaled < f64::from(i32::MIN) || scaled > f64::from(i32::MAX) {
        return None;
    }
    let factor_int = scaled as i32;
    if f64::from(factor_int) != scaled {
        return None;
    }
    // Bound the signed WGSL numerator for every midpoint/sample pair.
    let max_factor = i64::from(i32::MAX) / 255;
    let min_factor = i64::from(i32::MIN) / 255;
    if i64::from(factor_int) > max_factor.saturating_sub(1000)
        || i64::from(factor_int) < min_factor.saturating_add(1000)
    {
        return None;
    }
    for mean in 0..=255_i32 {
        for sample in 0..=255_i32 {
            let cpu = (f64::from(mean) + factor * f64::from(sample - mean)).clamp(0.0, 255.0) as u8;
            let numerator = mean * (1000 - factor_int) + sample * factor_int;
            let shader = numerator.div_euclid(1000).clamp(0, 255) as u8;
            if cpu != shader {
                return None;
            }
        }
    }
    Some(factor_int as u32)
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

/// Return the f32 parameters when the Add/Subtract WGSL kernel is exact for
/// every possible pair of byte samples.
///
/// The public Pillow operation evaluates the affine expression in f64 and
/// truncates after clamping. The shader evaluates the same expression in f32,
/// so checking all 256 × 256 byte pairs in scalar preflight proves the
/// selected parameterization before a vector dispatch is admitted. A zero
/// divisor remains CPU-only because WGSL division by zero is not a safe way to
/// represent Pillow's historical C behavior.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_chops_affine_params(scale: f64, offset: f64, subtract: bool) -> Option<[u32; 2]> {
    if !scale.is_finite() || scale == 0.0 || !offset.is_finite() {
        return None;
    }

    let scale_f32 = scale as f32;
    let offset_f32 = offset as f32;
    if !scale_f32.is_finite()
        || !offset_f32.is_finite()
        || scale_f32 as f64 != scale
        || offset_f32 as f64 != offset
    {
        return None;
    }

    for a in 0..=255_u32 {
        for b in 0..=255_u32 {
            let numerator = if subtract {
                f64::from(a) - f64::from(b)
            } else {
                f64::from(a) + f64::from(b)
            };
            let cpu = (numerator / scale + offset).clamp(0.0, 255.0) as u8;

            let numerator_f32 = if subtract {
                a as f32 - b as f32
            } else {
                a as f32 + b as f32
            };
            let shader = (numerator_f32 / scale_f32 + offset_f32).clamp(0.0, 255.0) as u8;
            if cpu != shader {
                return None;
            }
        }
    }

    Some([scale_f32.to_bits(), offset_f32.to_bits()])
}

/// Return the f32 alpha bits when the BlendModule WGSL kernel is exact for
/// every possible pair of byte samples.
///
/// Pillow evaluates the blend expression in f64 and truncates only after
/// clamping. The GPU kernel evaluates the same expression in f32. Prove the
/// selected parameterization over the complete byte domain before admitting
/// the vector path, including the documented extrapolation range.
#[cfg(feature = "gpu")]
pub(crate) fn gpu_blend_alpha_params(alpha: f64) -> Option<u32> {
    if !alpha.is_finite() {
        return None;
    }
    let alpha_f32 = alpha as f32;
    if !alpha_f32.is_finite() {
        return None;
    }

    for a in 0..=255_u32 {
        for b in 0..=255_u32 {
            let cpu = (f64::from(a) * (1.0 - alpha) + f64::from(b) * alpha).clamp(0.0, 255.0) as u8;
            let shader =
                ((a as f32) * (1.0 - alpha_f32) + (b as f32) * alpha_f32).clamp(0.0, 255.0) as u8;
            if cpu != shader {
                return None;
            }
        }
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
fn gpu_shader_contract_is_supported(op: &PipelineOp) -> bool {
    match op {
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
                )
        }
        // Quantize returns a palette-backed image in Pillow. The shader is a
        // per-channel uniform quantizer and cannot produce that public result.
        PipelineOp::Quantize { .. } => false,
        // Autocontrast and Equalize are admitted here because the GPU pool
        // expands them into their histogram/control/LUT/remap pass sequence.
        // The operation-level registry cannot inspect the source image, so
        // mode, mask, and dimensions remain contextual preflight checks.
        PipelineOp::Autocontrast { .. } | PipelineOp::Equalize => true,
        // Contrast receives Pillow's image-wide midpoint from scalar control
        // code and performs the channel blend in the real WGSL data path.
        PipelineOp::Contrast { factor } => gpu_contrast_factor_int(*factor).is_some(),
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
        // Brightness uses a scalar-verified fixed-point representation. The
        // helper admits every factor whose 256 byte results match Pillow,
        // rather than restricting the real vector kernel to endpoints.
        PipelineOp::Brightness { factor } => gpu_brightness_factor_int(*factor).is_some(),
        // Color saturation uses an integer luma anchor followed by the
        // scalar-proven fixed-point blend represented by the real shader.
        PipelineOp::ColorSaturation { factor } => {
            gpu_color_saturation_factor_int(*factor).is_some()
        }
        // Colorize uses the same integer floor-division LUT construction as
        // Pillow's ImageOps._lut path.  The shader consumes all six public
        // parameters, including the optional midpoint color.
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
        // Add/Subtract use a scalar exhaustive proof for the f32 uniform
        // representation instead of limiting the real vector kernel to the
        // unit-divisor endpoint.
        PipelineOp::Add { scale, offset, .. } => {
            gpu_chops_affine_params(*scale, *offset, false).is_some()
        }
        PipelineOp::Subtract { scale, offset, .. } => {
            gpu_chops_affine_params(*scale, *offset, true).is_some()
        }
        // Blend's CPU contract evaluates f64 alpha before the final
        // truncating cast. The real WGSL path uses f32 arithmetic, so admit
        // only alpha values exhaustively proven equivalent over all byte
        // pairs.
        PipelineOp::BlendModule { alpha, .. } => gpu_blend_alpha_params(*alpha).is_some(),
        // Color3DLut uses the same signed 12.4 table preparation and 18.15
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
        // Pillow's overlay family is implemented through exact 256x256 LUTs.
        // The integer WGSL formulas are the same LUT contract; alpha and CMYK
        // K are ordinary stored bands and are transformed just like RGB.
        PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. } => true,
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
        // These operations are exposed as eager Image-module constructors in
        // the public core. Their retained shader assets do not reproduce the
        // full mode-specific byte contracts: gradients support 1/P/I/F and
        // Mandelbrot has a different iteration/update ordering. Keep the
        // sources available for validation and future work, but route the
        // public operation to CPU until a matching output contract exists.
        PipelineOp::LinearGradient { .. }
        | PipelineOp::RadialGradient { .. }
        | PipelineOp::EffectMandelbrot { .. } => false,
        _ => true,
    }
}

/// Returns whether the SIMD backend has an implementation for `op`.
pub fn simd_supports(op: &PipelineOp) -> Result<bool, PilError> {
    // Keep this operation-level answer deliberately narrow.  A registered
    // adapter may still exist as a workbench for a future kernel, but it is
    // not a SIMD capability until its data plane is vectorized.  Contextual
    // support below then applies the image layout, logical mode, dimensions,
    // and secondary-operand checks.
    let native_operation = matches!(
        op,
        PipelineOp::Invert
            | PipelineOp::InvertChops
            | PipelineOp::Resize { .. }
            | PipelineOp::Scale { .. }
            | PipelineOp::Thumbnail { .. }
            | PipelineOp::Contain { .. }
            | PipelineOp::Cover { .. }
            | PipelineOp::Fit { .. }
            | PipelineOp::Transform { .. }
            | PipelineOp::Pad { .. }
            | PipelineOp::Convert { .. }
            | PipelineOp::Reduce { .. }
            | PipelineOp::Solarize { .. }
            | PipelineOp::Posterize { .. }
            | PipelineOp::Grayscale
            | PipelineOp::Colorize { .. }
            | PipelineOp::Brightness { .. }
            | PipelineOp::Contrast { .. }
            | PipelineOp::ColorSaturation { .. }
            | PipelineOp::Sharpness { .. }
            | PipelineOp::Autocontrast { .. }
            | PipelineOp::Equalize
            | PipelineOp::Eval { .. }
            | PipelineOp::RemapPalette { .. }
            | PipelineOp::PutData { .. }
            | PipelineOp::ExtractBand { .. }
            | PipelineOp::Offset { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Transpose { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Expand { .. }
            | PipelineOp::Constant { .. }
            | PipelineOp::Duplicate
            | PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::GaussianBlur { .. }
            | PipelineOp::MaxFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::RankFilter { .. }
            | PipelineOp::LinearGradient { .. }
            | PipelineOp::EffectNoise { .. }
            | PipelineOp::EffectSpread { .. }
            | PipelineOp::Color3DLut { .. }
            | PipelineOp::Rotate { .. }
            | PipelineOp::PutAlpha { .. }
            | PipelineOp::PutAlphaData { .. }
            | PipelineOp::PutPixel { .. }
            | PipelineOp::DrawLine { .. }
            | PipelineOp::DrawPoint { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::AlphaComposite { .. }
            | PipelineOp::Paste { .. }
            | PipelineOp::Merge { .. }
            | PipelineOp::BlendModule { .. }
            | PipelineOp::CompositeModule { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::Overlay { .. }
            | PipelineOp::HardLight { .. }
            | PipelineOp::SoftLight { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            | PipelineOp::LogicalAnd { .. }
            | PipelineOp::LogicalOr { .. }
            | PipelineOp::LogicalXor { .. }
            | PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
    );
    if !native_operation {
        return Ok(false);
    }
    Ok(registry()?
        .get(variant_key(op))
        .is_some_and(|e| e.simd_fn.is_some()))
}

/// Executes one operation on the scalar CPU backend.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when the operation is unknown to the
/// registry or has no CPU implementation.
pub fn execute_cpu(
    op: &PipelineOp,
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let key = variant_key(op);
    let entry = registry()?
        .get(key)
        .ok_or_else(|| PilError::ValueError(format!("CPU: unknown op {}", key)))?;
    let f = entry
        .cpu_fn
        .ok_or_else(|| PilError::ValueError(format!("CPU: no impl for {}", key)))?;
    f(img, op, mode)
}

/// Extracts GPU shader parameter words from a pipeline operation.
///
/// The returned `Vec<u32>` follows each shader's `Params` struct after the
/// shared four-word header: `width`, `height`, `pad0`, `pad1`. Operations with
/// no shader parameter block return an empty vector.
#[cfg(feature = "gpu")]
fn separable_box_blur_params(radius: u32) -> [u32; 3] {
    let radius = radius.min(64);
    let window = radius.saturating_mul(2).saturating_add(1);
    let weight = ((1u64 << 24) / u64::from(window)) as u32;
    [radius, weight, 0]
}

/// Compute the exact fixed-point parameters used by Pillow's one-dimensional
/// box blur for a finite non-negative radius.  The integer radius selects the
/// replicated samples in the rolling window; the fractional remainder is
/// represented by the two edge weights.  Keeping this calculation in the
/// scalar control plane lets the GPU data plane use the same 24-bit contract
/// for both tuple and fractional public inputs.
#[cfg(feature = "gpu")]
pub(crate) fn separable_box_blur_params_f32(radius: f32) -> Option<[u32; 3]> {
    if !radius.is_finite() || radius < 0.0 || radius > 64.0 {
        return None;
    }
    let integer_radius = radius as u32;
    let window = integer_radius.saturating_mul(2).saturating_add(1);
    let weight = (((1u64 << 24) as f32 / (radius * 2.0 + 1.0)) as u32).min(1 << 24);
    let used = u64::from(window).saturating_mul(u64::from(weight));
    let edge_weight = ((1u64 << 24).saturating_sub(used) / 2) as u32;
    Some([integer_radius, weight, edge_weight])
}

/// Compute the exact fractional box-blur parameters used by Pillow's
/// three-pass Gaussian approximation. The GPU horizontal/vertical kernels
/// consume these fixed-point words directly, so a Gaussian operation can be
/// expanded into six ordered dispatches without a CPU readback between them.
#[cfg(feature = "gpu")]
fn separable_gaussian_blur_radius_and_fraction(sigma: f32) -> Option<(u32, f32)> {
    if !sigma.is_finite() {
        return None;
    }
    // Pillow's ImagingGaussianBlur normalizes each radius by magnitude. Keep
    // the GPU parameter proof aligned with the CPU implementation and the
    // oracle: negative finite radii are valid and produce the same result as
    // their positive counterparts.
    let sigma = sigma.abs();
    if sigma == 0.0 {
        return Some((0, 0.0));
    }
    let sigma2 = f64::from(sigma) * f64::from(sigma) / 3.0;
    let l = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let a_den = 6.0 * (sigma2 - l1 * l1);
    let a = a_num / a_den;
    let radius = (l + a) as f32;
    // The rolling WGSL kernels and their bounded loop contract support the
    // complete radius range through 64.  Keep the host proof aligned with
    // that shader limit so small images with a large Pillow sigma do not take
    // an unnecessary CPU path.
    if !radius.is_finite() || radius < 0.0 || radius > 64.0 {
        return None;
    }
    Some((radius as u32, radius))
}

#[cfg(feature = "gpu")]
pub(crate) fn separable_gaussian_blur_radius(sigma: f32) -> Option<u32> {
    separable_gaussian_blur_radius_and_fraction(sigma).map(|(radius, _)| radius)
}

#[cfg(feature = "gpu")]
pub(crate) fn separable_gaussian_blur_params(sigma: f32) -> [u32; 3] {
    let Some((radius, fractional_radius)) = separable_gaussian_blur_radius_and_fraction(sigma)
    else {
        return separable_box_blur_params(0);
    };
    let window = radius.saturating_mul(2).saturating_add(1);
    let weight = (((1u64 << 24) as f32 / (fractional_radius * 2.0 + 1.0)) as u32).min(1 << 24);
    let used = u64::from(window) * u64::from(weight);
    let edge_weight = ((1u64 << 24).saturating_sub(used) / 2) as u32;
    [radius, weight, edge_weight]
}

#[cfg(feature = "gpu")]
pub fn extract_params(op: &PipelineOp) -> Vec<u32> {
    match op {
        // ── No-param ops ──
        PipelineOp::Invert
        | PipelineOp::Flip
        | PipelineOp::Mirror
        | PipelineOp::Duplicate
        | PipelineOp::InvertChops
        | PipelineOp::Grayscale
        | PipelineOp::AlphaComposite { .. }
        | PipelineOp::Eval { .. }
        | PipelineOp::PointOp { .. }
        | PipelineOp::Multiply { .. }
        | PipelineOp::Screen { .. }
        | PipelineOp::Difference { .. }
        | PipelineOp::Darker { .. }
        | PipelineOp::Lighter { .. }
        | PipelineOp::AddModulo { .. }
        | PipelineOp::SubtractModulo { .. }
        | PipelineOp::LogicalAnd { .. }
        | PipelineOp::LogicalOr { .. }
        | PipelineOp::LogicalXor { .. }
        | PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. } => vec![],

        // ── CompositeModule: mask_alpha ──
        PipelineOp::CompositeModule { mask_alpha, .. } => vec![*mask_alpha as u32],

        // ── Solarize: threshold ──
        PipelineOp::Solarize { threshold } => vec![*threshold as u32],

        // ── Posterize: bits ──
        PipelineOp::Posterize { bits } => vec![*bits as u32],

        // ── Brightness: scalar-verified fixed-point factor ──
        PipelineOp::Brightness { factor } => {
            vec![gpu_brightness_factor_int(*factor).unwrap_or(0)]
        }

        // ── Contrast: scalar-proven factor * 1000 ──
        PipelineOp::Contrast { factor } => {
            vec![gpu_contrast_factor_int(*factor).unwrap_or(0)]
        }

        // ── ColorSaturation: scalar-proven factor * 1000 ──
        PipelineOp::ColorSaturation { factor } => {
            vec![gpu_color_saturation_factor_int(*factor).unwrap_or(0)]
        }

        // ── Sharpness: scalar-proven fixed-point factor ──
        PipelineOp::Sharpness { factor } => {
            vec![gpu_sharpness_factor_int(*factor).unwrap_or(0)]
        }

        // ── Colorize: pack black/white/mid as u32 RGBA (0xAABBGGRR) ──
        PipelineOp::Colorize {
            black,
            white,
            mid,
            blackpoint,
            midpoint,
            whitepoint,
        } => {
            let bk = (black.0 as u32)
                | ((black.1 as u32) << 8)
                | ((black.2 as u32) << 16)
                | (0xff << 24);
            let wh = (white.0 as u32)
                | ((white.1 as u32) << 8)
                | ((white.2 as u32) << 16)
                | (0xff << 24);
            let mid_value = mid
                .map(|mid| {
                    (mid.0 as u32) | ((mid.1 as u32) << 8) | ((mid.2 as u32) << 16) | (0xff << 24)
                })
                .unwrap_or(bk);
            vec![
                bk,
                wh,
                mid_value,
                *blackpoint as u32,
                *midpoint as u32,
                *whitepoint as u32,
                u32::from(mid.is_some()),
            ]
        }

        // ── Constant: value ──
        PipelineOp::Constant { value } => vec![*value as u32],

        // ── Offset: dx, dy ──
        // Preserve the complete signed i32 bit pattern. The shader performs
        // the modulo using the image dimensions, so reducing here to a fixed
        // 16-bit range would change valid offsets on wide/tall images.
        PipelineOp::Offset { x, y } => vec![*x as u32, *y as u32],

        // ── Add / Subtract: scalar-proven f32 parameter bits ──
        PipelineOp::Add { scale, offset, .. } => gpu_chops_affine_params(*scale, *offset, false)
            .unwrap_or([0, 0])
            .to_vec(),
        PipelineOp::Subtract { scale, offset, .. } => {
            gpu_chops_affine_params(*scale, *offset, true)
                .unwrap_or([0, 0])
                .to_vec()
        }

        // ── BlendModule: scalar-proven f32 alpha bits ──
        PipelineOp::BlendModule { alpha, .. } => {
            vec![gpu_blend_alpha_params(*alpha).unwrap_or(0)]
        }

        // ── BoxBlur: horizontal and vertical radius/weight triplets ──
        PipelineOp::BoxBlur { radius } => {
            let params = separable_box_blur_params(*radius);
            params.into_iter().chain(params).collect()
        }
        // BoxBlurXY uses the same six-word parameter block. The pass count is
        // used by the GPU encoder to repeat the horizontal and vertical
        // dispatches, so it does not need to be copied into the uniform block.
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            let x = separable_box_blur_params_f32(*radius_x)
                .unwrap_or_else(|| separable_box_blur_params(0));
            let y = separable_box_blur_params_f32(*radius_y)
                .unwrap_or_else(|| separable_box_blur_params(0));
            let _ = passes;
            x.into_iter().chain(y).collect()
        }

        // ── MedianFilter / MaxFilter / MinFilter: size ──
        PipelineOp::MedianFilter { size }
        | PipelineOp::MaxFilter { size }
        | PipelineOp::MinFilter { size } => vec![*size],

        // ── RankFilter: size, rank ──
        PipelineOp::RankFilter { size, rank } => vec![*size, *rank],

        // ── Filter3x3: kernel [9] f32 bits + offset + rational denominator ──
        PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } => {
            let s = *scale;
            let mut params = Vec::with_capacity(11);
            for k in kernel.iter() {
                params.push((k / s).to_bits());
            }
            params.push(filter_offset_i32(*offset).unwrap_or_default() as u32);
            // The byte kernel follows Pillow's f32/FMA contraction order.
            // Keep the optional integer mode disabled here: integer
            // rationalization can move an exact half-way value across the
            // truncation boundary even when the coefficients reconstruct
            // their normalized f32 values.
            params.push(0);
            params
        }

        // ── Filter5x5: kernel [25] f32 bits + offset + rational denominator ──
        PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } => {
            let s = *scale;
            let mut params = Vec::with_capacity(27);
            for k in kernel.iter() {
                params.push((k / s).to_bits());
            }
            params.push(filter_offset_i32(*offset).unwrap_or_default() as u32);
            // See the 3x3 path: preserve Pillow's f32/FMA rounding rather
            // than replacing it with integer rational arithmetic.
            params.push(0);
            params
        }

        // ── Resize: dst_w, dst_h ──
        PipelineOp::Resize { w, h, .. } => vec![*w, *h],

        // ── Transpose: op_code ──
        PipelineOp::Transpose { method } => {
            let code: u32 = match method {
                TransposeMethod::FlipLeftRight => 0,
                TransposeMethod::FlipTopBottom => 1,
                TransposeMethod::Rotate90 => 2,
                TransposeMethod::Rotate180 => 3,
                TransposeMethod::Rotate270 => 4,
                TransposeMethod::Transpose => 5,
                TransposeMethod::Transverse => 6,
            };
            vec![code]
        }

        // ── Scale: factor as fixed-point (factor * 65536) ──
        PipelineOp::Scale { factor, .. } => vec![(factor * 65536.0) as u32],

        // ── Convert: encode ColorMode as u32 code ──
        PipelineOp::Convert { mode, .. } => {
            let code: u32 = match mode {
                ColorMode::L => 0,
                ColorMode::LA => 1,
                ColorMode::RGB => 2,
                ColorMode::RGBA => 3,
                ColorMode::CMYK => 4,
                ColorMode::YCbCr => 5,
                ColorMode::HSV => 6,
                ColorMode::I => 7,
                ColorMode::F => 8,
                ColorMode::P => 9,
                ColorMode::Mode1 => 10,
            };
            vec![code]
        }

        // ── Quantize: colors, levels, step ──
        PipelineOp::Quantize { colors, .. } => {
            let c = *colors;
            let levels = 256 / c.max(1);
            vec![c, levels, 256]
        }

        // ── PutAlpha: alpha as u32 ──
        PipelineOp::PutAlpha { alpha, mode } => vec![*alpha as u32, mode.code()],
        PipelineOp::PutAlphaData { mode, .. } => vec![0, mode.code()],

        // ── PutPixel: x, y, color packed as RGBA u32 ──
        PipelineOp::PutPixel { x, y, color, .. } => {
            let (r, g, b, a) = *color;
            let packed = (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24);
            vec![*x, *y, packed]
        }

        // ── Crop: left, top, width, height ──
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            vec![*left, *top, *right - *left, *bottom - *top]
        }

        // ── Reduce: horizontal and vertical factors ──
        PipelineOp::Reduce { x_factor, y_factor } => vec![*x_factor, *y_factor],

        // ── Thumbnail: dst_w, dst_h ──
        PipelineOp::Thumbnail { w, h, .. } => vec![*w, *h],

        // ── Contain: dst_w, dst_h ──
        PipelineOp::Contain { w, h, .. } => vec![*w, *h],

        // ── Cover: new_w, new_h, crop_x, crop_y ──
        PipelineOp::Cover { w, h, .. } => vec![*w, *h, 0, 0],

        // ── Fit: new_w, new_h, crop_x, crop_y ──
        PipelineOp::Fit { w, h, .. } => vec![*w, *h, 0, 0],

        // ── Paste: src_w, src_h, paste_x, paste_y, has_mask, mask_alpha ──
        PipelineOp::Paste {
            w,
            h,
            x,
            y,
            mask,
            mask_alpha,
            ..
        } => {
            vec![
                (*w).max(0) as u32,
                (*h).max(0) as u32,
                *x as u32,
                *y as u32,
                mask.is_some() as u32,
                *mask_alpha as u32,
            ]
        }

        // ── Transform: dst_w, dst_h, a..f bits, fill_color, filter_code ──
        PipelineOp::Transform {
            w,
            h,
            data,
            filter,
            fill,
            palette_fill,
            ..
        } => {
            let a = (data.first().copied().unwrap_or(0.0) as f32).to_bits();
            let b = (data.get(1).copied().unwrap_or(0.0) as f32).to_bits();
            let c = (data.get(2).copied().unwrap_or(0.0) as f32).to_bits();
            let d = (data.get(3).copied().unwrap_or(0.0) as f32).to_bits();
            let e = (data.get(4).copied().unwrap_or(0.0) as f32).to_bits();
            let f = (data.get(5).copied().unwrap_or(0.0) as f32).to_bits();
            let resolved_fill = palette_fill.map(|index| (index, 0, 0, 255)).or(*fill);
            let fill_color = match resolved_fill {
                Some((r, g, b, a)) => {
                    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24)
                }
                None => 0,
            };
            let filter_code = match filter {
                ResampleFilter::Nearest => 0u32,
                ResampleFilter::Bilinear => 1,
                ResampleFilter::Bicubic => 2,
                ResampleFilter::Lanczos => 3,
                ResampleFilter::Box => 4,
                ResampleFilter::Hamming => 5,
            };
            vec![*w, *h, a, b, c, d, e, f, fill_color, filter_code]
        }

        // ── PutData: data length ──
        PipelineOp::PutData { data, mode } => vec![data.len() as u32, mode.code()],

        // ── CropBorder: border ──
        PipelineOp::CropBorder { border } => vec![*border],

        // ── Expand: border, fill_color ──
        // src_w/src_h come from the header (cur_w/cur_h), not from extract_params.
        PipelineOp::Expand { border, fill } => {
            let (r, g, b, a) = *fill;
            vec![
                *border,
                (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24),
            ]
        }

        // ── Merge: num_bands ──
        PipelineOp::Merge { bands, .. } => vec![bands.len() as u32],

        // ── RemapPalette: LUT uploaded via extract_lut, no params ──
        PipelineOp::RemapPalette { .. } => vec![],

        // ── Pad: src_w, src_h, fill, centering_x, centering_y, scale_x, scale_y ──
        PipelineOp::Pad {
            w,
            h,
            color,
            centering,
            ..
        } => {
            let fill = match color {
                Some((r, g, b, a)) => {
                    (*r as u32) | ((*g as u32) << 8) | ((*b as u32) << 16) | ((*a as u32) << 24)
                }
                None => 0xff000000u32,
            };
            let cx = (centering.0.clamp(0.0, 1.0) * 65536.0) as u32;
            let cy = (centering.1.clamp(0.0, 1.0) * 65536.0) as u32;
            vec![*w, *h, fill, cx, cy, 65536u32, 65536u32]
        }

        // ── Rotate: cos_theta, sin_theta, fill, expand ──
        PipelineOp::Rotate {
            angle,
            fill,
            expand,
            ..
        } => {
            let cos_t = (angle.cos() as f32).to_bits();
            let sin_t = (angle.sin() as f32).to_bits();
            let fill_color = match fill {
                Some((r, g, b, a)) => {
                    (*r as u32) | ((*g as u32) << 8) | ((*b as u32) << 16) | ((*a as u32) << 24)
                }
                None => 0u32,
            };
            vec![cos_t, sin_t, fill_color, *expand as u32]
        }

        // ── GaussianBlur: identical horizontal/vertical triplets ──
        PipelineOp::GaussianBlur { sigma } => {
            let params = separable_gaussian_blur_params(*sigma);
            params.into_iter().chain(params).collect()
        }

        // ── Autocontrast: cutoff ──
        PipelineOp::Autocontrast { cutoff, .. } => vec![(*cutoff as f32).to_bits()],

        // ── Equalize: no params ──
        PipelineOp::Equalize => vec![],

        // ── Color3DLut: size dims ──
        PipelineOp::Color3DLut { size, .. } => vec![size.0, size.1, size.2],

        // ── ExtractBand: channel index ──
        PipelineOp::ExtractBand { index } => vec![*index as u32],

        // ── LinearGradient, RadialGradient: no params ──
        PipelineOp::LinearGradient { .. } => vec![],
        PipelineOp::RadialGradient { .. } => vec![],

        // ── EffectMandelbrot: extent + quality ──
        PipelineOp::EffectMandelbrot {
            w: _,
            h: _,
            x0,
            y0,
            x1,
            y1,
            quality,
        } => {
            vec![
                (*x0 as f32).to_bits(),
                (*y0 as f32).to_bits(),
                (*x1 as f32).to_bits(),
                (*y1 as f32).to_bits(),
                *quality,
            ]
        }

        // ── Everything else (no GPU support / no params) ──
        _ => vec![],
    }
}

// ── Registration table ───────────────────────────────────────────────────

fn register_all(m: &mut HashMap<&'static str, OpEntry>) -> Result<(), PilError> {
    use crate::compute::pool_cpu::ops::chops::{
        op_chops_add, op_chops_add_modulo, op_chops_constant, op_chops_darker, op_chops_difference,
        op_chops_duplicate, op_chops_hard_light, op_chops_invert, op_chops_lighter,
        op_chops_logical_and, op_chops_logical_or, op_chops_logical_xor, op_chops_multiply,
        op_chops_offset, op_chops_overlay, op_chops_screen, op_chops_soft_light, op_chops_subtract,
        op_chops_subtract_modulo,
    };
    use crate::compute::pool_cpu::ops::color::{op_convert, op_extract_band, op_remap_palette};
    use crate::compute::pool_cpu::ops::draw::{
        op_draw_arc, op_draw_chord, op_draw_circle, op_draw_ellipse, op_draw_line,
        op_draw_pieslice, op_draw_point, op_draw_polygon, op_draw_rectangle, op_draw_rounded_rect,
    };
    use crate::compute::pool_cpu::ops::effects::{
        op_alpha_composite, op_blend_module, op_color3dlut, op_composite_module, op_effect_noise,
        op_effect_spread, op_eval, op_merge, op_paste, op_put_alpha, op_put_alpha_data,
        op_put_data, op_put_pixel, op_transform,
    };
    use crate::compute::pool_cpu::ops::enhance::{
        op_enhance_brightness, op_enhance_color_saturation, op_enhance_contrast,
        op_enhance_sharpness,
    };
    use crate::compute::pool_cpu::ops::filter::{
        execute_box_blur, execute_box_blur_xy_with_passes, execute_filter3x3, execute_filter5x5,
        execute_gaussian_blur, execute_max_filter_with_mode, execute_median_filter_with_mode,
        execute_min_filter_with_mode, execute_rank_filter_with_mode,
    };
    use crate::compute::pool_cpu::ops::geometry::{
        execute_crop, execute_reduce, execute_resize, execute_rotate, execute_thumbnail,
        execute_transpose,
    };
    use crate::compute::pool_cpu::ops::imageops::{
        op_autocontrast, op_colorize, op_contain, op_cover, op_crop_border, op_equalize, op_expand,
        op_fit, op_flip, op_grayscale, op_invert, op_mirror, op_pad, op_posterize, op_scale,
        op_solarize,
    };

    // ── Geometry ──
    m.insert(
        "Resize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Resize { w, h, filter } = op {
                    execute_resize(img, *w, *h, filter, mode)
                } else {
                    Err(PilError::ValueError("expected Resize op".into()))
                }
            },
            "resize_nearest.wgsl"
        ),
    );
    m.insert(
        "Crop",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Crop {
                    left,
                    top,
                    right,
                    bottom,
                } = op
                {
                    execute_crop(img, *left, *top, *right, *bottom)
                } else {
                    Err(PilError::ValueError("expected Crop op".into()))
                }
            },
            "crop.wgsl"
        ),
    );
    m.insert(
        "Rotate",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Rotate {
                    angle,
                    expand,
                    fill,
                    center,
                    translate,
                    filter,
                    nearest,
                } = op
                {
                    execute_rotate(
                        img, *angle, *expand, *fill, *center, *translate, *filter, *nearest, mode,
                    )
                } else {
                    Err(PilError::ValueError("expected Rotate op".into()))
                }
            },
            "rotate.wgsl"
        ),
    );
    m.insert(
        "Transpose",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Transpose { method } = op {
                    execute_transpose(img, method)
                } else {
                    Err(PilError::ValueError("expected Transpose op".into()))
                }
            },
            "transpose.wgsl"
        ),
    );
    m.insert(
        "Thumbnail",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Thumbnail { w, h, filter } = op {
                    execute_thumbnail(img, *w, *h, filter, mode)
                } else {
                    Err(PilError::ValueError("expected Thumbnail op".into()))
                }
            },
            "thumbnail.wgsl"
        ),
    );
    m.insert(
        "Reduce",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Reduce { x_factor, y_factor } = op {
                    execute_reduce(img, *x_factor, *y_factor, mode)
                } else {
                    Err(PilError::ValueError("expected Reduce op".into()))
                }
            },
            "reduce.wgsl"
        ),
    );

    // ── Color ──
    m.insert(
        "Convert",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             source_mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Convert {
                    mode: cm,
                    matrix: _,
                    dither: _,
                } = op
                {
                    op_convert(img, cm, source_mode)
                } else {
                    Err(PilError::ValueError("expected Convert op".into()))
                }
            },
            "convert.wgsl"
        ),
    );
    m.insert(
        "RemapPalette",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::RemapPalette { dest_map } = op {
                    // Pass dest_map as-is (capped at 256 per PIL behavior).
                    // op_remap_palette handles short dest_maps correctly.
                    let end = dest_map.len().min(256);
                    op_remap_palette(img, &dest_map[..end], mode)
                } else {
                    Err(PilError::ValueError("expected RemapPalette op".into()))
                }
            },
            "remap_palette.wgsl"
        ),
    );

    // ── Filter ──
    m.insert(
        "Filter3x3",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Filter3x3 {
                    kernel,
                    scale,
                    offset,
                } = op
                {
                    execute_filter3x3(img, kernel, *scale, *offset, mode)
                } else {
                    Err(PilError::ValueError("expected Filter3x3 op".into()))
                }
            },
            "filter_3x3.wgsl"
        ),
    );
    m.insert(
        "Filter5x5",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Filter5x5 {
                    kernel,
                    scale,
                    offset,
                } = op
                {
                    execute_filter5x5(img, kernel, *scale, *offset, mode)
                } else {
                    Err(PilError::ValueError("expected Filter5x5 op".into()))
                }
            },
            "filter_5x5.wgsl"
        ),
    );
    m.insert(
        "GaussianBlur",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::GaussianBlur { sigma } = op {
                    execute_gaussian_blur(img, *sigma)
                } else {
                    Err(PilError::ValueError("expected GaussianBlur op".into()))
                }
            },
            "gaussian_blur.wgsl"
        ),
    );
    m.insert(
        "BoxBlur",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                match op {
                    PipelineOp::BoxBlur { radius } => execute_box_blur(img, *radius),
                    PipelineOp::BoxBlurXY {
                        radius_x,
                        radius_y,
                        passes,
                    } => execute_box_blur_xy_with_passes(img, *radius_x, *radius_y, *passes),
                    _ => Err(PilError::ValueError("expected BoxBlur op".into())),
                }
            },
            "box_blur.wgsl"
        ),
    );
    m.insert(
        "MedianFilter",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::MedianFilter { size } = op {
                    execute_median_filter_with_mode(img, *size, mode)
                } else {
                    Err(PilError::ValueError("expected MedianFilter op".into()))
                }
            },
            "median_filter.wgsl"
        ),
    );
    m.insert(
        "MaxFilter",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::MaxFilter { size } = op {
                    execute_max_filter_with_mode(img, *size, mode)
                } else {
                    Err(PilError::ValueError("expected MaxFilter op".into()))
                }
            },
            "max_filter.wgsl"
        ),
    );
    m.insert(
        "MinFilter",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::MinFilter { size } = op {
                    execute_min_filter_with_mode(img, *size, mode)
                } else {
                    Err(PilError::ValueError("expected MinFilter op".into()))
                }
            },
            "min_filter.wgsl"
        ),
    );
    m.insert(
        "RankFilter",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::RankFilter { size, rank } = op {
                    execute_rank_filter_with_mode(img, *size, *rank, mode)
                } else {
                    Err(PilError::ValueError("expected RankFilter op".into()))
                }
            },
            "rank_filter.wgsl"
        ),
    );

    // ── ImageOps ──
    m.insert(
        "Autocontrast",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let Some(m) = mode {
                    if m == "LA" || m == "RGBA" {
                        return Err(PilError::OsError(format!("not supported for mode {m}")));
                    }
                }
                if let PipelineOp::Autocontrast { cutoff, mask } = op {
                    op_autocontrast(img, *cutoff, mask.as_ref())
                } else {
                    Err(PilError::ValueError("expected Autocontrast op".into()))
                }
            },
            "autocontrast.wgsl"
        ),
    );
    m.insert(
        "Equalize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let Some(m) = mode {
                    if m == "LA" || m == "RGBA" {
                        return Err(PilError::OsError(format!("not supported for mode {m}")));
                    }
                }
                if matches!(op, PipelineOp::Equalize) {
                    op_equalize(img)
                } else {
                    Err(PilError::ValueError("expected Equalize op".into()))
                }
            },
            "equalize.wgsl"
        ),
    );
    m.insert(
        "Invert",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Invert) {
                    op_invert(img)
                } else {
                    Err(PilError::ValueError("expected Invert op".into()))
                }
            },
            "invert.wgsl"
        ),
    );
    m.insert(
        "Flip",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Flip) {
                    op_flip(img)
                } else {
                    Err(PilError::ValueError("expected Flip op".into()))
                }
            },
            "flip.wgsl"
        ),
    );
    m.insert(
        "Mirror",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Mirror) {
                    op_mirror(img)
                } else {
                    Err(PilError::ValueError("expected Mirror op".into()))
                }
            },
            "mirror.wgsl"
        ),
    );
    m.insert(
        "Posterize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Posterize { bits } = op {
                    op_posterize(img, *bits)
                } else {
                    Err(PilError::ValueError("expected Posterize op".into()))
                }
            },
            "posterize.wgsl"
        ),
    );
    m.insert(
        "Solarize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Solarize { threshold } = op {
                    op_solarize(img, *threshold)
                } else {
                    Err(PilError::ValueError("expected Solarize op".into()))
                }
            },
            "solarize.wgsl"
        ),
    );
    m.insert(
        "Grayscale",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Grayscale) {
                    op_grayscale(img, mode)
                } else {
                    Err(PilError::ValueError("expected Grayscale op".into()))
                }
            },
            "grayscale.wgsl"
        ),
    );
    m.insert(
        "Colorize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Colorize {
                    black,
                    white,
                    mid,
                    blackpoint,
                    midpoint,
                    whitepoint,
                } = op
                {
                    op_colorize(img, black, white, *mid, *blackpoint, *midpoint, *whitepoint)
                } else {
                    Err(PilError::ValueError("expected Colorize op".into()))
                }
            },
            "colorize.wgsl"
        ),
    );
    m.insert(
        "Contain",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Contain { w, h, filter } = op {
                    op_contain(img, *w, *h, *filter, mode)
                } else {
                    Err(PilError::ValueError("expected Contain op".into()))
                }
            },
            "contain.wgsl"
        ),
    );
    m.insert(
        "Cover",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Cover { w, h, filter } = op {
                    op_cover(img, *w, *h, *filter, mode)
                } else {
                    Err(PilError::ValueError("expected Cover op".into()))
                }
            },
            "cover.wgsl"
        ),
    );
    m.insert(
        "Fit",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Fit {
                    w,
                    h,
                    filter,
                    bleed,
                    centering,
                } = op
                {
                    op_fit(img, *w, *h, *filter, *bleed, *centering, mode)
                } else {
                    Err(PilError::ValueError("expected Fit op".into()))
                }
            },
            "fit.wgsl"
        ),
    );
    m.insert(
        "Pad",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Pad {
                    w,
                    h,
                    filter,
                    color,
                    centering,
                } = op
                {
                    op_pad(img, *w, *h, *filter, *color, *centering, mode)
                } else {
                    Err(PilError::ValueError("expected Pad op".into()))
                }
            },
            "pad.wgsl"
        ),
    );
    m.insert(
        "Scale",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Scale { factor, filter } = op {
                    op_scale(img, *factor, *filter, mode)
                } else {
                    Err(PilError::ValueError("expected Scale op".into()))
                }
            },
            "scale.wgsl"
        ),
    );
    m.insert(
        "Expand",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Expand { border, fill } = op {
                    op_expand(img, *border, *fill, mode)
                } else {
                    Err(PilError::ValueError("expected Expand op".into()))
                }
            },
            "expand.wgsl"
        ),
    );
    m.insert(
        "CropBorder",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::CropBorder { border } = op {
                    op_crop_border(img, *border)
                } else {
                    Err(PilError::ValueError("expected CropBorder op".into()))
                }
            },
            "crop_border.wgsl"
        ),
    );

    // ── ImageChops ──
    m.insert(
        "Add",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Add {
                    other,
                    scale,
                    offset,
                } = op
                {
                    op_chops_add(img, other, *scale, *offset)
                } else {
                    Err(PilError::ValueError("expected Add op".into()))
                }
            },
            "add.wgsl"
        ),
    );
    m.insert(
        "Subtract",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Subtract {
                    other,
                    scale,
                    offset,
                } = op
                {
                    op_chops_subtract(img, other, *scale, *offset)
                } else {
                    Err(PilError::ValueError("expected Subtract op".into()))
                }
            },
            "subtract.wgsl"
        ),
    );
    m.insert(
        "Multiply",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Multiply { other } = op {
                    op_chops_multiply(img, other)
                } else {
                    Err(PilError::ValueError("expected Multiply op".into()))
                }
            },
            "multiply.wgsl"
        ),
    );
    m.insert(
        "Screen",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Screen { other } = op {
                    op_chops_screen(img, other)
                } else {
                    Err(PilError::ValueError("expected Screen op".into()))
                }
            },
            "screen.wgsl"
        ),
    );
    m.insert(
        "Darker",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Darker { other } = op {
                    op_chops_darker(img, other)
                } else {
                    Err(PilError::ValueError("expected Darker op".into()))
                }
            },
            "darker.wgsl"
        ),
    );
    m.insert(
        "Lighter",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Lighter { other } = op {
                    op_chops_lighter(img, other)
                } else {
                    Err(PilError::ValueError("expected Lighter op".into()))
                }
            },
            "lighter.wgsl"
        ),
    );
    m.insert(
        "Difference",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Difference { other } = op {
                    op_chops_difference(img, other)
                } else {
                    Err(PilError::ValueError("expected Difference op".into()))
                }
            },
            "difference.wgsl"
        ),
    );
    m.insert(
        "Overlay",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Overlay { other } = op {
                    op_chops_overlay(img, other)
                } else {
                    Err(PilError::ValueError("expected Overlay op".into()))
                }
            },
            "overlay.wgsl"
        ),
    );
    m.insert(
        "HardLight",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::HardLight { other } = op {
                    op_chops_hard_light(img, other)
                } else {
                    Err(PilError::ValueError("expected HardLight op".into()))
                }
            },
            "hard_light.wgsl"
        ),
    );
    m.insert(
        "SoftLight",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::SoftLight { other } = op {
                    op_chops_soft_light(img, other)
                } else {
                    Err(PilError::ValueError("expected SoftLight op".into()))
                }
            },
            "soft_light.wgsl"
        ),
    );
    m.insert(
        "AddModulo",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::AddModulo { other } = op {
                    op_chops_add_modulo(img, other)
                } else {
                    Err(PilError::ValueError("expected AddModulo op".into()))
                }
            },
            "add_modulo.wgsl"
        ),
    );
    m.insert(
        "SubtractModulo",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::SubtractModulo { other } = op {
                    op_chops_subtract_modulo(img, other)
                } else {
                    Err(PilError::ValueError("expected SubtractModulo op".into()))
                }
            },
            "subtract_modulo.wgsl"
        ),
    );
    m.insert(
        "LogicalAnd",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::LogicalAnd { other } = op {
                    op_chops_logical_and(img, other)
                } else {
                    Err(PilError::ValueError("expected LogicalAnd op".into()))
                }
            },
            "logical_and.wgsl"
        ),
    );
    m.insert(
        "LogicalOr",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::LogicalOr { other } = op {
                    op_chops_logical_or(img, other)
                } else {
                    Err(PilError::ValueError("expected LogicalOr op".into()))
                }
            },
            "logical_or.wgsl"
        ),
    );
    m.insert(
        "LogicalXor",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::LogicalXor { other } = op {
                    op_chops_logical_xor(img, other)
                } else {
                    Err(PilError::ValueError("expected LogicalXor op".into()))
                }
            },
            "logical_xor.wgsl"
        ),
    );
    m.insert(
        "Constant",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Constant { value } = op {
                    Ok(op_chops_constant(img, *value))
                } else {
                    Err(PilError::ValueError("expected Constant op".into()))
                }
            },
            "constant.wgsl"
        ),
    );
    m.insert(
        "Offset",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Offset { x, y } = op {
                    Ok(op_chops_offset(img, *x, *y, mode))
                } else {
                    Err(PilError::ValueError("expected Offset op".into()))
                }
            },
            "offset.wgsl"
        ),
    );
    m.insert(
        "Duplicate",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Duplicate) {
                    Ok(op_chops_duplicate(img))
                } else {
                    Err(PilError::ValueError("expected Duplicate op".into()))
                }
            },
            "duplicate.wgsl"
        ),
    );
    m.insert(
        "InvertChops",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::InvertChops) {
                    op_chops_invert(img)
                } else {
                    Err(PilError::ValueError("expected InvertChops op".into()))
                }
            },
            "invert_chops.wgsl"
        ),
    );

    // ── Enhance ──
    m.insert(
        "Brightness",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Brightness { factor } = op {
                    op_enhance_brightness(img, *factor, mode)
                } else {
                    Err(PilError::ValueError("expected Brightness op".into()))
                }
            },
            "brightness.wgsl"
        ),
    );
    m.insert(
        "Contrast",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Contrast { factor } = op {
                    op_enhance_contrast(img, *factor, mode)
                } else {
                    Err(PilError::ValueError("expected Contrast op".into()))
                }
            },
            "contrast.wgsl"
        ),
    );
    m.insert(
        "ColorSaturation",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::ColorSaturation { factor } = op {
                    op_enhance_color_saturation(img, *factor, mode)
                } else {
                    Err(PilError::ValueError("expected ColorSaturation op".into()))
                }
            },
            "color_saturation.wgsl"
        ),
    );
    m.insert(
        "Sharpness",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Sharpness { factor } = op {
                    op_enhance_sharpness(img, *factor, mode)
                } else {
                    Err(PilError::ValueError("expected Sharpness op".into()))
                }
            },
            "sharpness.wgsl"
        ),
    );

    // ── Effects ──
    // Pillow 12.2.0 libImaging/Effects.c:117-159 consumes process-global
    // rand() sequentially and performs collision-prone scatter writes. The GPU
    // entry receives a host-generated relocation map, so its gather shader
    // preserves that order-dependent contract without inventing a second RNG.
    m.insert(
        "EffectSpread",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::EffectSpread { distance } = op {
                    op_effect_spread(img, *distance)
                } else {
                    Err(PilError::ValueError("expected EffectSpread op".into()))
                }
            },
            "effect_spread.wgsl"
        ),
    );
    m.insert(
        "Paste",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Paste {
                    source,
                    x,
                    y,
                    w: _,
                    h: _,
                    mask,
                    mask_alpha,
                } = op
                {
                    op_paste(img, source, *x as i64, *y as i64, mask, *mask_alpha, _mode)
                } else {
                    Err(PilError::ValueError("expected Paste op".into()))
                }
            },
            "paste.wgsl"
        ),
    );
    m.insert(
        "AlphaComposite",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::AlphaComposite { source, .. } = op {
                    op_alpha_composite(img, source)
                } else {
                    Err(PilError::ValueError("expected AlphaComposite op".into()))
                }
            },
            "alpha_composite.wgsl"
        ),
    );

    // ── Module fns ──
    // Merge packs the remaining single-band images into one auxiliary storage
    // range. The GPU planner validates the band count, modes, and dimensions
    // before constructing that range, so this shader only interleaves validated
    // bytes.
    m.insert(
        "Merge",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _source_mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Merge {
                    mode,
                    logical_mode: target_mode,
                    bands,
                } = op
                {
                    let arc_bands: Vec<std::sync::Arc<crate::image::Image>> = bands
                        .iter()
                        .map(|im| std::sync::Arc::new(im.clone()))
                        .collect();
                    op_merge(img, mode, &arc_bands, Some(target_mode))
                } else {
                    Err(PilError::ValueError("expected Merge op".into()))
                }
            },
            "merge.wgsl"
        ),
    );
    m.insert(
        "BlendModule",
        gpu_entry!(
            |img, op, mode| {
                if let PipelineOp::BlendModule { other, alpha } = op {
                    op_blend_module(img, other, *alpha, mode)
                } else {
                    Err(PilError::ValueError("expected BlendModule op".into()))
                }
            },
            "blend_module.wgsl"
        ),
    );
    m.insert(
        "CompositeModule",
        gpu_entry!(
            |img, op, mode| {
                if let PipelineOp::CompositeModule {
                    other,
                    mask,
                    mask_alpha,
                } = op
                {
                    op_composite_module(img, other, mask, *mask_alpha, mode)
                } else {
                    Err(PilError::ValueError("expected CompositeModule op".into()))
                }
            },
            "composite_module.wgsl"
        ),
    );
    m.insert(
        "Eval",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Eval { lut } = op {
                    op_eval(img, lut)
                } else {
                    Err(PilError::ValueError("expected Eval op".into()))
                }
            },
            "eval.wgsl"
        ),
    );
    m.insert(
        "EffectNoise",
        // Pillow's effect consumes one sequential libc RNG stream with a
        // rejection loop and returns a new L image. The GPU path uploads the
        // exact host-generated bytes and only performs the packed transport
        // gather, keeping the process-global stream deterministic.
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::EffectNoise { sigma } = op {
                    op_effect_noise(img, *sigma)
                } else {
                    Err(PilError::ValueError("expected EffectNoise op".into()))
                }
            },
            "effect_noise.wgsl"
        ),
    );

    // ── Point + Transform ──
    m.insert(
        "PointOp",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PointOp { lut } = op {
                    // PointOp is an internal LUT-fusion descriptor; share the
                    // public Eval implementation so validation and semantics
                    // cannot drift between the two operation forms.
                    op_eval(img, lut)
                } else {
                    Err(PilError::ValueError("expected PointOp op".into()))
                }
            },
            "point_op.wgsl"
        ),
    );
    m.insert(
        "Transform",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Transform {
                    w,
                    h,
                    method,
                    data,
                    filter,
                    fill,
                    fill_is_none,
                    palette_fill,
                } = op
                {
                    let resolved_fill = palette_fill.map(|index| (index, 0, 0, 255)).or(*fill);
                    op_transform(
                        img,
                        *w,
                        *h,
                        method,
                        data,
                        filter,
                        resolved_fill,
                        *fill_is_none,
                        mode,
                    )
                } else {
                    Err(PilError::ValueError("expected Transform op".into()))
                }
            },
            "transform.wgsl"
        ),
    );

    // ── Mutating ──
    m.insert(
        "PutPixel",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutPixel { x, y, color, .. } = op {
                    op_put_pixel(img, *x, *y, *color)
                } else {
                    Err(PilError::ValueError("expected PutPixel op".into()))
                }
            },
            "put_pixel.wgsl"
        ),
    );
    m.insert(
        "PutData",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutData {
                    data,
                    mode: data_mode,
                } = op
                {
                    op_put_data(img, data, *data_mode)
                } else {
                    Err(PilError::ValueError("expected PutData op".into()))
                }
            },
            "put_data.wgsl"
        ),
    );
    m.insert(
        "Color3DLut",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Color3DLut {
                    size,
                    table,
                    channels,
                    source_mode,
                    target_mode,
                } = op
                {
                    op_color3dlut(img, *size, table, *channels, *source_mode, *target_mode)
                } else {
                    Err(PilError::ValueError("expected Color3DLut op".into()))
                }
            },
            "color_3dlut.wgsl"
        ),
    );
    m.insert(
        "PutAlpha",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutAlpha {
                    alpha,
                    mode: alpha_mode,
                } = op
                {
                    Ok(op_put_alpha(img, *alpha, *alpha_mode))
                } else {
                    Err(PilError::ValueError("expected PutAlpha op".into()))
                }
            },
            "put_alpha.wgsl"
        ),
    );
    m.insert(
        "PutAlphaData",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutAlphaData { mask, mode } = op {
                    Ok(op_put_alpha_data(img, mask, *mode))
                } else {
                    Err(PilError::ValueError("expected PutAlphaData op".into()))
                }
            },
            "put_alpha_data.wgsl"
        ),
    );

    // ── ExtractBand: native byte-channel copy ──
    m.insert(
        "ExtractBand",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::ExtractBand { index } = op {
                    op_extract_band(img, *index)
                } else {
                    Err(PilError::ValueError("expected ExtractBand op".into()))
                }
            },
            "extract_band.wgsl"
        ),
    );

    // ── LinearGradient (native SIMD generator + retained shader contract) ──
    m.insert("LinearGradient", gpu_only_entry!("linear_gradient.wgsl"));

    // ── RadialGradient (deprecated shader contract only) ──
    m.insert("RadialGradient", gpu_only_entry!("radial_gradient.wgsl"));

    // ── EffectMandelbrot (deprecated shader contract only) ──
    m.insert(
        "EffectMandelbrot",
        gpu_only_entry!("effect_mandelbrot.wgsl"),
    );

    // ── ImageDraw ops ──
    // The exact Pillow scan conversion remains a scalar geometry/control
    // step; the GPU entry uploads its resulting packed canvas and performs the
    // native data-plane copy in draw.wgsl.
    macro_rules! draw_entry {
        ($key:expr, $func:expr) => {
            m.insert($key, gpu_entry!($func, "draw.wgsl"));
        };
    }
    draw_entry!("DrawLine", |img, op, mode| {
        if let PipelineOp::DrawLine {
            x0,
            y0,
            x1,
            y1,
            fill,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_line(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *fill,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawLine".into()))
        }
    });
    draw_entry!("DrawRectangle", |img, op, mode| {
        if let PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_rectangle(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawRectangle".into()))
        }
    });
    draw_entry!("DrawRoundedRect", |img, op, mode| {
        if let PipelineOp::DrawRoundedRect {
            x0,
            y0,
            x1,
            y1,
            radius,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_rounded_rect(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *radius,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawRoundedRect".into()))
        }
    });
    draw_entry!("DrawEllipse", |img, op, mode| {
        if let PipelineOp::DrawEllipse {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_ellipse(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawEllipse".into()))
        }
    });
    draw_entry!("DrawCircle", |img, op, mode| {
        if let PipelineOp::DrawCircle {
            cx,
            cy,
            radius,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_circle(
                img,
                *cx,
                *cy,
                *radius,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawCircle".into()))
        }
    });
    draw_entry!("DrawPolygon", |img, op, mode| {
        if let PipelineOp::DrawPolygon {
            points,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_polygon(img, points, *fill, *outline, *width, *alpha_blend_rgb, mode)
        } else {
            Err(PilError::ValueError("expected DrawPolygon".into()))
        }
    });
    draw_entry!("DrawArc", |img, op, mode| {
        if let PipelineOp::DrawArc {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_arc(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *start,
                *end,
                *fill,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawArc".into()))
        }
    });
    draw_entry!("DrawChord", |img, op, mode| {
        if let PipelineOp::DrawChord {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_chord(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *start,
                *end,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawChord".into()))
        }
    });
    draw_entry!("DrawPieslice", |img, op, mode| {
        if let PipelineOp::DrawPieslice {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_pieslice(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *start,
                *end,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawPieslice".into()))
        }
    });
    draw_entry!("DrawPoint", |img, op, mode| {
        if let PipelineOp::DrawPoint {
            points,
            fill,
            alpha_blend_rgb,
        } = op
        {
            op_draw_point(img, points, *fill, *alpha_blend_rgb, mode)
        } else {
            Err(PilError::ValueError("expected DrawPoint".into()))
        }
    });

    // ── SIMD registrations ───────────────────────────────────────────
    // Attach simd_fn only for adapters whose hot data plane is admitted by
    // simd_supports. Legacy scalar wrappers remain CPU-only and are not
    // exposed as SIMD registrations.

    fn simd_set(
        entries: &mut HashMap<&'static str, OpEntry>,
        key: &'static str,
        f: SimdOpFn,
    ) -> Result<(), PilError> {
        let entry = entries
            .get_mut(key)
            .ok_or_else(|| PilError::InternalError(format!("SIMD key not registered: {key}")))?;
        entry.simd_fn = Some(f);
        Ok(())
    }

    // Section A: Simple single-image ops with native SIMD data planes.
    simd_set(m, "Invert", adapters::simd_invert)?;
    simd_set(m, "InvertChops", adapters::simd_invert_chops)?;

    // Section B: Single-image byte transforms.
    simd_set(m, "Solarize", adapters::simd_solarize)?;
    simd_set(m, "Posterize", adapters::simd_posterize)?;
    simd_set(m, "Grayscale", adapters::simd_grayscale)?;
    simd_set(m, "Colorize", adapters::simd_colorize)?;
    simd_set(m, "Brightness", adapters::simd_brightness)?;
    simd_set(m, "Contrast", adapters::simd_contrast)?;
    simd_set(m, "ColorSaturation", adapters::simd_color_saturation)?;
    simd_set(m, "Sharpness", adapters::simd_sharpness)?;
    simd_set(m, "Autocontrast", adapters::simd_autocontrast)?;
    simd_set(m, "Equalize", adapters::simd_equalize)?;
    simd_set(m, "RemapPalette", adapters::simd_remap_palette)?;
    simd_set(m, "Resize", adapters::simd_resize)?;
    simd_set(m, "Scale", adapters::simd_scale)?;
    simd_set(m, "Thumbnail", adapters::simd_thumbnail)?;
    simd_set(m, "Contain", adapters::simd_contain)?;
    simd_set(m, "Cover", adapters::simd_cover)?;
    simd_set(m, "Fit", adapters::simd_fit)?;
    simd_set(m, "Transform", adapters::simd_transform)?;
    simd_set(m, "Pad", adapters::simd_pad)?;
    simd_set(m, "Convert", adapters::simd_convert)?;
    simd_set(m, "Reduce", adapters::simd_reduce)?;
    simd_set(m, "LinearGradient", adapters::simd_linear_gradient)?;
    simd_set(m, "EffectNoise", adapters::simd_effect_noise)?;
    simd_set(m, "EffectSpread", adapters::simd_effect_spread)?;
    simd_set(m, "Color3DLut", adapters::simd_color3dlut)?;
    simd_set(m, "Offset", adapters::simd_offset)?;

    // Section C: Spatial native-copy operations.
    simd_set(m, "Flip", adapters::simd_flip)?;
    simd_set(m, "Mirror", adapters::simd_mirror)?;
    simd_set(m, "Filter3x3", adapters::simd_filter_3x3)?;
    simd_set(m, "Filter5x5", adapters::simd_filter_5x5)?;
    simd_set(m, "MaxFilter", adapters::simd_max_filter)?;
    simd_set(m, "MinFilter", adapters::simd_min_filter)?;
    simd_set(m, "MedianFilter", adapters::simd_median_filter)?;
    simd_set(m, "RankFilter", adapters::simd_rank_filter)?;
    simd_set(m, "BoxBlur", adapters::simd_box_blur)?;
    simd_set(m, "GaussianBlur", adapters::simd_gaussian_blur)?;

    // Section D: Dual-image native-byte kernels.
    simd_set(m, "Add", adapters::simd_add)?;
    simd_set(m, "Subtract", adapters::simd_subtract)?;
    simd_set(m, "Multiply", adapters::simd_multiply)?;
    simd_set(m, "Screen", adapters::simd_screen)?;
    simd_set(m, "Darker", adapters::simd_darker)?;
    simd_set(m, "Lighter", adapters::simd_lighter)?;
    simd_set(m, "Difference", adapters::simd_difference)?;
    simd_set(m, "Overlay", adapters::simd_overlay)?;
    simd_set(m, "HardLight", adapters::simd_hard_light)?;
    simd_set(m, "SoftLight", adapters::simd_soft_light)?;
    simd_set(m, "AddModulo", adapters::simd_add_modulo)?;
    simd_set(m, "SubtractModulo", adapters::simd_subtract_modulo)?;
    simd_set(m, "LogicalAnd", adapters::simd_logical_and)?;
    simd_set(m, "LogicalOr", adapters::simd_logical_or)?;
    simd_set(m, "LogicalXor", adapters::simd_logical_xor)?;

    // Section E: Native-copy and channel operations.
    simd_set(m, "Transpose", adapters::simd_transpose)?;
    simd_set(m, "CropBorder", adapters::simd_crop_border)?;
    simd_set(m, "Crop", adapters::simd_crop)?;
    simd_set(m, "Expand", adapters::simd_expand)?;
    simd_set(m, "Constant", adapters::simd_constant)?;
    simd_set(m, "Duplicate", adapters::simd_duplicate)?;
    simd_set(m, "Rotate", adapters::simd_rotate)?;
    simd_set(m, "PutAlpha", adapters::simd_put_alpha)?;
    simd_set(m, "PutAlphaData", adapters::simd_put_alpha_data)?;
    simd_set(m, "PutPixel", adapters::simd_put_pixel)?;
    simd_set(m, "DrawLine", adapters::simd_draw_line)?;
    simd_set(m, "DrawPoint", adapters::simd_draw_point)?;
    simd_set(m, "DrawRectangle", adapters::simd_draw_rectangle)?;
    simd_set(m, "DrawRoundedRect", adapters::simd_draw_rounded_rect)?;
    simd_set(m, "DrawPolygon", adapters::simd_draw_polygon)?;
    simd_set(m, "DrawEllipse", adapters::simd_draw_ellipse)?;
    simd_set(m, "DrawCircle", adapters::simd_draw_circle)?;
    simd_set(m, "DrawArc", adapters::simd_draw_arc)?;
    simd_set(m, "DrawChord", adapters::simd_draw_chord)?;
    simd_set(m, "DrawPieslice", adapters::simd_draw_pieslice)?;
    simd_set(m, "Eval", adapters::simd_eval)?;
    simd_set(m, "PutData", adapters::simd_put_data)?;
    simd_set(m, "Paste", adapters::simd_paste)?;
    simd_set(m, "Merge", adapters::simd_merge)?;
    simd_set(m, "BlendModule", adapters::simd_blend_module)?;
    simd_set(m, "CompositeModule", adapters::simd_composite_module)?;
    simd_set(m, "AlphaComposite", adapters::simd_alpha_composite)?;
    simd_set(m, "ExtractBand", adapters::simd_extract_band)?;
    Ok(())
}
