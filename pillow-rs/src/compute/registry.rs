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
    if !gpu_shader_contract_is_supported(op) {
        return Ok(false);
    }
    if matches!(
        op,
        PipelineOp::Rotate { nearest: true, .. }
            | PipelineOp::Rotate {
                center: Some(_),
                ..
            }
            | PipelineOp::Rotate {
                translate: Some(_),
                ..
            }
    ) {
        return Ok(false);
    }
    Ok(registry()?
        .get(variant_key(op))
        .is_some_and(|e| e.gpu_shader.is_some()))
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
        // PIL resize uses a two-pass, fixed-point coefficient table (and has
        // additional premultiplied-alpha behavior). The retained shaders are
        // useful validation/future-work assets, but the current single-pass
        // transport is not an exact public Resize implementation for any
        // filter, so keep the operation on the CPU.
        PipelineOp::Resize { .. } => false,
        // These shaders do not currently carry the complete host-side sizing
        // and crop/contain contract. The pool already has a CPU fallback.
        PipelineOp::Thumbnail { .. }
        | PipelineOp::Contain { .. }
        | PipelineOp::Cover { .. }
        | PipelineOp::Fit { .. } => false,
        // Pad is a contain-then-paste operation. The retained shader places
        // pixels with a scale/offset, but it does not reproduce Pillow's
        // contain sizing, bankers-rounded placement, or mode-specific
        // default fill. Keep the source available for validation and route
        // the public operation through the CPU implementation.
        PipelineOp::Pad { .. } => false,
        // Scale's retained shader is nearest-neighbour only, but Pillow's
        // nearest path uses cumulative f64 stepping. The shader's f32 ratio
        // can select a neighboring source pixel at exact boundaries, so keep
        // the public Scale operation on CPU until the mapping is represented
        // exactly.
        PipelineOp::Scale { .. } => false,
        // The current shader implements only the four byte modes and has no
        // matrix or dithering parameter path.
        PipelineOp::Convert {
            mode,
            matrix,
            dither,
        } => {
            matrix.is_none()
                && dither.is_none()
                && matches!(mode, ColorMode::L | ColorMode::LA | ColorMode::RGB | ColorMode::RGBA)
        }
        // Quantize returns a palette-backed image in Pillow. The shader is a
        // per-channel uniform quantizer and cannot produce that public result.
        PipelineOp::Quantize { .. }
        // The active shader is missing Pillow's midpoint/blackpoint/whitepoint
        // piecewise colorize contract.
        | PipelineOp::Colorize { .. }
        // Autocontrast and Equalize require histogram/LUT passes; the active
        // single-pass sources cannot derive those values from the image.
        | PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize => false,
        // Contrast also depends on the image-wide mean. The retained shader
        // uses a fixed midpoint and therefore is not a public implementation.
        PipelineOp::Contrast { .. } => false,
        // The CPU filter path deliberately follows Pillow's f32 contraction
        // order (including fused multiply-adds on the pinned oracle). WGSL
        // `*`/`+` contraction is implementation-dependent across GPU
        // backends, so the retained convolution shaders are not a portable
        // bit-exact public implementation yet. Keep them for validation and
        // route Filter3x3/Filter5x5 through the CPU until their arithmetic is
        // represented with a backend-independent fixed-point contract.
        PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. } => false,
        // BoxBlurXY carries Pillow's tuple/fractional radius contract, which
        // the integer-radius shader cannot represent. Keep this descriptor
        // on the exact CPU path; the ordinary integer BoxBlur remains
        // eligible for the existing backend contract.
        PipelineOp::BoxBlurXY { .. } => false,
        // These shaders transport factors as fixed-point values while the
        // CPU contract evaluates f64 before truncation. Three-decimal
        // factors can differ by one at an integer boundary, so only the
        // identity/zero endpoints are universally exact for this dispatch.
        PipelineOp::Brightness { factor } | PipelineOp::ColorSaturation { factor } => {
            factor.is_finite() && (*factor == 0.0 || *factor == 1.0)
        }
        // Sharpness performs a floating-point convolution before its blend.
        // Even factor=0 can differ at a byte boundary across GPU contraction
        // rules; factor=1 is the exact identity endpoint.
        PipelineOp::Sharpness { factor } => factor.is_finite() && *factor == 1.0,
        // The active Add/Subtract shaders receive f32 uniforms.  Their
        // public implementation is f64 and truncates only after clamping,
        // so the single-dispatch path is exact only for the common unit
        // divisor and an exactly representable integral offset.  Keep
        // fractional/scaled requests on CPU instead of allowing a silent
        // f32 rounding difference at a byte boundary.
        PipelineOp::Add { scale, offset, .. } | PipelineOp::Subtract { scale, offset, .. } => {
            *scale == 1.0
                && offset.is_finite()
                && (*offset as f32) as f64 == *offset
                && offset.fract() == 0.0
        }
        // Blend's CPU contract evaluates arbitrary f64 alpha before the final
        // truncating cast.  Even alpha=p/255 can land one ulp below an
        // integer in that expression, while the integer shader lands on the
        // mathematical rational.  Only the endpoint alphas are universally
        // bit-exact across both paths; all interpolating requests stay CPU.
        PipelineOp::BlendModule { alpha, .. } => {
            alpha.is_finite() && (*alpha == 0.0 || *alpha == 1.0)
        }
        // The old Color3DLut source is intentionally a pass-through and is no
        // longer registered as a GPU implementation, but keep this guard next
        // to the other contract checks if a source is reintroduced.
        PipelineOp::Color3DLut { .. } => false,
        // Image.composite writes image2's canvas/mode, which the retained
        // single-dispatch module path does not fully carry. Keep this public
        // operation on CPU until the binding and output-mode contracts are
        // explicit.
        PipelineOp::CompositeModule { .. } => false,
        // Pillow's overlay family is implemented through exact 256x256 LUTs;
        // the compact WGSL formulas are only approximations at rounding
        // boundaries. Keep these operations on the CPU until the LUT is
        // uploaded as an explicit GPU resource.
        PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. } => false,
        // The WGSL rotate path does not reproduce Pillow's pixel-centered
        // affine mapping, fast paths for right angles, fill defaults, or
        // premultiplied-alpha interpolation. Keep it as a reviewed shader
        // asset, but do not advertise a partial public implementation.
        PipelineOp::Rotate { .. } => false,
        // Transform has the same pixel-center and premultiplied-alpha
        // requirements. Its affine shader is useful for future work, but the
        // current single dispatch is not a complete Pillow contract.
        PipelineOp::Transform { .. } => false,
        // Reduce uses Pillow's fixed-point division multiplier, including
        // partial edge blocks. The retained shader's rounded integer average
        // is not bit-exact at all inputs, so do not advertise it as public GPU
        // behavior until that coefficient path is implemented.
        PipelineOp::Reduce { .. } => false,
        // The shader remaps packed bytes independently and cannot preserve
        // Pillow's indexed palette metadata. Public P/PA inputs are normally
        // filtered earlier, but keep this guard correct for direct core use.
        PipelineOp::RemapPalette { .. } => false,
        // The packed shader transport currently represents only ordinary byte
        // modes. Indexed, integer, float, and CMYK alpha promotion must stay
        // on CPU until their native sample layouts are preserved end to end.
        PipelineOp::PutAlpha { mode, .. } => {
            matches!(mode, PixelMode::L | PixelMode::LA | PixelMode::RGB | PixelMode::RGBA)
        }
        PipelineOp::PutData { mode, .. } => {
            matches!(mode, PixelMode::L | PixelMode::LA | PixelMode::RGB | PixelMode::RGBA)
        }
        // A palette-index write is meaningful only in the indexed sample
        // layout, which the packed RGBA shader cannot identify from the
        // dynamic image alone.
        PipelineOp::PutPixel { palette_index, .. } => !palette_index,
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
    let radius = radius.min(16);
    let window = radius.saturating_mul(2).saturating_add(1);
    let weight = ((1u64 << 24) / u64::from(window)) as u32;
    [radius, weight, 0]
}

