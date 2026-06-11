//! Image module-level functions — merge, blend, composite, eval, and effects.
//! These correspond to PIL.Image.merge(), PIL.Image.blend(), PIL.Image.composite().

use std::sync::Arc;

use crate::error::PilError;
use crate::image::Image;
use crate::ops::convert::parse_mode;
use crate::pipeline::PipelineOp;

/// Merge single-band images into a multi-band image.
/// PIL: `Image.merge(mode, bands)` where mode determines the band count.
pub fn merge(mode: &str, bands: &[Image]) -> Result<Image, PilError> {
    let n_expected = match mode {
        "RGB" => 3,
        "RGBA" => 4,
        "LA" => 2,
        "L" => 1,
        _ => {
            return Err(PilError::ValueError(format!(
                "Unsupported merge mode: {}",
                mode
            )))
        }
    };

    if bands.len() != n_expected {
        return Err(PilError::ValueError(format!(
            "Wrong number of bands for mode {}: expected {}, got {}",
            mode,
            n_expected,
            bands.len()
        )));
    }

    let mode_enum = parse_mode(mode)?;
    Ok(Image::push_op(
        &bands[0],
        PipelineOp::Merge {
            mode: mode_enum,
            bands: bands.to_vec(),
        },
    ))
}

/// Linear interpolation between two images.
/// PIL: `Image.blend(im1, im2, alpha)` -> (1-alpha)*im1 + alpha*im2
pub fn blend(image1: &Image, image2: &Image, alpha: f64) -> Result<Image, PilError> {
    let alpha = alpha.clamp(0.0, 1.0);
    Ok(Image::push_op(
        image1,
        PipelineOp::BlendModule {
            other: Arc::new(image2.clone()),
            alpha,
        },
    ))
}

/// Composite image1 over image2 using mask.
/// PIL: `Image.composite(image1, image2, mask)`
pub fn composite(image1: &Image, image2: &Image, mask: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::CompositeModule {
            other: Arc::new(image2.clone()),
            mask: Arc::new(mask.clone()),
        },
    ))
}

/// Apply a lookup table to each pixel.
/// PIL: `Image.eval(image, lut)`
pub fn eval(image: &Image, lut: &[u8]) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image,
        PipelineOp::Eval {
            lut: lut.to_vec(),
        },
    ))
}

/// Generate an image with Gaussian noise.
/// PIL: `Image.effect_noise(size, sigma)`
/// Uses the source image only for dimensions.
pub fn effect_noise(image: &Image, sigma: f64) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image,
        PipelineOp::EffectNoise { sigma },
    ))
}

/// Spread pixels outward (visual effect).
/// Uses the source image and applies distance-based spread.
pub fn effect_spread(image: &Image, distance: u32) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image,
        PipelineOp::EffectSpread { distance },
    ))
}
