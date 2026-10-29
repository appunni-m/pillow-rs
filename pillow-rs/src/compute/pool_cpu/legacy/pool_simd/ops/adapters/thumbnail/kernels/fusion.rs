//! RGB PutPixel-to-Thumbnail pipeline fusion.

use super::{
    native_reduce_sparse_rgb_2x2, native_thumbnail_dimensions, native_thumbnail_filter,
    native_thumbnail_reduction_factors, native_thumbnail_supported_for_image,
    simd_resize_convolution_with_zero_row_elision, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, RgbImage};

/// Admit an RGB `PutPixel*` prefix that the SIMD executor can fold into the
/// sparse reducing-gap stage of a following thumbnail.
pub(crate) fn simd_rgb_putpixel_thumbnail_fusion_supported(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let DynamicImage::ImageRgb8(_) = img else {
        return false;
    };
    if !matches!(mode, None | Some("RGB"))
        || putpixel_ops.is_empty()
        || !putpixel_ops.iter().all(|op| {
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
    {
        return false;
    }
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return false;
    };
    let (source_width, source_height) = (img.width(), img.height());
    if source_width < 2
        || source_height < 2
        || source_width % 2 != 0
        || source_height % 2 != 0
        || (source_width as usize).saturating_mul(source_height as usize) < 512 * 512
        || matches!(filter, ResampleFilter::Nearest)
    {
        return false;
    }
    let Some((output_width, output_height)) =
        native_thumbnail_dimensions(source_width, source_height, *w, *h)
    else {
        return false;
    };
    let effective_filter = native_thumbnail_filter(mode, *filter);
    native_thumbnail_reduction_factors(
        source_width,
        source_height,
        output_width,
        output_height,
        effective_filter,
        false,
    ) == Some((2, 2))
        && native_thumbnail_supported_for_image(img, *w, *h, effective_filter, mode)
}

#[inline(always)]
fn apply_rgb_putpixel_overrides_to_reduction(
    source_bytes: &[u8],
    source_width: usize,
    reduced_width: usize,
    putpixel_ops: &[PipelineOp],
    reduced: &mut [u8],
) -> Option<()> {
    let mut pixel_overrides = std::collections::BTreeMap::<usize, [u8; 3]>::new();
    for op in putpixel_ops {
        let PipelineOp::PutPixel { x, y, color, .. } = op else {
            return None;
        };
        let pixel_index = *y as usize * source_width + *x as usize;
        pixel_overrides.insert(pixel_index, [color.0, color.1, color.2]);
    }
    let affected_blocks = pixel_overrides
        .keys()
        .map(|&pixel_index| {
            let x = pixel_index % source_width;
            let y = pixel_index / source_width;
            (y / 2) * reduced_width + x / 2
        })
        .collect::<std::collections::BTreeSet<_>>();
    for block_index in affected_blocks {
        let block_x = block_index % reduced_width;
        let block_y = block_index / reduced_width;
        let mut sums = [0u32; 3];
        for dy in 0..2usize {
            for dx in 0..2usize {
                let source_pixel_index = (block_y * 2 + dy) * source_width + block_x * 2 + dx;
                let sample = pixel_overrides
                    .get(&source_pixel_index)
                    .copied()
                    .unwrap_or_else(|| {
                        let source_start = source_pixel_index * 3;
                        [
                            source_bytes[source_start],
                            source_bytes[source_start + 1],
                            source_bytes[source_start + 2],
                        ]
                    });
                for channel in 0..3 {
                    sums[channel] += u32::from(sample[channel]);
                }
            }
        }
        let output_start = block_index * 3;
        for channel in 0..3 {
            reduced[output_start + channel] = ((sums[channel] + 2) >> 2) as u8;
        }
    }
    Some(())
}

/// Fold pending RGB writes into the existing sparse 2×2 reduction.
/// `None` leaves dense sources on the ordinary per-operation executor path.
pub(crate) fn simd_rgb_putpixel_thumbnail_prepare(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !simd_rgb_putpixel_thumbnail_fusion_supported(img, putpixel_ops, thumbnail, mode) {
        return Ok(None);
    }
    let DynamicImage::ImageRgb8(source) = img else {
        return Ok(None);
    };
    let PipelineOp::Thumbnail { .. } = thumbnail else {
        return Ok(None);
    };
    let Some((mut reduced, reduced_width, reduced_height)) = native_reduce_sparse_rgb_2x2(img)
    else {
        return Ok(None);
    };

    // Preserve write order: the final value for a repeated coordinate wins.
    // Recompute only affected 2×2 blocks against the original native bytes.
    let source_width = img.width() as usize;
    let reduced_width = reduced_width as usize;
    if apply_rgb_putpixel_overrides_to_reduction(
        source.as_raw(),
        source_width,
        reduced_width,
        putpixel_ops,
        &mut reduced,
    )
    .is_none()
    {
        return Ok(None);
    }

    let reduced = RgbImage::from_raw(reduced_width as u32, reduced_height, reduced)
        .ok_or_else(|| PilError::InternalError("RGB thumbnail reduction shape mismatch".into()))?;
    Ok(Some(DynamicImage::ImageRgb8(reduced)))
}

/// Resize the prepared reduced RGB image through the existing sparse SIMD
/// path, preserving the standard thumbnail kernel and vector receipt.
///
/// On the one-pixel ARM64 workload, CPU remains about 1.5× faster: its fused
/// reducer passes known active rows to the boxed resizer, while SIMD scans the
/// reduced rows and runs the generic two-pass vector convolution. The SIMD
/// profile concentrates in that convolution, so the row discovery and vector
/// setup are not amortized by the small number of active pixels. See the
/// matching benchmark and profile under
/// `docs/evidence/filter/thumbnail-rgb-cpu-simd-arm64-20261022/`.
pub(crate) fn simd_rgb_putpixel_thumbnail_resize(
    reduced: &DynamicImage,
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return Err(PilError::ValueError("expected Thumbnail op".into()));
    };
    if !matches!(reduced, DynamicImage::ImageRgb8(_)) || !matches!(mode, None | Some("RGB")) {
        return Err(simd_unsupported("Thumbnail"));
    }
    let (output_width, output_height) =
        native_thumbnail_dimensions(reduced.width(), reduced.height(), *w, *h)
            .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    simd_resize_convolution_with_zero_row_elision(
        reduced,
        output_width,
        output_height,
        native_thumbnail_filter(mode, *filter),
        3,
        false,
        true,
        0,
        false,
        true,
    )
}
