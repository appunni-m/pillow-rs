//! Prepared dispatch for exact-mode GetBBox strategies.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    PaletteKnowledge, RequestFacts, StrategyInputs, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::bbox::BBox;

/// Prepared direct GetBBox execution, retaining the exact mode and selection.
pub(crate) struct PreparedGetBBox<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    alpha_only: bool,
    selection: StrategySelection,
}

impl PreparedGetBBox<'_> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<BBox, PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "GetBBox prepared mode did not match its DynamicImage tag".into(),
            ));
        }
        if self.image.width() == 0 || self.image.height() == 0 {
            return Ok(None);
        }
        let selection = self.selection;
        let alpha_only = self.alpha_only;
        match self.image {
            DynamicImage::One(image) => {
                super::super::one::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageLuma8(image) => {
                super::super::luma8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageLumaA8(image) => {
                super::super::luma_a8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageLaPremultiplied8(image) => {
                super::super::la_premultiplied8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImagePalette8(image) => {
                super::super::palette8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImagePaletteAlpha8(image) => {
                super::super::palette_alpha8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageRgb8(image) => {
                super::super::rgb8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageRgba8(image) => {
                super::super::rgba8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageRgbPremultiplied8(image) => {
                super::super::rgb_premultiplied8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageRgbx8(image) => {
                super::super::rgbx8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageCmyk8(image) => {
                super::super::cmyk8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageYCbCr8(image) => {
                super::super::ycbcr8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageLab8(image) => {
                super::super::lab8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageHsv8(image) => {
                super::super::hsv8::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageI32(image) => {
                super::super::i32::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageF32(image) => {
                super::super::f32::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageI16(image) => {
                super::super::i16::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageI16L(image) => {
                super::super::i16l::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageI16B(image) => {
                super::super::i16b::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageI16N(image) => {
                super::super::i16n::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageLumaA16(image) => {
                super::super::luma_a16::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageRgb16(image) => {
                super::super::rgb16::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageRgba16(image) => {
                super::super::rgba16::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageRgb32F(image) => {
                super::super::rgb32f::ops::execute_getbbox(image, alpha_only, selection)
            }
            DynamicImage::ImageRgba32F(image) => {
                super::super::rgba32f::ops::execute_getbbox(image, alpha_only, selection)
            }
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Selects GetBBox from the injected CPU context before any scan is performed.
pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    alpha_only: bool,
    context: &ExecutionContext,
) -> Result<PreparedGetBBox<'a>, PilError> {
    let mode = image.mode();
    let selection = super::super::ops::GET_BBOX_SUPPORT.for_mode(mode).select(
        context,
        StrategyInputs {
            width: image.width(),
            height: image.height(),
            byte_len: Some(image.as_bytes().len()),
            palette: PaletteKnowledge::Unknown,
            request: RequestFacts::Unspecified,
        },
    )?;
    Ok(PreparedGetBBox {
        image,
        mode,
        alpha_only,
        selection,
    })
}
