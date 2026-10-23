//! CPU crop operation.

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

    // Native byte layouts can copy complete rows directly.  Keep the image
    // crate path for typed samples such as I;16, whose byte stride is wider
    // than its logical channel count and therefore needs its own layout
    // handling.
    let channels = img.color().channel_count() as usize;
    if matches!(
        img.color(),
        crate::raster::ColorType::L8
            | crate::raster::ColorType::La8
            | crate::raster::ColorType::Rgb8
            | crate::raster::ColorType::Rgba8
    ) {
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
