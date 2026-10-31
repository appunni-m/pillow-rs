//! Typed materialized facts supplied to operation support declarations.

use crate::pipeline::{ResampleFilter, TransposeMethod};
use crate::raster::Mode;
use super::{ModeChannelCapacity, PaletteKnowledge};

/// Operation parameters needed by an implementation's declared eligibility guard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RequestFacts {
    /// A mode-only query has not supplied operation-specific parameters.
    #[default]
    Unspecified,
    /// Normalized high-bit count for native-byte Posterize strategies.
    Posterize {
        /// Number of high bits retained per sample.
        bits: u8,
    },
    /// Maximum native-pixel displacement for EffectSpread.
    EffectSpread {
        /// Maximum horizontal and vertical offset range.
        distance: u32,
    },
    /// EffectNoise sigma represented by the exact f64 request bits.
    EffectNoise {
        /// Original input value retained without NaN normalization.
        sigma_bits: u64,
    },
    /// Mandelbrot's extent and quality, with output dimensions in `StrategyInputs`.
    EffectMandelbrot {
        /// Exact f64 bit patterns for `(x0, y0, x1, y1)`.
        extent_bits: (u64, u64, u64, u64),
        /// Escape iteration quality.
        quality: i32,
    },
    /// Expanded byte lookup table supplied to the exact-mode Eval operation.
    EvalLut {
        /// Number of table entries across all native bands.
        entries: usize,
    },
    /// Floating point LUT supplied to the exact-mode PointFloat operation.
    PointFloatLut {
        /// Number of table entries across all native bands.
        entries: usize,
    },
    /// Normalized source-block factors for the mode-owned Reduce strategy.
    Reduce {
        /// Horizontal source pixels per output sample.
        x_factor: u32,
        /// Vertical source pixels per output sample.
        y_factor: u32,
    },
    /// Final output bounds and sampling filter supplied to Thumbnail.
    Thumbnail {
        /// Requested output width after aspect-ratio normalization.
        width: u32,
        /// Requested output height after aspect-ratio normalization.
        height: u32,
        /// Filter selected by the public image wrapper.
        filter: ResampleFilter,
    },
    /// Contain's requested target and filter, before its aspect-ratio sizing.
    Contain {
        /// Target width.
        width: u32,
        /// Target height.
        height: u32,
        /// Filter supplied to the operation.
        filter: ResampleFilter,
    },
    /// Cover's requested target and filter, before its aspect-ratio sizing.
    Cover {
        /// Target width.
        width: u32,
        /// Target height.
        height: u32,
        /// Filter supplied to the operation.
        filter: ResampleFilter,
    },
    /// Fit parameters retained without normalizing floating-point values.
    Fit {
        /// Target width.
        width: u32,
        /// Target height.
        height: u32,
        /// Filter supplied to the operation.
        filter: ResampleFilter,
        /// Exact bleed input bits.
        bleed_bits: u64,
        /// Exact horizontal and vertical centering bits.
        centering_bits: (u64, u64),
    },
    /// Pad's output canvas, filter, fill color, and exact centering bits.
    Pad {
        /// Target width.
        width: u32,
        /// Target height.
        height: u32,
        /// Filter supplied to the operation.
        filter: ResampleFilter,
        /// Optional Pillow fill tuple.
        color: Option<(u8, u8, u8, u8)>,
        /// Exact horizontal and vertical centering bits.
        centering_bits: (u64, u64),
    },
    /// Scale's factor and resolved target dimensions.
    Scale {
        /// Exact factor input bits.
        factor_bits: u64,
        /// Rounded target width.
        width: u32,
        /// Rounded target height.
        height: u32,
        /// Filter supplied to the operation.
        filter: ResampleFilter,
    },
    /// Resize dimensions, filter, and boxed-path status for the FMA kernel guard.
    Resize {
        /// Requested output width.
        width: u32,
        /// Requested output height.
        height: u32,
        /// Filter selected by the public image wrapper.
        filter: ResampleFilter,
        /// Whether Resize uses fractional source bounds.
        boxed: bool,
        /// Float32-narrowed box request bits, when the caller supplied bounds.
        box_bounds_bits: Option<(u64, u64, u64, u64)>,
    },
    /// Odd square filter window and optional caller-selected rank.
    FilterWindow {
        /// Positive odd side length.
        size: u32,
        /// Rank within `size * size`; absent for Median/Min/Max.
        rank: Option<u32>,
    },
    /// Exact native Filter3x3 coefficients and scalar parameters.
    Filter3x3 {
        /// Coefficient bit patterns supplied by the operation.
        kernel_bits: [u32; 9],
        /// Scale bit pattern supplied by the operation.
        scale_bits: u32,
        /// Offset bit pattern supplied by the operation.
        offset_bits: u32,
    },
    /// Exact native Filter5x5 coefficients and scalar parameters.
    Filter5x5 {
        /// Coefficient bit patterns supplied by the operation.
        kernel_bits: [u32; 25],
        /// Scale bit pattern supplied by the operation.
        scale_bits: u32,
        /// Offset bit pattern supplied by the operation.
        offset_bits: u32,
    },
    /// GaussianBlur sigma represented by its exact f32 request bits.
    GaussianBlur {
        /// Original sigma bits retained without normalization.
        sigma_bits: u32,
    },
    /// Integer BoxBlur radius supplied by the exact operation request.
    BoxBlur { radius: u32 },
    /// Independent BoxBlurXY radii and pass count supplied by the request.
    BoxBlurXY {
        /// Horizontal radius bit pattern.
        radius_x_bits: u32,
        /// Vertical radius bit pattern.
        radius_y_bits: u32,
        /// Number of horizontal and vertical passes.
        passes: u32,
    },
    /// UnsharpMask blend parameters supplied by its direct eager entry point.
    UnsharpMask {
        /// Gaussian radius represented by its exact f32 request bits.
        sigma_bits: u32,
        /// Integer blend percent.
        percent: i32,
        /// Minimum absolute sample difference required for sharpening.
        threshold: i32,
    },
    /// Brightness factor retained for exact architecture eligibility.
    Brightness {
        /// Factor bit pattern supplied by the operation.
        factor_bits: u64,
    },
    /// Sharpness factor retained for exact architecture eligibility.
    Sharpness {
        /// Factor bit pattern supplied by the operation.
        factor_bits: u64,
    },
    /// ColorSaturation factor retained for exact vector eligibility.
    ColorSaturation {
        /// Factor bit pattern supplied by the operation.
        factor_bits: u64,
    },
    /// The internal byte-band selector, after public wrapper validation when applicable.
    ExtractBand {
        /// Requested index before the original defensive clamping behavior.
        index: u8,
    },
    /// Coordinates of a normalized internal pixel write.
    PutPixel {
        /// Destination column.
        x: u32,
        /// Destination row.
        y: u32,
    },
    /// Metadata for a run of native-byte writes.
    PutPixelBatch {
        /// Number of operations in the borrowed run.
        count: usize,
        /// Whether all writes satisfy the original specialized RGBA admission contract.
        writes: PixelWriteRunFacts,
    },
    /// Typed normalized byte-prefix parameters.
    PutData {
        /// Explicit payload identity, separate from the source image variant.
        mode: Mode,
        /// Available normalized bytes; excess bytes retain the existing prefix semantics.
        data_len: usize,
    },
    /// Secondary-image facts for the typed internal alpha-mask operation.
    PutAlphaData {
        /// Exact mask identity and accessible shape, without converting pixels.
        mask: SecondaryInputFacts,
    },
    /// Optional validated mask facts for ImageOps histogram operations.
    ImageOpsMask {
        /// `None` for an unmasked operation, otherwise exact mode and size facts.
        mask: Option<SecondaryInputFacts>,
    },
    /// Typed target identity and output channel count for Color3DLut.
    Color3DLut {
        /// Exact output identity selected by the operation request.
        target_mode: Mode,
        /// Number of LUT values produced per pixel.
        channels: u32,
        /// LUT side lengths in Pillow's x, y, z order.
        size: (u32, u32, u32),
        /// Number of supplied table entries before fixed-point preparation.
        table_len: usize,
    },
    /// Exact typed target identity for a deferred Convert operation.
    ConvertTarget {
        /// Output mode selected by the caller.
        target_mode: Mode,
    },
    /// Exact secondary image identity and shape for Chops byte operations.
    ChopsBinary {
        /// Materialized secondary raster facts.
        other: SecondaryInputFacts,
        /// Float32 scale supplied to Add/Sub, retained as bits for exact facts.
        affine_scale_bits: Option<u32>,
        /// Float32 offset supplied to Add/Sub, retained as bits for exact facts.
        affine_offset_bits: Option<u32>,
    },
    /// Materialized secondary raster for an exact same-mode, same-size operation.
    SameModeSecondaryImage {
        /// Secondary raster identity and dimensions.
        other: SecondaryInputFacts,
    },
    /// Exact source, optional mask, and region facts for a Paste request.
    Paste {
        /// Source raster identity and dimensions after public normalization.
        source: SecondaryInputFacts,
        /// Optional mask raster identity and dimensions.
        mask: Option<SecondaryInputFacts>,
        /// Requested paste region dimensions.
        region: (u32, u32),
        /// Whether mask values come from the mask alpha sample.
        mask_alpha: bool,
    },
    /// Typed Rotate parameters retained for architecture eligibility guards.
    Rotate {
        /// Exact normalized angle bits supplied by the public operation.
        angle_bits: u64,
        /// Whether Pillow's expanded canvas was requested.
        expand: bool,
        /// Optional native-byte fill tuple.
        fill: Option<(u8, u8, u8, u8)>,
        /// Exact center coordinates, preserving NaN payloads for inspection.
        center_bits: Option<(u64, u64)>,
        /// Exact post-rotation translation coordinates.
        translate_bits: Option<(u64, u64)>,
        /// Resampling selector: 0 nearest, 1 bilinear, 2 bicubic.
        filter: u8,
        /// Whether the wrapper requested nearest-neighbor sampling.
        nearest: bool,
    },
    /// A typed Pillow transpose method used by architecture-specific selection.
    Transpose {
        /// Operation method, supplied before preparing any pixel loop.
        method: TransposeMethod,
    },
    /// Border width used by `ImageOps.crop` before prepared execution.
    CropBorder {
        /// Pixels removed from every edge.
        border: u32,
    },
    /// Normalized source rectangle for a direct Crop operation.
    Crop {
        /// Source column of the crop's upper-left corner.
        left: u32,
        /// Source row of the crop's upper-left corner.
        top: u32,
        /// Width of the crop in pixels.
        width: u32,
        /// Height of the crop in pixels.
        height: u32,
        /// Width of the original source image.
        source_width: u32,
        /// Height of the original source image.
        source_height: u32,
    },
    /// Border width used by native-mode ImageOps.expand.
    Expand {
        /// Pixels added at each edge.
        border: u32,
    },
}

