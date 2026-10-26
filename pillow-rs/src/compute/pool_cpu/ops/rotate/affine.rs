//! Fixed-point affine coordinate and native-channel rotation kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;
#[cfg(not(feature = "parallel"))]
use crate::ops::rotate::nearest_contiguous_rgb_run_len;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, GenericImageView};

use super::cubic::{rotate_cubic_horizontal_f64, rotate_cubic_vertical_f64};
use super::scalar::rotate_arbitrary_scalar;

// ── Generic rotation & transform helpers (mode-aware) ──

#[path = "affine/coordinates.rs"]
mod coordinates;
#[cfg(not(feature = "parallel"))]
pub(crate) use coordinates::*;

pub(crate) fn affine_nearest_fixed(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    channels: usize,
    affine: [f64; 6],
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
) {
    // Pillow's ImagingTransformAffine nearest path rounds the six affine
    // coefficients to signed 16.16 values once, then advances those integers
    // across each output row. Recomputing the same coordinates as f64 changes
    // which source pixel wins at exact integer boundaries.
    let (step_x_x, step_y_x, step_x_y, step_y_y, origin_x, origin_y) =
        affine_nearest_fixed_coordinates(affine);
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    if destination_width == 0 || destination_height == 0 {
        return;
    }

    let process_row = |y: u32, row: &mut [u8]| {
        affine_nearest_fixed_process_row(
            source,
            source_width,
            source_height,
            destination_width,
            channels,
            origin_x,
            origin_y,
            step_x_x,
            step_y_x,
            step_x_y,
            step_y_y,
            fill,
            y,
            row,
        );
    };

    #[cfg(not(feature = "parallel"))]
    if channels == 3 {
        affine_nearest_fixed_rgb(
            source,
            source_width,
            source_height,
            destination_width,
            destination_height,
            origin_x,
            origin_y,
            step_x_x,
            step_y_x,
            step_x_y,
            step_y_y,
            fill,
            output,
            &process_row,
        );
        return;
    }

    #[cfg(not(feature = "parallel"))]
    if channels == 4 {
        affine_nearest_fixed_rgba(
            source,
            source_width,
            source_height,
            destination_width,
            destination_height,
            origin_x,
            origin_y,
            step_x_x,
            step_y_x,
            step_x_y,
            step_y_y,
            fill,
            output,
            &process_row,
        );
        return;
    }

    #[cfg(feature = "parallel")]
    const PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
    let output_stride = destination_width as usize * channels;
    #[cfg(feature = "parallel")]
    if (destination_width as usize).saturating_mul(destination_height as usize)
        >= PARALLEL_PIXEL_THRESHOLD
    {
        crate::par_rows_mut!(
            output,
            output_stride,
            destination_height as usize,
            |_row_start, _row_end, y, row| {
                process_row(y, row);
            }
        );
    } else {
        for (y, row) in output
            .chunks_exact_mut(output_stride)
            .take(destination_height as usize)
            .enumerate()
        {
            process_row(y as u32, row);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in output
        .chunks_exact_mut(output_stride)
        .take(destination_height as usize)
        .enumerate()
    {
        process_row(y as u32, row);
    }
}

#[inline(always)]
fn affine_nearest_fixed_coordinates(affine: [f64; 6]) -> (i64, i64, i64, i64, i64, i64) {
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let [a, b, c, d, e, f] = affine;
    let step_x_x = fixed(a);
    let step_y_x = fixed(b);
    let step_x_y = fixed(d);
    let step_y_y = fixed(e);
    let origin_x = fixed(c + a * 0.5 + b * 0.5);
    let origin_y = fixed(f + d * 0.5 + e * 0.5);
    (step_x_x, step_y_x, step_x_y, step_y_y, origin_x, origin_y)
}

#[inline(always)]
fn affine_nearest_fixed_process_row(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    channels: usize,
    origin_x: i64,
    origin_y: i64,
    step_x_x: i64,
    step_y_x: i64,
    step_x_y: i64,
    step_y_y: i64,
    fill: (u8, u8, u8, u8),
    y: u32,
    row: &mut [u8],
) {
    let mut source_x = origin_x + i64::from(y) * step_y_x;
    let mut source_y = origin_y + i64::from(y) * step_y_y;
    for x in 0..destination_width {
        let input_x = source_x >> 16;
        let input_y = source_y >> 16;
        let output_index = x as usize * channels;
        if input_x >= 0
            && input_x < i64::from(source_width)
            && input_y >= 0
            && input_y < i64::from(source_height)
        {
            let input_index = (input_y as u32 * source_width + input_x as u32) as usize * channels;
            row[output_index..output_index + channels]
                .copy_from_slice(&source[input_index..input_index + channels]);
        } else {
            for channel in 0..channels.min(4) {
                let value = if channels == 2 && channel == 1 {
                    // LA/PA normalize their second sample as alpha in fill.3;
                    // fill.1 is the duplicated luma/index component.
                    fill.3
                } else {
                    match channel {
                        0 => fill.0,
                        1 => fill.1,
                        2 => fill.2,
                        _ => fill.3,
                    }
                };
                row[output_index + channel] = value;
            }
        }
        source_x += step_x_x;
        source_y += step_x_y;
    }
}

#[cfg(not(feature = "parallel"))]
#[inline(always)]
fn affine_nearest_fixed_rgb(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
    origin_x: i64,
    origin_y: i64,
    step_x_x: i64,
    step_y_x: i64,
    step_x_y: i64,
    step_y_y: i64,
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
    process_row: &impl Fn(u32, &mut [u8]),
) {
    let channels = 3;
    let fill_pixel = [fill.0, fill.1, fill.2];
    let output_stride = destination_width as usize * channels;
    for (y, row) in output
        .chunks_exact_mut(output_stride)
        .take(destination_height as usize)
        .enumerate()
    {
        let Some((start, end, mut source_x, mut source_y)) = nearest_positive_step_in_bounds_span(
            y as u32,
            destination_width,
            source_width,
            source_height,
            origin_x,
            origin_y,
            step_x_x,
            step_y_x,
            step_x_y,
            step_y_y,
        ) else {
            process_row(y as u32, row);
            continue;
        };

        let start_byte = start * channels;
        let end_byte = end * channels;
        fill_rgb_pixels(&mut row[..start_byte], fill_pixel);
        fill_rgb_pixels(&mut row[end_byte..], fill_pixel);

        let source_width = source_width as usize;
        let span_length = end - start;
        let mut offset = 0;
        let mut contiguous_copies = true;
        while offset < span_length {
            let Some(run_length) = nearest_contiguous_rgb_run_len(
                source_x,
                source_y,
                step_x_x,
                step_x_y,
                span_length - offset,
            ) else {
                contiguous_copies = false;
                break;
            };
            // Short copies do not repay the run-bound calculation. Keep
            // the original fixed-point per-pixel path for those rows.
            if run_length < 4 {
                contiguous_copies = false;
                break;
            }

            let input_x = (source_x >> 16) as usize;
            let input_y = (source_y >> 16) as usize;
            let source_start = (input_y * source_width + input_x) * 3;
            let destination_start = start_byte + offset * 3;
            let copy_bytes = run_length * 3;
            row[destination_start..destination_start + copy_bytes]
                .copy_from_slice(&source[source_start..source_start + copy_bytes]);

            offset += run_length;
            if offset < span_length {
                let run = run_length as i128;
                let Some(next_x) =
                    i64::try_from(i128::from(source_x) + i128::from(step_x_x) * run).ok()
                else {
                    contiguous_copies = false;
                    break;
                };
                let Some(next_y) =
                    i64::try_from(i128::from(source_y) + i128::from(step_x_y) * run).ok()
                else {
                    contiguous_copies = false;
                    break;
                };
                source_x = next_x;
                source_y = next_y;
            }
        }
        if !contiguous_copies {
            process_row(y as u32, row);
        }
    }
}

#[cfg(not(feature = "parallel"))]
#[inline(always)]
fn affine_nearest_fixed_rgba(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
    origin_x: i64,
    origin_y: i64,
    step_x_x: i64,
    step_y_x: i64,
    step_x_y: i64,
    step_y_y: i64,
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
    process_row: &impl Fn(u32, &mut [u8]),
) {
    let channels = 4;
    let fill_pixel = [fill.0, fill.1, fill.2, fill.3];
    let output_stride = destination_width as usize * channels;
    for (y, row) in output
        .chunks_exact_mut(output_stride)
        .take(destination_height as usize)
        .enumerate()
    {
        let Some((start, end, mut source_x, mut source_y)) = nearest_positive_step_in_bounds_span(
            y as u32,
            destination_width,
            source_width,
            source_height,
            origin_x,
            origin_y,
            step_x_x,
            step_y_x,
            step_x_y,
            step_y_y,
        ) else {
            process_row(y as u32, row);
            continue;
        };

        let start_byte = start * channels;
        let end_byte = end * channels;
        fill_rgba_pixels(&mut row[..start_byte], fill_pixel);
        fill_rgba_pixels(&mut row[end_byte..], fill_pixel);

        let source_width = source_width as usize;
        let span_length = end - start;
        let mut pixels = row[start_byte..end_byte].chunks_exact_mut(16);
        let group_count = span_length / 4;
        let tail_count = span_length % 4;
        for (group_index, group) in pixels.by_ref().enumerate() {
            let input_x = (source_x >> 16) as usize;
            let input_y = (source_y >> 16) as usize;
            let input_index = (input_y * source_width + input_x) * 4;
            group[0..4].copy_from_slice(&source[input_index..input_index + 4]);
            source_x += step_x_x;
            source_y += step_x_y;

            let input_x = (source_x >> 16) as usize;
            let input_y = (source_y >> 16) as usize;
            let input_index = (input_y * source_width + input_x) * 4;
            group[4..8].copy_from_slice(&source[input_index..input_index + 4]);
            source_x += step_x_x;
            source_y += step_x_y;

            let input_x = (source_x >> 16) as usize;
            let input_y = (source_y >> 16) as usize;
            let input_index = (input_y * source_width + input_x) * 4;
            group[8..12].copy_from_slice(&source[input_index..input_index + 4]);
            source_x += step_x_x;
            source_y += step_x_y;

            let input_x = (source_x >> 16) as usize;
            let input_y = (source_y >> 16) as usize;
            let input_index = (input_y * source_width + input_x) * 4;
            group[12..16].copy_from_slice(&source[input_index..input_index + 4]);
            if group_index + 1 < group_count || tail_count > 0 {
                source_x += step_x_x;
                source_y += step_x_y;
            }
        }
        for (offset, pixel) in pixels.into_remainder().chunks_exact_mut(4).enumerate() {
            let input_x = (source_x >> 16) as usize;
            let input_y = (source_y >> 16) as usize;
            let input_index = (input_y * source_width + input_x) * 4;
            pixel.copy_from_slice(&source[input_index..input_index + 4]);
            if offset + 1 < tail_count {
                source_x += step_x_x;
                source_y += step_x_y;
            }
        }
    }
}

/// Rotate an image by an arbitrary angle, working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling.
pub(crate) fn rotate_arbitrary_generic(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    filter: ResampleFilter,
    nearest: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = img.dimensions();
    let sw = w as f64;
    let sh = h as f64;
    // Pillow builds the reverse affine transform (destination -> source),
    // rounding the trigonometric coefficients to 15 decimal places before
    // calculating the expanded canvas.
    // Pillow's affine coefficients map destination pixels back into the
    // source image. The inverse mapping uses the negative angle; using the
    // forward sign mirrors the exposed fill region for arbitrary angles.
    let rad = -angle.to_radians();
    let aff_a = crate::ops::rotate::round_rotate_coefficient(rad.cos());
    let aff_b = crate::ops::rotate::round_rotate_coefficient(rad.sin());
    let aff_d = crate::ops::rotate::round_rotate_coefficient(-rad.sin());
    let aff_e = aff_a;
    // Pillow's Image.rotate composes post-translation into the reverse affine
    // matrix before calculating expand bounds; applying it after sampling
    // changes both the canvas size and the selected source pixels.
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let mut aff_c =
        aff_a * (-center_x - translate_x) + aff_b * (-center_y - translate_y) + center_x;
    let mut aff_f =
        aff_d * (-center_x - translate_x) + aff_e * (-center_y - translate_y) + center_y;
    // Python's rotate() constructs the matrix and expanded bounds with
    // ordinary Python float operations. Keep that order for the matrix
    // translation/bounds pass; the native Geometry.c callback uses the
    // target's fused multiply-add grouping only when sampling pixels.
    let transform =
        |x: f64, y: f64, c: f64, f: f64| (aff_a * x + aff_b * y + c, aff_d * x + aff_e * y + f);
    // Pillow rounds each outer edge independently. This differs from taking
    // ceil(max - min) whenever the transformed minimum is fractional.
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(cx, cy) in &corners {
        let (rx, ry) = transform(cx, cy, aff_c, aff_f);
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }
    let (dw, dh) = if expand {
        (
            (max_x.ceil() - min_x.floor()) as u32,
            (max_y.ceil() - min_y.floor()) as u32,
        )
    } else {
        (w, h)
    };

    if expand {
        let shift_x = -(dw as f64 - sw) / 2.0;
        let shift_y = -(dh as f64 - sh) / 2.0;
        (aff_c, aff_f) = transform(shift_x, shift_y, aff_c, aff_f);
    }

    if let DynamicImage::ImageLuma16(source) = img {
        return rotate_arbitrary_luma16(
            source,
            (w, h),
            (dw, dh),
            angle,
            expand,
            fill,
            filter,
            nearest,
            explicit_mode,
            center,
            translate,
            [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
        );
    }

    let raw = img.as_bytes();
    let fill_color = fill.unwrap_or((0, 0, 0, 0));

    let mut out = CheckedDims::new(dw, dh, channels as u8)?.alloc_buffer();

    if nearest {
        affine_nearest_fixed(
            raw,
            (w, h),
            (dw, dh),
            channels,
            [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
            fill_color,
            &mut out,
        );
    } else if matches!(explicit_mode, Some("F") | Some("I")) && channels == 4 {
        rotate_arbitrary_scalar(
            raw,
            (w, h),
            (dw, dh),
            channels,
            [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
            fill_color,
            filter,
            explicit_mode == Some("F"),
            &mut out,
        );
    } else {
        rotate_arbitrary_byte_filter(
            raw,
            (w, h),
            (dw, dh),
            channels,
            [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
            filter,
            fill_color,
            &mut out,
        );
    }

    raw_bytes_to_image(dw, dh, out, channels)
}

fn rotate_arbitrary_luma16(
    source: &crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
    source_size: (u32, u32),
    destination_size: (u32, u32),
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    filter: ResampleFilter,
    nearest: bool,
    explicit_mode: Option<&str>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    affine: [f64; 6],
) -> Result<DynamicImage, PilError> {
    let (w, h) = source_size;
    let (dw, dh) = destination_size;
    let [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f] = affine;
    let sw = f64::from(w);
    let sh = f64::from(h);
    let fill_value = fill.map_or(0, |value| u16::from_le_bytes([value.0, value.1]));
    let mut output = vec![fill_value; dw as usize * dh as usize];
    if nearest {
        // SPECIAL modes use Geometry.c's generic mapping and copy both bytes.
        for y in 0..dh {
            for x in 0..dw {
                let x_center = f64::from(x) + 0.5;
                let y_center = f64::from(y) + 0.5;
                let sx = aff_a.mul_add(x_center, aff_b * y_center) + aff_c;
                let sy = aff_d.mul_add(x_center, aff_e * y_center) + aff_f;
                if sx >= 0.0 && sx < sw && sy >= 0.0 && sy < sh {
                    output[(y * dw + x) as usize] = source.get_pixel(sx as u32, sy as u32)[0];
                }
            }
        }
    } else {
        // Pillow's filtered I;16 path selects filter8: it samples the first
        // `width` bytes of each source row and writes only destination byte 0.
        let big_endian = explicit_mode == Some("I;16B")
            || (cfg!(target_endian = "big") && matches!(explicit_mode, Some("I;16" | "I;16N")));
        let encode = |value: u16| {
            if big_endian {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        let fill_bytes = encode(fill_value);
        let mut plane = Vec::with_capacity(w as usize * h as usize);
        for y in 0..h as usize {
            plane.extend(
                source.as_raw()[y * w as usize..(y + 1) * w as usize]
                    .iter()
                    .flat_map(|&value| encode(value))
                    .take(w as usize),
            );
        }
        let plane = crate::raster::GrayImage::from_raw(w, h, plane)
            .ok_or_else(|| PilError::InternalError("rotate I;16 source shape mismatch".into()))?;
        let sampled = rotate_arbitrary_generic(
            &DynamicImage::ImageLuma8(plane),
            angle,
            expand,
            fill.map(|_| (fill_bytes[0], 0, 0, 0)),
            filter,
            false,
            center,
            translate,
            Some("L"),
        )?;
        for (value, &byte) in output.iter_mut().zip(sampled.as_bytes()) {
            let bytes = [byte, fill_bytes[1]];
            *value = if big_endian {
                u16::from_be_bytes(bytes)
            } else {
                u16::from_le_bytes(bytes)
            };
        }
    }
    crate::raster::ImageBuffer::from_raw(dw, dh, output)
        .map(DynamicImage::ImageLuma16)
        .ok_or_else(|| PilError::InternalError("rotate I;16 result shape mismatch".into()))
}

#[inline(always)]
fn rotate_arbitrary_byte_filter(
    raw: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    channels: usize,
    affine: [f64; 6],
    filter: ResampleFilter,
    fill_color: (u8, u8, u8, u8),
    out: &mut [u8],
) {
    let (w, h) = source_size;
    let (dw, dh) = destination_size;
    let sw = w as f64;
    let sh = h as f64;
    let [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f] = affine;
    let sample_transform = |x: f64, y: f64| {
        (
            aff_a.mul_add(x, aff_b * y) + aff_c,
            aff_d.mul_add(x, aff_e * y) + aff_f,
        )
    };

    for dy in 0..dh {
        for dx in 0..dw {
            // Map the destination center, then subtract half a pixel for
            // Geometry.c's filtered source-corner coordinates.
            let (sx_rel, sy_rel) = sample_transform(dx as f64 + 0.5, dy as f64 + 0.5);
            let (sx_rel, sy_rel) = (sx_rel - 0.5, sy_rel - 0.5);
            let out_idx = (dy * dw + dx) as usize * channels;
            let in_filter_support =
                sx_rel >= -0.5 && sx_rel < sw - 0.5 && sy_rel >= -0.5 && sy_rel < sh - 0.5;
            if in_filter_support {
                let sx_rel = if matches!(filter, ResampleFilter::Bicubic) {
                    sx_rel
                } else {
                    sx_rel.clamp(0.0, sw - 1.0)
                };
                let sy_rel = if matches!(filter, ResampleFilter::Bicubic) {
                    sy_rel
                } else {
                    sy_rel.clamp(0.0, sh - 1.0)
                };
                let bilinear_x = sx_rel.floor() as i64;
                let bilinear_y = sy_rel.floor() as i64;
                let bicubic_x = bilinear_x - 1;
                let bicubic_y = bilinear_y - 1;
                let fx = sx_rel - bilinear_x as f64;
                let fy = sy_rel - bilinear_y as f64;
                for c in 0..channels {
                    let value = match filter {
                        ResampleFilter::Bicubic => {
                            let cubic_fx = sx_rel - (bicubic_x + 1) as f64;
                            let cubic_fy = sy_rel - (bicubic_y + 1) as f64;
                            let mut rows = [0.0f64; 4];
                            for (row, output) in rows.iter_mut().enumerate() {
                                let yy = bicubic_y + row as i64;
                                let cy = yy.clamp(0, i64::from(h - 1)) as usize;
                                let samples = [
                                    raw[(cy * w as usize
                                        + bicubic_x.clamp(0, i64::from(w - 1)) as usize)
                                        * channels
                                        + c] as f64,
                                    raw[(cy * w as usize
                                        + (bicubic_x + 1).clamp(0, i64::from(w - 1)) as usize)
                                        * channels
                                        + c] as f64,
                                    raw[(cy * w as usize
                                        + (bicubic_x + 2).clamp(0, i64::from(w - 1)) as usize)
                                        * channels
                                        + c] as f64,
                                    raw[(cy * w as usize
                                        + (bicubic_x + 3).clamp(0, i64::from(w - 1)) as usize)
                                        * channels
                                        + c] as f64,
                                ];
                                *output = rotate_cubic_horizontal_f64(samples, cubic_fx);
                            }
                            rotate_cubic_vertical_f64(rows, cubic_fy)
                        }
                        _ => {
                            let sx = bilinear_x.clamp(0, i64::from(w - 1)) as u32;
                            let sy = bilinear_y.clamp(0, i64::from(h - 1)) as u32;
                            let sx1 = (bilinear_x + 1).clamp(0, i64::from(w - 1)) as u32;
                            let sy1 = (bilinear_y + 1).clamp(0, i64::from(h - 1)) as u32;
                            let p00 = raw[(sy * w + sx) as usize * channels + c] as f64;
                            let p10 = raw[(sy * w + sx1) as usize * channels + c] as f64;
                            let p01 = raw[(sy1 * w + sx) as usize * channels + c] as f64;
                            let p11 = raw[(sy1 * w + sx1) as usize * channels + c] as f64;
                            let top = (p10 - p00).mul_add(fx, p00);
                            let bottom = (p11 - p01).mul_add(fx, p01);
                            (bottom - top).mul_add(fy, top)
                        }
                    };
                    out[out_idx + c] = if matches!(filter, ResampleFilter::Bicubic) {
                        value.clamp(0.0, 255.0) as u8
                    } else {
                        value as u8
                    };
                }
            } else {
                for c in 0..channels.min(4) {
                    out[out_idx + c] = match c {
                        0 => fill_color.0,
                        1 => fill_color.1,
                        2 => fill_color.2,
                        _ => fill_color.3,
                    };
                }
            }
        }
    }
}
