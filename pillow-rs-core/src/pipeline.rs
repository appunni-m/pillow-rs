//! Streaming pipeline — all image-producing operations recorded as PipelineOp variants.
//! Execution is deferred until materialize() or save()/tobytes().

use crate::image::Image;
use std::sync::Arc;

/// Every image-producing operation maps to one variant.
/// Input image is the source; output image is the result of applying this op.
#[derive(Debug, Clone)]
pub enum PipelineOp {
    // ── Geometry ──
    Resize {
        w: u32,
        h: u32,
        filter: ResampleFilter,
    },
    Crop {
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
    },
    Rotate {
        angle: f64,
        expand: bool,
        fill: Option<(u8, u8, u8, u8)>,
    },
    Transpose {
        method: TransposeMethod,
    },
    Thumbnail {
        w: u32,
        h: u32,
        filter: ResampleFilter,
    },
    Reduce {
        factor: u32,
    },

    // ── Color/Convert ──
    Convert {
        mode: ColorMode,
        matrix: Option<Vec<f64>>,
        dither: Option<DitherMethod>,
    },
    Quantize {
        colors: u32,
        dither: bool,
    },
    RemapPalette {
        dest_map: Vec<u8>,
    },

    // ── Filters (3×3 and 5×5 convolution) ──
    Filter3x3 {
        kernel: [f32; 9],
        scale: f32,
        offset: i32,
    },
    Filter5x5 {
        kernel: [f32; 25],
        scale: f32,
        offset: i32,
    },
    GaussianBlur {
        sigma: f32,
    },
    BoxBlur {
        radius: u32,
    },
    MedianFilter {
        size: u32,
    },
    MaxFilter {
        size: u32,
    },
    MinFilter {
        size: u32,
    },
    RankFilter {
        size: u32,
        rank: u32,
    },

    // ── ImageOps ──
    Autocontrast {
        cutoff: f64,
    },
    Equalize,
    Invert,
    Flip,
    Mirror,
    Posterize {
        bits: u8,
    },
    Solarize {
        threshold: u8,
    },
    Grayscale,
    Colorize {
        black: (u8, u8, u8),
        white: (u8, u8, u8),
    },
    Contain {
        w: u32,
        h: u32,
        filter: ResampleFilter,
    },
    Cover {
        w: u32,
        h: u32,
        filter: ResampleFilter,
    },
    Fit {
        w: u32,
        h: u32,
        filter: ResampleFilter,
        bleed: f64,
        centering: (f64, f64),
    },
    Pad {
        w: u32,
        h: u32,
        filter: ResampleFilter,
        color: Option<(u8, u8, u8, u8)>,
        centering: (f64, f64),
    },
    Scale {
        factor: f64,
        filter: ResampleFilter,
    },
    Expand {
        border: u32,
        fill: (u8, u8, u8, u8),
    },
    CropBorder {
        border: u32,
    },

    // ── ImageChops ──
    Add {
        other: Arc<Image>,
        scale: f64,
        offset: f64,
    },
    Subtract {
        other: Arc<Image>,
        scale: f64,
        offset: f64,
    },
    Multiply {
        other: Arc<Image>,
    },
    Screen {
        other: Arc<Image>,
    },
    Darker {
        other: Arc<Image>,
    },
    Lighter {
        other: Arc<Image>,
    },
    Difference {
        other: Arc<Image>,
    },
    Overlay {
        other: Arc<Image>,
    },
    HardLight {
        other: Arc<Image>,
    },
    SoftLight {
        other: Arc<Image>,
    },
    AddModulo {
        other: Arc<Image>,
    },
    SubtractModulo {
        other: Arc<Image>,
    },
    LogicalAnd {
        other: Arc<Image>,
    },
    LogicalOr {
        other: Arc<Image>,
    },
    LogicalXor {
        other: Arc<Image>,
    },
    Constant {
        value: u8,
    },
    Offset {
        x: i32,
        y: i32,
    },
    Blend {
        other: Arc<Image>,
        alpha: f64,
    },
    Composite {
        other: Arc<Image>,
        mask: Arc<Image>,
    },
    Duplicate,
    InvertChops,

    // ── Enhance ──
    Brightness {
        factor: f64,
    },
    Contrast {
        factor: f64,
    },
    ColorSaturation {
        factor: f64,
    },
    Sharpness {
        factor: f64,
    },

    // ── Effects ──
    EffectSpread {
        distance: u32,
    },
    Paste {
        source: Arc<Image>,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        mask: Option<Arc<Image>>,
    },
    AlphaComposite {
        source: Arc<Image>,
        dest: (i32, i32),
        src: (i32, i32),
    },

    // ── Module fns ──
    Merge {
        mode: ColorMode,
        bands: Vec<Image>,
    },
    BlendModule {
        other: Arc<Image>,
        alpha: f64,
    },
    CompositeModule {
        other: Arc<Image>,
        mask: Arc<Image>,
    },
    Eval {
        lut: Vec<u8>,
    },
    EffectNoise {
        sigma: f64,
    },

    // ── Point operations (lookup table) ──
    PointOp {
        lut: Vec<u8>,
    },
    Transform {
        w: u32,
        h: u32,
        method: TransformMethod,
        data: Vec<f64>,
        filter: ResampleFilter,
        fill: Option<(u8, u8, u8, u8)>,
    },

    // ── Mutating ops (pipelined — replace self with Pipeline) ──
    PutPixel {
        x: u32,
        y: u32,
        color: (u8, u8, u8, u8),
    },
    PutData {
        data: Vec<u8>,
    },
    PutAlpha {
        alpha: u8,
    },
}

// ── Support types ──

#[derive(Debug, Clone, Copy)]
pub enum ResampleFilter {
    Nearest,
    Bilinear,
    Bicubic,
    Lanczos,
    Box,
    Hamming,
}

#[derive(Debug, Clone)]
pub enum TransposeMethod {
    FlipLeftRight,
    FlipTopBottom,
    Rotate90,
    Rotate180,
    Rotate270,
    Transpose,
    Transverse,
}

#[derive(Debug, Clone)]
pub enum TransformMethod {
    Affine,
    Perspective,
    Quad,
    Mesh,
}

#[derive(Debug, Clone)]
pub enum ColorMode {
    L,
    LA,
    RGB,
    RGBA,
    CMYK,
    YCbCr,
    HSV,
    I,
    F,
    P,
    Mode1,
}

#[derive(Debug, Clone)]
pub enum DitherMethod {
    None,
    FloydSteinberg,
}