/// Secondary raster metadata used by an operation's linked guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondaryInputFacts {
    /// Exact raster identity carried by this input's enum variant.
    pub mode: Mode,
    /// Secondary image width in pixels.
    pub width: u32,
    /// Secondary image height in pixels.
    pub height: u32,
    /// Accessible bytes, or unresolved materialized storage.
    pub byte_len: Option<usize>,
}

/// Facts needed only when admitting the existing specialized RGBA write run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelWriteRunFacts {
    /// The caller has not inspected operation variants, coordinates, and palette tags.
    Unknown,
    /// All operations are in-bounds PutPixel values with no palette-index tag.
    NativeRgba,
    /// The run does not meet that specialization's admission contract.
    Ineligible,
}

/// Known source facts used by the same guard during assessment and preparation.
#[derive(Debug, Clone, Copy)]
pub struct StrategyInputs {
    /// Width of the source in pixels.
    pub width: u32,
    /// Height of the source in pixels.
    pub height: u32,
    /// Exact accessible sample bytes, when materialized.
    pub byte_len: Option<usize>,
    /// Palette resolution state; absence has an explicit mode-specific meaning.
    pub palette: PaletteKnowledge,
    /// Typed parameters, without mode strings or execution-policy flags.
    pub request: RequestFacts,
}

