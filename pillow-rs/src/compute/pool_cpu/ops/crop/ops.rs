//! Crop entry point, exhaustive mode support, and prepared strategy composition.
//!
//! TODO(pillow-rs#3ptb): expand parity across every exact mode and available
//! architecture, including the archived SIMD admissions, unsupported-mode
//! fallbacks, I;16 vector-copy tails, and CropBorder empty/error cases. Source
//! review found the archived ordinary-byte SIMD route only copied row spans;
//! it admitted 1/L/P, LA/PA, RGB/HSV/YCbCr, and RGBA/CMYK/RGBa/RGBX/I/F when
//! storage matched. The active Default `ByteCrop` preserves the same native
//! byte layout for all exact byte modes. The archived direct I;16 route copied
//! opaque two-byte samples with vector blocks and a scalar tail; those four
//! exact identities now select that kernel by architecture. Archived
//! CropBorder admitted only the ordinary byte layouts, accepted empty outputs,
//! and preserved width-before-height errors. The maintained owner preserves
//! that public validation order and uses a typed Default for empty outputs;
//! its nonempty I;16 vector route is a native extension.

// NO_PILLOW_SUPPORT: The five internal high-depth raster identities below use
// exact typed sample-copy strategies even though Pillow has no mode spellings
// for those carriers.

use crate::compute::{ExecutionContext, registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

use super::helpers::crop::CropRegion;

/// Coordinates already normalized by the public image API.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CropRequest {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) right: u32,
    pub(crate) bottom: u32,
}

impl CropRequest {
    fn region(self, size: (u32, u32)) -> Result<CropRegion, PilError> {
        CropRegion::new(size.0, size.1, self.left, self.top, self.right, self.bottom)
    }
}

macro_rules! declare_crop_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedCrop<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::SUPPORT,)+
            }
        }

        fn assess_mode(
            mode: Mode,
            region: CropRegion,
            ctx: &ExecutionContext,
        ) -> Result<SupportAssessment, PilError> {
            match mode {
                $(Mode::$mode => super::$module::ops::assess(region, ctx),)+
            }
        }

        fn prepare_mode<'a>(
            image: &'a DynamicImage,
            region: CropRegion,
            ctx: &ExecutionContext,
        ) -> Result<PreparedCrop<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare(input, region, ctx)
                    .map(PreparedCrop::$mode),)+
            }
        }

        impl PreparedCrop<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self {
                    $(Self::$mode(strategy) => strategy.selection(),)+
                }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self {
                    $(Self::$mode(strategy) => super::$module::ops::execute(strategy),)+
                }
            }
        }
    };
}

declare_crop_modes! {
    One => one,
    ImageLuma8 => luma8,
    ImageLumaA8 => luma_a8,
    ImageLaPremultiplied8 => la_premultiplied8,
    ImagePalette8 => palette8,
    ImagePaletteAlpha8 => palette_alpha8,
    ImageRgb8 => rgb8,
    ImageRgba8 => rgba8,
    ImageRgbPremultiplied8 => rgb_premultiplied8,
    ImageRgbx8 => rgbx8,
    ImageCmyk8 => cmyk8,
    ImageYCbCr8 => ycbcr8,
    ImageLab8 => lab8,
    ImageHsv8 => hsv8,
    ImageI32 => i32,
    ImageF32 => f32,
    ImageI16 => i16,
    ImageI16L => i16l,
    ImageI16B => i16b,
    ImageI16N => i16n,
    ImageLumaA16 => luma_a16,
    ImageRgb16 => rgb16,
    ImageRgba16 => rgba16,
    ImageRgb32F => rgb32f,
    ImageRgba32F => rgba32f,
}

/// The operation's complete support view, bound to the concrete mode strategies.
pub(crate) static SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Crop,
    &[InputContract::NormalizedCrop],
    classify_mode,
);

pub(crate) fn assess(
    mode: Mode,
    size: (u32, u32),
    request: CropRequest,
    ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    assess_mode(mode, request.region(size)?, ctx)
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    request: CropRequest,
    ctx: &ExecutionContext,
) -> Result<PreparedCrop<'a>, PilError> {
    prepare_mode(image, request.region((image.width(), image.height()))?, ctx)
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Crop { left, top, right, bottom } = op else {
        return Err(PilError::ValueError("expected Crop op".into()));
    };
    let request = CropRequest { left: *left, top: *top, right: *right, bottom: *bottom };
    prepare(input.image, request, ctx).map(crate::compute::registry::PreparedOperation::from_crop)
}
