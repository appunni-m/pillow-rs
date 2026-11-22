//! Default transparent conversion from native RGB samples.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::color::helpers::convert::transparent::TransparentOutput;
use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::raster::{Mode, RgbImage};

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn execute(
    source: &RgbImage,
    target: Mode,
    key: [u8; 3],
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), 3)?;
    if source.as_raw().len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "ConvertTransparent RGB source shape mismatch".into(),
        ));
    }
    let mut output = TransparentOutput::new(source.width(), source.height(), target)?;
    let hidden = source.hidden_pixel_lanes();
    for (index, pixel) in source.as_raw().chunks_exact(3).enumerate() {
        let rgb = [pixel[0], pixel[1], pixel[2]];
        let matches_key = match target {
            Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 => rgb == key,
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 => {
                let padding = hidden
                    .and_then(|lanes| lanes.for_pixel(index))
                    .map_or(u8::MAX, |lanes| lanes[0]);
                rgb == key && padding == u8::MAX
            }
            _ => {
                return Err(PilError::InternalError(
                    "ConvertTransparent RGB strategy received an unsupported target".into(),
                ));
            }
        };
        let luma = crate::color::rgb_to_luma_u8(rgb[0], rgb[1], rgb[2]);
        output.push_rgb(rgb, luma, matches_key, key);
    }
    output.finish()
}
