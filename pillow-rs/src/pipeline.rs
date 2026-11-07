//! Streaming image pipeline operation descriptors.
//!
//! [`crate::PipelineOp`] is the normalized contract between
//! high-level Pillow-style methods, compute backend routing, and backend
//! implementations. A variant records the operation and all normalized Rust
//! arguments needed to execute it later. Execution is deferred until an image is
//! materialized, saved, or serialized.
//!
//! Coordinates are integer pixel coordinates unless a field explicitly says it
//! is a normalized factor. Direct color fields are RGBA tuples in byte order;
//! `PadTyped` and `ExpandTyped` retain validated scalar/component inputs for
//! native-width CPU mode strategies.
//!
//! # Descriptor Contract
//!
//! Descriptors are re-exported from the crate root for Rust callers
//! and backend integration without Python or JavaScript objects. Call methods
//! on [`crate::Image`] for Pillow-style argument validation. The unshipped
//! ImageBatch wrapper was never part of the public API; the GPU feature exposes
//! explicit scheduling through `GpuBatchExecutor`.
//!
//! Every new operation must keep these tables aligned:
//!
//! - [`crate::pipeline::PipelineOp`] variant.
//! - `PipelineOp::operation_id` and `PipelineOp::support`.
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
use crate::raster::Mode;
use bytes::Bytes;
use std::sync::Arc;

mod support;
pub(crate) use support::TransposeTransform;
pub use support::{
    ColorMode, DitherMethod, ResampleFilter, TransformMethod, TransposeMethod,
};

/// Independent margins used by the Pillow `ImageOps.expand` operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExpandMargins {
    /// Pixels added on the left, or removed when negative.
    pub left: i64,
    /// Pixels added on the top, or removed when negative.
    pub top: i64,
    /// Pixels added on the right, or removed when negative.
    pub right: i64,
    /// Pixels added on the bottom, or removed when negative.
    pub bottom: i64,
}

impl ExpandMargins {
    /// Creates equal margins on every edge.
    pub const fn uniform(border: u32) -> Self {
        let border = border as i64;
        Self { left: border, top: border, right: border, bottom: border }
    }

    /// Returns the nonnegative symmetric margin when all edges match.
    pub const fn uniform_nonnegative(self) -> Option<u32> {
        if self.left == self.top
            && self.left == self.right
            && self.left == self.bottom
            && self.left >= 0
            && self.left <= u32::MAX as i64
        {
            Some(self.left as u32)
        } else {
            None
        }
    }
}

/// Unnarrowed scalar or component input for a native-sample geometry fill.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct NativeColorInput(NativeColorInputValue);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeColorInputValue {
    IntegerScalar(i64),
    IntegerComponents { values: [i64; 4], len: u8 },
    FloatScalar(u64),
    FloatComponents { values: [u64; 4], len: u8 },
}

impl NativeColorInput {
    pub(crate) const fn scalar(value: i64) -> Self {
        Self(NativeColorInputValue::IntegerScalar(value))
    }

    pub(crate) fn components(values: &[i64]) -> Option<Self> {
        if values.len() > 4 {
            return None;
        }
        let mut components = [0; 4];
        components[..values.len()].copy_from_slice(values);
        Some(Self(NativeColorInputValue::IntegerComponents {
            values: components,
            len: values.len() as u8,
        }))
    }

    pub(crate) fn floating_scalar(value: f64) -> Self {
        Self(NativeColorInputValue::FloatScalar(value.to_bits()))
    }

    pub(crate) fn floating_components(values: &[f64]) -> Option<Self> {
        if values.len() > 4 {
            return None;
        }
        let mut components = [0; 4];
        for (destination, value) in components.iter_mut().zip(values) {
            *destination = value.to_bits();
        }
        Some(Self(NativeColorInputValue::FloatComponents {
            values: components,
            len: values.len() as u8,
        }))
    }

    /// Returns one input band or the exact-mode default for a missing band.
    pub(crate) const fn component(self, band: usize, default: i64) -> i64 {
        match self.0 {
            NativeColorInputValue::IntegerScalar(value) if band == 0 => value,
            NativeColorInputValue::IntegerScalar(_) => default,
            NativeColorInputValue::IntegerComponents { values, len } if band < len as usize => {
                values[band]
            }
            NativeColorInputValue::IntegerComponents { .. }
            | NativeColorInputValue::FloatScalar(_)
            | NativeColorInputValue::FloatComponents { .. } => default,
        }
    }

