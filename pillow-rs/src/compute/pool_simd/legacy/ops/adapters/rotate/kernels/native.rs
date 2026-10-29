//! Native Rotate kernels that are shared by the Rotate family dispatcher.

use super::super as rotate;
use super::super::super::transpose::native_transpose;
use super::super::super::{TransposeMethod, native_copy_image_bytes};
use super::super::{
    native_rotate_layout, rotate_discrete_fast_angle, rotate_fill_sample, simd_rotate_geometry,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::{DynamicImage, GenericImageView};
use wide::u8x16;

mod bilinear;
mod nearest;
mod typed_bilinear;

pub(in crate::compute::pool_simd::ops::adapters::rotate) use self::bilinear::simd_bilinear_rotate_native;
pub(in crate::compute::pool_simd::ops::adapters::rotate) use self::nearest::simd_nearest_rotate_native;
pub(in crate::compute::pool_simd::ops::adapters::rotate) use self::typed_bilinear::simd_typed_bilinear_rotate_native;

pub(in crate::compute::pool_simd::ops::adapters::rotate) fn native_identity_rotate(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    native_copy_image_bytes(img, mode)
}

/// Execute Pillow's right-angle fast paths in the source's native layout.
///
/// Expanded rotations are direct transposes. Non-expanded 90/270-degree
/// rotations use the same centered clipping offsets as the CPU path and fill
/// only the exposed canvas. The operation is bandwidth-bound, so it is
/// classified as `native-copy`; the non-expanded path additionally records
/// its complete native-byte vector blocks.
pub(in crate::compute::pool_simd::ops::adapters::rotate) fn simd_right_angle_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(degree) = rotate_discrete_fast_angle(angle, center, translate) else {
        return Ok(None);
    };
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let method = match degree {
        90 => TransposeMethod::Rotate90,
        180 => TransposeMethod::Rotate180,
        270 => TransposeMethod::Rotate270,
        _ => return Ok(None),
    };
    if expand || degree == 180 {
        let Some((result, vector_blocks, scalar_tail, kernel_path)) =
            native_transpose(img, mode, method)
        else {
            return Ok(None);
        };
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path(kernel_path);
        return Ok(Some(result));
    }

    let width = width as usize;
    let height = height as usize;
    let row_bytes = width
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD rotate row dimensions overflow".into()))?;
    let output_len = row_bytes
        .checked_mul(height)
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != output_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let width_i32 = i32::try_from(width)
        .map_err(|_| PilError::ValueError("SIMD rotate width exceeds coordinate range".into()))?;
    let height_i32 = i32::try_from(height)
        .map_err(|_| PilError::ValueError("SIMD rotate height exceeds coordinate range".into()))?;
    let width_gap = width_i32 - height_i32;
    let height_gap = height_i32 - width_i32;
    let (x_offset, y_offset) = if degree == 90 {
        (
            (f64::from(width_gap) / 2.0).floor() as i32,
            (f64::from(height_gap) / 2.0).ceil() as i32,
        )
    } else {
        (
            (f64::from(width_gap) / 2.0).ceil() as i32,
            (f64::from(height_gap) / 2.0).floor() as i32,
        )
    };
    let mut output = vec![0u8; output_len];
    let (vector_blocks, scalar_tail) = rotate::copy_right_angle_pixels(
        source,
        &mut output,
        width,
        height,
        channels,
        width_i32,
        height_i32,
        degree,
        x_offset,
        y_offset,
        fill,
    );

    rotate::finish_native_rotation(
        img,
        width as u32,
        height as u32,
        output,
        channels,
        vector_blocks,
        scalar_tail,
        "native-copy",
    )
}

/// Rotate an empty native byte image through the scalar geometry control
/// plane. Pillow still computes an expanded destination for a zero-width or
/// zero-height source; every destination sample is the fill value because no
/// source coordinate can be valid. Fill construction and storage use a
/// padded `u8x16` block, so this is a native SIMD no-data path rather than a
/// CPU fallback or an early capability refusal.
pub(in crate::compute::pool_simd::ops::adapters::rotate) fn simd_empty_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if img.width() != 0 && img.height() != 0 {
        return Ok(None);
    }
    if matches!(img, DynamicImage::ImageLuma16(_)) {
        return Ok(None);
    }
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    let Some(geometry) =
        simd_rotate_geometry(img.width(), img.height(), angle, expand, center, translate)
    else {
        return Ok(None);
    };
    let source_len = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    if source_len != img.as_bytes().len() {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let output_len = (geometry.width as usize)
        .checked_mul(geometry.height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let fill_block =
        std::array::from_fn(|index| rotate_fill_sample(fill, channels, index % channels));
    let fill_vector = u8x16::new(fill_block);
    let fill_bytes = fill_vector.to_array();
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    for chunk in output.chunks_mut(16) {
        let active = chunk.len();
        chunk.copy_from_slice(&fill_bytes[..active]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_path("vector");
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image_allow_empty(
        geometry.width,
        geometry.height,
        output,
        channels,
    )
    .map(|result| Some(preserve_mode(img, result)))
}
