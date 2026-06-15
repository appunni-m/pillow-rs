//! Unified operation registry — maps every PipelineOp to its per-backend implementation.
//!
//! CPU ops live in `pool_cpu/ops/`, GPU shaders in `pool_gpu/shaders/`,
//! WebGPU shaders in `pool_webgpu/shaders/`, SIMD ops in `pool_simd/ops/`.
//! All backends query this single registry for `supports()` and `execute()`.
//!
//! Performance: `variant_key()` returns a `&'static str` for O(1) HashMap lookup.
//! No allocations on the hot path.

use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp, ResampleFilter, TransposeMethod};
use image::DynamicImage;
use std::collections::HashMap;
use std::sync::OnceLock;

// ── Op function types ────────────────────────────────────────────────────────────

pub type CpuOpFn =
    fn(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> Result<DynamicImage, PilError>;

/// SIMD operation function — same signature as CPU but vectorized internally.
pub type SimdOpFn =
    fn(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> Result<DynamicImage, PilError>;

// ── OpEntry — one entry per operation, all backends ──────────────────────────────

pub struct OpEntry {
    pub cpu_fn: Option<CpuOpFn>,
    pub gpu_shader: Option<&'static str>,
    pub gpu_source: Option<&'static str>,
    pub simd_fn: Option<SimdOpFn>,
}

impl OpEntry {
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

/// Create an OpEntry with CPU function + SIMD accelerated function.
macro_rules! simd_entry {
    ($cpu:expr, $simd:expr) => {
        $crate::compute::registry::OpEntry {
            cpu_fn: Some($cpu as $crate::compute::registry::CpuOpFn),
            gpu_shader: None,
            gpu_source: None,
            simd_fn: Some($simd as $crate::compute::registry::SimdOpFn),
        }
    };
}

pub(crate) use gpu_entry;
pub(crate) use simd_entry;

static REGISTRY: OnceLock<HashMap<&'static str, OpEntry>> = OnceLock::new();

pub fn registry() -> &'static HashMap<&'static str, OpEntry> {
    REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        register_all(&mut m);
        m
    })
}

// ── GPU op types ──────────────────────────────────────────────────────────────

/// Operation identifier for GPU compute pipelines.
/// Each variant maps to a compiled wgpu shader pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpId {
    Invert,
    Grayscale,
    Solarize,
    Posterize,
    Brightness,
    Contrast,
    ColorSaturation,
    Colorize,
    Constant,
    Offset,
    Flip,
    Mirror,
    Duplicate,
    Multiply,
    Screen,
    Difference,
    Darker,
    Lighter,
    AddModulo,
    SubtractModulo,
    Add,
    Subtract,
    LogicalAnd,
    LogicalOr,
    LogicalXor,
    Overlay,
    HardLight,
    SoftLight,
    Blend,
    BlendModule,
    Composite,
    CompositeModule,
    BoxBlur,
    MedianFilter,
    MaxFilter,
    MinFilter,
    RankFilter,
    Paste,
    AlphaComposite,
    EffectSpread,
    Filter3x3,
    Filter5x5,
    ResizeNearest,
    ResizeBilinear,
    Eval,
    PointOp,
    Transpose,
    Sharpen,
    InvertChops,
    Scale,
    Convert,
    Quantize,
    EffectNoise,
    PutAlpha,
    PutPixel,
    Crop,
    Reduce,
    Thumbnail,
    Contain,
    Cover,
    Fit,
    Transform,
    PutData,
}

/// GPU operation definition — holds compiled shader metadata.
#[derive(Debug, Clone)]
pub struct OpDef {
    pub id: OpId,
    pub variant_name: &'static str,
    pub shader_source: &'static str,
    pub input_count: u8,
    pub has_params: bool,
    pub is_multi_pass: bool,
    pub pass_count: u8,
}

/// GPU operation descriptor returned by `map_op_to_gpu`.
pub struct GpuOp {
    pub variant_name: &'static str,
    pub shader: &'static str,
}

// GPU registry — populated by op_map.rs via `build_registry`.
static GPU_REGISTRY: OnceLock<std::sync::Mutex<Vec<OpDef>>> = OnceLock::new();

fn gpu_registry_inner() -> &'static std::sync::Mutex<Vec<OpDef>> {
    GPU_REGISTRY.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Build the GPU operation registry from a list of definitions.
/// Called by `op_map::init()` at startup.
pub fn build_registry(defs: Vec<OpDef>) {
    let mut r = gpu_registry_inner().lock().unwrap();
    *r = defs;
}

/// Get all registered GPU operation definitions.
pub fn get_registry() -> Vec<OpDef> {
    gpu_registry_inner().lock().unwrap().clone()
}

