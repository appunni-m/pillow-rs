//! Native Rotate kernels that are shared by the Rotate family dispatcher.

use super::super as rotate;
use super::super::super::transpose::native_transpose;
use super::super::super::{
    SIMD_BYTE_VECTOR_LANES, SIMD_F64_LANES, SimdRotateSample, TransposeMethod,
    bilinear_rotate_scalar, bilinear_rotate_vector, gather_rotate_neighborhood_vectors,
    native_copy_image_bytes, rotate_fused_mul_add, rotate_unpremultiply_vector,
    simd_rotate_sample_coordinates,
};
use super::super::{
    native_rotate_layout, rotate_discrete_fast_angle, rotate_fill_sample, simd_rotate_geometry,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::{DynamicImage, GenericImageView};
use wide::{f32x8, f64x8, i32x8, u8x16};

pub(in crate::compute::pool_simd::ops::adapters::rotate) fn native_identity_rotate(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    native_copy_image_bytes(img, mode)
}

/// Execute the exact nearest-neighbor affine contract for ordinary byte
/// layouts. Coordinate progression follows Pillow's signed 16.16 affine
/// sampler; each complete group of pixels is gathered into native `u8x16`
/// blocks and written without an RGBA expansion.
pub(in crate::compute::pool_simd::ops::adapters::rotate) fn simd_nearest_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    let Some(geometry) =
        simd_rotate_geometry(img.width(), img.height(), angle, expand, center, translate)
    else {
        return Ok(None);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let destination_width = geometry.width as usize;
    let destination_height = geometry.height as usize;
    let source_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    if destination_width == 0 || destination_height == 0 {
        return Ok(None);
    }
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let mut output = vec![0u8; destination_len];
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let [a, b, c, d, e, f] = geometry.affine;
    let step_x_x = fixed(a);
    let step_y_x = fixed(b);
    let step_x_y = fixed(d);
    let step_y_y = fixed(e);
    let origin_x = fixed(c + a * 0.5 + b * 0.5);
    let origin_y = fixed(f + d * 0.5 + e * 0.5);
    let pixels_per_vector = (SIMD_BYTE_VECTOR_LANES / channels).max(1);
    let block_bytes = pixels_per_vector * channels;
    // Small positive angles keep nearest samples in one source row long
    // enough to copy native RGB spans. Other affine slopes retain the gather
    // kernel below, so this run detector does not tax arbitrary-angle calls.
    #[cfg(not(feature = "parallel"))]
    if channels == 3
        && step_x_x >= (65_536_i64 * 9 / 10)
        && step_x_x <= (65_536_i64 * 11 / 10)
        && (0_i64..=65_536_i64 / 8).contains(&step_x_y)
        && let Some((vector_blocks, scalar_tail)) = rotate::copy_contiguous_rgb_runs(
            source,
            &mut output,
            width,
            height,
            destination_width,
            destination_height,
            origin_x,
            origin_y,
            step_x_x,
            step_y_x,
            step_x_y,
            step_y_y,
            [fill.0, fill.1, fill.2],
        )
    {
        return rotate::finish_native_rotation(
            img,
            geometry.width,
            geometry.height,
            output,
            channels,
            vector_blocks,
            scalar_tail,
            "native-copy",
        );
    }

    let (vector_blocks, scalar_tail) = rotate::copy_nearest_affine_pixels(
        source,
        &mut output,
        width,
        height,
        destination_width,
        destination_height,
        channels,
        pixels_per_vector,
        block_bytes,
        origin_x,
        origin_y,
        step_x_x,
        step_y_x,
        step_x_y,
        step_y_y,
        fill,
    );

    rotate::finish_native_rotation(
        img,
        geometry.width,
        geometry.height,
        output,
        channels,
        vector_blocks,
        scalar_tail,
        "vector",
    )
}

#[inline]
fn typed_bilinear_rotate_vector(
    p00: [f64; SIMD_F64_LANES],
    p10: [f64; SIMD_F64_LANES],
    p01: [f64; SIMD_F64_LANES],
    p11: [f64; SIMD_F64_LANES],
    fx: f64x8,
    fy: f64x8,
    is_float: bool,
) -> f64x8 {
    // Geometry.c subtracts source samples in their native type before
    // widening for interpolation. F rounds that subtraction to f32; I wraps
    // the i32 difference and evaluates the subsequent multiply/add separately.
    let difference = |right: [f64; SIMD_F64_LANES], left: [f64; SIMD_F64_LANES]| {
        if is_float {
            let difference = f32x8::new(right.map(|value| value as f32))
                - f32x8::new(left.map(|value| value as f32));
            f64x8::new(difference.to_array().map(f64::from))
        } else {
            let difference = i32x8::new(right.map(|value| value as i32))
                - i32x8::new(left.map(|value| value as i32));
            f64x8::new(difference.to_array().map(f64::from))
        }
    };
    let top_delta = difference(p10, p00);
    let bottom_delta = difference(p11, p01);
    if is_float {
        let top = rotate_fused_mul_add(top_delta, fx, f64x8::new(p00));
        let bottom = rotate_fused_mul_add(bottom_delta, fx, f64x8::new(p01));
        rotate_fused_mul_add(bottom - top, fy, top)
    } else {
        let top = top_delta * fx + f64x8::new(p00);
        let bottom = bottom_delta * fx + f64x8::new(p01);
        (bottom - top) * fy + top
    }
}

/// Rotate native four-byte `I`/`F` samples without treating their scalar
/// bytes as independent color channels.
///
/// Affine coordinate construction and gathers stay scalar control because a
/// portable gather is not exposed by `wide`; the bilinear sample arithmetic
/// is evaluated in eight `f64` lanes. Partial output blocks are padded and
/// only their active lanes are stored, so small public images still execute
/// the same vector data path rather than being rejected or sent to CPU.
pub(in crate::compute::pool_simd::ops::adapters::rotate) fn simd_typed_bilinear_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let is_float = match mode {
        Some("F") => true,
        Some("I") => false,
        _ => return Ok(None),
    };
    if !matches!(img, DynamicImage::ImageRgba8(_)) {
        return Ok(None);
    }
    let Some(geometry) =
        simd_rotate_geometry(img.width(), img.height(), angle, expand, center, translate)
    else {
        return Ok(None);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let destination_width = geometry.width as usize;
    let destination_height = geometry.height as usize;
    if width == 0 || height == 0 || destination_width == 0 || destination_height == 0 {
        return Ok(None);
    }
    let source_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let scalar_fill = if is_float {
        f32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    } else {
        i32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    };
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let active = (destination_width - destination_x).min(SIMD_F64_LANES);
            let mut p00 = [scalar_fill; SIMD_F64_LANES];
            let mut p10 = [scalar_fill; SIMD_F64_LANES];
            let mut p01 = [scalar_fill; SIMD_F64_LANES];
            let mut p11 = [scalar_fill; SIMD_F64_LANES];
            let mut fx = [0.0; SIMD_F64_LANES];
            let mut fy = [0.0; SIMD_F64_LANES];

            for lane in 0..active {
                let x = destination_x + lane;
                let sample = simd_rotate_sample_coordinates(
                    geometry.affine,
                    width,
                    height,
                    x,
                    destination_y,
                );
                if sample.valid {
                    let SimdRotateSample { x0, x1, y0, y1, .. } = sample;
                    fx[lane] = sample.fx;
                    fy[lane] = sample.fy;
                    let read = |source_x: usize, source_y: usize| {
                        let index = (source_y * width + source_x) * 4;
                        if is_float {
                            f32::from_le_bytes([
                                source[index],
                                source[index + 1],
                                source[index + 2],
                                source[index + 3],
                            ]) as f64
                        } else {
                            i32::from_le_bytes([
                                source[index],
                                source[index + 1],
                                source[index + 2],
                                source[index + 3],
                            ]) as f64
                        }
                    };
                    p00[lane] = read(x0, y0);
                    p10[lane] = read(x1, y0);
                    p01[lane] = read(x0, y1);
                    p11[lane] = read(x1, y1);
                }
            }

            let values = typed_bilinear_rotate_vector(
                p00,
                p10,
                p01,
                p11,
                f64x8::new(fx),
                f64x8::new(fy),
                is_float,
            )
            .to_array();
            for lane in 0..active {
                let bytes = if is_float {
                    (values[lane] as f32).to_le_bytes()
                } else {
                    (values[lane] as i32).to_le_bytes()
                };
                let output_index = (destination_y * destination_width + destination_x + lane) * 4;
                output[output_index..output_index + 4].copy_from_slice(&bytes);
            }
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += active;
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(geometry.width, geometry.height, output, 4)
        .map(|result| Some(preserve_mode(img, result)))
}

/// Vectorized ordinary-byte bilinear rotation.
///
/// Affine coordinate construction and source gathers are scalar control;
/// neighbor interpolation runs eight output pixels at a time in `f64x8`.
/// LA/RGBA retain Pillow's premultiplied-alpha byte-rounding boundaries while
/// avoiding a whole-frame mode conversion.
pub(in crate::compute::pool_simd::ops::adapters::rotate) fn simd_bilinear_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    if matches!(mode, Some("I" | "F")) {
        return Ok(None);
    }
    let Some(geometry) =
        simd_rotate_geometry(img.width(), img.height(), angle, expand, center, translate)
    else {
        return Ok(None);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let destination_width = geometry.width as usize;
    let destination_height = geometry.height as usize;
    if width == 0 || height == 0 || destination_width == 0 || destination_height == 0 {
        return Ok(None);
    }
    let source_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let pa_mode = mode == Some("PA");
    // Pillow's rotate/transform path treats RGBa as already-premultiplied
    // stored samples and RGBX/CMYK as raw four-byte planes. Only RGBA has a
    // straight-alpha channel that needs premultiply/interpolate/unpremultiply.
    let alpha_channel = match (channels, mode) {
        (2, _) if !pa_mode => Some(1),
        (4, None | Some("RGBA")) => Some(3),
        _ => None,
    };
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..destination_height {
        let output_row = y * destination_width * channels;
        let vector_width = destination_width;
        let mut x = 0usize;
        while x < vector_width {
            let active = (destination_width - x).min(SIMD_F64_LANES);
            let samples = std::array::from_fn(|lane| {
                if lane < active {
                    simd_rotate_sample_coordinates(geometry.affine, width, height, x + lane, y)
                } else {
                    SimdRotateSample::default()
                }
            });
            let fx = f64x8::new(samples.map(|sample| sample.fx));
            let fy = f64x8::new(samples.map(|sample| sample.fy));
            let mut block = [0u8; SIMD_F64_LANES * 4];
            let mut alpha = [0u8; SIMD_F64_LANES];
            if let Some(alpha_channel) = alpha_channel {
                let neighbors = gather_rotate_neighborhood_vectors(
                    source,
                    width,
                    channels,
                    alpha_channel,
                    fill,
                    &samples,
                    None,
                );
                alpha = bilinear_rotate_vector(
                    neighbors[0],
                    neighbors[1],
                    neighbors[2],
                    neighbors[3],
                    fx,
                    fy,
                )
                .to_array()
                .map(|value| value as u8);
                for (lane, &value) in alpha.iter().enumerate() {
                    block[lane * channels + alpha_channel] = value;
                }
            }
            for channel in 0..channels {
                if Some(channel) == alpha_channel {
                    continue;
                }
                let neighbors = gather_rotate_neighborhood_vectors(
                    source,
                    width,
                    channels,
                    channel,
                    fill,
                    &samples,
                    alpha_channel,
                );
                let interpolated = bilinear_rotate_vector(
                    neighbors[0],
                    neighbors[1],
                    neighbors[2],
                    neighbors[3],
                    fx,
                    fy,
                );
                let values = if alpha_channel.is_some() {
                    rotate_unpremultiply_vector(interpolated, alpha)
                } else {
                    interpolated.to_array().map(|value| value as u8)
                };
                for (lane, &value) in values.iter().enumerate() {
                    block[lane * channels + channel] = value;
                }
            }
            let output_start = output_row + x * channels;
            let block_bytes = active * channels;
            output[output_start..output_start + block_bytes].copy_from_slice(&block[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += active;
        }
        while x < destination_width {
            let sample = simd_rotate_sample_coordinates(geometry.affine, width, height, x, y);
            let output_start = output_row + x * channels;
            let mut alpha = 0u8;
            if let Some(alpha_channel) = alpha_channel {
                let alpha_value = if sample.valid {
                    let read = |source_x: usize, source_y: usize| {
                        source[(source_y * width + source_x) * channels + alpha_channel] as f64
                    };
                    bilinear_rotate_scalar(
                        read(sample.x0, sample.y0),
                        read(sample.x1, sample.y0),
                        read(sample.x0, sample.y1),
                        read(sample.x1, sample.y1),
                        sample.fx,
                        sample.fy,
                    )
                } else {
                    rotate_fill_sample(fill, channels, alpha_channel) as f64
                };
                alpha = alpha_value as u8;
                output[output_start + alpha_channel] = alpha;
            }
            for channel in 0..channels {
                if Some(channel) == alpha_channel {
                    continue;
                }
                let value = if sample.valid {
                    let read = |source_x: usize, source_y: usize| {
                        source[(source_y * width + source_x) * channels + channel] as f64
                    };
                    let mut p00 = read(sample.x0, sample.y0);
                    let mut p10 = read(sample.x1, sample.y0);
                    let mut p01 = read(sample.x0, sample.y1);
                    let mut p11 = read(sample.x1, sample.y1);
                    if alpha_channel.is_some() {
                        let alpha_channel = alpha_channel.expect("checked above");
                        let read_alpha = |source_x: usize, source_y: usize| {
                            source[(source_y * width + source_x) * channels + alpha_channel] as f64
                        };
                        let premultiply =
                            |value: f64, alpha: f64| (value * alpha / 255.0 + 0.5) as u8 as f64;
                        p00 = premultiply(p00, read_alpha(sample.x0, sample.y0));
                        p10 = premultiply(p10, read_alpha(sample.x1, sample.y0));
                        p01 = premultiply(p01, read_alpha(sample.x0, sample.y1));
                        p11 = premultiply(p11, read_alpha(sample.x1, sample.y1));
                    }
                    bilinear_rotate_scalar(p00, p10, p01, p11, sample.fx, sample.fy)
                } else {
                    rotate_fill_sample(fill, channels, channel) as f64
                };
                let value = value as u8;
                output[output_start + channel] = if alpha_channel.is_some() && alpha != 0 {
                    (value as f64 * 255.0 / alpha as f64) as u8
                } else {
                    value
                };
            }
            x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(geometry.width, geometry.height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
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
