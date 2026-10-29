//! Archived CMYK ink arithmetic, with no color-space guessing or scheduling.

use crate::compute::registry::support::{InputContract, PaletteKnowledge, StrategyInputs};
use crate::error::PilError;
use crate::raster::{GrayImage, Rgba, RgbaImage};
use super::super::super::helpers::luma::Checked;

pub(crate) type Input = RgbaImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgba<u8>>;

const SOURCE: InputContract = InputContract::ExactNativeBytes {
    channels: 4,
    overflow_reason: "CMYK grayscale size overflow",
    mismatch_reason: "cmyk_to_grayscale source buffer mismatch",
};

pub(crate) const INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
    SOURCE,
];

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    let prepared = Checked::new(input)?;
    SOURCE.validate_materialized(StrategyInputs {
        width: input.width(), height: input.height(), byte_len: Some(input.as_raw().len()),
        palette: PaletteKnowledge::Unknown, request: Default::default(),
    })?;
    Ok(prepared)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let source = input.input;
    let (w, h) = source.dimensions();
    let cmyk = source.as_raw();
    // The validated source contains exactly one four-byte sample per output
    // pixel. Collecting writes the final grayscale bytes once and avoids
    // zero-initializing a buffer that is immediately overwritten.
    let gray: Vec<u8> = cmyk
        .chunks_exact(4)
        .map(|p| {
            let c = p[0];
            let m = p[1];
            let y_ = p[2];
            let nk = u8::MAX - p[3];
            // For byte inks, rounded `muldiv255(ink, nk)` is bounded by `nk`, so
            // reconstructed RGB channels remain in [0, 255]. Pillow's luma
            // weights sum to 65,536; substituting the channel conversions fuses
            // three rounded CMYK components into one exact luma subtraction.
            let weighted_ink = 19_595 * u32::from(crate::color::grayscale_cmyk_muldiv255_byte(c, nk))
                + 38_470 * u32::from(crate::color::grayscale_cmyk_muldiv255_byte(m, nk))
                + 7_471 * u32::from(crate::color::grayscale_cmyk_muldiv255_byte(y_, nk));
            let luma_ink = (weighted_ink + 32_767) >> 16;
            (u32::from(nk) - luma_ink) as u8
        })
        .collect();
    if !gray.is_empty() {
        crate::compute::record_pipeline_allocation(gray.len());
    }
    crate::raster::GrayImage::from_raw(w, h, gray)
        .ok_or_else(|| PilError::InternalError("cmyk_to_grayscale buffer mismatch".to_string()))
}

