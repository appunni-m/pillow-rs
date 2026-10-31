//! Typed request facts and shared declaration guards.

use crate::error::PilError;
use crate::raster::Mode;

mod contracts;
mod facts;
mod guards;

pub use facts::{PixelWriteRunFacts, RequestFacts, SecondaryInputFacts, StrategyInputs};

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
    /// Filter3x3 receives its exact kernel coefficients, scale, and offset.
    Filter3x3Parameters,
    /// Filter5x5 receives its exact kernel coefficients, scale, and offset.
    Filter5x5Parameters,
    /// GaussianBlur receives the requested sigma without normalization.
    GaussianBlurSigma,
    /// BoxBlur receives exactly one integer or independent-axis radius request.
    BoxBlurParameters,
    /// UnsharpMask receives its Gaussian radius, blend percent, and threshold.
    UnsharpMaskParameters,
    /// Sharpness receives its exact blend factor, including non-finite values.
    SharpnessFactor,
    /// Posterize receives the wrapper-normalized number of retained high bits.
    PosterizeBits,
    /// EffectSpread consumes a maximum native-pixel displacement distance.
    EffectSpreadDistance,
    /// EffectNoise consumes typed f64 sigma and rounds at Pillow's f32 boundary.
    EffectNoiseSigma,
    /// EffectMandelbrot consumes output dimensions, extent, and quality.
    EffectMandelbrotParameters,
    /// Rotate receives normalized angle, filter, fill, center, and translation facts.
    Rotate,
    /// Eval receives exactly 256 byte-LUT entries for each native band.
    ByteLutEntries {
        /// Number of byte bands in this exact mode's native representation.
        channels: u8,
    },
    /// PointFloat receives exactly 256 f64 entries for each source band.
    PointFloatLutEntries {
        /// Number of byte bands in this exact mode's native representation.
        channels: u8,
    },
    /// Reduce receives normalized factors; indexed modes may permit identity only.
    ReduceFactors {
        /// Whether this exact mode supports only a no-op reduction.
        identity_only: bool,
    },
    /// Positive output bounds and filter supplied to a materialized Thumbnail.
    Thumbnail,
    /// Contain target dimensions and resampling filter.
    ContainParameters,
    /// Cover target dimensions and resampling filter.
    CoverParameters,
    /// Fit dimensions, filter, bleed, and centering.
    FitParameters,
    /// Pad target dimensions, filter, fill color, and centering.
    PadParameters,
    /// Scale factor and resampling filter.
    ScaleParameters,
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
    /// Exact target identities supported by one source mode's Convert strategy.
    ConvertTarget {
        /// Exhaustive accepted target modes for this source-mode declaration.
        targets: &'static [Mode],
        /// Accepted target identities whose source/target pair is a core-only extension.
        core_only_targets: &'static [Mode],
        /// Why the listed target pairs have no Pillow parity contract.
        core_only_reason: Option<&'static str>,
        /// Existing conversion error when the requested pair is not declared.
        rejection_reason: &'static str,
    },
    /// Exact secondary raster identities accepted by a Chops byte strategy.
    ChopsBinaryModes {
        /// Secondary identities whose native byte layout is compatible.
        modes: &'static [Mode],
        /// Existing rejection when the secondary identity is incompatible.
        rejection_reason: &'static str,
    },
    /// Secondary raster must use this exact mode and match the source dimensions.
    SameModeSecondaryImage {
        /// Required secondary identity; supplied by the source mode strategy.
        mode: Mode,
        /// Existing rejection for an identity or size mismatch.
        rejection_reason: &'static str,
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
    /// Odd, positive square neighborhood used by Pillow rank filters.
    OddRankWindow {
        /// Whether this operation requires a caller-selected rank inside the window.
        rank_required: bool,
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

/// Existing native-layout eligibility condition for a specialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyGuard {
    /// No additional request condition.
    Always,
    /// Filtered Resize has work for the scalar x86 FMA implementation.
    ResizeFilteredFma,
    /// ImageI32 Thumbnail can stream one even 2x2 reducing-gap pass.
    ThumbnailI32Stream2x2,
    /// Unboxed I;16 Resize has exact two-byte samples and safe vector dimensions.
    ResizeLuma16Vector,
    /// Native-byte nearest Resize has exact storage and safe vector dimensions.
    ResizeByteNearestVector {
        /// Physical byte channels in the exact mode's storage.
        channels: u8,
    },
    /// Exact mode's declared native-byte Resize strategies for this architecture.
    ResizeByteNativeVector {
        /// Physical byte channels in the exact mode's storage.
        channels: u8,
    },
    /// Boxed native-byte Resize has valid bounds, dimensions, and coefficients.
    ResizeByteBoxedVector {
        /// Physical byte channels in the exact mode's storage.
        channels: u8,
    },
    /// Boxed native-byte nearest Resize with valid bounds and dimensions.
    ResizeByteBoxedNearestVector {
        /// Physical byte channels in the exact mode's storage.
        channels: u8,
    },
    /// Unboxed I32 Resize has exact four-byte samples and safe NEON dimensions.
    ResizeI32Neon,
    /// Native byte Brightness kernel with a finite factor and exact storage.
    BrightnessVector {
        /// Physical byte channels in this exact mode's stored pixel.
        channels: u8,
    },
    /// Native-byte Sharpness with a finite factor and an exact 3x3 layout.
    SharpnessNativeBytesVectorizable {
        /// Physical byte channels in this exact mode's stored pixel.
        channels: u8,
    },
    /// Native float32 ColorSaturation kernel with an exact byte layout.
    ColorSaturationVector {
        /// Physical byte channels in this exact mode's stored pixel.
        channels: u8,
    },
    /// Native-byte Filter3x3 parameters and shape accepted by a vector kernel.
    Filter3x3NativeBytesVectorizable {
        /// Physical byte channels in this exact mode's storage.
        channels: u8,
    },
    /// Native-byte Filter5x5 convolution with one complete interior block.
    Filter5x5NativeBytesVectorizable {
        /// Physical byte channels in this exact mode's storage.
        channels: u8,
    },
    /// Native-byte BoxBlur or GaussianBlur with an exact mode layout.
    FilterNativeByteBlur {
        /// Physical byte channels in this exact mode's storage.
        channels: u8,
    },
    /// RGBA GaussianBlur radius-seven NEON path with a sufficiently large image.
    FilterRgbaGaussianBlurRadiusSeven,
    /// L-mode rank filter window accepted by the byte selector.
    FilterLumaRankWindow,
    /// Native-byte MaxFilter or MinFilter window accepted by vector extrema kernels.
    FilterNativeByteExtrema {
        /// Number of interleaved native bytes in the exact mode's pixel.
        channels: u8,
    },
    /// Native-byte order-statistic window supported by the vector selector.
    FilterNativeByteRankWindow {
        /// Number of interleaved native bytes in the exact mode's pixel.
        channels: u8,
    },
    /// F-mode vector rank window with exact native four-byte samples.
    FilterF32RankWindow,
    /// Native RGB 150% UnsharpMask blend with one complete vector block.
    FilterUnsharpRgb150,
    /// I32 AVX2/FMA Filter3x3's finite kernel and complete vector block.
    Filter3x3I32Vectorizable {
        /// Native byte channels in this exact mode's storage.
        channels: u8,
    },
    /// One exact typed target mode selects this implementation.
    ConvertTargetMode(Mode),
    /// One exact typed target selects a vector route only for nonempty images.
    ConvertTargetModeNonEmpty(Mode),
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
    /// AlphaComposite's eight-pixel vector kernel over exact native LA or RGBA bytes.
    AlphaCompositeVector {
        /// Physical number of byte channels in the exact mode's stored pixel.
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
    /// Eight-lane Mandelbrot generation with at least one complete vector per row.
    EffectMandelbrotVector,
    /// Equal primary and secondary dimensions required by native-byte Chops vectors.
    ChopsEqualDimensions,
    /// Chops Add/Sub request admitted by the native byte-vector kernel.
    ChopsAddSubtractVector {
        /// Native bytes consumed per pixel by the exact mode strategy.
        channels: u8,
    },
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
    /// Exact F-mode float samples and a non-identity Reduce request.
    ReduceF32Vector,
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
            Self::ResizeFilteredFma => guards::resize_filtered_fma_guard(input),
            Self::ThumbnailI32Stream2x2 => guards::thumbnail_i32_stream_guard(input),
            Self::ResizeLuma16Vector => guards::resize_luma16_vector_guard(input),
            Self::ResizeByteNearestVector { channels } => {
                guards::resize_byte_nearest_vector_guard(input, channels)
            }
            Self::ResizeByteNativeVector { channels } => {
                guards::resize_byte_native_vector_guard(input, channels)
            }
            Self::ResizeByteBoxedVector { channels } => {
                guards::resize_byte_boxed_vector_guard(input, channels)
            }
            Self::ResizeByteBoxedNearestVector { channels } => {
                guards::resize_byte_boxed_nearest_vector_guard(input, channels)
            }
            Self::ResizeI32Neon => guards::resize_i32_neon_guard(input),
            Self::BrightnessVector { channels } => {
                guards::brightness_vector_guard(input, channels)
            }
            Self::SharpnessNativeBytesVectorizable { channels } => {
                guards::sharpness_vector_guard(input, channels)
            }
            Self::ColorSaturationVector { channels } => {
                guards::color_saturation_vector_guard(input, channels)
            }
            Self::ChopsEqualDimensions => guards::chops_equal_dimensions_guard(input),
            Self::ChopsAddSubtractVector { channels } => {
                guards::chops_add_subtract_guard(input, channels)
            }
            Self::Filter3x3NativeBytesVectorizable { channels } => {
                guards::filter3x3_native_bytes_vectorizable_guard(input, channels)
            }
            Self::Filter5x5NativeBytesVectorizable { channels } => {
                guards::filter5x5_native_bytes_vectorizable_guard(input, channels)
            }
            Self::FilterNativeByteBlur { channels } => {
                guards::filter_native_byte_blur_guard(input, channels)
            }
            Self::FilterRgbaGaussianBlurRadiusSeven => {
                guards::filter_rgba_gaussian_blur_radius_seven_guard(input)
            }
            Self::FilterLumaRankWindow => guards::filter_luma_rank_window_guard(input),
            Self::FilterNativeByteExtrema { channels } => {
                guards::filter_native_byte_extrema_guard(input, channels)
            }
            Self::FilterNativeByteRankWindow { channels } => {
                guards::filter_native_byte_rank_window_guard(input, channels)
            }
            Self::FilterF32RankWindow => guards::filter_f32_rank_window_guard(input),
            Self::FilterUnsharpRgb150 => guards::filter_unsharp_rgb150_guard(input),
            Self::Filter3x3I32Vectorizable { channels } => {
                guards::filter3x3_i32_vectorizable_guard(input, channels)
            }
            Self::ConvertTargetMode(target) => {
                guards::convert_target_guard(input, target, false)
            }
            Self::ConvertTargetModeNonEmpty(target) => {
                guards::convert_target_guard(input, target, true)
            }
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
            Self::ReduceF32Vector => guards::reduce_f32_vector_guard(input),
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
            Self::EffectMandelbrotVector => guards::effect_mandelbrot_vector_guard(input),
            Self::NativeBytes { channels } => guards::native_bytes_guard(input, channels),
            Self::AlphaCompositeVector { channels } => {
                guards::alpha_composite_vector_guard(input, channels)
            }
            Self::ExpandVector { channels } => guards::expand_vector_guard(input, channels),
            Self::CheckedNativeBytes { channels } =>
                guards::checked_native_bytes_guard(input, channels),
        }
    }
}
