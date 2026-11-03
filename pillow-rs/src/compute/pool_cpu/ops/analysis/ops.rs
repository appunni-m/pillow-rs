//! Direct analysis and GetData support contracts with prepared CPU selection.
//! TODO(pillow-rs#3ptb): include all exact-mode GetData band counts and direct
//! output-buffer architecture receipts in the frozen CPU parity matrix.

use crate::compute::{Backend, ExecutionContext};
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationModeRoute, OperationRouteGuard,
    OperationRoutePath, OperationSupport, OperationValueResult, OutputModeRule, PixelValueShape,
    RejectionKind, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) use super::helpers::histogram_execution::{
    prepare_entropy_histogram, prepare_histogram,
};
pub(crate) use super::helpers::bbox_execution::prepare as prepare_get_bbox;
pub(crate) use super::helpers::extrema_execution::prepare as prepare_get_extrema;
pub(crate) use super::helpers::extrema_bytes_execution::{
    prepare as prepare_get_extrema_bytes,
};
pub(crate) use super::helpers::getcolors_execution::prepare_getcolors;
pub(crate) use super::helpers::stat_execution::prepare_stat;

#[path = "ops/get_bbox.rs"]
mod get_bbox;
pub(crate) use get_bbox::GET_BBOX_SUPPORT;

#[path = "ops/get_projection.rs"]
mod get_projection;
pub(crate) use get_projection::{
    execute_cmyk8_selected, execute_rgb8_selected, prepare, PreparedProjection,
    ProjectionOutput, SUPPORT,
};


const NO_INPUTS: &[InputContract] = &[];
const NO_RASTER_OUTPUT: OutputModeRule = OutputModeRule::NoRasterResult;
const HISTOGRAM_MASK_MODES: &[Mode] = &[Mode::One, Mode::ImageLuma8];
const HISTOGRAM_EXTREMA_MODES: &[Mode] = &[Mode::ImageI32, Mode::ImageF32];
const HISTOGRAM_MASK_INPUT: InputContract = InputContract::HistogramMask {
    modes: HISTOGRAM_MASK_MODES,
    mode_rejection: "bad transparency mask",
    size_rejection: "images do not match",
};
const ANALYSIS_MASK_INPUTS: &[InputContract] = &[HISTOGRAM_MASK_INPUT];
pub(super) const HISTOGRAM_INPUTS: &[InputContract] = &[
    HISTOGRAM_MASK_INPUT,
    InputContract::HistogramExtrema {
        modes: HISTOGRAM_EXTREMA_MODES,
    },
];

fn pixel_value_shape(mode: Mode) -> PixelValueShape {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => PixelValueShape::U8Scalar,
        Mode::ImageI32 | Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            PixelValueShape::I32Scalar
        }
        Mode::ImageF32 => PixelValueShape::F64Scalar,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            PixelValueShape::U8Components { bands: 2 }
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            PixelValueShape::U8Components { bands: 3 }
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => PixelValueShape::U8Components { bands: 4 },
        Mode::ImageLumaA16 => PixelValueShape::U16Components { bands: 2 },
        Mode::ImageRgb16 => PixelValueShape::U16Components { bands: 3 },
        Mode::ImageRgba16 => PixelValueShape::U16Components { bands: 4 },
        Mode::ImageRgb32F => PixelValueShape::F32Components { bands: 3 },
        Mode::ImageRgba32F => PixelValueShape::F32Components { bands: 4 },
    }
}

fn classify_get_pixel_mode(mode: Mode) -> ModeSupport {
    let support = match mode {
        // NO_PILLOW_SUPPORT: internal high-depth identities expose native samples.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetPixel,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "native high-depth GetPixel values extend Pillow's mode contract",
        ),
        _ => ModeSupport::cpu(
            OperationId::GetPixel,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
    };
    support.with_value_result(OperationValueResult::Pixel(pixel_value_shape(mode)))
}

/// Exact-mode result shapes and Default-only strategies for Pillow-shaped GetPixel.
pub(crate) static GET_PIXEL_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::GetPixel,
    NO_INPUTS,
    classify_get_pixel_mode,
);

fn classify_get_pixel_rgba_mode(mode: Mode) -> ModeSupport {
    // NO_PILLOW_SUPPORT: this Rust/JavaScript convenience route projects every mode to RGBA8.
    ModeSupport::cpu_core_only(
        OperationId::GetPixelRgba,
        mode,
        NO_RASTER_OUTPUT,
        NO_INPUTS,
        &[],
        "the Rust/JavaScript accessor returns a fixed RGBA8 value",
    )
    .with_value_result(OperationValueResult::Pixel(PixelValueShape::Rgba8))
}

