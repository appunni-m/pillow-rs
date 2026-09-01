//! Pillow `ImageChops`-style channel operations.
//!
//! Functions return lazy pipeline images that combine one or two inputs. Shape
//! and mode compatibility are checked when the pipeline materializes.

use std::sync::Arc;

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

fn binary_mode_class(mode: &str) -> Option<u8> {
    match mode {
        "1" | "L" | "P" => Some(1),
        "LA" | "PA" => Some(2),
        "RGB" | "YCbCr" | "HSV" => Some(3),
        "RGBA" | "CMYK" | "RGBa" | "RGBX" => Some(4),
        _ => None,
    }
}

/// Validate the native byte-layout compatibility enforced by
/// `ImagingChops` before it allocates a result image. Modes in the same byte
/// width family are intentionally compatible (for example `P` with `L` and
/// `YCbCr` with `RGB`); scalar `I`/`F` and 16-bit modes are rejected by the
/// underlying byte-oriented Chops entry points.
fn validate_binary_operands(image1: &Image, image2: &Image) -> Result<(), PilError> {
    let mode1 = image1.mode()?;
    let mode2 = image2.mode()?;
    let Some(class1) = binary_mode_class(&mode1) else {
        return Err(PilError::ValueError("image has wrong mode".into()));
    };
    let Some(class2) = binary_mode_class(&mode2) else {
        return Err(PilError::ValueError("images do not match".into()));
    };
    if image1.size()? != image2.size()? || class1 != class2 {
        return Err(PilError::ValueError("images do not match".into()));
    }
    Ok(())
}

fn validate_logical_operands(image1: &Image, image2: &Image) -> Result<(), PilError> {
    if image1.mode()? != "1" || image2.mode()? != "1" {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    if image1.size()? != image2.size()? {
        return Err(PilError::ValueError("images do not match".into()));
    }
    Ok(())
}

/// Adds two images with scale and offset.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn add(image1: &Image, image2: &Image, scale: f64, offset: f64) -> Result<Image, PilError> {
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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
    validate_logical_operands(image1, image2)?;
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
    validate_logical_operands(image1, image2)?;
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
    validate_logical_operands(image1, image2)?;
    Ok(Image::push_op(
        image1,
        PipelineOp::LogicalXor {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Duplicate an image through the compute pipeline.
///
/// Keeping this as a real operation preserves the public ImageChops entry
/// point for backend selection, including the SIMD duplicate path, while
/// retaining Pillow's mode and palette metadata through `Image::push_op`.
pub fn duplicate(image: &Image) -> Image {
    Image::push_op(image, PipelineOp::Duplicate)
}

/// Inverts an image through the ImageChops compatibility surface.
///
/// # Errors
///
/// Returns [`PilError`] while determining the image mode or materializing
/// the pipeline result.
pub fn invert(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::InvertChops))
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

/// Offsets image contents, using `xoffset` for the omitted y offset.
pub fn offset_with_default(
    image: &Image,
    xoffset: i32,
    yoffset: Option<i32>,
) -> Result<Image, PilError> {
    offset(image, xoffset, yoffset.unwrap_or(xoffset))
}

/// Adds two images modulo 256.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; size or mode mismatches are reported during
/// materialization.
pub fn add_modulo(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    validate_binary_operands(image1, image2)?;
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
    validate_binary_operands(image1, image2)?;
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

#[cfg(test)]
mod tests {
    use super::constant;
    use crate::image::Image;

    #[test]
    fn constant_returns_l_mode_for_explicit_source_modes() {
        let cases: &[(&str, &[u8])] = &[
            ("1", &[0x80]),
            ("CMYK", &[7, 8, 9, 10]),
            ("YCbCr", &[7, 8, 9]),
            ("HSV", &[7, 8, 9]),
            ("I", &[7, 0, 0, 0]),
            ("F", &[0, 0, 224, 64]),
        ];

        for &(mode, bytes) in cases {
            let source = Image::frombytes(mode, (1, 1), bytes).expect("source image");
            let result = constant(&source, 127).expect("constant image");
            assert_eq!(
                result.mode().expect("result mode"),
                "L",
                "source mode {mode}"
            );
            assert_eq!(
                result.tobytes().expect("result bytes"),
                [127],
                "source mode {mode}"
            );
        }
    }
}
