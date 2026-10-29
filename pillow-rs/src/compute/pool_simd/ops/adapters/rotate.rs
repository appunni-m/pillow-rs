//! Rotate-family native kernels selected by the SIMD adapter.

mod interpolation;
mod kernels;

#[cfg(not(feature = "parallel"))]
pub(super) use self::kernels::copy_contiguous_rgb_runs;

use self::kernels::{
    native_identity_rotate, simd_bilinear_rotate_native, simd_empty_rotate_native,
    simd_nearest_rotate_native, simd_right_angle_rotate_native, simd_typed_bilinear_rotate_native,
};
use super::{SimdImageShape, SimdLayout, simd_unsupported};
use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;
use wide::u8x16;

pub(super) use self::interpolation::{
    bilinear_rotate_scalar, bilinear_rotate_vector, gather_rotate_neighborhood_vectors,
    rotate_fused_mul_add, rotate_unpremultiply_vector, simd_rotate_sample_coordinates,
};

#[derive(Clone, Copy, Default)]
pub(super) struct SimdRotateSample {
    pub(super) valid: bool,
    pub(super) x0: usize,
    pub(super) x1: usize,
    pub(super) y0: usize,
    pub(super) y1: usize,
    pub(super) fx: f64,
    pub(super) fy: f64,
}

/// Return the native byte layout accepted by rotation. Indexed images remain
/// raw sample planes during Pillow rotation: `1`/`P` are one-byte nearest
/// data and `PA` is a two-byte index/alpha plane. CMYK, RGBa, and RGBX also
/// remain raw four-byte samples; only straight-alpha `LA`/`RGBA` gets
/// alpha-aware interpolation. These modes must not use the narrower
/// ordinary-byte helper just because their storage is backed by the same Rust
/// image type.
fn native_rotate_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, Some("1" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if mode == Some("PA") => Some(2),
        DynamicImage::ImageLuma16(_)
            if matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N")) =>
        {
            Some(2)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, Some("CMYK" | "RGBa" | "RGBX" | "I" | "F")) =>
        {
            Some(4)
        }
        _ => super::native_byte_layout(img, mode),
    }
}

fn shape_native_rotate_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    match (shape.layout, mode) {
        (SimdLayout::Luma8, Some("1" | "P")) => Some(1),
        (SimdLayout::LumaA8, Some("PA")) => Some(2),
        (SimdLayout::Luma16, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N")) => Some(2),
        (SimdLayout::Rgba8, Some("CMYK" | "RGBa" | "RGBX" | "I" | "F")) => Some(4),
        _ => super::shape_native_byte_channels(shape, mode),
    }
}

