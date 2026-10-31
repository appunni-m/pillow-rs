//! Sparse RGB PutPixel and Thumbnail fusion for the scalar CPU strategy.

use std::collections::{BTreeMap, BTreeSet};

use crate::compute::pool_cpu::ops::resize::rgb8::default::resize_impl;
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, GenericImageView, RgbImage};

/// Apply admitted RGB writes during the exact 2x2 thumbnail reduction.
pub(crate) fn execute_rgb_putpixel_thumbnail(
    image: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
) -> Result<DynamicImage, PilError> {
    execute_admitted_rgb_putpixel_thumbnail(image, putpixel_ops, thumbnail)
}

fn execute_admitted_rgb_putpixel_thumbnail(
    image: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgb8(source) = image else {
        return Err(PilError::InternalError(
            "RGB thumbnail fusion source changed after admission".into(),
        ));
    };
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return Err(PilError::InternalError(
            "RGB thumbnail fusion operation changed after admission".into(),
        ));
    };
    let (source_width, source_height) = image.dimensions();
    let output_width = (*w).max(1).min(source_width);
    let output_height = (*h).max(1).min(source_height);
    let reduced_width = source_width / 2;
    let reduced_height = source_height / 2;
    let overrides = collect_rgb_pixel_overrides(putpixel_ops, source_width, source_height)?;
    let Some((mut reduced_bytes, mut sparse_nonzero_rows)) =
        crate::compute::pool_cpu::ops::reduce::helpers::native_bytes::reduce_rgb_with_sparse_rows(
            source.as_raw(),
            source_width,
            source_height,
            2,
            2,
        )?
    else {
        return Err(PilError::InternalError(
            "RGB thumbnail reduction rejected its native source".into(),
        ));
    };

    apply_rgb_pixel_overrides(
        source.as_raw(),
        source_width,
        reduced_width,
        &overrides,
        &mut reduced_bytes,
        sparse_nonzero_rows.as_mut(),
    );
    let reduced = RgbImage::from_raw(reduced_width, reduced_height, reduced_bytes)
        .ok_or_else(|| PilError::InternalError("RGB reduction buffer shape mismatch".into()))?;
    resize_reduced_rgb_thumbnail(
        &reduced,
        (source_width, source_height),
        (output_width, output_height),
        *filter,
        sparse_nonzero_rows.as_deref(),
    )
}

fn collect_rgb_pixel_overrides(
    putpixel_ops: &[PipelineOp],
    source_width: u32,
    source_height: u32,
) -> Result<BTreeMap<usize, [u8; 3]>, PilError> {
    let mut overrides = BTreeMap::new();
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
        overrides.insert(pixel_index, [color.0, color.1, color.2]);
    }
    Ok(overrides)
}

#[inline(always)]
fn apply_rgb_pixel_overrides(
    source_bytes: &[u8],
    source_width: u32,
    reduced_width: u32,
    overrides: &BTreeMap<usize, [u8; 3]>,
    reduced_bytes: &mut [u8],
    mut sparse_nonzero_rows: Option<&mut Vec<bool>>,
) {
    let affected_blocks = overrides
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
                let sample = overrides
                    .get(&source_pixel_index)
                    .copied()
                    .unwrap_or_else(|| {
                        let offset = source_pixel_index * 3;
                        [
                            source_bytes[offset],
                            source_bytes[offset + 1],
                            source_bytes[offset + 2],
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
    reduced: &RgbImage,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    filter: ResampleFilter,
    sparse_nonzero_rows: Option<&[bool]>,
) -> Result<DynamicImage, PilError> {
    let (source_width, source_height) = source_dimensions;
    let (output_width, output_height) = output_dimensions;
    let box_right = f64::from(source_width) / 2.0;
    let box_bottom = f64::from(source_height) / 2.0;
    let bounds = (0.0, 0.0, box_right, box_bottom);
    let output = if let Some(nonzero_rows) = sparse_nonzero_rows {
        resize_impl::execute_boxed_with_sparse_rows(
            reduced,
            output_width,
            output_height,
            bounds,
            filter,
            nonzero_rows,
        )?
    } else {
        resize_impl::execute(
            reduced,
            output_width,
            output_height,
            filter,
            Some(bounds),
        )?
    };
    Ok(DynamicImage::ImageRgb8(output))
}