/// Exact-mode Default strategies for the Rust/JavaScript RGBA8 accessor.
pub(crate) static GET_PIXEL_RGBA_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::GetPixelRgba,
    NO_INPUTS,
    classify_get_pixel_rgba_mode,
);

fn classify_get_extrema_bytes_mode(mode: Mode) -> ModeSupport {
    match mode {
        // NO_PILLOW_SUPPORT: this Rust/JS compatibility API returns byte
        // projections for modes whose Pillow-facing extrema are typed.
        Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetExtremaBytes,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "Rust/JS byte-valued extrema preserves the exact-mode result contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8 => ModeSupport::cpu(
            OperationId::GetExtremaBytes,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
    }
}

/// Exhaustive mode support for byte-valued Rust/JS compatibility extrema.
/// TODO(pillow-rs#3ptb): freeze each mode's byte-result contract and Default
/// fallback receipt for every non-Default Architecture request in CPU parity.
pub(crate) static GET_EXTREMA_BYTES_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::GetExtremaBytes,
    NO_INPUTS,
    classify_get_extrema_bytes_mode,
);

fn classify_analysis_mode(operation: OperationId, mode: Mode) -> ModeSupport {
    let inputs = if matches!(operation, OperationId::Histogram | OperationId::Entropy) {
        HISTOGRAM_INPUTS
    } else {
        ANALYSIS_MASK_INPUTS
    };
    match mode {
        // NO_PILLOW_SUPPORT: exact native sample analysis extends Pillow's
        // operation surface while preserving the established 256-bin layout.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            operation,
            mode,
            NO_RASTER_OUTPUT,
            inputs,
            &[],
            match operation {
                OperationId::Histogram => {
                    "native high-depth Histogram extends Pillow's mode contract"
                }
                OperationId::Entropy => {
                    "native high-depth Entropy extends Pillow's mode contract"
                }
                _ => unreachable!("unsupported operation in analysis mode contract"),
            },
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            operation,
            mode,
            NO_RASTER_OUTPUT,
            inputs,
            &[],
        ),
    }
}

fn classify_histogram_mode(mode: Mode) -> ModeSupport {
    let result_bands = match mode {
        // Preserve the previous RGBA8 projection's synthesized opaque band.
        Mode::ImageRgb32F => 4,
        _ => mode.band_count(),
    };
    classify_analysis_mode(OperationId::Histogram, mode).with_value_result(
        OperationValueResult::Histogram {
            result_bands,
            bins_per_band: 256,
        },
    )
}

fn classify_entropy_mode(mode: Mode) -> ModeSupport {
    classify_analysis_mode(OperationId::Entropy, mode)
}

/// Exhaustive Histogram contract, including mask inputs and its per-mode
/// result band/bin layout.
pub(crate) static HISTOGRAM_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Histogram, HISTOGRAM_INPUTS, classify_histogram_mode);

/// Exhaustive Entropy support; the implementation composes the Histogram child.
pub(crate) static ENTROPY_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Entropy,
    ANALYSIS_MASK_INPUTS,
    classify_entropy_mode,
    &[OperationId::Histogram],
);

fn classify_getcolors_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            ModeSupport::Rejected {
                mode,
                reason: "image has wrong mode",
                kind: RejectionKind::ValueError,
            }
        }
        // NO_PILLOW_SUPPORT: GetColors returns native-width tuples for these modes.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetColors,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "native high-depth GetColors extends Pillow's mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32 => ModeSupport::cpu(
            OperationId::GetColors,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
    }
}

/// Exhaustive GetColors mode contract, including Pillow's I;16 rejection.
/// TODO(pillow-rs#3ptb): freeze CPU parity and terminal-receipt cases for the
/// 25-mode × Architecture matrix before marking this operation complete.
pub(crate) static GET_COLORS_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetColors, NO_INPUTS, classify_getcolors_mode);

fn classify_stat_mode(mode: Mode) -> ModeSupport {
    match mode {
        // NO_PILLOW_SUPPORT: native high-depth histogram bins extend Stat.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::Stat,
            mode,
            NO_RASTER_OUTPUT,
            ANALYSIS_MASK_INPUTS,
            &[],
            "native-sample Stat extends Pillow's mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::Stat,
            mode,
            NO_RASTER_OUTPUT,
            ANALYSIS_MASK_INPUTS,
            &[],
        ),
    }
}