impl StrategyInputs {
    /// Supplies the band index to the same guard used by assessment and preparation.
    #[must_use]
    pub const fn with_extract_band_index(mut self, index: u8) -> Self {
        self.request = RequestFacts::ExtractBand { index };
        self
    }

    /// Supplies factors to the exact Reduce mode contract.
    #[must_use]
    pub const fn with_reduce_factors(mut self, x_factor: u32, y_factor: u32) -> Self {
        self.request = RequestFacts::Reduce { x_factor, y_factor };
        self
    }

    /// Supplies final dimensions and filter to the exact Thumbnail contract.
    #[must_use]
    pub const fn with_thumbnail(
        mut self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
    ) -> Self {
        self.request = RequestFacts::Thumbnail { width, height, filter };
        self
    }

    /// Supplies Contain's typed target and sampling parameters.
    #[must_use]
    pub const fn with_contain(
        mut self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
    ) -> Self {
        self.request = RequestFacts::Contain { width, height, filter };
        self
    }

    /// Supplies Cover's typed target and sampling parameters.
    #[must_use]
    pub const fn with_cover(
        mut self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
    ) -> Self {
        self.request = RequestFacts::Cover { width, height, filter };
        self
    }

    /// Supplies Fit's typed target and exact floating-point request facts.
    #[must_use]
    pub const fn with_fit(
        mut self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
        bleed: f64,
        centering: (f64, f64),
    ) -> Self {
        self.request = RequestFacts::Fit {
            width,
            height,
            filter,
            bleed_bits: bleed.to_bits(),
            centering_bits: (centering.0.to_bits(), centering.1.to_bits()),
        };
        self
    }

