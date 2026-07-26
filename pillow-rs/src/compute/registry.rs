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
//! - [`OpId`] variants are dispatch keys, not public operation names.
//! - Specialized GPU IDs such as [`OpId::ResizeNearest`] and
//!   [`OpId::ResizeBilinear`] may map from one higher-level
//!   [`crate::pipeline::PipelineOp`] variant.

use crate::compute::pool_simd::ops::adapters;
use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp, ResampleFilter, TransposeMethod};
use image_slash_star::DynamicImage;
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
    pub gpu_shader: Option<&'static str>,
    /// Embedded WGSL shader source for GPU execution.
    pub gpu_source: Option<&'static str>,
    /// SIMD implementation, present when a vectorized adapter exists.
    pub simd_fn: Option<SimdOpFn>,
}

impl OpEntry {
    /// Creates a registry entry with only a CPU implementation.
    pub const fn cpu_only(f: CpuOpFn) -> Self {
        OpEntry {
            cpu_fn: Some(f),
            gpu_shader: None,
            gpu_source: None,
            simd_fn: None,
        }
    }
}

// ── Registration macros — one per backend ────────────────────────────────────────

/// Create an OpEntry with CPU function + GPU shader (wgpu — native + WASM WebGPU).
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

// ── GPU op types ──────────────────────────────────────────────────────────────