/// Exhaustive Stat support; non-scalar modes compose the Histogram child.
/// TODO(pillow-rs#3ptb): freeze all mode/architecture and lazy terminal cases.
pub(crate) static STAT_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Stat,
    ANALYSIS_MASK_INPUTS,
    classify_stat_mode,
    &[OperationId::Histogram],
);

const GET_DATA_BAND_1_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetDataBandIndex { bands: 1 }];
const GET_DATA_BAND_2_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetDataBandIndex { bands: 2 }];
const GET_DATA_BAND_3_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetDataBandIndex { bands: 3 }];
const GET_DATA_BAND_4_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetDataBandIndex { bands: 4 }];
const GET_CHANNEL_1_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetChannelIndex { bands: 1 }];
const GET_CHANNEL_2_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetChannelIndex { bands: 2 }];
const GET_CHANNEL_3_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetChannelIndex { bands: 3 }];
const GET_CHANNEL_4_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetChannelIndex { bands: 4 }];
fn classify_split_mode(mode: Mode) -> ModeSupport {
    match mode {
        // NO_PILLOW_SUPPORT: native high-depth bands are exposed as exact-mode
        // children through the composed ExtractBand operation.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::Split,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "native high-depth Split extends the Pillow mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::Split,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
    }
}

const SPLIT_PALETTE_COPY_PATH: &[OperationRoutePath] =
    &[OperationRoutePath::PalettePreservingCopy];
const SPLIT_IDENTITY_COPY_PATH: &[OperationRoutePath] =
    &[OperationRoutePath::IdentityCopy];
const SPLIT_EXTRACT_BANDS_PATH: &[OperationRoutePath] =
    &[OperationRoutePath::PipelineChildrenPerBand(OperationId::ExtractBand)];

fn classify_split_route(mode: Mode) -> OperationModeRoute {
    let support = classify_split_mode(mode);
    let paths = if mode == Mode::ImagePalette8 {
        SPLIT_PALETTE_COPY_PATH
    } else if mode.band_count() == 1 {
        SPLIT_IDENTITY_COPY_PATH
    } else {
        SPLIT_EXTRACT_BANDS_PATH
    };
    OperationModeRoute::WrapperOwned { mode, support, paths }
}

/// Exhaustive Split support; each mode exposes its exact copy or child route.
pub(crate) static SPLIT_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::Split,
    NO_INPUTS,
    classify_split_route,
    &[OperationId::ExtractBand],
);

fn classify_get_extrema_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
        // NO_PILLOW_SUPPORT: native-width extrema extend Pillow's byte-only
        // multiband result while preserving u16/f32 samples.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetExtrema,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "native-width multiband extrema extend Pillow's mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16 => ModeSupport::cpu(
            OperationId::GetExtrema,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
    }
}

/// Exhaustive native-mode extrema support and architecture fallback contract.
pub(crate) static GET_EXTREMA_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetExtrema, NO_INPUTS, classify_get_extrema_mode);

fn get_data_band_inputs(mode: Mode) -> &'static [InputContract] {
    match mode.band_count() {
        1 => GET_DATA_BAND_1_INPUTS,
        2 => GET_DATA_BAND_2_INPUTS,
        3 => GET_DATA_BAND_3_INPUTS,
        4 => GET_DATA_BAND_4_INPUTS,
        _ => NO_INPUTS,
    }
}

fn get_channel_inputs(mode: Mode) -> &'static [InputContract] {
    match mode.band_count() {
        1 => GET_CHANNEL_1_INPUTS,
        2 => GET_CHANNEL_2_INPUTS,
        3 => GET_CHANNEL_3_INPUTS,
        4 => GET_CHANNEL_4_INPUTS,
        _ => NO_INPUTS,
    }
}

