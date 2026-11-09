//! Direct La-to-LA unpremultiplication fused with alpha replacement.

use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_lanes_for_bytes;
use crate::compute::pool_cpu::helpers::bytes::unpremultiply::unpremultiply_byte;
use crate::compute::pool_cpu::ops::put::helpers::alpha_mask::{MaskStrategy, normalize_sample};
use crate::error::PilError;
use crate::raster::{GrayAlphaImage, Mode};

pub(crate) fn replace_alpha(
    source: &GrayAlphaImage,
    alpha_samples: impl ExactSizeIterator<Item = u8>,
) -> Result<GrayAlphaImage, PilError> {
    replace_alpha_samples(source, alpha_samples)
}

pub(crate) fn replace_alpha_mask(
    source: &GrayAlphaImage,
    mask: MaskStrategy<'_>,
) -> Result<GrayAlphaImage, PilError> {
    mask.with_samples(
        |samples| replace_alpha_mask_samples::<true>(source, samples),
        |samples| replace_alpha_mask_samples::<false>(source, samples),
    )
}

fn replace_alpha_mask_samples<const ONE: bool>(
    source: &GrayAlphaImage,
    alpha_samples: &[u8],
) -> Result<GrayAlphaImage, PilError> {
    replace_alpha_samples(
        source,
        alpha_samples.iter().copied().map(normalize_sample::<ONE>),
    )
}

fn replace_alpha_samples(
    source: &GrayAlphaImage,
    mut alpha_samples: impl ExactSizeIterator<Item = u8>,
) -> Result<GrayAlphaImage, PilError> {
    let pixel_count = source.as_raw().len() / 2;
    if alpha_samples.len() != pixel_count {
        return Err(PilError::InternalError(
            "PutAlpha La samples did not match the source extent".into(),
        ));
    }

    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for ((destination, pixel), alpha) in output
        .as_mut()
        .chunks_exact_mut(2)
        .zip(source.as_raw().chunks_exact(2))
        .zip(&mut alpha_samples)
    {
        destination[0] = unpremultiply_byte(pixel[0], pixel[1]);
        destination[1] = alpha;
    }

    let lanes = default_hidden_lanes_for_bytes(Mode::ImageLumaA8, output.as_raw(), pixel_count)?;
    if !output.replace_hidden_pixel_lanes(lanes) {
        return Err(PilError::InternalError(
            "PutAlpha La result lanes did not match its extent".into(),
        ));
    }
    Ok(output)
}