/// Stable dispatch key for GPU compute pipelines.
///
/// `OpId` is the compact operation identity used after a [`crate::pipeline::PipelineOp`] has
/// been mapped to a shader-compatible form. Variants must stay synchronized
/// with `op_id`, shader registration, parameter extraction, and the WGSL
/// parameter layout for the corresponding operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpId {
    /// Dispatch key for the shader that inverts color channels.
    Invert,
    /// Dispatch key for the shader that converts RGB-family pixels to luma.
    Grayscale,
    /// Dispatch key for the shader that solarizes channels above a threshold.
    Solarize,
    /// Dispatch key for the shader that keeps the high bits of each channel.
    Posterize,
    /// Dispatch key for the shader that applies Pillow brightness enhancement.
    Brightness,
    /// Dispatch key for the shader that applies Pillow contrast enhancement.
    Contrast,
    /// Dispatch key for the shader that adjusts color saturation.
    ColorSaturation,
    /// Dispatch key for the shader that maps luma between two RGB endpoints.
    Colorize,
    /// Dispatch key for the shader that emits a constant byte value.
    Constant,
    /// Dispatch key for the shader that wraps pixels by an x/y offset.
    Offset,
    /// Dispatch key for the shader that flips rows top-to-bottom.
    Flip,
    /// Dispatch key for the shader that mirrors columns left-to-right.
    Mirror,
    /// Dispatch key for the shader that copies pixels without changing them.
    Duplicate,
    /// Dispatch key for the shader that performs ImageChops multiply.
    Multiply,
    /// Dispatch key for the shader that performs ImageChops screen.
    Screen,
    /// Dispatch key for the shader that computes absolute channel difference.
    Difference,
    /// Dispatch key for the shader that keeps per-channel minima.
    Darker,
    /// Dispatch key for the shader that keeps per-channel maxima.
    Lighter,
    /// Dispatch key for the shader that adds channels modulo 256.
    AddModulo,
    /// Dispatch key for the shader that subtracts channels modulo 256.
    SubtractModulo,
    /// Dispatch key for the shader that adds with scale and offset.
    Add,
    /// Dispatch key for the shader that subtracts with scale and offset.
    Subtract,
    /// Dispatch key for the shader that applies bitwise AND.
    LogicalAnd,
    /// Dispatch key for the shader that applies bitwise OR.
    LogicalOr,
    /// Dispatch key for the shader that applies bitwise XOR.
    LogicalXor,
    /// Dispatch key for the shader that performs overlay blending.
    Overlay,
    /// Dispatch key for the shader that performs hard-light blending.
    HardLight,
    /// Dispatch key for the shader that performs soft-light blending.
    SoftLight,
    /// Dispatch key for the shader that blends two images by alpha.
    Blend,
    /// Dispatch key for the module-level blend shader path.
    BlendModule,
    /// Dispatch key for the shader that composites with a mask.
    Composite,
    /// Dispatch key for the module-level composite shader path.
    CompositeModule,
    /// Dispatch key for the shader that applies box blur.
    BoxBlur,
    /// Dispatch key for the shader that applies median filtering.
    MedianFilter,
    /// Dispatch key for the shader that applies maximum filtering.
    MaxFilter,
    /// Dispatch key for the shader that applies minimum filtering.
    MinFilter,
    /// Dispatch key for the shader that applies rank filtering.
    RankFilter,
    /// Dispatch key for the shader that pastes a source image or color.
    Paste,
    /// Dispatch key for the shader that performs alpha compositing.
    AlphaComposite,
    /// Dispatch key for the shader that applies a 3x3 convolution kernel.
    Filter3x3,
    /// Dispatch key for the shader that applies a 5x5 convolution kernel.
    Filter5x5,
    /// Dispatch key for the nearest-neighbor resize shader specialization.
    ResizeNearest,
    /// Dispatch key for the bilinear resize shader specialization.
    ResizeBilinear,
    /// Dispatch key for the shader that evaluates a lookup table.
    Eval,
    /// Dispatch key for the shader that applies point-operation lookup tables.
    PointOp,
    /// Dispatch key for the shader that performs transpose-family operations.
    Transpose,
    /// Dispatch key for the shader that applies sharpness enhancement.
    Sharpen,
    /// Dispatch key for the ImageChops invert shader path.
    InvertChops,
    /// Dispatch key for the shader that scales image dimensions.
    Scale,
    /// Dispatch key for the shader that converts between supported modes.
    Convert,
    /// Dispatch key for the shader that quantizes pixel colors.
    Quantize,
    /// Dispatch key for the shader that replaces or sets alpha.
    PutAlpha,
    /// Dispatch key for the shader that writes a single pixel.
    PutPixel,
    /// Dispatch key for the shader that crops a source rectangle.
    Crop,
    /// Dispatch key for the shader that reduces by an integer factor.
    Reduce,
    /// Dispatch key for the shader that creates a bounded thumbnail.
    Thumbnail,
    /// Dispatch key for the shader that resizes to contain within a box.
    Contain,
    /// Dispatch key for the shader that resizes to cover a box.
    Cover,
    /// Dispatch key for the shader that resizes and crops to fit.
    Fit,
    /// Dispatch key for the shader that applies geometric transforms.
    Transform,
    /// Dispatch key for the shader that replaces raw image data.
    PutData,
    // ── New GPU ops (manifest-driven) ──
    /// Dispatch key for the shader that crops an equal border.
    CropBorder,
    /// Dispatch key for the shader that expands with a border fill.
    Expand,
    /// Dispatch key for the shader that merges bands into one image.
    Merge,
    /// Dispatch key for the shader that remaps palette indices.
    RemapPalette,
    /// Dispatch key for the shader that resizes and pads to a box.
    Pad,
    /// Dispatch key for the shader that rotates an image.
    Rotate,
    /// Dispatch key for the shader that applies a 3D color lookup table.
    Color3DLut,
    // ── Category B fixes (shaders existed, dispatch was broken) ──
    /// Dispatch key for the shader that applies Gaussian blur.
    GaussianBlur,
    /// Dispatch key for the shader that applies autocontrast.
    Autocontrast,
    /// Dispatch key for the shader that equalizes channel histograms.
    Equalize,
    /// Dispatch key for the shader that extracts one band.
    ExtractBand,
    /// Dispatch key for the shader that generates a linear gradient.
    LinearGradient,
    /// Dispatch key for the shader that generates a radial gradient.
    RadialGradient,
    /// Dispatch key for the shader that generates a Mandelbrot image.
    EffectMandelbrot,
}

/// Metadata describing one GPU shader operation.
///
/// `OpDef` is used by GPU routing and validation paths that need the full
/// shader-level contract after a higher-level pipeline operation has been
/// mapped to an [`OpId`].
#[derive(Debug, Clone)]
pub struct OpDef {
    /// Stable GPU operation identifier used by shader dispatch.
    pub id: OpId,
    /// Pipeline variant name associated with this GPU definition.
    pub variant_name: &'static str,
    /// Embedded WGSL shader source.
    pub shader_source: &'static str,
    /// Number of input images expected by the shader.
    pub input_count: u8,
    /// Whether the shader consumes an operation-specific parameter block.
    pub has_params: bool,
    /// Whether execution requires multiple shader passes.
    pub is_multi_pass: bool,
    /// Number of shader passes when [`OpDef::is_multi_pass`] is true.
    pub pass_count: u8,
}