fn classify_get_channel_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageI32 | Mode::ImageF32 => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
        // NO_PILLOW_SUPPORT: native high-depth band extraction keeps sample width.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16 => ModeSupport::cpu_core_only(
            OperationId::GetChannel,
            mode,
            OutputModeRule::Fixed(Mode::ImageI16),
            get_channel_inputs(mode),
            &[],
            "native u16 GetChannel extends Pillow while preserving sample width",
        ),
        Mode::ImageRgb32F | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetChannel,
            mode,
            OutputModeRule::Fixed(Mode::ImageF32),
            get_channel_inputs(mode),
            &[],
            "native f32 GetChannel extends Pillow while preserving sample width",
        ),
        // Wrapper owns `1`/P identity (P keeps its palette) and the I;16* extension below.
        Mode::One | Mode::ImagePalette8 => ModeSupport::cpu(
            OperationId::GetChannel,
            mode,
            OutputModeRule::Preserve,
            get_channel_inputs(mode),
            &[],
        ),
        // NO_PILLOW_SUPPORT: Pillow rejects IMAGING_TYPE_I16; Rust preserves native I;16 identity.
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N =>
            ModeSupport::cpu_core_only(
                OperationId::GetChannel,
                mode,
                OutputModeRule::Preserve,
                get_channel_inputs(mode),
                &[],
                "native I;16 GetChannel identity extends Pillow's I16 mode rejection",
            ),
        Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8 => ModeSupport::cpu(
            OperationId::GetChannel, mode, OutputModeRule::Fixed(Mode::ImageLuma8),
            get_channel_inputs(mode), &[],
        ),
    }
}

const GET_CHANNEL_COPY_PATH: &[OperationRoutePath] =
    &[OperationRoutePath::IdentityCopy];
const GET_CHANNEL_PALETTE_COPY_PATH: &[OperationRoutePath] =
    &[OperationRoutePath::PalettePreservingCopy];
const GET_CHANNEL_SHARED_OR_CHILD_PATH: &[OperationRoutePath] = &[
    OperationRoutePath::SharedBufferIdentity {
        guards: &[
            OperationRouteGuard::LoadedImage,
            OperationRouteGuard::RequestedBackendIn(&[Backend::Cpu]),
            OperationRouteGuard::ByteStorageLengthMatchesPixelCount,
        ],
    },
    OperationRoutePath::PipelineChild(OperationId::ExtractBand),
];
const GET_CHANNEL_EAGER_OR_CHILD_PATH: &[OperationRoutePath] = &[
    OperationRoutePath::EagerCpuChild {
        operation: OperationId::ExtractBand,
        guards: &[
            OperationRouteGuard::LoadedImage,
            OperationRouteGuard::RequestedBackendIn(&[Backend::Cpu, Backend::Simd]),
            OperationRouteGuard::PixelCountAtMost(256),
        ],
    },
    OperationRoutePath::PipelineChild(OperationId::ExtractBand),
];
const GET_CHANNEL_CHILD_PATH: &[OperationRoutePath] =
    &[OperationRoutePath::PipelineChild(OperationId::ExtractBand)];

fn classify_get_channel_route(mode: Mode) -> OperationModeRoute {
    let support = classify_get_channel_mode(mode);
    if matches!(support, ModeSupport::Rejected { .. }) {
        return OperationModeRoute::Prepared {
            support,
            intermediate_mode: None,
            paths: &[],
        };
    }
    let paths = match mode {
        Mode::One | Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            GET_CHANNEL_COPY_PATH
        }
        Mode::ImagePalette8 => GET_CHANNEL_PALETTE_COPY_PATH,
        Mode::ImageLuma8 => GET_CHANNEL_SHARED_OR_CHILD_PATH,
        Mode::ImageRgb8 => GET_CHANNEL_EAGER_OR_CHILD_PATH,
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => GET_CHANNEL_CHILD_PATH,
        Mode::ImageI32 | Mode::ImageF32 => unreachable!("rejected GetChannel mode returned above"),
    };
    OperationModeRoute::WrapperOwned { mode, support, paths }
}

/// Exhaustive GetChannel contract, including identity, eager, and child routes.
pub(crate) static GET_CHANNEL_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::GetChannel,
    NO_INPUTS,
    classify_get_channel_route,
    &[OperationId::ExtractBand],
);

fn classify_get_data_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::GetData,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
        // NO_PILLOW_SUPPORT: GetData narrows native samples at this byte-read boundary.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetData,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "native high-depth GetData extends the Pillow mode contract",
        ),
    }
}

/// Exhaustive support declaration for the direct full-buffer GetData read.
pub(crate) static GET_DATA_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetData, NO_INPUTS, classify_get_data_mode);

fn classify_get_data_band_mode(mode: Mode) -> ModeSupport {
    let inputs = get_data_band_inputs(mode);
    match mode {
        // NO_PILLOW_SUPPORT: these internal identities expose native samples through GetData.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetDataBand,
            mode,
            NO_RASTER_OUTPUT,
            inputs,
            &[],
            "native high-depth GetDataBand extends the Pillow mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::GetDataBand,
            mode,
            NO_RASTER_OUTPUT,
            inputs,
            &[],
        ),
    }
}

