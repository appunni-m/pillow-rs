//! Typed request facts and shared declaration guards.

use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::Mode;

mod contracts;
mod guards;

/// Output channel capacity for one exact operation target mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeChannelCapacity {
    /// Exact output mode identity.
    pub mode: Mode,
    /// Maximum output samples produced per pixel.
    pub channels: u8,
}

/// Input requirements exposed by an operation's support contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputContract {
    /// Posterize receives the wrapper-normalized number of retained high bits.
    PosterizeBits,
    /// EffectSpread consumes a maximum native-pixel displacement distance.
    EffectSpreadDistance,
    /// EffectNoise consumes typed f64 sigma and rounds at Pillow's f32 boundary.
    EffectNoiseSigma,
    /// Rotate receives normalized angle, filter, fill, center, and translation facts.
    Rotate,
    /// Eval receives exactly 256 byte-LUT entries for each native band.
    ByteLutEntries {
        /// Number of byte bands in this exact mode's native representation.
        channels: u8,
    },
    /// Reduce receives normalized factors; indexed modes may permit identity only.
    ReduceFactors {
        /// Whether this exact mode supports only a no-op reduction.
        identity_only: bool,
    },
    /// Source bounds and coordinate subtraction preconditions of a normalized crop.
    NormalizedCrop,
    /// Equal border removal, including Pillow's ordered bounds errors and valid empty output.
    CropBorder,
    /// Existing checked allocation policy for a native byte output.
    CheckedByteDimensions {
        /// Native storage width used by the allocation/stride contract.
        bytes_per_pixel: u8,
        /// Whether this algorithm's existing checked-dimension policy admits empty output.
        allow_empty: bool,
    },
    /// Native source shape and border-expanded allocation for ImageOps.expand.
    ExpandedDimensions {
        /// Number of stored bytes per pixel in this exact mode.
        bytes_per_pixel: u8,
    },
    /// The wrapper supplies palette colors and optional palette alpha separately.
    PaletteMetadata,
    /// ImageOps autocontrast/equalize masks are validated as same-size L/1 images.
    ImageOpsLumaMask,
    /// Existing defensive index handling for direct internal band requests.
    ExtractBandIndex(BandIndexRule),
    /// Image.putpixel validates coordinates before constructing the internal write.
    CallerValidatedPixelCoordinates,
    /// Internal batch execution retains ordered variant/bounds checks and last-write-wins.
    OrderedPixelWrites,
    /// Normalize a secondary mask with the existing typed-to-L conversion,
    /// then consume its pixel prefix through the original zip traversal.
    /// Public Image.putalpha separately requires a same-size 1/L mask.
    AlphaMaskLumaPrefix {
        /// Exact mask identities whose original conversion is retained.
        modes: &'static [Mode],
        /// Identity of the normalized mask consumed by the alpha loop.
        normalized: Mode,
    },
    /// Accepted normalized byte payload identities, independently of the source mode.
    ByteDataPayload {
        /// Exhaustive accepted payload identities declared with their executable strategies.
        modes: &'static [Mode],
        /// Rejection for a typed payload outside that declaration.
        rejection_reason: &'static str,
    },
    /// Color3DLut output mode and channel bound supplied before strategy selection.
    Color3DLutTarget {
        /// Exact target identities and their accepted channel capacities.
        targets: &'static [ModeChannelCapacity],
    },
    /// A semantic source-buffer requirement; failure rejects instead of falling back.
    ExactNativeBytes {
        /// Exact native byte channels required by this implementation.
        channels: u8,
        /// Existing overflow error for computing the expected source length.
        overflow_reason: &'static str,
        /// Existing rejection when the source length is not exact.
        mismatch_reason: &'static str,
    },
}

/// Defensive indexing of the internal ExtractBand operation, after public validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandIndexRule {
    /// Indices zero through two copy luma; higher indices produce opaque alpha.
    LumaWithOpaqueAlpha,
    /// Indices zero through two select RGB; higher indices produce opaque alpha.
    RgbWithOpaqueAlpha,
    /// Clamp to the final native channel, preserving LA/PA and four-byte carriers.
    ClampToNative {
        /// Number of native channels in this mode's existing implementation.
        channels: u8,
    },
}

