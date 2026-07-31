//! Pillow `ImageChops`-style channel operations.
//!
//! Functions return lazy pipeline images that combine one or two inputs. Shape
//! and mode compatibility are checked when the pipeline materializes.

use std::sync::Arc;

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Adds two images with scale and offset.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn add(image1: &Image, image2: &Image, scale: f64, offset: f64) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Add {
            other: Arc::new(image2.clone()),
            scale,
            offset,
        },
    ))
}

/// Subtracts `image2` from `image1` with scale and offset.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn subtract(
    image1: &Image,
    image2: &Image,
    scale: f64,
    offset: f64,
) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Subtract {
            other: Arc::new(image2.clone()),
            scale,
            offset,
        },
    ))
}

/// Multiplies two images channel-by-channel.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn multiply(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Multiply {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Applies screen blend mode.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn screen(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Screen {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Keeps the darker pixel at each position.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn darker(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Darker {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Keeps the lighter pixel at each position.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn lighter(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Lighter {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Computes absolute channel difference between two images.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn difference(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Difference {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Applies overlay blend mode.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn overlay(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Overlay {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Applies soft-light blend mode.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn soft_light(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::SoftLight {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Applies hard-light blend mode.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn hard_light(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::HardLight {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Applies bitwise AND.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn logical_and(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::LogicalAnd {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Applies bitwise OR.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn logical_or(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::LogicalOr {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Applies bitwise XOR.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn logical_xor(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::LogicalXor {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Duplicate an image.
#[allow(dead_code)]
pub(crate) fn duplicate(image: &Image) -> Image {
    image.copy()
}

/// Inverts an image through the ImageChops compatibility surface.
///
/// # Errors
///
/// Returns [`PilError`] while determining the image mode or materializing
/// the pipeline result.
pub fn invert(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;

    let mut result = Image::push_op(image, PipelineOp::InvertChops);
    if mode == "P"
        && let Image::Pipeline {
            palette,
            palette_alpha,
            ..
        } = &mut result
    {
        // Pillow's ImagingChopInvert allocates a fresh P core without copying
        // Image.palette. The bytes are inverted indices and getpalette() is [].
        *palette = Some(Vec::new());
        *palette_alpha = None;
    }
    Ok(result)
}

/// Offsets image contents by wrapping pixels around both axes.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn offset(image: &Image, xoffset: i32, yoffset: i32) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image,
        PipelineOp::Offset {
            x: xoffset,
            y: yoffset,
        },
    ))
}

/// Adds two images modulo 256.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn add_modulo(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::AddModulo {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Subtracts two images modulo 256.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn subtract_modulo(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::SubtractModulo {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Fills active channels with a constant byte value.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn constant(image: &Image, value: u8) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Constant { value }))
}
