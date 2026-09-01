//! Streaming image pipeline operation descriptors.
//!
//! [`crate::pipeline::PipelineOp`] is an internal-public contract between
//! high-level Pillow-style methods, compute backend routing, and backend
//! implementations. A variant records the operation and all normalized Rust
//! arguments needed to execute it later. Execution is deferred until an image is
//! materialized, saved, or serialized.
//!
//! Coordinates are integer pixel coordinates unless a field explicitly says it
//! is a normalized factor. Colors are normalized RGBA tuples in byte order.
//!
//! # Internal Contract
//!
//! This module is public so integration tests, binding-adjacent code, and
//! backend registries can describe operations without depending on Python or
//! JavaScript objects. It is not intended to be the ergonomic end-user API;
//! users should normally call methods on [`crate::Image`].
//!
//! Every new operation must keep these tables aligned:
//!
//! - [`crate::pipeline::PipelineOp`] variant.
//! - `compute::registry::variant_key`.
//! - CPU implementation registration.
//! - optional GPU/SIMD registration and shader parameter extraction.
//!
//! # Field Conventions
//!
//! - `w` and `h` are output dimensions in pixels.
//! - `x` and `y` are destination coordinates unless a field says `src`.
//! - Crop-like `right` and `bottom` fields are exclusive edges.
//! - `fill`, `outline`, and `color` are normalized `(r, g, b, a)` bytes.
//! - `centering` is normalized to the Pillow `0.0..=1.0` convention.
//! - `other`, `source`, and `mask` are shared image handles used when the
//!   operation needs more than the pipeline source image.

use crate::image::Image;
use std::sync::Arc;

/// Every image-producing operation maps to one variant.
/// Input image is the source; output image is the result of applying this op.
#[derive(Debug, Clone)]
pub enum PipelineOp {
    // ── Geometry ──
    /// Resize to an exact output size.
    Resize {
        /// Target width of the produced image, in pixels.
        w: u32,
        /// Target height of the produced image, in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
    },
    /// Crop to a rectangular box.
    Crop {
        /// Left edge in pixels.
        left: u32,
        /// Top edge in pixels.
        top: u32,
        /// Right edge in pixels.
        right: u32,
        /// Bottom edge in pixels.
        bottom: u32,
    },
    /// Rotate by an arbitrary angle.
    Rotate {
        /// Rotation angle in degrees.
        angle: f64,
        /// Whether to expand the canvas to fit the rotated image.
        expand: bool,
        /// Optional fill color for newly exposed pixels.
        fill: Option<(u8, u8, u8, u8)>,
        /// Optional center of rotation in source-image coordinates.
        center: Option<(f64, f64)>,
        /// Optional post-rotation translation in pixels.
        translate: Option<(f64, f64)>,
        /// Whether nearest-neighbor resampling was requested.
        nearest: bool,
    },
    /// Apply one Pillow transpose operation.
    Transpose {
        /// Transpose method.
        method: TransposeMethod,
    },
    /// Resize in-place to fit within a bounding box.
    Thumbnail {
        /// Maximum width in pixels.
        w: u32,
        /// Maximum height in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
    },
    /// Reduce image dimensions by an integer factor.
    Reduce {
        /// Horizontal reduction factor.
        x_factor: u32,
        /// Vertical reduction factor.
        y_factor: u32,
    },

    // ── Color/Convert ──
    /// Convert to another Pillow color mode.
    Convert {
        /// Destination color mode.
        mode: ColorMode,
        /// Optional conversion matrix.
        matrix: Option<Arc<[f64]>>,
        /// Optional dither method.
        dither: Option<DitherMethod>,
    },
    /// Legacy deferred quantization descriptor.
    ///
    /// The public [`Image::quantize`](crate::Image::quantize) implementation
    /// owns quantization and materializes the palette directly. This variant
    /// remains only for compatibility with operation metadata; it has no
    /// registered executor and is not produced by a supported public pipeline.
    #[deprecated(note = "legacy deferred quantization; use Image::quantize instead")]
    Quantize {
        /// Requested palette color count.
        colors: u32,
        /// Whether dithering is enabled.
        dither: bool,
    },
    /// Remap palette indices through a destination map.
    RemapPalette {
        /// Destination palette index map.
        dest_map: Arc<[u8]>,
    },