/// Shader descriptor returned after a pipeline operation is GPU-compatible.
pub struct GpuOp {
    /// Pipeline variant name.
    pub variant_name: &'static str,
    /// Embedded WGSL shader source.
    pub shader: &'static str,
}

// GPU registry — populated by op_map.rs via `build_registry`.
static GPU_REGISTRY: OnceLock<std::sync::Mutex<Vec<OpDef>>> = OnceLock::new();

fn gpu_registry_inner() -> &'static std::sync::Mutex<Vec<OpDef>> {
    GPU_REGISTRY.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Builds the auxiliary GPU operation registry from shader definitions.
///
/// This registry is separate from the CPU/SIMD operation map. It is used by GPU
/// setup and validation paths that need [`OpDef`] metadata.
pub fn build_registry(defs: Vec<OpDef>) -> Result<(), PilError> {
    let mut r = gpu_registry_inner()
        .lock()
        .map_err(|_| PilError::InternalError("GPU registry mutex poisoned".to_string()))?;
    *r = defs;
    Ok(())
}

/// Returns all registered GPU operation definitions.
pub fn get_registry() -> Result<Vec<OpDef>, PilError> {
    Ok(gpu_registry_inner()
        .lock()
        .map_err(|_| PilError::InternalError("GPU registry mutex poisoned".to_string()))?
        .clone())
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
        PipelineOp::Blend { .. } => "Blend",
        PipelineOp::Composite { .. } => "Composite",
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
pub fn gpu_supports(op: &PipelineOp) -> Result<bool, PilError> {
    Ok(registry()?
        .get(variant_key(op))
        .is_some_and(|e| e.gpu_shader.is_some()))
}

/// Returns whether the SIMD backend has an implementation for `op`.
pub fn simd_supports(op: &PipelineOp) -> Result<bool, PilError> {
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

/// Returns the registered GPU shader file name for `op`.
pub fn gpu_shader_name(op: &PipelineOp) -> Result<Option<&'static str>, PilError> {
    Ok(registry()?.get(variant_key(op)).and_then(|e| e.gpu_shader))
}

/// Returns embedded WGSL source for a registered variant key.
pub fn gpu_shader_source_for_key(key: &str) -> Result<Option<&'static str>, PilError> {
    Ok(registry()?.get(key).and_then(|e| e.gpu_source))
}

/// Returns embedded WGSL source for the GPU implementation of `op`.
pub fn gpu_shader_source(op: &PipelineOp) -> Result<Option<&'static str>, PilError> {
    gpu_shader_source_for_key(variant_key(op))
}