// ── Lookup helpers ────────────────────────────────────────────────────────────

/// O(1) lookup — returns the static key for any PipelineOp variant.
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
        PipelineOp::PutPixel { .. } => "PutPixel",
        PipelineOp::PutData { .. } => "PutData",
        PipelineOp::PutAlpha { .. } => "PutAlpha",
    }
}

pub fn cpu_supports(op: &PipelineOp) -> bool {
    registry()
        .get(variant_key(op))
        .is_some_and(|e| e.cpu_fn.is_some())
}

pub fn gpu_supports(op: &PipelineOp) -> bool {
    registry()
        .get(variant_key(op))
        .is_some_and(|e| e.gpu_shader.is_some())
}

pub fn simd_supports(op: &PipelineOp) -> bool {
    registry()
        .get(variant_key(op))
        .is_some_and(|e| e.simd_fn.is_some())
}

pub fn execute_cpu(
    op: &PipelineOp,
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let key = variant_key(op);
    let entry = registry()
        .get(key)
        .ok_or_else(|| PilError::ValueError(format!("CPU: unknown op {}", key)))?;
    let f = entry
        .cpu_fn
        .ok_or_else(|| PilError::ValueError(format!("CPU: no impl for {}", key)))?;
    f(img, op, mode)
}

pub fn gpu_shader_name(op: &PipelineOp) -> Option<&'static str> {
    registry().get(variant_key(op)).and_then(|e| e.gpu_shader)
}

/// Get the embedded WGSL source for a GPU shader by variant key.
pub fn gpu_shader_source_for_key(key: &str) -> Option<&'static str> {
    registry().get(key).and_then(|e| e.gpu_source)
}

/// Get the embedded WGSL source for a GPU shader.
pub fn gpu_shader_source(op: &PipelineOp) -> Option<&'static str> {
    gpu_shader_source_for_key(variant_key(op))
}

/// Map a PipelineOp to its GPU OpId, if a GPU implementation exists.
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
        PipelineOp::EffectSpread { .. } => Some(OpId::EffectSpread),
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
        PipelineOp::EffectNoise { .. } => Some(OpId::EffectNoise),
        PipelineOp::Transform { .. } => Some(OpId::Transform),
        PipelineOp::PutPixel { .. } => Some(OpId::PutPixel),
        PipelineOp::PutData { .. } => Some(OpId::PutData),
        PipelineOp::PutAlpha { .. } => Some(OpId::PutAlpha),
        _ => None,
    }
}

/// Map a PipelineOp to its GpuOp descriptor (returns Some if GPU-supported).
pub fn map_op_to_gpu(op: &PipelineOp) -> Option<GpuOp> {
    let _id = op_id(op)?;
    let shader = gpu_shader_name(op)?;
    Some(GpuOp {
        variant_name: variant_key(op),
        shader,
    })
}