/// Whether palette metadata has been resolved, independently of its contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteKnowledge {
    /// A lazy input has not supplied its palette state yet.
    Unknown,
    /// Palette resolution completed and no palette is attached.
    Absent,
    /// Palette resolution completed and supplies palette colors.
    Present,
}

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
    /// Expanded byte lookup table supplied to the exact-mode Eval operation.
    EvalLut {
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
}

/// Existing native-layout eligibility condition for a specialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyGuard {
    /// No additional request condition.
    Always,
    /// Exact native byte storage with at least one original alpha vector group.
    PutAlphaBytes {
        /// Byte width selected by this mode's concrete implementation.
        channels: u8,
        /// Original operation-specific minimum vector group.
        minimum_pixels: usize,
    },
    /// Original native alpha-mask vector admission, including its L-only mask.
    PutAlphaMaskBytes {
        /// Native source byte width selected by this mode strategy.
        channels: u8,
        /// Original minimum number of pixels for a vector group.
        minimum_pixels: usize,
    },
    /// One complete tightly packed native byte sample per pixel.
    NativeBytes {
        /// Physical number of byte channels in this particular implementation.
        channels: u8,
    },
    /// Finite, non-discrete nearest or bilinear Rotate over native byte rows.
    RotateVectorBytes {
        /// Physical number of bytes in this exact mode's stored pixel.
        channels: u8,
        /// Whether the exact mode forces nearest sampling (for example, `1` or `P`).
        force_nearest: bool,
        /// Whether this mode's architecture strategy also implements bilinear sampling.
        bilinear: bool,
    },
    /// SIMD Expand layout used by the archived byte-fill/copy strategy.
    ///
    /// One- and two-byte layouts vectorize every non-identity expansion. The
    /// three- and four-byte layouts use this strategy only for border-zero
    /// identity copies, matching the operations that contain actual vector
    /// work in the archived adapter.
    ExpandVector {
        /// Physical byte channels in this exact mode's implementation.
        channels: u8,
    },
    /// Native u16 crop region and exact source bytes for the archived vector copy.
    NativeCropBytes {
        /// Physical bytes moved per source pixel.
        channels: u8,
    },
    /// Exact native source bytes and nonempty CropBorder result for I;16 copying.
    NativeCropBorderBytes {
        /// Physical bytes moved per source pixel.
        channels: u8,
    },
    /// Native byte rows are present; trailing storage is retained but not transformed.
    NativeBytePrefix {
        /// Physical number of byte channels in this particular implementation.
        channels: u8,
    },
    /// Nonempty rows with enough native bytes for the horizontal mirror kernel.
    NativeMirrorRows {
        /// Physical number of byte channels in this particular implementation.
        channels: u8,
    },
    /// Nonempty, exact native bytes for a same-mode byte-wise point operation.
    NativePointwiseBytes {
        /// Physical number of byte channels in this particular implementation.
        channels: u8,
    },
    /// Nonempty, exact native byte rows for opaque sample-preserving movement.
    ExactNativeRows {
        /// Physical number of bytes in each sample pixel.
        channels: u8,
    },
    /// The native byte allocation also satisfies the nonempty checked-dimension policy.
    CheckedNativeBytes {
        /// Physical number of byte channels in this particular implementation.
        channels: u8,
    },
    /// Nonempty exact native storage and an index inside that native layout.
    ExtractBandBytes {
        /// Channels addressed by this concrete byte-movement implementation.
        channels: u8,
    },
    /// Native byte storage used by EffectSpread's architecture implementation.
    EffectSpreadBytes {
        /// Physical byte channels in this exact mode's representation.
        channels: u8,
    },
    /// Nonempty image dimensions and finite sigma for EffectNoise's vector data plane.
    EffectNoiseVector,
    /// One in-bounds byte write over the mode's native raster, allowing trailing storage.
    PutPixelSpan {
        /// Native byte stride fixed by this mode implementation.
        channels: u8,
    },
    /// At least two admitted writes over an exact RGBA raster with one complete vector.
    RgbaPixelWriteRun,
    /// RGBA axis-swap method with exact bytes and dimensions large enough for AVX2 tiles.
    TransposeRgbaAvx2,
    /// Native byte transpose method over an exact source layout.
    TransposeNativeBytes {
        /// Physical bytes moved together for one source pixel.
        channels: u8,
    },
    /// Eight-lane Color3DLut interpolation over an exact native byte source.
    Color3DLutVector {
        /// Physical number of bytes in each input pixel.
        source_channels: u8,
    },
    /// Exact native byte layout and bounded lane sums for AVX2 Reduce.
    ReduceNativeByteVector {
        /// Physical number of bytes in this exact mode's stored pixel.
        channels: u8,
    },
    /// Exact native source bytes and a payload with the same byte width.
    PutDataNative {
        /// Fixed byte width of this mode's native copy implementation.
        channels: u8,
        /// Compatible payloads from the same declaration that constructs payload strategies.
        payloads: &'static [Mode],
    },
}