/// Maps a pipeline operation to its GPU dispatch ID.
///
/// Returns `None` when the operation has no GPU specialization or cannot be
/// represented by a single shader dispatch ID.
pub fn op_id(op: &PipelineOp) -> Option<OpId> {
    match op {
        PipelineOp::Resize { filter, .. } => match filter {
            ResampleFilter::Nearest | ResampleFilter::Box => Some(OpId::ResizeNearest),
            _ => Some(OpId::ResizeBilinear),
        },
        PipelineOp::Transpose { .. } => Some(OpId::Transpose),
        PipelineOp::Filter3x3 { .. } => Some(OpId::Filter3x3),
        PipelineOp::Filter5x5 { .. } => Some(OpId::Filter5x5),
        PipelineOp::BoxBlur { .. } => Some(OpId::BoxBlur),
        PipelineOp::MedianFilter { .. } => Some(OpId::MedianFilter),
        PipelineOp::MaxFilter { .. } => Some(OpId::MaxFilter),
        PipelineOp::MinFilter { .. } => Some(OpId::MinFilter),
        PipelineOp::RankFilter { .. } => Some(OpId::RankFilter),
        PipelineOp::Invert => Some(OpId::Invert),
        PipelineOp::Flip => Some(OpId::Flip),
        PipelineOp::Mirror => Some(OpId::Mirror),
        PipelineOp::Posterize { .. } => Some(OpId::Posterize),
        PipelineOp::Solarize { .. } => Some(OpId::Solarize),
        PipelineOp::Grayscale => Some(OpId::Grayscale),
        PipelineOp::Colorize { .. } => Some(OpId::Colorize),
        PipelineOp::Scale { .. } => Some(OpId::Scale),
        PipelineOp::Add { .. } => Some(OpId::Add),
        PipelineOp::Subtract { .. } => Some(OpId::Subtract),
        PipelineOp::Multiply { .. } => Some(OpId::Multiply),
        PipelineOp::Screen { .. } => Some(OpId::Screen),
        PipelineOp::Darker { .. } => Some(OpId::Darker),
        PipelineOp::Lighter { .. } => Some(OpId::Lighter),
        PipelineOp::Difference { .. } => Some(OpId::Difference),
        PipelineOp::Overlay { .. } => Some(OpId::Overlay),
        PipelineOp::HardLight { .. } => Some(OpId::HardLight),
        PipelineOp::SoftLight { .. } => Some(OpId::SoftLight),
        PipelineOp::AddModulo { .. } => Some(OpId::AddModulo),
        PipelineOp::SubtractModulo { .. } => Some(OpId::SubtractModulo),
        PipelineOp::LogicalAnd { .. } => Some(OpId::LogicalAnd),
        PipelineOp::LogicalOr { .. } => Some(OpId::LogicalOr),
        PipelineOp::LogicalXor { .. } => Some(OpId::LogicalXor),
        PipelineOp::Constant { .. } => Some(OpId::Constant),
        PipelineOp::Offset { .. } => Some(OpId::Offset),
        PipelineOp::Blend { .. } => Some(OpId::Blend),
        PipelineOp::Composite { .. } => Some(OpId::Composite),
        PipelineOp::Duplicate => Some(OpId::Duplicate),
        PipelineOp::InvertChops => Some(OpId::InvertChops),
        PipelineOp::Brightness { .. } => Some(OpId::Brightness),
        PipelineOp::Contrast { .. } => Some(OpId::Contrast),
        PipelineOp::ColorSaturation { .. } => Some(OpId::ColorSaturation),
        PipelineOp::Sharpness { .. } => Some(OpId::Sharpen),
        PipelineOp::Paste { .. } => Some(OpId::Paste),
        PipelineOp::AlphaComposite { .. } => Some(OpId::AlphaComposite),
        PipelineOp::BlendModule { .. } => Some(OpId::BlendModule),
        PipelineOp::CompositeModule { .. } => Some(OpId::CompositeModule),
        PipelineOp::Eval { .. } => Some(OpId::Eval),
        PipelineOp::PointOp { .. } => Some(OpId::PointOp),
        PipelineOp::Convert { .. } => Some(OpId::Convert),
        PipelineOp::Quantize { .. } => Some(OpId::Quantize),
        PipelineOp::Crop { .. } => Some(OpId::Crop),
        PipelineOp::Reduce { .. } => Some(OpId::Reduce),
        PipelineOp::Thumbnail { .. } => Some(OpId::Thumbnail),
        PipelineOp::Contain { .. } => Some(OpId::Contain),
        PipelineOp::Cover { .. } => Some(OpId::Cover),
        PipelineOp::Fit { .. } => Some(OpId::Fit),
        PipelineOp::Transform { .. } => Some(OpId::Transform),
        PipelineOp::PutPixel { .. } => Some(OpId::PutPixel),
        PipelineOp::PutData { .. } => Some(OpId::PutData),
        PipelineOp::PutAlpha { .. } => Some(OpId::PutAlpha),
        // ── New GPU ops ──
        PipelineOp::CropBorder { .. } => Some(OpId::CropBorder),
        PipelineOp::Expand { .. } => Some(OpId::Expand),
        PipelineOp::Merge { .. } => Some(OpId::Merge),
        PipelineOp::RemapPalette { .. } => Some(OpId::RemapPalette),
        PipelineOp::Pad { .. } => Some(OpId::Pad),
        PipelineOp::Rotate { .. } => Some(OpId::Rotate),
        // ── Category B fixes ──
        PipelineOp::GaussianBlur { .. } => Some(OpId::GaussianBlur),
        PipelineOp::Autocontrast { .. } => Some(OpId::Autocontrast),
        PipelineOp::Equalize => Some(OpId::Equalize),
        PipelineOp::Color3DLut { .. } => Some(OpId::Color3DLut),
        PipelineOp::ExtractBand { .. } => Some(OpId::ExtractBand),
        PipelineOp::LinearGradient { .. } => Some(OpId::LinearGradient),
        PipelineOp::RadialGradient { .. } => Some(OpId::RadialGradient),
        PipelineOp::EffectMandelbrot { .. } => Some(OpId::EffectMandelbrot),
        _ => None,
    }
}