    /// Returns one native float sample, preserving floating-point input values.
    pub(crate) fn float_component(self, band: usize, default: f64) -> f64 {
        match self.0 {
            NativeColorInputValue::IntegerScalar(value) if band == 0 => value as f64,
            NativeColorInputValue::IntegerScalar(_) => default,
            NativeColorInputValue::IntegerComponents { values, len } if band < len as usize => {
                values[band] as f64
            }
            NativeColorInputValue::IntegerComponents { .. } => default,
            NativeColorInputValue::FloatScalar(value) if band == 0 => f64::from_bits(value),
            NativeColorInputValue::FloatScalar(_) => default,
            NativeColorInputValue::FloatComponents { values, len } if band < len as usize => {
                f64::from_bits(values[band])
            }
            NativeColorInputValue::FloatComponents { .. } => default,
        }
    }
}

impl PartialEq for NativeColorInput {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for NativeColorInput {}

/// Compatibility name for callers that used the original Pad fill payload.
#[doc(hidden)]
pub type PadColorInput = NativeColorInput;

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
    /// Resize a source region without discarding the surrounding filter taps.
    ResizeBoxed {
        /// Target width in pixels.
        w: u32,
        /// Target height in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
        /// Floating source bounds, narrowed to Pillow's float32 box domain.
        box_coords: (f64, f64, f64, f64),
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
        /// Optional precomputed reverse-affine coefficients from a host wrapper.
        affine: Option<[f64; 6]>,
        /// Optional host-computed destination size paired with `affine`.
        output_size: Option<(u32, u32)>,
        /// Resampling filter requested by the public rotate call.
        filter: ResampleFilter,
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
        /// Optional Pillow integer-reduction gap; `None` disables reduction.
        reducing_gap: Option<f64>,
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
    /// Convert normalized RGB8 pixels to Pillow's packed LAB byte format.
    ///
    /// LAB uses RGB storage at the raster boundary, so it has a dedicated
    /// operation instead of being represented as an ordinary `ColorMode`.
    ConvertLab {
        /// Output identity profile Pillow installs in `Image.info`.
        icc_profile: Arc<[u8]>,
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
        /// Additive output offset, represented in Pillow's `f32` arithmetic.
        offset: f32,
    },
    /// Apply a 5x5 convolution kernel.
    Filter5x5 {
        /// Kernel coefficients in row-major order.
        kernel: [f32; 25],
        /// Kernel scale divisor.
        scale: f32,
        /// Additive output offset, represented in Pillow's `f32` arithmetic.
        offset: f32,
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
        /// Exact low/high histogram cutoff percentages.
        options: AutocontrastOptions,
        /// Optional `1`/`L` histogram mask used to choose the contrast range.
        mask: Option<Arc<Image>>,
    },
    /// Equalize image histogram.
    Equalize,
    /// Equalize using only nonzero mask pixels to construct the histogram.
    EqualizeMasked {
        /// A validated `1`/`L` mask; the resulting LUT applies to every pixel.
        mask: Arc<Image>,
    },
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
        /// Optional byte-normalized pad fill color for direct pipeline callers.
        color: Option<(u8, u8, u8, u8)>,
        /// Normalized horizontal and vertical centering.
        centering: (f64, f64),
    },
    /// Resize and pad while retaining native-width scalar/component input.
    ///
    /// `color` remains the normalized compatibility view; `input` reaches the
    /// exact-mode CPU fill strategy without sample-width narrowing.
    #[doc(hidden)]
    PadTyped {
        /// Target width of the produced image, in pixels.
        w: u32,
        /// Target height of the produced image, in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
        /// Byte-normalized view used by generic request facts and fallback.
        color: Option<(u8, u8, u8, u8)>,
        /// Original scalar/component color passed to the exact-mode owner.
        input: PadColorInput,
        /// Normalized horizontal and vertical centering.
        centering: (f64, f64),
    },
    /// Resize and paste into a canvas at Pillow's resolved integer position.
    ///
    /// ImageOps.pad computes the position in Python, where custom centering
    /// values can affect `min`, `max`, multiplication, `round`, and paste-box
    /// conversion. Carry the final C-int position without normalizing it back
    /// into a centering ratio.
    #[doc(hidden)]
    PadPositioned {
        /// Target width of the produced image, in pixels.
        w: u32,
        /// Target height of the produced image, in pixels.
        h: u32,
        /// Filter used when sampling source pixels.
        filter: ResampleFilter,
        /// Byte-normalized view used by generic request facts and fallback.
        color: Option<(u8, u8, u8, u8)>,
        /// Original native-width fill input, when applicable.
        native_color: Option<PadColorInput>,
        /// Signed C-int destination coordinate passed to Pillow's Paste.
        position: (i32, i32),
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
        /// Independent left, top, right, and bottom margins.
        border: ExpandMargins,
        /// Border fill color.
        fill: (u8, u8, u8, u8),
    },
    /// Add a border while retaining a native-width scalar/component fill.
    #[doc(hidden)]
    ExpandTyped {
        /// Independent left, top, right, and bottom margins.
        border: ExpandMargins,
        /// Byte-normalized compatibility view of the fill.
        fill: (u8, u8, u8, u8),
        /// Original scalar/component value consumed by the exact-mode CPU owner.
        input: NativeColorInput,
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
        /// Exact destination mode identity, including aliases with shared storage.
        mode: Mode,
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

    // ── 3D Color Lookup Table ──
    /// Apply a 3D color lookup table.
    Color3DLut {
        /// LUT dimensions.
        size: (u32, u32, u32),
        /// LUT table values.
        table: Arc<[f64]>,
        /// Number of output channels.
        channels: u32,
        /// Exact source identity captured by the Pillow image wrapper.
        /// TODO(pillow-rs#3ptb): remove this snapshot after GPU parameter
        /// encoding derives it from the prepared mode transition.
        source_mode: Mode,
        /// Exact output identity selected by the caller.
        target_mode: Mode,
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
        /// Whether Pillow's public call omitted `fillcolor`.
        ///
        /// Pillow initializes an output image with an explicit fill and lets
        /// failed mesh samples leave that value in place. When no fill is
        /// supplied, `ImagingGenericTransform` instead clears each failed
        /// sample, including a later mesh record overwriting an earlier one.
        fill_is_none: bool,
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
        /// Immutable, reference-counted raw pixel data.
        data: Bytes,
        /// Physical pixel lanes omitted by the compact logical carrier when
        /// normalized values specify them.
        hidden_lanes: Option<Bytes>,
        /// Exact typed identity of the normalized byte payload layout.
        payload_mode: Mode,
    },
    /// Replace or set alpha channel.
    PutAlpha {
        /// Constant alpha value.
        alpha: u8,
        /// Exact Pillow-owned source mode before alpha promotion.
        mode: Mode,
    },
    /// Replaces alpha from a typed `1` or `L` mask while preserving its mode.
    PutAlphaData {
        /// The original one-byte mask image, retaining its exact mode identity.
        mask: Arc<crate::raster::DynamicImage>,
        /// Exact Pillow-owned source mode before alpha promotion.
        mode: Mode,
    },

    // ── Channel extraction ──
    /// Extract one band by index.
    ExtractBand {
        /// Band index.
        index: u8,
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
        /// Signed C-int outline width after Pillow's polygon width transform.
        outline_width: Option<i32>,
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

/// Typed arguments to ImageOps autocontrast.
#[derive(Debug, Clone, PartialEq)]
pub struct AutocontrastOptions {
    /// Percent removed from the low and high ends of each histogram.
    pub cutoff: (f64, f64),
    /// Histogram bins to ignore, with negative indices counting from 255.
    pub ignore: Option<Vec<i64>>,
    /// Build one luma histogram and apply that LUT to every source channel.
    pub preserve_tone: bool,
}

impl AutocontrastOptions {
    /// Creates the Pillow-default request with one cutoff used at both ends.
    pub fn symmetric(cutoff: f64) -> Self {
        Self { cutoff: (cutoff, cutoff), ignore: None, preserve_tone: false }
    }
}

#[cfg(test)]
mod transpose_tests;
