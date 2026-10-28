//! CPU crop orchestration with explicit Pillow-mode storage routing.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::DynamicImage;

/// Execute a Crop operation.
///
/// `Image::crop` owns Pillow's signed/out-of-bounds normalization before it
/// queues `PipelineOp::Crop`. The compute operation therefore receives only a
/// positive, in-bounds box; keeping padding logic here would duplicate that
/// public contract and leave an unreachable second crop implementation.
pub(crate) fn execute_crop(
    img: &DynamicImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    debug_assert!(
        left < iw && top < ih && right <= iw && bottom <= ih,
        "crop pipeline coordinates must be normalized before execution"
    );
    let width = right.checked_sub(left).ok_or_else(|| {
        PilError::InternalError("crop pipeline width underflow after normalization".into())
    })?;
    let height = bottom.checked_sub(top).ok_or_else(|| {
        PilError::InternalError("crop pipeline height underflow after normalization".into())
    })?;

    // Pillow modes determine how many bytes form one stored sample group.
    // The wrapper mode stays separate from DynamicImage: for example, I and F
    // both use four-byte transport storage while retaining scalar semantics.
    // Untagged images keep the native storage route; typed 16-bit samples use
    // DynamicImage's crop implementation below.
    if let Some(channels) = native_crop_channels(img, mode) {
        let source = img.as_bytes();
        let source_stride = iw as usize * channels;
        let output_stride = width as usize * channels;
        let output_dims = CheckedDims::new(width, height, channels as u8)?;
        #[cfg(feature = "parallel")]
        if output_dims.total_bytes() >= 4 * 1024 * 1024 && output_stride != 0 {
            let mut output = output_dims.alloc_buffer();
            crate::par_rows_mut!(
                &mut output,
                output_stride,
                height as usize,
                |_row_start, _row_end, y, row| {
                    let source_start =
                        (top as usize + y as usize) * source_stride + left as usize * channels;
                    row.copy_from_slice(&source[source_start..source_start + output_stride]);
                }
            );
            return raw_bytes_to_image(width, height, output, channels);
        }

        // Every output row comes from one source-row span, so
        // avoid zero-filling a destination immediately before copying it.
        let mut output = Vec::with_capacity(output_dims.total_bytes());
        crate::compute::record_pipeline_allocation(output_dims.total_bytes());
        for y in 0..height as usize {
            let source_start = (top as usize + y) * source_stride + left as usize * channels;
            output.extend_from_slice(&source[source_start..source_start + output_stride]);
        }
        return raw_bytes_to_image(width, height, output, channels);
    }
    Ok(img.crop_imm(left, top, width, height))
}

fn native_crop_channels(image: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match (mode, image) {
        (Some("1" | "L" | "P"), DynamicImage::ImageLuma8(_)) => Some(1),
        (Some("LA" | "La" | "PA"), DynamicImage::ImageLumaA8(_)) => Some(2),
        (Some("RGB" | "HSV" | "LAB" | "YCbCr"), DynamicImage::ImageRgb8(_)) => Some(3),
        (Some("RGBA" | "RGBa" | "RGBX" | "CMYK" | "I" | "F"), DynamicImage::ImageRgba8(_)) => {
            Some(4)
        }
        (None, image) => match image.color() {
            crate::raster::ColorType::L8 => Some(1),
            crate::raster::ColorType::La8 => Some(2),
            crate::raster::ColorType::Rgb8 => Some(3),
            crate::raster::ColorType::Rgba8 => Some(4),
            _ => None,
        },
        _ => None,
    }
}