    /// Supplies Pad's typed canvas, fill, filter, and centering parameters.
    #[must_use]
    pub const fn with_pad(
        mut self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
        color: Option<(u8, u8, u8, u8)>,
        centering: (f64, f64),
    ) -> Self {
        self.request = RequestFacts::Pad {
            width,
            height,
            filter,
            color,
            centering_bits: (centering.0.to_bits(), centering.1.to_bits()),
        };
        self
    }

    /// Supplies Scale's exact factor, resolved dimensions, and filter.
    #[must_use]
    pub const fn with_scale(
        mut self,
        factor: f64,
        width: u32,
        height: u32,
        filter: ResampleFilter,
    ) -> Self {
        self.request = RequestFacts::Scale {
            factor_bits: factor.to_bits(),
            width,
            height,
            filter,
        };
        self
    }

    /// Supplies the materialized Resize request to architecture guards.
    #[must_use]
    pub const fn with_resize(
        mut self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
        boxed: bool,
    ) -> Self {
        self.request = RequestFacts::Resize {
            width,
            height,
            filter,
            boxed,
            box_bounds_bits: None,
        };
        self
    }

    /// Supplies the exact source box used to assess a boxed Resize strategy.
    #[must_use]
    pub const fn with_boxed_resize(
        mut self,
        width: u32,
        height: u32,
        filter: ResampleFilter,
        bounds: (f64, f64, f64, f64),
    ) -> Self {
        self.request = RequestFacts::Resize {
            width,
            height,
            filter,
            boxed: true,
            box_bounds_bits: Some((
                bounds.0.to_bits(),
                bounds.1.to_bits(),
                bounds.2.to_bits(),
                bounds.3.to_bits(),
            )),
        };
        self
    }

    /// Supplies a rank-family filter window to its declared input contract.
    #[must_use]
    pub const fn with_filter_window(mut self, size: u32, rank: Option<u32>) -> Self {
        self.request = RequestFacts::FilterWindow { size, rank };
        self
    }

    /// Supplies exact Filter3x3 inputs to architecture-specific guards.
    #[must_use]
    pub fn with_filter3x3(mut self, kernel: &[f32; 9], scale: f32, offset: f32) -> Self {
        self.request = RequestFacts::Filter3x3 {
            kernel_bits: kernel.map(f32::to_bits),
            scale_bits: scale.to_bits(),
            offset_bits: offset.to_bits(),
        };
        self
    }

    /// Supplies exact Filter5x5 inputs to architecture-specific guards.
    #[must_use]
    pub fn with_filter5x5(mut self, kernel: &[f32; 25], scale: f32, offset: f32) -> Self {
        self.request = RequestFacts::Filter5x5 {
            kernel_bits: kernel.map(f32::to_bits),
            scale_bits: scale.to_bits(),
            offset_bits: offset.to_bits(),
        };
        self
    }

    /// Supplies the integer BoxBlur radius to its architecture guard.
    #[must_use]
    pub const fn with_box_blur(mut self, radius: u32) -> Self {
        self.request = RequestFacts::BoxBlur { radius };
        self
    }

    /// Supplies the independent BoxBlurXY radii and pass count to its guard.
    #[must_use]
    pub fn with_box_blur_xy(mut self, radius_x: f32, radius_y: f32, passes: u32) -> Self {
        self.request = RequestFacts::BoxBlurXY {
            radius_x_bits: radius_x.to_bits(),
            radius_y_bits: radius_y.to_bits(),
            passes,
        };
        self
    }

