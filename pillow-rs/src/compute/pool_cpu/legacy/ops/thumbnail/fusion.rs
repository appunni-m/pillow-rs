use std::collections::{BTreeMap, BTreeSet};

use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::pil_resize_boxed;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, GenericImageView, RgbImage};

use super::{execute_reduce_rgb_with_sparse_rows, rgb_putpixel_thumbnail_fusion_supported};

/// Apply admitted RGB PutPixel operations before the same reducing-gap resample.
/// This avoids cloning the full source image to change a small set of pixels.
pub(crate) fn execute_rgb_putpixel_thumbnail_fusion(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !rgb_putpixel_thumbnail_fusion_supported(img, putpixel_ops, thumbnail, mode) {
        return Err(PilError::InternalError(
            "unsupported RGB PutPixel thumbnail fusion request".into(),
        ));
    }
    let DynamicImage::ImageRgb8(source) = img else {
        return Err(PilError::InternalError(
            "RGB thumbnail fusion source changed after admission".into(),
        ));
    };
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return Err(PilError::InternalError(
            "RGB thumbnail fusion operation changed after admission".into(),
        ));
    };
    let (source_width, source_height) = img.dimensions();
    let new_width = (*w).max(1).min(source_width);
    let new_height = (*h).max(1).min(source_height);
    let reduced_width = source_width / 2;
    let reduced_height = source_height / 2;
    let pixel_overrides = collect_rgb_pixel_overrides(putpixel_ops, source_width, source_height)?;

    let Some((reduced_image, mut sparse_nonzero_rows)) =
        execute_reduce_rgb_with_sparse_rows(img, 2, 2, Some("RGB"))?
    else {
        return Err(PilError::InternalError(
            "RGB thumbnail reduction rejected its native source".into(),
        ));
    };
    let mut reduced_bytes = match reduced_image {
        DynamicImage::ImageRgb8(reduced) => reduced.into_raw(),
        _ => {
            return Err(PilError::InternalError(
                "RGB thumbnail reduction changed the native mode".into(),
            ));
        }
    };

    apply_rgb_pixel_overrides(
        source.as_raw(),
        source_width,
        reduced_width,
        &pixel_overrides,
        &mut reduced_bytes,
        sparse_nonzero_rows.as_mut(),
    );
    let reduced = RgbImage::from_raw(reduced_width, reduced_height, reduced_bytes)
        .ok_or_else(|| PilError::InternalError("RGB reduction buffer shape mismatch".into()))?;
    let reduced = DynamicImage::ImageRgb8(reduced);
    let resized = resize_reduced_rgb_thumbnail(
        &reduced,
        (source_width, source_height),
        (new_width, new_height),
        *filter,
        sparse_nonzero_rows.as_deref(),
    );
    Ok(preserve_mode(img, resized))
}

fn collect_rgb_pixel_overrides(
    putpixel_ops: &[PipelineOp],
    source_width: u32,
    source_height: u32,
) -> Result<BTreeMap<usize, [u8; 3]>, PilError> {
    let mut pixel_overrides = BTreeMap::<usize, [u8; 3]>::new();
    for op in putpixel_ops {
        let PipelineOp::PutPixel {
            x,
            y,
            color,
            palette_index: false,
        } = op
        else {
            return Err(PilError::InternalError(
                "RGB PutPixel fusion received a different operation".into(),
            ));
        };
        if *x >= source_width || *y >= source_height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let pixel_index = *y as usize * source_width as usize + *x as usize;
        pixel_overrides.insert(pixel_index, [color.0, color.1, color.2]);
    }
    Ok(pixel_overrides)
}

#[inline(always)]
fn apply_rgb_pixel_overrides(
    source_bytes: &[u8],
    source_width: u32,
    reduced_width: u32,
    pixel_overrides: &BTreeMap<usize, [u8; 3]>,
    reduced_bytes: &mut [u8],
    mut sparse_nonzero_rows: Option<&mut Vec<bool>>,
) {
    let affected_blocks = pixel_overrides
        .keys()
        .map(|&pixel_index| {
            let x = pixel_index % source_width as usize;
            let y = pixel_index / source_width as usize;
            (y / 2) * reduced_width as usize + x / 2
        })
        .collect::<BTreeSet<_>>();
    for block_index in affected_blocks {
        let block_x = block_index % reduced_width as usize;
        let block_y = block_index / reduced_width as usize;
        let mut sums = [0u32; 3];
        for dy in 0..2usize {
            for dx in 0..2usize {
                let source_pixel_index =
                    (block_y * 2 + dy) * source_width as usize + block_x * 2 + dx;
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
            reduced_bytes[output_start + channel] = ((sums[channel] + 2) >> 2) as u8;
        }
        if reduced_bytes[output_start..output_start + 3]
            .iter()
            .any(|&sample| sample != 0)
            && let Some(nonzero_rows) = sparse_nonzero_rows.as_deref_mut()
        {
            nonzero_rows[block_index / reduced_width as usize] = true;
        }
    }
}

#[inline(always)]
fn resize_reduced_rgb_thumbnail(
    reduced: &DynamicImage,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    filter: ResampleFilter,
    sparse_nonzero_rows: Option<&[bool]>,
) -> DynamicImage {
    let (source_width, source_height) = source_dimensions;
    let (new_width, new_height) = output_dimensions;
    let box_right = f64::from(source_width) / 2.0;
    let box_bottom = f64::from(source_height) / 2.0;
    if let Some(nonzero_rows) = sparse_nonzero_rows {
        crate::ops::pil_resize::pil_resize_boxed_with_sparse_rgb_rows(
            reduced,
            new_width,
            new_height,
            0.0,
            0.0,
            box_right,
            box_bottom,
            filter,
            Some("RGB"),
            nonzero_rows,
        )
    } else {
        pil_resize_boxed(
            reduced,
            new_width,
            new_height,
            0.0,
            0.0,
            box_right,
            box_bottom,
            filter,
            Some("RGB"),
        )
    }
}
