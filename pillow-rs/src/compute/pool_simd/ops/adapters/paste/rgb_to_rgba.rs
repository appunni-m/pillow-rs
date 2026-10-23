//! RGB-to-RGBA Paste specialization with native row writes.

#[cfg(feature = "parallel")]
use super::super::SIMD_RGB_TO_RGBA_PARALLEL_PIXEL_THRESHOLD;
use super::super::put_alpha::simd_put_alpha_rgb_row;
use super::{NativePasteRegion, native_paste_region, simd_unsupported};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::{DynamicImage, RgbaImage};

struct RgbToRgbaRegionPlan {
    region: NativePasteRegion,
    source_stride: usize,
    destination_stride: usize,
    source_x: std::ops::Range<usize>,
    destination_x: std::ops::Range<usize>,
    source_rows: std::ops::Range<usize>,
    destination_rows: std::ops::Range<usize>,
}

fn rgb_to_rgba_region_plan(
    image_width: u32,
    source_width: u32,
    region: NativePasteRegion,
) -> Result<RgbToRgbaRegionPlan, PilError> {
    let source_stride = (source_width as usize)
        .checked_mul(3)
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let destination_stride = (image_width as usize)
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let source_x_start = region
        .source_left
        .checked_mul(3)
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let source_x_end = source_x_start
        .checked_add(
            region
                .width
                .checked_mul(3)
                .ok_or_else(|| simd_unsupported("Paste"))?,
        )
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let destination_x_start = region
        .destination_left
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let destination_x_end = destination_x_start
        .checked_add(
            region
                .width
                .checked_mul(4)
                .ok_or_else(|| simd_unsupported("Paste"))?,
        )
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let source_rows_start = region
        .source_top
        .checked_mul(source_stride)
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let source_rows_end = source_rows_start
        .checked_add(
            region
                .height
                .checked_mul(source_stride)
                .ok_or_else(|| simd_unsupported("Paste"))?,
        )
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let destination_rows_start = region
        .destination_top
        .checked_mul(destination_stride)
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let destination_rows_end = destination_rows_start
        .checked_add(
            region
                .height
                .checked_mul(destination_stride)
                .ok_or_else(|| simd_unsupported("Paste"))?,
        )
        .ok_or_else(|| simd_unsupported("Paste"))?;
    Ok(RgbToRgbaRegionPlan {
        region,
        source_stride,
        destination_stride,
        source_x: source_x_start..source_x_end,
        destination_x: destination_x_start..destination_x_end,
        source_rows: source_rows_start..source_rows_end,
        destination_rows: destination_rows_start..destination_rows_end,
    })
}

fn copy_rgb_to_rgba_rows(
    source: &[u8],
    output: &mut [u8],
    plan: &RgbToRgbaRegionPlan,
) -> Result<(), PilError> {
    let source_rows = source
        .get(plan.source_rows.clone())
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let destination_rows = output
        .get_mut(plan.destination_rows.clone())
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let copy_row = |source_row: &[u8], destination_row: &mut [u8]| {
        simd_put_alpha_rgb_row(
            &source_row[plan.source_x.clone()],
            &mut destination_row[plan.destination_x.clone()],
            plan.region.width,
            255,
        );
    };
    #[cfg(feature = "parallel")]
    if plan.region.height > 1
        && plan.region.width.saturating_mul(plan.region.height)
            >= SIMD_RGB_TO_RGBA_PARALLEL_PIXEL_THRESHOLD
    {
        super::super::simd_rows_mut_serial!(
            destination_rows,
            plan.destination_stride,
            plan.region.height,
            |_row_start, _row_end, row_index, destination_row| {
                let source_start = row_index as usize * plan.source_stride;
                copy_row(
                    &source_rows[source_start..source_start + plan.source_stride],
                    destination_row,
                );
            }
        );
    } else {
        for (source_row, destination_row) in source_rows
            .chunks_exact(plan.source_stride)
            .zip(destination_rows.chunks_exact_mut(plan.destination_stride))
        {
            copy_row(source_row, destination_row);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (source_row, destination_row) in source_rows
        .chunks_exact(plan.source_stride)
        .zip(destination_rows.chunks_exact_mut(plan.destination_stride))
    {
        copy_row(source_row, destination_row);
    }
    Ok(())
}

pub(in crate::compute::pool_simd::ops::adapters) fn simd_paste_rgb_to_rgba(
    img: &DynamicImage,
    source: &DynamicImage,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<DynamicImage, PilError> {
    let (destination, source) = match (img, source) {
        (DynamicImage::ImageRgba8(destination), DynamicImage::ImageRgb8(source)) => {
            (destination, source)
        }
        _ => return Err(simd_unsupported("Paste")),
    };
    let source_width = u32::try_from(width).map_err(|_| simd_unsupported("Paste"))?;
    let source_height = u32::try_from(height).map_err(|_| simd_unsupported("Paste"))?;
    if source.dimensions() != (source_width, source_height) {
        return Err(simd_unsupported("Paste"));
    }
    let expected_destination = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Paste"))?;
    let expected_source = (source_width as usize)
        .checked_mul(source_height as usize)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| simd_unsupported("Paste"))?;
    if destination.as_raw().len() != expected_destination
        || source.as_raw().len() != expected_source
    {
        return Err(simd_unsupported("Paste"));
    }

    let region = native_paste_region(img.width(), img.height(), source_width, source_height, x, y)
        .unwrap_or(NativePasteRegion {
            source_left: 0,
            source_top: 0,
            destination_left: 0,
            destination_top: 0,
            width: 0,
            height: 0,
        });
    if region.width == 0 || region.height == 0 {
        crate::compute::record_pipeline_operation_path("vector-rgb-to-rgba-copy");
        return Ok(preserve_mode(
            img,
            DynamicImage::ImageRgba8(destination.clone()),
        ));
    }

    let plan = rgb_to_rgba_region_plan(img.width(), source_width, region)?;
    let mut output = destination.as_raw().clone();
    copy_rgb_to_rgba_rows(source.as_raw(), &mut output, &plan)?;
    let vector_blocks = (region.width / 4).saturating_mul(region.height) as u64;
    let scalar_tail = (region.width % 4).saturating_mul(region.height) as u64;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path("vector-rgb-to-rgba-copy");
    let result = RgbaImage::from_raw(img.width(), img.height(), output)
        .ok_or_else(|| PilError::InternalError("SIMD Paste RGBA buffer shape mismatch".into()))?;
    Ok(preserve_mode(img, DynamicImage::ImageRgba8(result)))
}
