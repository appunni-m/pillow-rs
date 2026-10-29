//! SIMD PutPixel family adapters and native byte kernels.

mod kernels;

use super::{SimdImageShape, SimdLayout};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

/// Admit only ordered, in-bounds writes over an exact native RGBA byte plane.
pub(crate) fn simd_rgba_putpixel_batch_supported(
    img: &DynamicImage,
    ops: &[PipelineOp],
    mode: Option<&str>,
) -> bool {
    let DynamicImage::ImageRgba8(rgba) = img else {
        return false;
    };
    if !matches!(mode, None | Some("RGBA")) || ops.len() < 2 {
        return false;
    }
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    if expected_bytes < 16 || rgba.as_raw().len() != expected_bytes {
        return false;
    }
    ops.iter().all(|op| {
        matches!(
            op,
            PipelineOp::PutPixel {
                x,
                y,
                palette_index: false,
                ..
            } if *x < img.width() && *y < img.height()
        )
    })
}

/// Apply a validated adjacent write run with one native RGBA copy.
///
/// Each write retains the existing masked 16-byte SIMD store and source order,
/// including last-write-wins behavior for repeated coordinates.
pub(crate) fn simd_rgba_putpixel_batch(
    img: &DynamicImage,
    ops: &[PipelineOp],
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !simd_rgba_putpixel_batch_supported(img, ops, mode) {
        return Ok(None);
    }
    let DynamicImage::ImageRgba8(rgba) = img else {
        return Ok(None);
    };
    let row_bytes = (img.width() as usize)
        .checked_mul(4)
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel row stride overflow".into()))?;
    let mut output = rgba.as_raw().to_vec();
    if !kernels::apply_rgba_batch(&mut output, row_bytes, ops)? {
        return Ok(None);
    }
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
    for _ in ops {
        crate::compute::begin_pipeline_operation_telemetry("PutPixel");
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(1);
        crate::compute::finish_pipeline_operation_telemetry();
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::{simd_rgba_putpixel_batch, simd_rgba_putpixel_batch_supported};
    use crate::compute::pool_simd::ops::adapters::simd_put_pixel;
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, ImageBuffer, Rgba};

    fn put_pixel(x: u32, y: u32, color: [u8; 4]) -> PipelineOp {
        PipelineOp::PutPixel {
            x,
            y,
            color: (color[0], color[1], color[2], color[3]),
            palette_index: false,
        }
    }

    #[test]
    fn rgba_putpixel_batch_matches_ordered_single_writes_at_vector_edges() {
        for (width, height) in [(4, 1), (1, 4), (17, 3), (31, 5)] {
            let source = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, y| {
                Rgba([(x * 7) as u8, (y * 13) as u8, (x + y) as u8, 255])
            }));
            let ops = vec![
                put_pixel(0, 0, [1, 2, 3, 4]),
                put_pixel(width - 1, height - 1, [5, 6, 7, 8]),
                put_pixel(width / 2, height / 2, [9, 10, 11, 12]),
                put_pixel(0, 0, [13, 14, 15, 16]),
            ];
            assert!(simd_rgba_putpixel_batch_supported(
                &source,
                &ops,
                Some("RGBA")
            ));
            let batched = simd_rgba_putpixel_batch(&source, &ops, Some("RGBA"))
                .unwrap()
                .unwrap();
            let mut sequential = source;
            for op in &ops {
                sequential = simd_put_pixel(&sequential, op, Some("RGBA")).unwrap();
            }
            assert_eq!(batched.as_bytes(), sequential.as_bytes());
        }
    }

    #[test]
    fn rgba_putpixel_batch_rejects_unmatched_or_unsafe_inputs() {
        let source = DynamicImage::ImageRgba8(ImageBuffer::new(4, 1));
        let two_writes = vec![put_pixel(0, 0, [1, 2, 3, 4]), put_pixel(3, 0, [5, 6, 7, 8])];
        assert!(!simd_rgba_putpixel_batch_supported(
            &source,
            &two_writes,
            Some("RGB")
        ));
        assert!(!simd_rgba_putpixel_batch_supported(
            &source,
            &two_writes[..1],
            Some("RGBA")
        ));
        let out_of_bounds = vec![put_pixel(0, 0, [1, 2, 3, 4]), put_pixel(4, 0, [5, 6, 7, 8])];
        assert!(!simd_rgba_putpixel_batch_supported(
            &source,
            &out_of_bounds,
            Some("RGBA")
        ));
        let tiny = DynamicImage::ImageRgba8(ImageBuffer::new(3, 1));
        assert!(!simd_rgba_putpixel_batch_supported(
            &tiny,
            &two_writes,
            Some("RGBA")
        ));
    }

    #[test]
    fn simd_pool_batches_adjacent_rgba_putpixels_and_reports_each_vector_store() {
        use crate::compute::{Backend, BackendImpl};

        struct RestoreTelemetry(bool);
        impl Drop for RestoreTelemetry {
            fn drop(&mut self) {
                Backend::set_pipeline_telemetry_enabled(self.0);
            }
        }

        let source = DynamicImage::ImageRgba8(ImageBuffer::from_fn(16, 16, |x, y| {
            Rgba([x as u8, y as u8, (x + y) as u8, 255])
        }));
        let ops = vec![
            put_pixel(0, 0, [1, 2, 3, 4]),
            put_pixel(15, 0, [5, 6, 7, 8]),
            put_pixel(0, 15, [9, 10, 11, 12]),
            put_pixel(15, 15, [13, 14, 15, 16]),
        ];
        let _restore = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        crate::compute::reset_pipeline_operation_telemetry();
        let result = crate::compute::pool_simd::SimdPool
            .execute_batch(&ops, &source, Some("RGBA"))
            .unwrap();
        let receipts = crate::compute::take_pipeline_operation_telemetry();
        assert_eq!(receipts.len(), ops.len());
        for receipt in receipts {
            assert_eq!(receipt.operation, "PutPixel");
            assert_eq!(receipt.path, "vector");
            assert_eq!(receipt.vector_block_count, 1);
            assert_eq!(receipt.scalar_tail_count, 0);
        }
        let resources = crate::compute::take_pipeline_resource_telemetry().unwrap();
        assert_eq!(resources.fused_operation_count, ops.len() as u64);
        assert_eq!(resources.host_buffer_count, 2);
        assert_eq!(
            resources.host_buffer_bytes,
            (source.as_bytes().len() * 2) as u64
        );

        let mut sequential = source;
        for op in &ops {
            sequential = simd_put_pixel(&sequential, op, Some("RGBA")).unwrap();
        }
        assert_eq!(result.as_bytes(), sequential.as_bytes());
    }
}