fn rotate_identity_contract(
    angle: f64,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> bool {
    angle.is_finite()
        && angle.rem_euclid(360.0) == 0.0
        && center.is_none_or(|(x, y)| x.is_finite() && y.is_finite())
        && translate.is_none_or(|(x, y)| x == 0.0 && y == 0.0)
}

#[derive(Clone, Copy)]
struct SimdRotateGeometry {
    affine: [f64; 6],
    width: u32,
    height: u32,
}

fn simd_rotate_destination_dimensions(
    affine: [f64; 6],
    width: u32,
    height: u32,
    expand: bool,
) -> Option<(u32, u32)> {
    let [affine_a, affine_b, affine_c, affine_d, affine_e, affine_f] = affine;
    let sw = f64::from(width);
    let sh = f64::from(height);
    let transform = |x: f64, y: f64| {
        (
            affine_a * x + affine_b * y + affine_c,
            affine_d * x + affine_e * y + affine_f,
        )
    };
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(corner_x, corner_y) in &corners {
        let (rotated_x, rotated_y) = transform(corner_x, corner_y);
        min_x = min_x.min(rotated_x);
        max_x = max_x.max(rotated_x);
        min_y = min_y.min(rotated_y);
        max_y = max_y.max(rotated_y);
    }
    let (destination_width, destination_height) = if expand {
        (max_x.ceil() - min_x.floor(), max_y.ceil() - min_y.floor())
    } else {
        (sw, sh)
    };
    if !destination_width.is_finite()
        || !destination_height.is_finite()
        || destination_width < 0.0
        || destination_height < 0.0
        || destination_width > f64::from(u32::MAX)
        || destination_height > f64::from(u32::MAX)
    {
        return None;
    }
    Some((destination_width as u32, destination_height as u32))
}

/// Build the same reverse affine matrix and expanded canvas as Pillow's
/// `Image.rotate` implementation.
///
/// Keeping this calculation separate from the sampler makes it the scalar
/// control plane: the vector kernel below only gathers native bytes and
/// stores complete output blocks after these coordinates have been fixed.
fn simd_rotate_geometry(
    width: u32,
    height: u32,
    angle: f64,
    expand: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> Option<SimdRotateGeometry> {
    if !angle.is_finite()
        || center.is_some_and(|(x, y)| !x.is_finite() || !y.is_finite())
        || translate.is_some_and(|(x, y)| !x.is_finite() || !y.is_finite())
    {
        return None;
    }
    let sw = f64::from(width);
    let sh = f64::from(height);
    let radians = -angle.to_radians();
    let affine_a = crate::ops::rotate::round_rotate_coefficient(radians.cos());
    let affine_b = crate::ops::rotate::round_rotate_coefficient(radians.sin());
    let affine_d = crate::ops::rotate::round_rotate_coefficient(-radians.sin());
    let affine_e = affine_a;
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let mut affine_c =
        affine_a * (-center_x - translate_x) + affine_b * (-center_y - translate_y) + center_x;
    let mut affine_f =
        affine_d * (-center_x - translate_x) + affine_e * (-center_y - translate_y) + center_y;
    let (destination_width, destination_height) = simd_rotate_destination_dimensions(
        [affine_a, affine_b, affine_c, affine_d, affine_e, affine_f],
        width,
        height,
        expand,
    )?;
    if expand {
        let shift_x = -(f64::from(destination_width) - sw) / 2.0;
        let shift_y = -(f64::from(destination_height) - sh) / 2.0;
        affine_c += affine_a * shift_x + affine_b * shift_y;
        affine_f += affine_d * shift_x + affine_e * shift_y;
    }
    Some(SimdRotateGeometry {
        affine: [affine_a, affine_b, affine_c, affine_d, affine_e, affine_f],
        width: destination_width,
        height: destination_height,
    })
}

/// `execute_rotate` has exact right-angle fast paths. Leave only those exact
/// angles on the established implementation until a native transpose kernel
/// is selected explicitly; nearby fractional angles must use Pillow's affine
/// sampler rather than being rounded into a discrete rotation.
fn rotate_uses_discrete_fast_path(
    angle: f64,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    expand: bool,
    width: u32,
    height: u32,
) -> bool {
    rotate_discrete_fast_angle(angle, center, translate)
        .is_some_and(|degree| degree == 180 || expand || width == height)
}

pub(super) fn rotate_discrete_fast_angle(
    angle: f64,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> Option<u32> {
    if center.is_some() || translate.is_some() || !angle.is_finite() {
        return None;
    }
    let degree = angle.rem_euclid(360.0);
    [90.0, 180.0, 270.0]
        .into_iter()
        .find(|fast_angle| degree == *fast_angle)
        .map(|fast_angle| fast_angle as u32)
}

pub(super) fn rotate_fill_sample(fill: (u8, u8, u8, u8), channels: usize, channel: usize) -> u8 {
    if channels == 2 && channel == 1 {
        fill.3
    } else {
        match channel {
            0 => fill.0,
            1 => fill.1,
            2 => fill.2,
            _ => fill.3,
        }
    }
}

fn rotate_nearest_supported_for_shape(
    width: u32,
    height: u32,
    channels: Option<usize>,
    angle: f64,
    expand: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    nearest: bool,
) -> bool {
    let Some(channels) = channels else {
        return false;
    };
    if !nearest
        || width == 0
        || height == 0
        || rotate_uses_discrete_fast_path(angle, center, translate, expand, width, height)
    {
        return false;
    }
    let Some(geometry) = simd_rotate_geometry(width, height, angle, expand, center, translate)
    else {
        return false;
    };
    // The gather kernel pads a partial final block and writes only active
    // lanes, so even a single output byte follows the same native SIMD path.
    // Keep zero-sized geometry out of the admission contract above.
    geometry
        .width
        .checked_mul(geometry.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|bytes| bytes != 0)
}

fn rotate_bilinear_supported_for_shape(
    width: u32,
    height: u32,
    channels: Option<usize>,
    angle: f64,
    expand: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    nearest: bool,
) -> bool {
    if nearest
        || channels.is_none()
        || width == 0
        || height == 0
        || rotate_uses_discrete_fast_path(angle, center, translate, expand, width, height)
    {
        return false;
    }
    let Some(geometry) = simd_rotate_geometry(width, height, angle, expand, center, translate)
    else {
        return false;
    };
    geometry.width != 0 && geometry.height != 0
}

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let PipelineOp::Rotate {
        angle,
        expand,
        center,
        translate,
        filter,
        nearest,
        ..
    } = op
    else {
        return false;
    };
    let native_channels = native_rotate_layout(img, mode);
    let nearest = *nearest || matches!(mode, Some("1" | "P"));
    if (img.width() == 0 || img.height() == 0) && !matches!(img, DynamicImage::ImageLuma16(_)) {
        native_channels.is_some_and(|channels| {
            super::has_empty_native_bytes(img, channels)
                && simd_rotate_geometry(
                    img.width(),
                    img.height(),
                    *angle,
                    *expand,
                    *center,
                    *translate,
                )
                .is_some()
        })
    } else if rotate_identity_contract(*angle, *center, *translate) {
        super::native_copy_layout(img, mode).is_some_and(|channels| {
            img.width()
                .checked_mul(img.height())
                .and_then(|pixels| pixels.checked_mul(channels as u32))
                .is_some_and(|expected| expected != 0 && img.as_bytes().len() == expected as usize)
        })
    } else if rotate_uses_discrete_fast_path(
        *angle,
        *center,
        *translate,
        *expand,
        img.width(),
        img.height(),
    ) {
        native_channels.is_some()
            && (img.width() as usize).saturating_mul(img.height() as usize) != 0
    } else if nearest {
        rotate_nearest_supported_for_shape(
            img.width(),
            img.height(),
            native_channels,
            *angle,
            *expand,
            *center,
            *translate,
            nearest,
        )
    } else if matches!(img, DynamicImage::ImageLuma16(_))
        || !matches!(filter, ResampleFilter::Bilinear)
    {
        false
    } else {
        rotate_bilinear_supported_for_shape(
            img.width(),
            img.height(),
            native_channels,
            *angle,
            *expand,
            *center,
            *translate,
            nearest,
        )
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let PipelineOp::Rotate {
        angle,
        expand,
        center,
        translate,
        filter,
        nearest,
        ..
    } = op
    else {
        return false;
    };
    let native_channels = shape_native_rotate_channels(shape, mode);
    let pixel_count = (shape.width as usize).saturating_mul(shape.height as usize);
    let nearest = *nearest || matches!(mode, Some("1" | "P"));
    if (shape.width == 0 || shape.height == 0) && shape.layout != SimdLayout::Luma16 {
        native_channels.is_some()
            && simd_rotate_geometry(
                shape.width,
                shape.height,
                *angle,
                *expand,
                *center,
                *translate,
            )
            .is_some()
    } else if rotate_identity_contract(*angle, *center, *translate) {
        super::shape_native_identity_copy_channels(shape, mode).is_some_and(|channels| {
            (shape.width as usize)
                .checked_mul(shape.height as usize)
                .and_then(|pixels| pixels.checked_mul(channels))
                .is_some_and(|bytes| bytes != 0)
        })
    } else if rotate_uses_discrete_fast_path(
        *angle,
        *center,
        *translate,
        *expand,
        shape.width,
        shape.height,
    ) {
        native_channels.is_some() && pixel_count != 0
    } else if nearest {
        rotate_nearest_supported_for_shape(
            shape.width,
            shape.height,
            native_channels,
            *angle,
            *expand,
            *center,
            *translate,
            nearest,
        )
    } else if shape.layout == SimdLayout::Luma16 || !matches!(filter, ResampleFilter::Bilinear) {
        false
    } else {
        rotate_bilinear_supported_for_shape(
            shape.width,
            shape.height,
            native_channels,
            *angle,
            *expand,
            *center,
            *translate,
            nearest,
        )
    }
}

#[inline(always)]
pub(super) fn copy_nearest_affine_pixels(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    destination_width: usize,
    destination_height: usize,
    channels: usize,
    pixels_per_vector: usize,
    block_bytes: usize,
    origin_x: i64,
    origin_y: i64,
    step_x_x: i64,
    step_y_x: i64,
    step_x_y: i64,
    step_y_y: i64,
    fill: (u8, u8, u8, u8),
) -> (u64, u64) {
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let output_row_bytes = destination_width * channels;
    for (destination_y, output_row) in output
        .chunks_exact_mut(output_row_bytes)
        .take(destination_height)
        .enumerate()
    {
        let mut source_x = origin_x + destination_y as i64 * step_y_x;
        let mut source_y = origin_y + destination_y as i64 * step_y_y;
        let mut destination_x = 0usize;
        while destination_x + pixels_per_vector <= destination_width {
            let mut block = [0u8; 16];
            for pixel in 0..pixels_per_vector {
                let input_x = source_x >> 16;
                let input_y = source_y >> 16;
                let block_start = pixel * channels;
                if input_x >= 0 && input_x < width as i64 && input_y >= 0 && input_y < height as i64
                {
                    let source_start = (input_y as usize * width + input_x as usize) * channels;
                    block[block_start..block_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                } else {
                    for channel in 0..channels {
                        block[block_start + channel] = rotate_fill_sample(fill, channels, channel);
                    }
                }
                source_x += step_x_x;
                source_y += step_x_y;
            }
            let output_start = destination_x * channels;
            output_row[output_start..output_start + block_bytes]
                .copy_from_slice(&u8x16::new(block).to_array()[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += pixels_per_vector;
        }
        while destination_x < destination_width {
            let input_x = source_x >> 16;
            let input_y = source_y >> 16;
            let output_start = destination_x * channels;
            if input_x >= 0 && input_x < width as i64 && input_y >= 0 && input_y < height as i64 {
                let source_start = (input_y as usize * width + input_x as usize) * channels;
                output_row[output_start..output_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            } else {
                for channel in 0..channels {
                    output_row[output_start + channel] =
                        rotate_fill_sample(fill, channels, channel);
                }
            }
            source_x += step_x_x;
            source_y += step_x_y;
            destination_x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    (vector_blocks, scalar_tail)
}

#[inline(always)]
pub(super) fn copy_right_angle_pixels(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    width_i32: i32,
    height_i32: i32,
    degree: u32,
    x_offset: i32,
    y_offset: i32,
    fill: (u8, u8, u8, u8),
) -> (u64, u64) {
    let pixels_per_vector = (super::SIMD_BYTE_VECTOR_LANES / channels).max(1);
    let block_bytes = pixels_per_vector * channels;
    let row_bytes = width * channels;
    let vector_width = width / pixels_per_vector * pixels_per_vector;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for destination_y in 0..height {
        let output_row = destination_y * row_bytes;
        let mut destination_x = 0usize;
        while destination_x < vector_width {
            let mut block = [0u8; 16];
            for pixel in 0..pixels_per_vector {
                let x = destination_x + pixel;
                let (source_x, source_y) = if degree == 90 {
                    (
                        width_i32 - 1 - destination_y as i32 + y_offset,
                        x as i32 - x_offset,
                    )
                } else {
                    (
                        destination_y as i32 - y_offset,
                        height_i32 - 1 - x as i32 + x_offset,
                    )
                };
                let block_start = pixel * channels;
                if source_x >= 0 && source_x < width_i32 && source_y >= 0 && source_y < height_i32 {
                    let source_start = (source_y as usize * width + source_x as usize) * channels;
                    block[block_start..block_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                } else {
                    for channel in 0..channels {
                        block[block_start + channel] = rotate_fill_sample(fill, channels, channel);
                    }
                }
            }
            let output_start = output_row + destination_x * channels;
            output[output_start..output_start + block_bytes]
                .copy_from_slice(&u8x16::new(block).to_array()[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += pixels_per_vector;
        }
        while destination_x < width {
            let x = destination_x;
            let (source_x, source_y) = if degree == 90 {
                (
                    width_i32 - 1 - destination_y as i32 + y_offset,
                    x as i32 - x_offset,
                )
            } else {
                (
                    destination_y as i32 - y_offset,
                    height_i32 - 1 - x as i32 + x_offset,
                )
            };
            let output_start = output_row + x * channels;
            if source_x >= 0 && source_x < width_i32 && source_y >= 0 && source_y < height_i32 {
                let source_start = (source_y as usize * width + source_x as usize) * channels;
                output[output_start..output_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            } else {
                for channel in 0..channels {
                    output[output_start + channel] = rotate_fill_sample(fill, channels, channel);
                }
            }
            destination_x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    (vector_blocks, scalar_tail)
}

#[inline(always)]
pub(super) fn finish_native_rotation(
    img: &DynamicImage,
    width: u32,
    height: u32,
    output: Vec<u8>,
    channels: usize,
    vector_blocks: u64,
    scalar_tail: u64,
    path: &'static str,
) -> Result<Option<DynamicImage>, PilError> {
    crate::compute::record_pipeline_operation_path(path);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

pub(crate) fn simd_rotate(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    simd_rotate_in_context(
        &ExecutionContext::for_backend(Backend::Simd),
        img,
        op,
        mode,
    )
}

pub(super) fn simd_rotate_in_context(
    context: &ExecutionContext,
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if context.backend() != Backend::Simd || !context.architecture().supports_simd() {
        return Err(simd_unsupported("Rotate"));
    }
    let PipelineOp::Rotate {
        angle,
        expand,
        fill,
        center,
        translate,
        filter,
        nearest,
        ..
    } = op
    else {
        return Err(PilError::ValueError("expected Rotate op".into()));
    };
    // Indexed Pillow images are defined to rotate with nearest-neighbor
    // sampling. The operation flag records only the explicit resample
    // request, so include the logical mode before choosing the data kernel.
    let nearest = *nearest || matches!(mode, Some("1" | "P"));
    if let Some(result) =
        simd_empty_rotate_native(img, *angle, *expand, *fill, *center, *translate, mode)?
    {
        return Ok(result);
    }
    if rotate_identity_contract(*angle, *center, *translate)
        && let Some(result) = native_identity_rotate(img, mode)?
    {
        return Ok(preserve_mode(img, result));
    }
    if rotate_uses_discrete_fast_path(
        *angle,
        *center,
        *translate,
        *expand,
        img.width(),
        img.height(),
    ) && let Some(result) =
        simd_right_angle_rotate_native(
            context,
            img,
            *angle,
            *expand,
            *fill,
            *center,
            *translate,
            mode,
        )?
    {
        return Ok(result);
    }
    if nearest
        && !rotate_uses_discrete_fast_path(
            *angle,
            *center,
            *translate,
            *expand,
            img.width(),
            img.height(),
        )
        && let Some(result) =
            simd_nearest_rotate_native(img, *angle, *expand, *fill, *center, *translate, mode)?
    {
        return Ok(result);
    }
    if !nearest
        && matches!(filter, ResampleFilter::Bilinear)
        && !rotate_uses_discrete_fast_path(
            *angle,
            *center,
            *translate,
            *expand,
            img.width(),
            img.height(),
        )
        && let Some(result) = simd_typed_bilinear_rotate_native(
            img, *angle, *expand, *fill, *center, *translate, mode,
        )?
    {
        return Ok(result);
    }
    if !nearest
        && matches!(filter, ResampleFilter::Bilinear)
        && !rotate_uses_discrete_fast_path(
            *angle,
            *center,
            *translate,
            *expand,
            img.width(),
            img.height(),
        )
        && let Some(result) =
            simd_bilinear_rotate_native(img, *angle, *expand, *fill, *center, *translate, mode)?
    {
        return Ok(result);
    }
    Err(simd_unsupported("Rotate"))
}

#[cfg(test)]
mod tests;