    /// Supplies GaussianBlur's sigma to its typed request contract.
    #[must_use]
    pub fn with_gaussian_blur(mut self, sigma: f32) -> Self {
        self.request = RequestFacts::GaussianBlur {
            sigma_bits: sigma.to_bits(),
        };
        self
    }

    /// Supplies UnsharpMask's radius and blend parameters to its strategy.
    #[must_use]
    pub fn with_unsharp_mask(mut self, sigma: f32, percent: i32, threshold: i32) -> Self {
        self.request = RequestFacts::UnsharpMask {
            sigma_bits: sigma.to_bits(),
            percent,
            threshold,
        };
        self
    }

    /// Supplies Brightness's factor to the declared vector eligibility guard.
    #[must_use]
    pub const fn with_brightness(mut self, factor: f64) -> Self {
        self.request = RequestFacts::Brightness {
            factor_bits: factor.to_bits(),
        };
        self
    }

    /// Supplies Sharpness's factor to its declared vector eligibility guard.
    #[must_use]
    pub const fn with_sharpness(mut self, factor: f64) -> Self {
        self.request = RequestFacts::Sharpness {
            factor_bits: factor.to_bits(),
        };
        self
    }

    /// Supplies ColorSaturation's factor to its declared vector guard.
    #[must_use]
    pub const fn with_color_saturation(mut self, factor: f64) -> Self {
        self.request = RequestFacts::ColorSaturation {
            factor_bits: factor.to_bits(),
        };
        self
    }

    /// Supplies the border used by the exact-mode ImageOps.expand strategy.
    #[must_use]
    pub const fn with_expand_border(mut self, border: u32) -> Self {
        self.request = RequestFacts::Expand { border };
        self
    }

    /// Supplies the normalized rectangle to native Crop architecture guards.
    #[must_use]
    pub const fn with_crop_region(
        mut self,
        left: u32,
        top: u32,
        width: u32,
        height: u32,
        source_width: u32,
        source_height: u32,
    ) -> Self {
        self.request = RequestFacts::Crop {
            left,
            top,
            width,
            height,
            source_width,
            source_height,
        };
        self
    }

    /// Supplies the expanded LUT length to the exact-mode Eval contract.
    #[must_use]
    pub const fn with_eval_lut_entries(mut self, entries: usize) -> Self {
        self.request = RequestFacts::EvalLut { entries };
        self
    }

    /// Supplies the expanded f64 table length to the exact-mode PointFloat contract.
    #[must_use]
    pub const fn with_point_float_lut_entries(mut self, entries: usize) -> Self {
        self.request = RequestFacts::PointFloatLut { entries };
        self
    }

    /// Supplies Convert's exact typed output identity.
    #[must_use]
    pub const fn with_convert_target(mut self, target_mode: Mode) -> Self {
        self.request = RequestFacts::ConvertTarget { target_mode };
        self
    }

    /// Supplies the materialized secondary raster to a Chops byte contract.
    #[must_use]
    pub const fn with_chops_secondary(mut self, other: SecondaryInputFacts) -> Self {
        self.request = RequestFacts::ChopsBinary {
            other,
            affine_scale_bits: None,
            affine_offset_bits: None,
        };
        self
    }

    /// Supplies the secondary raster to an exact same-mode, same-size contract.
    #[must_use]
    pub const fn with_same_mode_secondary_image(mut self, other: SecondaryInputFacts) -> Self {
        self.request = RequestFacts::SameModeSecondaryImage { other };
        self
    }

    /// Supplies the exact source and optional mask metadata for Paste.
    #[must_use]
    pub const fn with_paste(
        mut self,
        source: SecondaryInputFacts,
        mask: Option<SecondaryInputFacts>,
        region: (u32, u32),
        mask_alpha: bool,
    ) -> Self {
        self.request = RequestFacts::Paste {
            source,
            mask,
            region,
            mask_alpha,
        };
        self
    }

    /// Supplies Add/Sub's narrowed float parameters and the materialized operand.
    #[must_use]
    pub fn with_chops_affine_secondary(
        mut self,
        other: SecondaryInputFacts,
        scale: f32,
        offset: f32,
    ) -> Self {
        self.request = RequestFacts::ChopsBinary {
            other,
            affine_scale_bits: Some(scale.to_bits()),
            affine_offset_bits: Some(offset.to_bits()),
        };
        self
    }
}