    // ── Filters (3×3 and 5×5 convolution) ──
    /// Apply a 3x3 convolution kernel.
    Filter3x3 {
        /// Kernel coefficients in row-major order.
        kernel: [f32; 9],
        /// Kernel scale divisor.
        scale: f32,
        /// Additive output offset.
        offset: i32,
    },
    /// Apply a 5x5 convolution kernel.
    Filter5x5 {
        /// Kernel coefficients in row-major order.
        kernel: [f32; 25],
        /// Kernel scale divisor.
        scale: f32,
        /// Additive output offset.
        offset: i32,
    },
    /// Apply Gaussian blur.
    GaussianBlur {
        /// Gaussian sigma.
        sigma: f32,
    },
    /// Apply box blur.
    BoxBlur {
        /// Box blur radius.
        radius: u32,
    },
    /// Apply box blur with independent horizontal and vertical radii.
    ///
    /// The uniform integer form above is retained for the established GPU
    /// and SIMD contract. Pillow also accepts fractional and two-axis radii;
    /// those use this exact CPU-compatible descriptor.
    BoxBlurXY {
        /// Horizontal box blur radius.
        radius_x: f32,
        /// Vertical box blur radius.
        radius_y: f32,
        /// Number of horizontal and vertical box passes.
        passes: u32,
    },
    /// Apply median filter.
    MedianFilter {
        /// Filter window size.
        size: u32,
    },
    /// Apply maximum filter.
    MaxFilter {
        /// Filter window size.
        size: u32,
    },
    /// Apply minimum filter.
    MinFilter {
        /// Filter window size.
        size: u32,
    },
    /// Apply rank filter.
    RankFilter {
        /// Filter window size.
        size: u32,
        /// Rank selected from the sorted window.
        rank: u32,
    },

    // ── ImageOps ──
    /// Apply autocontrast.
    Autocontrast {
        /// Cutoff percentage.
        cutoff: f64,
        /// Optional `1`/`L` histogram mask used to choose the contrast range.
        mask: Option<Arc<Image>>,
    },
    /// Equalize image histogram.
    Equalize,
    /// Invert image channels.
    Invert,
    /// Flip image top-to-bottom.
    Flip,
    /// Mirror image left-to-right.
    Mirror,
    /// Posterize by keeping high bits.
    Posterize {
        /// Number of bits to keep.
        bits: u8,
    },
    /// Solarize pixels above a threshold.
    Solarize {
        /// Solarize threshold.
        threshold: u8,
    },
    /// Convert to grayscale.
    Grayscale,
    /// Colorize a grayscale image.
    Colorize {
        /// RGB color mapped from black.
        black: (u8, u8, u8),
        /// RGB color mapped from white.
        white: (u8, u8, u8),
        /// Optional RGB color mapped from the midpoint.
        mid: Option<(u8, u8, u8)>,
        /// Gray value mapped to `black`.
        blackpoint: u8,
        /// Gray value mapped to `mid`.
        midpoint: u8,
        /// Gray value mapped to `white`.
        whitepoint: u8,
    },
    /// Resize to fit within a bounding box.
    Contain {
        /// Bounding width in pixels.
        w: u32,
        /// Bounding height in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
    },
    /// Resize to cover a bounding box.
    Cover {
        /// Bounding width in pixels.
        w: u32,
        /// Bounding height in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
    },
    /// Resize and crop to fit a requested box.
    Fit {
        /// Target width of the produced image, in pixels.
        w: u32,
        /// Target height of the produced image, in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
        /// Normalized edge bleed.
        bleed: f64,
        /// Normalized horizontal and vertical centering.
        centering: (f64, f64),
    },
    /// Resize and pad to a requested box.
    Pad {
        /// Target width of the produced image, in pixels.
        w: u32,
        /// Target height of the produced image, in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
        /// Optional pad fill color.
        color: Option<(u8, u8, u8, u8)>,
        /// Normalized horizontal and vertical centering.
        centering: (f64, f64),
    },
    /// Scale dimensions by a factor.
    Scale {
        /// Scale factor.
        factor: f64,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
    },
    /// Add a border around the image.
    Expand {
        /// Border width in pixels.
        border: u32,
        /// Border fill color.
        fill: (u8, u8, u8, u8),
    },
    /// Crop an equal border from every side.
    CropBorder {
        /// Border width in pixels.
        border: u32,
    },