/// Extract GPU compute shader parameters from a PipelineOp.
/// The returned Vec<u32> follows each shader's Params struct (fields after the
/// 4-element header: width, height, pad0, pad1).
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
        | PipelineOp::Composite { .. }
        | PipelineOp::CompositeModule { .. } => vec![],

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

        // ── EffectSpread: distance ──
        PipelineOp::EffectSpread { distance } => vec![*distance],

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

        // ── EffectNoise: sigma bits, seed ──
        PipelineOp::EffectNoise { sigma } => {
            vec![(*sigma as f32).to_bits(), 0]
        }

        // ── PutAlpha: alpha as u32 ──
        PipelineOp::PutAlpha { alpha } => vec![*alpha as u32],

        // ── PutPixel: x, y, color packed as RGBA u32 ──
        PipelineOp::PutPixel { x, y, color } => {
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

        // ── Paste: src_w, src_h, paste_x, paste_y, has_mask ──
        PipelineOp::Paste {
            w, h, x, y, mask, ..
        } => {
            vec![
                (*w).max(0) as u32,
                (*h).max(0) as u32,
                (*x).max(0) as u32,
                (*y).max(0) as u32,
                mask.is_some() as u32,
            ]
        }

        // ── Transform: dst_w, dst_h, a..f bits, fill_color, filter_code ──
        PipelineOp::Transform {
            w,
            h,
            data,
            filter,
            fill,
            ..
        } => {
            let a = (data.first().copied().unwrap_or(0.0) as f32).to_bits();
            let b = (data.get(1).copied().unwrap_or(0.0) as f32).to_bits();
            let c = (data.get(2).copied().unwrap_or(0.0) as f32).to_bits();
            let d = (data.get(3).copied().unwrap_or(0.0) as f32).to_bits();
            let e = (data.get(4).copied().unwrap_or(0.0) as f32).to_bits();
            let f = (data.get(5).copied().unwrap_or(0.0) as f32).to_bits();
            let fill_color = match fill {
                Some((r, g, b, a)) => {
                    (*r as u32) | ((*g as u32) << 8) | ((*b as u32) << 16) | ((*a as u32) << 24)
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
        PipelineOp::PutData { data } => vec![data.len() as u32],

        // ── Everything else (no GPU support / no params) ──
        _ => vec![],
    }
}

// ── Registration table ───────────────────────────────────────────────────

fn register_all(m: &mut HashMap<&'static str, OpEntry>) {
    use crate::compute::pool_cpu::ops::chops::{
        op_chops_add, op_chops_add_modulo, op_chops_blend, op_chops_composite, op_chops_constant,
        op_chops_darker, op_chops_difference, op_chops_duplicate, op_chops_hard_light,
        op_chops_invert, op_chops_lighter, op_chops_logical_and, op_chops_logical_or,
        op_chops_logical_xor, op_chops_multiply, op_chops_offset, op_chops_overlay,
        op_chops_screen, op_chops_soft_light, op_chops_subtract, op_chops_subtract_modulo,
    };
    use crate::compute::pool_cpu::ops::color::{op_convert, op_quantize, op_remap_palette};
    use crate::compute::pool_cpu::ops::effects::{
        op_alpha_composite, op_blend_module, op_composite_module, op_effect_noise,
        op_effect_spread, op_eval, op_merge, op_paste, op_point, op_put_alpha, op_put_data,
        op_put_pixel, op_transform,
    };
    use crate::compute::pool_cpu::ops::enhance::{
        op_enhance_brightness, op_enhance_color_saturation, op_enhance_contrast,
        op_enhance_sharpness,
    };
    use crate::compute::pool_cpu::ops::filter::{
        execute_box_blur, execute_filter3x3, execute_filter5x5, execute_gaussian_blur,
        execute_max_filter, execute_max_filter_with_mode, execute_median_filter,
        execute_median_filter_with_mode, execute_min_filter, execute_min_filter_with_mode,
        execute_rank_filter, execute_rank_filter_with_mode,
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
        OpEntry::cpu_only(|img, op, mode| {
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
        }),
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
        OpEntry::cpu_only(|img, op, mode| {
            if let PipelineOp::RemapPalette { dest_map } = op {
                let arr: &[u8; 256] = dest_map.as_slice().try_into().map_err(|_| {
                    PilError::ValueError("remap_palette: expected 256-byte dest_map".into())
                })?;
                op_remap_palette(img, arr, mode)
            } else {
                Err(PilError::ValueError("expected RemapPalette op".into()))
            }
        }),
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
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
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
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
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
        OpEntry::cpu_only(|img, op, mode| {
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
        }),
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
        OpEntry::cpu_only(|img, op, _mode| {
            if let PipelineOp::Expand { border, fill } = op {
                op_expand(img, *border, *fill)
            } else {
                Err(PilError::ValueError("expected Expand op".into()))
            }
        }),
    );
    m.insert(
        "CropBorder",
        OpEntry::cpu_only(|img, op, _mode| {
            if let PipelineOp::CropBorder { border } = op {
                op_crop_border(img, *border)
            } else {
                Err(PilError::ValueError("expected CropBorder op".into()))
            }
        }),
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
                    Ok(op_chops_invert(img))
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
                } = op
                {
                    op_paste(img, source, *x as i64, *y as i64, mask)
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
    m.insert(
        "Merge",
        OpEntry::cpu_only(|img, op, _mode| {
            if let PipelineOp::Merge { mode, bands } = op {
                let arc_bands: Vec<std::sync::Arc<crate::image::Image>> = bands
                    .iter()
                    .map(|im| std::sync::Arc::new(im.clone()))
                    .collect();
                op_merge(img, mode, &arc_bands)
            } else {
                Err(PilError::ValueError("expected Merge op".into()))
            }
        }),
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
                if let PipelineOp::CompositeModule { other, mask } = op {
                    op_composite_module(img, other, mask, mode)
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
                } = op
                {
                    op_transform(img, *w, *h, method, data, filter, *fill, mode)
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
                if let PipelineOp::PutPixel { x, y, color } = op {
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
                if let PipelineOp::PutData { data } = op {
                    op_put_data(img, data)
                } else {
                    Err(PilError::ValueError("expected PutData op".into()))
                }
            },
            "put_data.wgsl"
        ),
    );
    m.insert(
        "PutAlpha",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutAlpha { alpha } = op {
                    Ok(op_put_alpha(img, *alpha))
                } else {
                    Err(PilError::ValueError("expected PutAlpha op".into()))
                }
            },
            "put_alpha.wgsl"
        ),
    );
}