pub(super) enum GuardAssessment {
    Satisfied,
    Ineligible,
    Deferred,
}

/// The next unresolved fact required by an operation's declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnresolvedGuard {
    /// A concrete specialization needs materialized layout facts.
    Implementation(StrategyGuard),
    /// Palette presence or absence has not yet been resolved by the wrapper.
    PaletteMetadata,
    /// A semantic input requirement needs materialized source facts.
    InputContract(InputContract),
}

impl InputContract {
    pub(crate) fn validate_materialized(self, input: StrategyInputs) -> Result<(), PilError> {
        if self.assess(input)?.is_some() {
            return Err(PilError::InternalError(
                "materialized input has unresolved contract facts".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn assess(self, input: StrategyInputs) -> Result<Option<UnresolvedGuard>, PilError> {
        contracts::assess(self, input)
    }
}

impl StrategyGuard {
    pub(super) fn assess(self, input: StrategyInputs) -> GuardAssessment {
        match self {
            Self::Always => GuardAssessment::Satisfied,
            Self::PutAlphaMaskBytes { channels, minimum_pixels } =>
                guards::alpha_mask_guard(input, channels, minimum_pixels),
            Self::PutAlphaBytes { channels, minimum_pixels } =>
                guards::put_alpha_bytes_guard(input, channels, minimum_pixels),
            Self::ExtractBandBytes { channels } =>
                guards::extract_band_bytes_guard(input, channels),
            Self::RotateVectorBytes { channels, force_nearest, bilinear } =>
                guards::rotate_vector_bytes_guard(input, channels, force_nearest, bilinear),
            Self::PutPixelSpan { channels } => guards::pixel_span_guard(input, channels),
            Self::RgbaPixelWriteRun => guards::pixel_write_run_guard(input),
            Self::TransposeRgbaAvx2 => guards::rgba_transpose_avx2_guard(input),
            Self::TransposeNativeBytes { channels } =>
                guards::native_transpose_bytes_guard(input, channels),
            Self::Color3DLutVector { source_channels } =>
                guards::color3dlut_guard(input, source_channels),
            Self::ReduceNativeByteVector { channels } =>
                guards::reduce_native_byte_vector_guard(input, channels),
            Self::NativeBytePrefix { channels } =>
                guards::native_byte_prefix_guard(input, channels),
            Self::NativeMirrorRows { channels } =>
                guards::native_mirror_rows_guard(input, channels),
            Self::NativePointwiseBytes { channels } =>
                guards::native_pointwise_bytes_guard(input, channels),
            Self::NativeCropBytes { channels } =>
                guards::native_crop_bytes_guard(input, channels),
            Self::NativeCropBorderBytes { channels } =>
                guards::native_crop_border_bytes_guard(input, channels),
            Self::ExactNativeRows { channels } =>
                guards::exact_native_rows_guard(input, channels),
            Self::PutDataNative { channels, payloads } =>
                guards::put_data_native_guard(input, channels, payloads),
            Self::EffectSpreadBytes { channels } =>
                guards::effect_spread_bytes_guard(input, channels),
            Self::EffectNoiseVector => guards::effect_noise_vector_guard(input),
            Self::NativeBytes { channels } => guards::native_bytes_guard(input, channels),
            Self::ExpandVector { channels } => guards::expand_vector_guard(input, channels),
            Self::CheckedNativeBytes { channels } =>
                guards::checked_native_bytes_guard(input, channels),
        }
    }
}