/// Returns a shader descriptor when `op` is GPU-supported.
pub fn map_op_to_gpu(op: &PipelineOp) -> Result<Option<GpuOp>, PilError> {
    let Some(_id) = op_id(op) else {
        return Ok(None);
    };
    let Some(shader) = gpu_shader_name(op)? else {
        return Ok(None);
    };
    Ok(Some(GpuOp {
        variant_name: variant_key(op),
        shader,
    }))
}

/// Extracts GPU shader parameter words from a pipeline operation.
///
/// The returned `Vec<u32>` follows each shader's `Params` struct after the
/// shared four-word header: `width`, `height`, `pad0`, `pad1`. Operations with
/// no shader parameter block return an empty vector.
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
        | PipelineOp::SoftLight { .. }
        | PipelineOp::Composite { .. } => vec![],

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

        // ── Colorize: pack black and white as u32 RGBA (0xAABBGGRR) ──
        PipelineOp::Colorize { black, white } => {
            let bk = (black.0 as u32)
                | ((black.1 as u32) << 8)
                | ((black.2 as u32) << 16)
                | (0xff << 24);
            let wh = (white.0 as u32)
                | ((white.1 as u32) << 8)
                | ((white.2 as u32) << 16)
                | (0xff << 24);
            vec![bk, wh]
        }

        // ── Constant: value ──
        PipelineOp::Constant { value } => vec![*value as u32],

        // ── Offset: dx, dy ──
        PipelineOp::Offset { x, y } => {
            vec![x.rem_euclid(65536) as u32, y.rem_euclid(65536) as u32]
        }

        // ── Add / Subtract: scale (f32 bits as u32), offset (f32 bits as u32) ──
        PipelineOp::Add { scale, offset, .. } | PipelineOp::Subtract { scale, offset, .. } => {
            vec![(*scale as f32).to_bits(), (*offset as f32).to_bits()]
        }

        // ── Blend / BlendModule: alpha * 255 as u32 ──
        PipelineOp::Blend { alpha, .. } | PipelineOp::BlendModule { alpha, .. } => {
            vec![(alpha.clamp(0.0, 1.0) * 255.0) as u32]
        }

        // ── BoxBlur: radius ──
        PipelineOp::BoxBlur { radius } => vec![*radius],

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

        // ── Reduce: factor ──
        PipelineOp::Reduce { factor } => vec![*factor],

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

        // ── GaussianBlur: sigma ──
        PipelineOp::GaussianBlur { sigma } => vec![(*sigma).to_bits()],

        // ── Autocontrast: cutoff ──
        PipelineOp::Autocontrast { cutoff } => vec![(*cutoff as f32).to_bits()],

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
        op_chops_add, op_chops_add_modulo, op_chops_blend, op_chops_composite, op_chops_constant,
        op_chops_darker, op_chops_difference, op_chops_duplicate, op_chops_hard_light,
        op_chops_invert, op_chops_lighter, op_chops_logical_and, op_chops_logical_or,
        op_chops_logical_xor, op_chops_multiply, op_chops_offset, op_chops_overlay,
        op_chops_screen, op_chops_soft_light, op_chops_subtract, op_chops_subtract_modulo,
    };
    use crate::compute::pool_cpu::ops::color::{
        op_convert, op_extract_band, op_quantize, op_remap_palette,
    };
    use crate::compute::pool_cpu::ops::draw::{
        op_draw_arc, op_draw_chord, op_draw_circle, op_draw_ellipse, op_draw_line,
        op_draw_pieslice, op_draw_point, op_draw_polygon, op_draw_rectangle, op_draw_rounded_rect,
    };
    use crate::compute::pool_cpu::ops::effects::{
        op_alpha_composite, op_blend_module, op_color3dlut, op_composite_module,
        op_effect_mandelbrot, op_effect_noise, op_effect_spread, op_eval, op_merge, op_paste,
        op_point, op_put_alpha, op_put_data, op_put_pixel, op_transform,
    };
    use crate::compute::pool_cpu::ops::enhance::{
        op_enhance_brightness, op_enhance_color_saturation, op_enhance_contrast,
        op_enhance_sharpness,
    };
    use crate::compute::pool_cpu::ops::filter::{
        execute_box_blur, execute_filter3x3, execute_filter5x5, execute_gaussian_blur,
        execute_max_filter_with_mode, execute_median_filter_with_mode,
        execute_min_filter_with_mode, execute_rank_filter_with_mode,
    };
    use crate::compute::pool_cpu::ops::geometry::{
        execute_crop, execute_reduce, execute_resize, execute_rotate, execute_thumbnail,
        execute_transpose,
    };
    use crate::compute::pool_cpu::ops::imageops::{
        op_autocontrast, op_colorize, op_contain, op_cover, op_crop_border, op_equalize, op_expand,
        op_fit, op_flip, op_grayscale, op_invert, op_linear_gradient, op_mirror, op_pad,
        op_posterize, op_radial_gradient, op_scale, op_solarize,
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
                } = op
                {
                    execute_rotate(img, *angle, *expand, *fill, mode)
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
                if let PipelineOp::Reduce { factor } = op {
                    execute_reduce(img, *factor)
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
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Convert {
                    mode: cm,
                    matrix: _,
                    dither,
                } = op
                {
                    op_convert(img, cm, dither.as_ref(), mode, None)
                } else {
                    Err(PilError::ValueError("expected Convert op".into()))
                }
            },
            "convert.wgsl"
        ),
    );
    m.insert(
        "Quantize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Quantize { colors, dither } = op {
                    let _ = dither;
                    op_quantize(img, *colors as usize, None)
                } else {
                    Err(PilError::ValueError("expected Quantize op".into()))
                }
            },
            "quantize.wgsl"
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
            "box_blur.wgsl"
        ),
    );
    m.insert(
        "BoxBlur",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::BoxBlur { radius } = op {
                    execute_box_blur(img, *radius)
                } else {
                    Err(PilError::ValueError("expected BoxBlur op".into()))
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
                if let PipelineOp::Autocontrast { cutoff } = op {
                    op_autocontrast(img, *cutoff)
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
                if let PipelineOp::Colorize { black, white } = op {
                    op_colorize(img, black, white)
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
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Expand { border, fill } = op {
                    op_expand(img, *border, *fill)
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
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Offset { x, y } = op {
                    Ok(op_chops_offset(img, *x, *y))
                } else {
                    Err(PilError::ValueError("expected Offset op".into()))
                }
            },
            "offset.wgsl"
        ),
    );
    m.insert(
        "Blend",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Blend { other, alpha } = op {
                    op_chops_blend(img, other, *alpha)
                } else {
                    Err(PilError::ValueError("expected Blend op".into()))
                }
            },
            "blend.wgsl"
        ),
    );
    m.insert(
        "Composite",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Composite { other, mask } = op {
                    op_chops_composite(img, other, mask)
                } else {
                    Err(PilError::ValueError("expected Composite op".into()))
                }
            },
            "composite.wgsl"
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
    // algorithms, so EffectSpread is deliberately registered on CPU only.
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
        // rejection loop and returns a new L image. The per-pixel GPU/SIMD
        // implementations used independent hashes and preserved the input
        // mode, so advertising those paths returned non-Pillow results.
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
                    op_point(img, lut)
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

    // ── ExtractBand (CPU-only for now, GPU/SIMD later) ──
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

    // ── LinearGradient (CPU-only for now) ──
    m.insert(
        "LinearGradient",
        gpu_entry!(
            |_img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::LinearGradient { mode } = op {
                    op_linear_gradient(mode)
                } else {
                    Err(PilError::ValueError("expected LinearGradient op".into()))
                }
            },
            "linear_gradient.wgsl"
        ),
    );

    // ── RadialGradient ──
    m.insert(
        "RadialGradient",
        gpu_entry!(
            |_img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::RadialGradient { mode } = op {
                    op_radial_gradient(mode)
                } else {
                    Err(PilError::ValueError("expected RadialGradient op".into()))
                }
            },
            "radial_gradient.wgsl"
        ),
    );

    // ── EffectMandelbrot ──
    m.insert(
        "EffectMandelbrot",
        gpu_entry!(
            |_img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::EffectMandelbrot {
                    w,
                    h,
                    x0,
                    y0,
                    x1,
                    y1,
                    quality,
                } = op
                {
                    op_effect_mandelbrot(*w, *h, *x0, *y0, *x1, *y1, *quality)
                } else {
                    Err(PilError::ValueError("expected EffectMandelbrot op".into()))
                }
            },
            "effect_mandelbrot.wgsl"
        ),
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
        } = op
        {
            op_draw_line(img, *x0, *y0, *x1, *y1, *fill, *width, mode)
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
        } = op
        {
            op_draw_rectangle(img, *x0, *y0, *x1, *y1, *fill, *outline, *width, mode)
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
        } = op
        {
            op_draw_rounded_rect(
                img, *x0, *y0, *x1, *y1, *radius, *fill, *outline, *width, mode,
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
        } = op
        {
            op_draw_ellipse(img, *x0, *y0, *x1, *y1, *fill, *outline, *width, mode)
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
        } = op
        {
            op_draw_circle(img, *cx, *cy, *radius, *fill, *outline, *width, mode)
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
        } = op
        {
            op_draw_polygon(img, points, *fill, *outline, *width, mode)
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
        } = op
        {
            op_draw_arc(img, *x0, *y0, *x1, *y1, *start, *end, *fill, *width, mode)
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
        } = op
        {
            op_draw_chord(
                img, *x0, *y0, *x1, *y1, *start, *end, *fill, *outline, *width, mode,
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
        } = op
        {
            op_draw_pieslice(
                img, *x0, *y0, *x1, *y1, *start, *end, *fill, *outline, *width, mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawPieslice".into()))
        }
    });
    draw_entry!("DrawPoint", |img, op, mode| {
        if let PipelineOp::DrawPoint { points, fill } = op {
            op_draw_point(img, points, *fill, mode)
        } else {
            Err(PilError::ValueError("expected DrawPoint".into()))
        }
    });

    // ── SIMD registrations ───────────────────────────────────────────
    // Register SIMD-accelerated functions for all backed scalar ops.
    // The simd_fn slot is added alongside existing cpu_fn + gpu_shader.

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

    // Section A: Simple single-image ops
    simd_set(m, "Invert", adapters::simd_invert)?;
    simd_set(m, "Grayscale", adapters::simd_grayscale)?;
    simd_set(m, "Duplicate", adapters::simd_duplicate)?;
    simd_set(m, "InvertChops", adapters::simd_invert_chops)?;

    // Section B: Single-image with params
    simd_set(m, "Solarize", adapters::simd_solarize)?;
    simd_set(m, "Posterize", adapters::simd_posterize)?;
    simd_set(m, "Brightness", adapters::simd_brightness)?;
    simd_set(m, "Contrast", adapters::simd_contrast)?;
    simd_set(m, "ColorSaturation", adapters::simd_color_saturation)?;
    simd_set(m, "Sharpness", adapters::simd_sharpness)?;
    simd_set(m, "Colorize", adapters::simd_colorize)?;
    simd_set(m, "Constant", adapters::simd_constant)?;
    simd_set(m, "Offset", adapters::simd_offset)?;

    // Section C: Spatial single-image
    simd_set(m, "Flip", adapters::simd_flip)?;
    simd_set(m, "Mirror", adapters::simd_mirror)?;
    simd_set(m, "Equalize", adapters::simd_equalize)?;
    simd_set(m, "Autocontrast", adapters::simd_autocontrast)?;
    // Section D: Filter/window ops
    simd_set(m, "MedianFilter", adapters::simd_median_filter)?;
    simd_set(m, "MaxFilter", adapters::simd_max_filter)?;
    simd_set(m, "MinFilter", adapters::simd_min_filter)?;
    simd_set(m, "RankFilter", adapters::simd_rank_filter)?;
    simd_set(m, "Filter3x3", adapters::simd_filter_3x3)?;
    simd_set(m, "Filter5x5", adapters::simd_filter_5x5)?;
    simd_set(m, "BoxBlur", adapters::simd_box_blur)?;
    simd_set(m, "GaussianBlur", adapters::simd_gaussian_blur)?;
    simd_set(m, "Quantize", adapters::simd_quantize)?;

    // Section E: Dual-image per-pixel ops
    simd_set(m, "Add", adapters::simd_add)?;
    simd_set(m, "Subtract", adapters::simd_subtract)?;
    simd_set(m, "Multiply", adapters::simd_multiply)?;
    simd_set(m, "Screen", adapters::simd_screen)?;
    simd_set(m, "Darker", adapters::simd_darker)?;
    simd_set(m, "Lighter", adapters::simd_lighter)?;
    simd_set(m, "Difference", adapters::simd_difference)?;
    simd_set(m, "AddModulo", adapters::simd_add_modulo)?;
    simd_set(m, "SubtractModulo", adapters::simd_subtract_modulo)?;
    simd_set(m, "LogicalAnd", adapters::simd_logical_and)?;
    simd_set(m, "LogicalOr", adapters::simd_logical_or)?;
    simd_set(m, "LogicalXor", adapters::simd_logical_xor)?;
    simd_set(m, "Overlay", adapters::simd_overlay)?;
    simd_set(m, "HardLight", adapters::simd_hard_light)?;
    simd_set(m, "SoftLight", adapters::simd_soft_light)?;
    simd_set(m, "Blend", adapters::simd_blend)?;
    simd_set(m, "BlendModule", adapters::simd_blend_module)?;
    simd_set(m, "Composite", adapters::simd_composite)?;
    simd_set(m, "CompositeModule", adapters::simd_composite_module)?;

    // Section F: Ops that change dimensions
    simd_set(m, "Transpose", adapters::simd_transpose)?;
    simd_set(m, "Resize", adapters::simd_resize)?;
    simd_set(m, "Thumbnail", adapters::simd_thumbnail)?;
    simd_set(m, "Contain", adapters::simd_contain)?;
    simd_set(m, "Cover", adapters::simd_cover)?;
    simd_set(m, "Fit", adapters::simd_fit)?;
    simd_set(m, "Scale", adapters::simd_scale)?;
    simd_set(m, "Pad", adapters::simd_pad)?;
    simd_set(m, "Expand", adapters::simd_expand)?;
    simd_set(m, "CropBorder", adapters::simd_crop_border)?;
    simd_set(m, "Crop", adapters::simd_crop)?;
    simd_set(m, "Rotate", adapters::simd_rotate)?;
    simd_set(m, "Reduce", adapters::simd_reduce)?;
    simd_set(m, "Convert", adapters::simd_convert)?;
    simd_set(m, "RemapPalette", adapters::simd_remap_palette)?;
    simd_set(m, "Transform", adapters::simd_transform)?;

    // Section G: Special/mutating ops
    simd_set(m, "PutPixel", adapters::simd_put_pixel)?;
    simd_set(m, "PutData", adapters::simd_put_data)?;
    simd_set(m, "PutAlpha", adapters::simd_put_alpha)?;
    simd_set(m, "Eval", adapters::simd_eval)?;
    simd_set(m, "PointOp", adapters::simd_point_op)?;
    simd_set(m, "Paste", adapters::simd_paste)?;
    simd_set(m, "AlphaComposite", adapters::simd_alpha_composite)?;
    simd_set(m, "Merge", adapters::simd_merge)?;
    // ── Additional SIMD wirings (GPU ops missing SIMD) ──
    simd_set(m, "Autocontrast", adapters::simd_autocontrast)?;
    simd_set(m, "BlendModule", adapters::simd_blend_module)?;
    simd_set(m, "CompositeModule", adapters::simd_composite_module)?;
    simd_set(m, "ColorSaturation", adapters::simd_color_saturation)?;
    simd_set(m, "GaussianBlur", adapters::simd_gaussian_blur)?;
    simd_set(m, "InvertChops", adapters::simd_invert_chops)?;
    simd_set(m, "MedianFilter", adapters::simd_median_filter)?;
    simd_set(m, "RemapPalette", adapters::simd_remap_palette)?;
    simd_set(m, "SubtractModulo", adapters::simd_subtract_modulo)?;
    Ok(())
}