    // ── ImageChops ──
    /// Add another image with scale and offset.
    Add {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
        /// Scale divisor.
        scale: f64,
        /// Additive offset.
        offset: f64,
    },
    /// Subtract another image with scale and offset.
    Subtract {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
        /// Scale divisor.
        scale: f64,
        /// Additive offset.
        offset: f64,
    },
    /// Multiply with another image.
    Multiply {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Screen blend with another image.
    Screen {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Per-pixel darker operation with another image.
    Darker {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Per-pixel lighter operation with another image.
    Lighter {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Absolute difference with another image.
    Difference {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Overlay blend with another image.
    Overlay {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Hard-light blend with another image.
    HardLight {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Soft-light blend with another image.
    SoftLight {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Add another image modulo 256.
    AddModulo {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Subtract another image modulo 256.
    SubtractModulo {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Logical AND with another image.
    LogicalAnd {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Logical OR with another image.
    LogicalOr {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Logical XOR with another image.
    LogicalXor {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
    },
    /// Fill with a constant value.
    Constant {
        /// Constant byte value.
        value: u8,
    },
    /// Offset image pixels.
    Offset {
        /// Horizontal offset.
        x: i32,
        /// Vertical offset.
        y: i32,
    },
    /// Duplicate the image.
    Duplicate,
    /// Invert image channels through ImageChops.
    InvertChops,

    // ── Enhance ──
    /// Adjust brightness.
    Brightness {
        /// Enhancement factor.
        factor: f64,
    },
    /// Adjust contrast.
    Contrast {
        /// Enhancement factor.
        factor: f64,
    },
    /// Adjust color saturation.
    ColorSaturation {
        /// Enhancement factor.
        factor: f64,
    },
    /// Adjust sharpness.
    Sharpness {
        /// Enhancement factor.
        factor: f64,
    },

    // ── Effects ──
    /// Randomly spread pixels within a distance.
    EffectSpread {
        /// Maximum spread distance.
        distance: u32,
    },
    /// Paste an image, optional color source, or masked source.
    Paste {
        /// Source image.
        source: Arc<Image>,
        /// Destination x coordinate.
        x: i32,
        /// Destination y coordinate.
        y: i32,
        /// Paste width.
        w: i32,
        /// Paste height.
        h: i32,
        /// Optional mask image.
        mask: Option<Arc<Image>>,
        /// Whether the mask value comes from its alpha band instead of luma.
        mask_alpha: bool,
    },
    /// Alpha-composite one image over another.
    AlphaComposite {
        /// Source image.
        source: Arc<Image>,
        /// Destination offset.
        dest: (i32, i32),
        /// Source offset.
        src: (i32, i32),
    },

    // ── Module fns ──
    /// Merge bands into a multi-band image.
    Merge {
        /// Destination color mode.
        mode: ColorMode,
        /// Original Pillow mode spelling. The raster core uses canonical
        /// storage variants for aliases such as `LAB` and `RGBa`, while this
        /// tag keeps target-specific encoding and backend admission exact.
        logical_mode: String,
        /// Band images.
        bands: Arc<[Image]>,
    },
    /// Module-level blend operation.
    BlendModule {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
        /// Blend alpha.
        alpha: f64,
    },
    /// Module-level composite operation.
    CompositeModule {
        /// Second image argument supplied to the operation.
        other: Arc<Image>,
        /// Mask image.
        mask: Arc<Image>,
        /// Whether the mask value comes from its alpha band.
        mask_alpha: bool,
    },
    /// Apply a lookup table to every pixel value.
    Eval {
        /// Lookup table.
        lut: Arc<[u8]>,
    },
    /// Generate noise with a Gaussian sigma.
    EffectNoise {
        /// Noise sigma.
        sigma: f64,
    },

    // ── Point operations (lookup table) ──
    /// Legacy CPU point-operation descriptor.
    ///
    /// Public point/eval calls use [`Eval`](Self::Eval). This descriptor is
    /// retained for internal GPU LUT fusion and is not a public pipeline
    /// construction path.
    #[deprecated(note = "legacy point descriptor; use PipelineOp::Eval")]
    PointOp {
        /// Lookup table.
        lut: Arc<[u8]>,
    },
    // ── 3D Color Lookup Table ──
    /// Apply a 3D color lookup table.
    Color3DLut {
        /// LUT dimensions.
        size: (u32, u32, u32),
        /// LUT table values.
        table: Arc<[f64]>,
        /// Number of output channels.
        channels: u32,
        /// Logical Pillow mode of the input samples.
        source_mode: PixelMode,
        /// Logical Pillow mode requested for the output samples.
        target_mode: PixelMode,
    },
    /// Apply a geometric transform.
    Transform {
        /// Target width of the produced image, in pixels.
        w: u32,
        /// Target height of the produced image, in pixels.
        h: u32,
        /// Transform method.
        method: TransformMethod,
        /// Method-specific transform coefficients.
        data: Arc<[f64]>,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
        /// Optional fill color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Raw palette index used to fill out-of-bounds `P` samples.
        ///
        /// Pillow distinguishes scalar palette indices from tuple/string
        /// colors, which resolve to index zero for affine transforms.
        palette_fill: Option<u8>,
    },

    // ── Mutating ops (pipelined — replace self with Pipeline) ──
    /// Set one pixel to a color.
    PutPixel {
        /// Pixel x coordinate.
        x: u32,
        /// Pixel y coordinate.
        y: u32,
        /// Pixel color.
        color: (u8, u8, u8, u8),
        /// Whether the scalar value is proven to be a palette index.
        palette_index: bool,
    },
    /// Replace image data from raw bytes.
    PutData {
        /// Raw pixel data.
        data: Arc<[u8]>,
        /// Logical Pillow mode whose sample layout `data` follows.
        mode: PixelMode,
    },
    /// Replace or set alpha channel.
    PutAlpha {
        /// Constant alpha value.
        alpha: u8,
        /// Logical Pillow mode of the image before alpha promotion.
        mode: PixelMode,
    },
    /// Replaces the alpha channel from an L-mask image.
    PutAlphaData {
        /// L-mask pixel data.
        mask: Arc<crate::raster::DynamicImage>,
        /// Logical Pillow mode of the image before alpha promotion.
        mode: PixelMode,
    },

    // ── Channel extraction ──
    /// Extract one band by index.
    ExtractBand {
        /// Band index.
        index: u8,
    },

    // ── Gradient generation (ImageModule) ──
    /// Legacy deferred linear-gradient descriptor.
    ///
    /// The public module constructor materializes the gradient eagerly. This
    /// deprecated descriptor remains available for operation metadata and can
    /// use the native SIMD generator when explicitly executed by a pipeline.
    #[deprecated(note = "legacy deferred gradient; use the public linear_gradient constructor")]
    LinearGradient {
        /// Output mode.
        mode: ColorMode,
    },
    /// Legacy deferred radial-gradient descriptor.
    ///
    /// The public module constructor materializes the gradient eagerly. This
    /// deprecated descriptor is retained for operation metadata and shader
    /// contract inspection only; it has no registered executor.
    #[deprecated(note = "legacy deferred gradient; use the public radial_gradient constructor")]
    RadialGradient {
        /// Output mode.
        mode: ColorMode,
    },

    // ── Fractal generation (ImageModule) ──
    /// Legacy deferred Mandelbrot descriptor.
    ///
    /// The public module constructor materializes the effect eagerly. This
    /// deprecated descriptor is retained for operation metadata and shader
    /// contract inspection only; it has no registered executor.
    #[deprecated(note = "legacy deferred Mandelbrot; use the public effect_mandelbrot constructor")]
    EffectMandelbrot {
        /// Target width of the produced image, in pixels.
        w: u32,
        /// Target height of the produced image, in pixels.
        h: u32,
        /// Left coordinate in the complex plane.
        x0: f64,
        /// Top coordinate in the complex plane.
        y0: f64,
        /// Right coordinate in the complex plane.
        x1: f64,
        /// Bottom coordinate in the complex plane.
        y1: f64,
        /// Iteration quality parameter.
        quality: u32,
    },

    // ── ImageDraw ops (geometric) ──
    /// Draw a line segment.
    DrawLine {
        /// Start x coordinate.
        x0: i32,
        /// Start y coordinate.
        y0: i32,
        /// End x coordinate.
        x1: i32,
        /// End y coordinate.
        y1: i32,
        /// Fill color.
        fill: (u8, u8, u8, u8),
        /// Stroke width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw a rectangle.
    DrawRectangle {
        /// Left edge.
        x0: i32,
        /// Top edge.
        y0: i32,
        /// Right edge.
        x1: i32,
        /// Bottom edge.
        y1: i32,
        /// Optional fill color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Optional outline color.
        outline: Option<(u8, u8, u8, u8)>,
        /// Outline width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw a rounded rectangle.
    DrawRoundedRect {
        /// Left edge.
        x0: i32,
        /// Top edge.
        y0: i32,
        /// Right edge.
        x1: i32,
        /// Bottom edge.
        y1: i32,
        /// Corner radius.
        radius: f64,
        /// Optional fill color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Optional outline color.
        outline: Option<(u8, u8, u8, u8)>,
        /// Outline width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw an ellipse.
    DrawEllipse {
        /// Left edge of bounding box.
        x0: i32,
        /// Top edge of bounding box.
        y0: i32,
        /// Right edge of bounding box.
        x1: i32,
        /// Bottom edge of bounding box.
        y1: i32,
        /// Optional fill color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Optional outline color.
        outline: Option<(u8, u8, u8, u8)>,
        /// Outline width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw a circle.
    DrawCircle {
        /// Center x coordinate.
        cx: i32,
        /// Center y coordinate.
        cy: i32,
        /// Radius in pixels.
        radius: i32,
        /// Optional fill color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Optional outline color.
        outline: Option<(u8, u8, u8, u8)>,
        /// Outline width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw a polygon.
    DrawPolygon {
        /// Polygon vertices.
        points: Arc<[(i32, i32)]>,
        /// Optional fill color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Optional outline color.
        outline: Option<(u8, u8, u8, u8)>,
        /// Outline width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw an arc.
    DrawArc {
        /// Left edge of bounding box.
        x0: i32,
        /// Top edge of bounding box.
        y0: i32,
        /// Right edge of bounding box.
        x1: i32,
        /// Bottom edge of bounding box.
        y1: i32,
        /// Start angle in degrees.
        start: f64,
        /// End angle in degrees.
        end: f64,
        /// Stroke color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Stroke width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw a chord.
    DrawChord {
        /// Left edge of bounding box.
        x0: i32,
        /// Top edge of bounding box.
        y0: i32,
        /// Right edge of bounding box.
        x1: i32,
        /// Bottom edge of bounding box.
        y1: i32,
        /// Start angle in degrees.
        start: f64,
        /// End angle in degrees.
        end: f64,
        /// Optional fill color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Optional outline color.
        outline: Option<(u8, u8, u8, u8)>,
        /// Outline width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw a pie slice.
    DrawPieslice {
        /// Left edge of bounding box.
        x0: i32,
        /// Top edge of bounding box.
        y0: i32,
        /// Right edge of bounding box.
        x1: i32,
        /// Bottom edge of bounding box.
        y1: i32,
        /// Start angle in degrees.
        start: f64,
        /// End angle in degrees.
        end: f64,
        /// Optional fill color.
        fill: Option<(u8, u8, u8, u8)>,
        /// Optional outline color.
        outline: Option<(u8, u8, u8, u8)>,
        /// Outline width.
        width: u32,
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
    /// Draw one or more points.
    DrawPoint {
        /// Point coordinates.
        points: Arc<[(i32, i32)]>,
        /// Fill color.
        fill: (u8, u8, u8, u8),
        /// Blend RGBA ink into an RGB destination instead of replacing pixels.
        alpha_blend_rgb: bool,
    },
}

// ── Support types ──

/// Resampling filter used by resize-like operations.
#[derive(Debug, Clone, Copy)]
pub enum ResampleFilter {
    /// Nearest-neighbor sampling.
    Nearest,
    /// Bilinear interpolation.
    Bilinear,
    /// Bicubic interpolation.
    Bicubic,
    /// Lanczos-windowed sinc interpolation.
    Lanczos,
    /// Box filter sampling.
    Box,
    /// Hamming-windowed sampling.
    Hamming,
}

/// Pillow transpose operation.
#[derive(Debug, Clone)]
pub enum TransposeMethod {
    /// Mirror left-to-right.
    FlipLeftRight,
    /// Mirror top-to-bottom.
    FlipTopBottom,
    /// Rotate 90 degrees counter-clockwise.
    Rotate90,
    /// Rotate 180 degrees.
    Rotate180,
    /// Rotate 270 degrees counter-clockwise.
    Rotate270,
    /// Transpose across the top-left to bottom-right diagonal.
    Transpose,
    /// Transpose across the top-right to bottom-left diagonal.
    Transverse,
}

/// Coordinate transform algorithm.
#[derive(Debug, Clone)]
pub enum TransformMethod {
    /// Affine transform with six coefficients.
    Affine,
    /// Perspective transform with eight coefficients.
    Perspective,
    /// Quadrilateral mapping.
    Quad,
    /// Mesh transform made of local quadrilateral mappings.
    Mesh,
}

/// Pillow image mode used by pipeline operations.
///
/// This enum carries mode intent through the pipeline before conversion to a
/// concrete image-buffer representation.
#[derive(Debug, Clone)]
pub enum ColorMode {
    /// 8-bit luma.
    L,
    /// 8-bit luma plus alpha.
    LA,
    /// 8-bit red, green, and blue.
    RGB,
    /// 8-bit red, green, blue, and alpha.
    RGBA,
    /// Cyan, magenta, yellow, and key/black.
    CMYK,
    /// Luma and chroma channels.
    YCbCr,
    /// Hue, saturation, and value.
    HSV,
    /// 32-bit signed integer pixels.
    I,
    /// 32-bit floating-point pixels.
    F,
    /// Palette-indexed pixels.
    P,
    /// Pillow mode `"1"` binary pixels.
    Mode1,
}

/// Logical Pillow sample layout for mutating pixel operations.
///
/// `DynamicImage` cannot distinguish raw `P`, `CMYK`, `I`, or `F` samples
/// from the standard buffers used to store them. Mutating operations carry
/// this mode alongside their bytes so every compute backend interprets the
/// same samples without relying on output-mode metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PixelMode {
    /// 8-bit luma.
    L = 0,
    /// 8-bit luma plus alpha.
    LA = 1,
    /// 8-bit red, green, and blue.
    RGB = 2,
    /// 8-bit red, green, blue, and alpha.
    RGBA = 3,
    /// Palette indices.
    P = 4,
    /// Palette indices plus per-pixel alpha.
    PA = 5,
    /// Cyan, magenta, yellow, and key/black.
    CMYK = 6,
    /// Pillow mode `"1"` binary pixels.
    Mode1 = 7,
    /// Luma and chroma channels.
    YCbCr = 8,
    /// Hue, saturation, and value.
    HSV = 9,
    /// 32-bit signed integer pixels.
    I = 10,
    /// 32-bit floating-point pixels.
    F = 11,
}

impl PixelMode {
    /// Parses a Pillow mode name used by the core image API.
    pub(crate) fn from_name(mode: &str) -> Option<Self> {
        match mode {
            "1" => Some(Self::Mode1),
            "L" => Some(Self::L),
            "LA" => Some(Self::LA),
            "P" => Some(Self::P),
            "PA" => Some(Self::PA),
            "RGB" => Some(Self::RGB),
            "RGBA" | "RGBa" | "RGBX" => Some(Self::RGBA),
            "CMYK" => Some(Self::CMYK),
            "YCbCr" => Some(Self::YCbCr),
            "HSV" => Some(Self::HSV),
            "I" => Some(Self::I),
            "F" => Some(Self::F),
            _ => None,
        }
    }

    /// Number of raw bytes per pixel used by `putdata`.
    pub(crate) const fn channels(self) -> usize {
        match self {
            Self::L | Self::P | Self::Mode1 => 1,
            Self::LA | Self::PA => 2,
            Self::RGB | Self::YCbCr | Self::HSV => 3,
            Self::RGBA | Self::CMYK | Self::I | Self::F => 4,
        }
    }

    /// Stable code shared with scalar and WGSL kernels.
    #[cfg(feature = "gpu")]
    pub(crate) const fn code(self) -> u32 {
        self as u32
    }
}

/// Dither algorithm used by conversion or quantization operations.
#[derive(Debug, Clone)]
pub enum DitherMethod {
    /// No dithering.
    None,
    /// Floyd-Steinberg error diffusion.
    FloydSteinberg,
}
