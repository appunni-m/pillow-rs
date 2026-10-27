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

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use std::collections::HashMap;
use std::sync::OnceLock;

#[cfg(feature = "gpu")]
mod gpu_params;
#[cfg(feature = "gpu")]
mod gpu_support;
mod registration;
#[cfg(feature = "gpu")]
pub use gpu_params::extract_params;
#[cfg(feature = "gpu")]
pub(crate) use gpu_params::{separable_box_blur_params_f32, separable_gaussian_blur_radius};
#[cfg(feature = "gpu")]
pub use gpu_support::gpu_supports;
#[cfg(feature = "gpu")]
pub(crate) use gpu_support::{
    filter_offset_i32, gpu_blend_alpha_params, gpu_brightness_factor_int,
    gpu_chops_affine_supported, gpu_color_saturation_factor_params, gpu_filter_kernel_is_exact,
    gpu_filter_rational_denominator, gpu_sharpness_factor_int,
};

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
    #[cfg(not(feature = "gpu"))]
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
        PipelineOp::Resize { .. } | PipelineOp::ResizeBoxed { .. } => "Resize",
        PipelineOp::Crop { .. } => "Crop",
        PipelineOp::Rotate { .. } => "Rotate",
        PipelineOp::Transpose { .. } => "Transpose",
        PipelineOp::Thumbnail { .. } => "Thumbnail",
        PipelineOp::Reduce { .. } => "Reduce",
        PipelineOp::Convert { .. } => "Convert",
        PipelineOp::ConvertLab { .. } => "ConvertLab",
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
        PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => "Equalize",
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
        PipelineOp::Transform { .. } => "Transform",
        PipelineOp::Color3DLut { .. } => "Color3DLut",
        PipelineOp::PutPixel { .. } => "PutPixel",
        PipelineOp::PutData { .. } => "PutData",
        PipelineOp::PutAlpha { .. } => "PutAlpha",
        PipelineOp::PutAlphaData { .. } => "PutAlphaData",
        PipelineOp::ExtractBand { .. } => "ExtractBand",
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

fn simd_native_operation(op: &PipelineOp) -> bool {
    // Keep this operation-level answer deliberately narrow.  A registered
    // adapter may still exist as a workbench for a future kernel, but it is
    // not a SIMD capability until its data plane is vectorized.  Contextual
    // support below then applies the image layout, logical mode, dimensions,
    // and secondary-operand checks.
    matches!(
        op,
        PipelineOp::Invert
            | PipelineOp::InvertChops
            | PipelineOp::Resize { .. }
            | PipelineOp::ResizeBoxed { .. }
            | PipelineOp::Scale { .. }
            | PipelineOp::Thumbnail { .. }
            | PipelineOp::Contain { .. }
            | PipelineOp::Cover { .. }
            | PipelineOp::Fit { .. }
            | PipelineOp::Transform { .. }
            | PipelineOp::Pad { .. }
            | PipelineOp::Convert { .. }
            | PipelineOp::ConvertLab { .. }
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
            | PipelineOp::EqualizeMasked { .. }
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
    )
}

/// Returns whether the SIMD backend has an implementation for `op`.
pub fn simd_supports(op: &PipelineOp) -> Result<bool, PilError> {
    if !simd_native_operation(op) {
        return Ok(false);
    }
    Ok(registry()?
        .get(variant_key(op))
        .is_some_and(|entry| entry.simd_fn.is_some()))
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

fn register_all(m: &mut HashMap<&'static str, OpEntry>) -> Result<(), PilError> {
    registration::register_all(m)
}