/// Compute the exact fractional box-blur parameters used by Pillow's
/// three-pass Gaussian approximation. The GPU horizontal/vertical kernels
/// consume these fixed-point words directly, so a Gaussian operation can be
/// expanded into six ordered dispatches without a CPU readback between them.
#[cfg(feature = "gpu")]
pub(crate) fn separable_gaussian_blur_params(sigma: f32) -> [u32; 3] {
    if !sigma.is_finite() || sigma <= 0.0 {
        return separable_box_blur_params(0);
    }
    let sigma2 = f64::from(sigma) * f64::from(sigma) / 3.0;
    let l = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let a_den = 6.0 * (sigma2 - l1 * l1);
    let a = a_num / a_den;
    let radius = (l + a) as f32;
    if !radius.is_finite() || radius < 0.0 {
        return separable_box_blur_params(0);
    }
    let radius = radius.min(16.0);
    let radius_int = radius as u32;
    let window = radius_int.saturating_mul(2).saturating_add(1);
    let weight = (((1u64 << 24) as f32 / (radius * 2.0 + 1.0)) as u32).min(1 << 24);
    let used = u64::from(window) * u64::from(weight);
    let edge_weight = ((1u64 << 24).saturating_sub(used) / 2) as u32;
    [radius_int, weight, edge_weight]
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

        // ── Brightness / Contrast / ColorSaturation: factor * 1000 as u32 ──
        PipelineOp::Brightness { factor }
        | PipelineOp::Contrast { factor }
        | PipelineOp::ColorSaturation { factor } => vec![(factor * 1000.0) as u32],

        // ── Sharpness: factor * 1000 as u32 ──
        PipelineOp::Sharpness { factor } => vec![(factor * 1000.0) as u32],

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
            ]
        }

        // ── Constant: value ──
        PipelineOp::Constant { value } => vec![*value as u32],

        // ── Offset: dx, dy ──
        // Preserve the complete signed i32 bit pattern. The shader performs
        // the modulo using the image dimensions, so reducing here to a fixed
        // 16-bit range would change valid offsets on wide/tall images.
        PipelineOp::Offset { x, y } => vec![*x as u32, *y as u32],

        // ── Add / Subtract: scale (f32 bits as u32), offset (f32 bits as u32) ──
        PipelineOp::Add { scale, offset, .. } | PipelineOp::Subtract { scale, offset, .. } => {
            vec![(*scale as f32).to_bits(), (*offset as f32).to_bits()]
        }

        // ── BlendModule: alpha * 255 as u32 ──
        PipelineOp::BlendModule { alpha, .. } => {
            vec![(alpha.clamp(0.0, 1.0) * 255.0) as u32]
        }

        // ── BoxBlur: radius ──
        PipelineOp::BoxBlur { radius } => separable_box_blur_params(*radius).to_vec(),
        // BoxBlurXY is deliberately CPU-only. Keep a deterministic parameter
        // shape here for GPU contract inspection if a caller constructs the
        // descriptor directly, while the support gate prevents dispatch.
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            let params = [*radius_x, *radius_y]
                .into_iter()
                .map(|radius| {
                    let integer_radius = radius as u32;
                    separable_box_blur_params(integer_radius)
                })
                .flatten()
                .chain(std::iter::once(*passes))
                .collect();
            params
        }

        // ── MedianFilter / MaxFilter / MinFilter: size ──
        PipelineOp::MedianFilter { size }
        | PipelineOp::MaxFilter { size }
        | PipelineOp::MinFilter { size } => vec![*size],

        // ── RankFilter: size, rank ──
        PipelineOp::RankFilter { size, rank } => vec![*size, *rank],

        // ── Filter3x3: kernel [9] f32 bits + offset bits ──
        PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } => {
            let s = if scale.abs() < 1e-10 { 1.0 } else { *scale };
            let mut params = Vec::with_capacity(10);
            for k in kernel.iter() {
                params.push((k / s).to_bits());
            }
            params.push(*offset as u32);
            params
        }

        // ── Filter5x5: kernel [25] f32 bits + offset bits ──
        PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } => {
            let s = if scale.abs() < 1e-10 { 1.0 } else { *scale };
            let mut params = Vec::with_capacity(26);
            for k in kernel.iter() {
                params.push((k / s).to_bits());
            }
            params.push(*offset as u32);
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

        // ── GaussianBlur: radius, box weight, fractional edge weight ──
        PipelineOp::GaussianBlur { sigma } => separable_gaussian_blur_params(*sigma).to_vec(),

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
            "resize_bilinear.wgsl"
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
                    nearest,
                } = op
                {
                    execute_rotate(
                        img, *angle, *expand, *fill, *center, *translate, *nearest, mode,
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
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Reduce { x_factor, y_factor } = op {
                    execute_reduce(img, *x_factor, *y_factor)
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
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Convert {
                    mode: cm,
                    matrix: _,
                    dither: _,
                } = op
                {
                    op_convert(img, cm)
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
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Grayscale) {
                    op_grayscale(img)
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
    // rand() sequentially and performs collision-prone scatter writes. The
    // former per-pixel SIMD LCG and GPU hash/gather paths were different
    // algorithms, so the general scatter contract is deliberately registered
    // on CPU; SIMD adds only the exact distance<=1 identity contract below.
    m.insert(
        "EffectSpread",
        OpEntry::cpu_only(
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
    // Merge is CPU-only: GPU requires multi-band data packing that doesn't fit the
    // current single/dual-image upload infrastructure. The binding layout (4 bindings
    // with extra_bands as storage read) is incompatible with existing LUT/dual-input
    // layout detection. A future enhancement can add a custom binding layout for it.
    m.insert(
        "Merge",
        OpEntry::cpu_only(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Merge { mode, bands } = op {
                    let arc_bands: Vec<std::sync::Arc<crate::image::Image>> = bands
                        .iter()
                        .map(|im| std::sync::Arc::new(im.clone()))
                        .collect();
                    op_merge(img, mode, &arc_bands)
                } else {
                    Err(PilError::ValueError("expected Merge op".into()))
                }
            },
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
        // rejection loop and returns a new L image. SIMD owns the scalar RNG
        // control sequence but vectorizes the per-sample affine/clamp data
        // plane; GPU remains unsupported because it cannot reproduce the
        // process-global stream deterministically.
        OpEntry::cpu_only(
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
                    palette_fill,
                } = op
                {
                    let resolved_fill = palette_fill.map(|index| (index, 0, 0, 255)).or(*fill);
                    op_transform(img, *w, *h, method, data, filter, resolved_fill, mode)
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
        // The former WGSL shader was a pass-through and never uploaded or
        // sampled the LUT. Do not advertise GPU support until that backend
        // implements Pillow's signed fixed-point interpolation exactly.
        OpEntry::cpu_only(
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
        OpEntry::cpu_only(
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

    // ── ImageDraw ops (CPU-only for now) ──
    macro_rules! draw_entry {
        ($key:expr, $func:expr) => {
            m.insert($key, OpEntry::cpu_only($func));
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