/// `Image.putpixel` stores the already-normalized LAB sample triplet directly
/// into the shared RGB8 raster. Keep this support specific to PutPixel so the
/// broader drawing adapters do not accidentally inherit color-space semantics.
pub(super) fn native_putpixel_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    if matches!((img, mode), (DynamicImage::ImageRgb8(_), Some("LAB"))) {
        Some(3)
    } else {
        super::native_draw_layout(img, mode)
    }
}

pub(super) fn native_putpixel_supported_for_image(
    img: &DynamicImage,
    x: u32,
    y: u32,
    mode: Option<&str>,
) -> bool {
    native_putpixel_layout(img, mode)
        .is_some_and(|channels| x < img.width() && y < img.height() && channels != 0)
}

fn shape_putpixel_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    if matches!((shape.layout, mode), (SimdLayout::Rgb8, Some("LAB"))) {
        Some(3)
    } else {
        super::shape_draw_channels(shape, mode)
    }
}

pub(super) fn native_putpixel_supported_for_shape(
    shape: SimdImageShape,
    x: u32,
    y: u32,
    mode: Option<&str>,
) -> bool {
    shape_putpixel_channels(shape, mode)
        .is_some_and(|channels| x < shape.width && y < shape.height && channels != 0)
}

/// Set one validated pixel through a masked native-byte vector store.  The
/// surrounding pixels are preserved, so this keeps `P`, `CMYK`, and the
/// ordinary byte layouts in their logical storage without packed RGBA
/// conversion.  Palette resolution and coordinate validation happen before
/// the pipeline operation is queued.
fn simd_put_pixel_native(
    img: &DynamicImage,
    x: u32,
    y: u32,
    color: (u8, u8, u8, u8),
    channels: usize,
) -> Result<Option<DynamicImage>, PilError> {
    if x >= img.width() || y >= img.height() {
        return Ok(None);
    }
    let row_bytes = (img.width() as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel row stride overflow".into()))?;
    let target_start = (y as usize)
        .checked_mul(row_bytes)
        .and_then(|row| row.checked_add((x as usize).checked_mul(channels)?))
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel offset overflow".into()))?;
    let target_end = target_start
        .checked_add(channels)
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel end overflow".into()))?;
    let mut output = img.as_bytes().to_vec();
    if target_end > output.len() {
        return Ok(None);
    }
    // A single pixel can sit in a row narrower than one vector.  Use a
    // masked block over the flat native buffer instead of rejecting the
    // operation: bytes from the adjacent row are loaded and stored
    // unchanged, while only this pixel's channel lanes are enabled.  The
    // channel pattern is based on the global byte offset, so crossing a row
    // boundary does not change the logical sample mapping.
    if output.len() < 16 {
        let mut block = [0u8; 16];
        block[..output.len()].copy_from_slice(&output);
        super::draw_line_vector_block(
            &mut block,
            0,
            target_start,
            target_end,
            channels,
            color,
            false,
        );
        let output_len = output.len();
        output.copy_from_slice(&block[..output_len]);
    } else {
        let block_start = target_start.min(output.len() - 16);
        super::draw_line_vector_block(
            &mut output,
            block_start,
            target_start,
            target_end,
            channels,
            color,
            false,
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(1);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels).map(Some)
}

pub(crate) fn simd_put_pixel(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutPixel { x, y, color, .. } = op else {
        return Err(PilError::ValueError("expected PutPixel op".into()));
    };
    let Some(channels) = native_putpixel_layout(img, mode) else {
        return Err(super::simd_unsupported("PutPixel"));
    };
    simd_put_pixel_native(img, *x, *y, *color, channels)?
        .ok_or_else(|| super::simd_unsupported("PutPixel"))
}