/// Allocated band reads compose caller-buffer and ExtractBand strategies where eligible.
pub(crate) static GET_DATA_BAND_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::GetDataBand,
    NO_INPUTS,
    classify_get_data_band_mode,
    &[OperationId::GetDataBandInto, OperationId::ExtractBand],
);

fn classify_get_data_band_into_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageI32
        | Mode::ImageF32 => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
        // NO_PILLOW_SUPPORT: internal high-depth bands are native Rust extensions.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetDataBandInto,
            mode,
            NO_RASTER_OUTPUT,
            get_data_band_inputs(mode),
            &[],
            "native high-depth GetDataBandInto extends the Pillow mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8 => ModeSupport::cpu(
            OperationId::GetDataBandInto,
            mode,
            NO_RASTER_OUTPUT,
            get_data_band_inputs(mode),
            &[],
        ),
    }
}

/// Caller-owned band reads declare the existing scalar-mode rejection contract.
pub(crate) static GET_DATA_BAND_INTO_SUPPORT: OperationSupport =
    OperationSupport::composed(
        OperationId::GetDataBandInto,
        NO_INPUTS,
        classify_get_data_band_into_mode,
        &[OperationId::ExtractBand],
    );

/// Selects the exact-mode, serial formatted pixel reader.
pub(crate) fn select_get_pixel(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_PIXEL_SUPPORT.for_mode(mode).select_serial(context)
}

/// Selects the exact-mode, serial RGBA8 convenience reader.
pub(crate) fn select_get_pixel_rgba(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_PIXEL_RGBA_SUPPORT.for_mode(mode).select_serial(context)
}

fn validate_pixel_selection(
    selection: StrategySelection,
    operation: OperationId,
    mode: Mode,
) -> Result<(), PilError> {
    if selection.implementation.operation != operation
        || selection.implementation.mode != mode
        || selection.implementation.backend != Backend::Cpu
        || selection.implementation.architecture != crate::compute::Architecture::Default
    {
        return Err(PilError::InternalError(
            "GetPixel selection did not match its exact mode and Default implementation".into(),
        ));
    }
    Ok(())
}

/// Executes Pillow-shaped GetPixel after its declared exact-mode selection.
pub(crate) fn execute_get_pixel(
    image: &DynamicImage,
    x: u32,
    y: u32,
    selection: StrategySelection,
) -> Result<crate::image::FormattedPixelValue, PilError> {
    validate_pixel_selection(selection, OperationId::GetPixel, image.mode())?;
    super::helpers::pixel_access::formatted_pixel(image, x, y)
}

/// Executes the allocation-free RGB binding path under the GetPixel declaration.
pub(crate) fn execute_get_pixel_rgb(
    image: &DynamicImage,
    x: u32,
    y: u32,
    selection: StrategySelection,
) -> Result<Option<[u8; 3]>, PilError> {
    validate_pixel_selection(selection, OperationId::GetPixel, image.mode())?;
    Ok(super::helpers::pixel_access::rgb_pixel(image, x, y))
}

/// Executes the Rust/JavaScript fixed-RGBA8 pixel accessor.
pub(crate) fn execute_get_pixel_rgba(
    image: &DynamicImage,
    x: u32,
    y: u32,
    selection: StrategySelection,
) -> Result<[u8; 4], PilError> {
    validate_pixel_selection(selection, OperationId::GetPixelRgba, image.mode())?;
    Ok(super::helpers::pixel_access::rgba_pixel(image, x, y))
}

/// Selects the direct full-buffer read for the mode carried by the raster.
pub(crate) fn select_get_data(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_SUPPORT.for_mode(mode).select_serial(context)
}

/// Selects the allocated band-read strategy for the exact raster mode.
pub(crate) fn select_get_data_band(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_BAND_SUPPORT
        .for_mode(mode)
        .select_serial(context)
}

/// Selects the caller-owned band-read CPU strategy for the exact raster mode.
pub(crate) fn select_get_data_band_into(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_BAND_INTO_SUPPORT
        .for_mode(mode)
        .select_serial(context)
}

#[path = "helpers/get_data_execution.rs"]
mod get_data_execution;
pub(crate) use get_data_execution::{
    get_data, get_data_band, get_data_band_byte_view, get_data_band_into,
    get_data_band_into_count, get_data_integer_samples, get_data_palette_indices_band_into,
    get_data_palette_indices_view, get_data_scalar_samples, supports_get_data_band_into,
    supports_get_data_band_terminal,
};
