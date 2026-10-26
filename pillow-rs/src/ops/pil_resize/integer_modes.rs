#[cfg(not(feature = "parallel"))]
use super::alpha::{premultiply_channel, unpremultiply_channel};
#[cfg(not(feature = "parallel"))]
use super::boxed::raw_to_dynamic_owned;
#[cfg(not(feature = "parallel"))]
use super::coefficients::{fixed_point_to_u8_i32, resize_u8_coefficients_fit_i32};
#[cfg(all(test, not(feature = "parallel")))]
use super::horizontal_passes::{horizontal_pass_rows, horizontal_pass_rows_alpha};
#[cfg(not(feature = "parallel"))]
use super::vertical_passes::{should_transpose_vertical, transpose_resize_intermediate};
#[cfg(all(test, not(feature = "parallel")))]
use super::vertical_passes::{vertical_pass_rows, vertical_pass_rows_alpha};
#[cfg(not(feature = "parallel"))]
use super::*;
#[cfg(not(feature = "parallel"))]
use crate::pipeline::ResampleFilter;
#[cfg(not(feature = "parallel"))]
use crate::raster::DynamicImage;

#[cfg(not(feature = "parallel"))]
fn resize_coefficients_fit_source(coeffs: &FilterCoeffs, source_extent: usize) -> bool {
    if coeffs.xmin.len() != coeffs.count.len() || coeffs.xmin.len() != coeffs.offsets.len() {
        return false;
    }
    coeffs
        .xmin
        .iter()
        .zip(&coeffs.count)
        .all(|(&first, &count)| {
            usize::try_from(first)
                .ok()
                .and_then(|first| first.checked_add(count))
                .is_some_and(|end| end <= source_extent)
        })
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_luma_i32(source_row: &[u8], coeffs: &FilterCoeffs, output_row: &mut [u8]) {
    for (output_x, output) in output_row.iter_mut().enumerate() {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let mut sum = 0i32;
        let source_x = coeffs.xmin[output_x] as usize;
        if weights.len() == 4 {
            sum = i32::from(source_row[source_x]) * weights[0] as i32;
            sum += i32::from(source_row[source_x + 1]) * weights[1] as i32;
            sum += i32::from(source_row[source_x + 2]) * weights[2] as i32;
            sum += i32::from(source_row[source_x + 3]) * weights[3] as i32;
        } else if weights.len() == 5 {
            sum = i32::from(source_row[source_x]) * weights[0] as i32;
            sum += i32::from(source_row[source_x + 1]) * weights[1] as i32;
            sum += i32::from(source_row[source_x + 2]) * weights[2] as i32;
            sum += i32::from(source_row[source_x + 3]) * weights[3] as i32;
            sum += i32::from(source_row[source_x + 4]) * weights[4] as i32;
        } else if weights.len() == 6 {
            sum = i32::from(source_row[source_x]) * weights[0] as i32;
            sum += i32::from(source_row[source_x + 1]) * weights[1] as i32;
            sum += i32::from(source_row[source_x + 2]) * weights[2] as i32;
            sum += i32::from(source_row[source_x + 3]) * weights[3] as i32;
            sum += i32::from(source_row[source_x + 4]) * weights[4] as i32;
            sum += i32::from(source_row[source_x + 5]) * weights[5] as i32;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                sum += i32::from(source_row[source_x + tap]) * weight as i32;
            }
        }
        *output = fixed_point_to_u8_i32(sum);
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_luma_i32(
    intermediate: &[u8],
    source_height: usize,
    output_width: usize,
    coeffs: &FilterCoeffs,
    transposed: bool,
    output: &mut [u8],
) {
    for (output_y, output_row) in output.chunks_exact_mut(output_width).enumerate() {
        let weights = coeffs.weights_for(output_y);
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let source_y = coeffs.xmin[output_y] as usize;
        if transposed {
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let source_start = output_x * source_height + source_y;
                let mut sum = i32::from(intermediate[source_start]) * weights[0] as i32;
                for (tap, &weight) in weights.iter().enumerate().skip(1) {
                    sum += i32::from(intermediate[source_start + tap]) * weight as i32;
                }
                *pixel = fixed_point_to_u8_i32(sum);
            }
        } else if weights.len() == 4 {
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let row0 = source_y * output_width;
            let row1 = row0 + output_width;
            let row2 = row1 + output_width;
            let row3 = row2 + output_width;
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let mut sum = i32::from(intermediate[row0 + output_x]) * weight0;
                sum += i32::from(intermediate[row1 + output_x]) * weight1;
                sum += i32::from(intermediate[row2 + output_x]) * weight2;
                sum += i32::from(intermediate[row3 + output_x]) * weight3;
                *pixel = fixed_point_to_u8_i32(sum);
            }
        } else if weights.len() == 5 {
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let weight4 = weights[4] as i32;
            let row0 = source_y * output_width;
            let row1 = row0 + output_width;
            let row2 = row1 + output_width;
            let row3 = row2 + output_width;
            let row4 = row3 + output_width;
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let mut sum = i32::from(intermediate[row0 + output_x]) * weight0;
                sum += i32::from(intermediate[row1 + output_x]) * weight1;
                sum += i32::from(intermediate[row2 + output_x]) * weight2;
                sum += i32::from(intermediate[row3 + output_x]) * weight3;
                sum += i32::from(intermediate[row4 + output_x]) * weight4;
                *pixel = fixed_point_to_u8_i32(sum);
            }
        } else if weights.len() == 6 {
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let weight4 = weights[4] as i32;
            let weight5 = weights[5] as i32;
            let row0 = source_y * output_width;
            let row1 = row0 + output_width;
            let row2 = row1 + output_width;
            let row3 = row2 + output_width;
            let row4 = row3 + output_width;
            let row5 = row4 + output_width;
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let mut sum = i32::from(intermediate[row0 + output_x]) * weight0;
                sum += i32::from(intermediate[row1 + output_x]) * weight1;
                sum += i32::from(intermediate[row2 + output_x]) * weight2;
                sum += i32::from(intermediate[row3 + output_x]) * weight3;
                sum += i32::from(intermediate[row4 + output_x]) * weight4;
                sum += i32::from(intermediate[row5 + output_x]) * weight5;
                *pixel = fixed_point_to_u8_i32(sum);
            }
        } else {
            for (output_x, pixel) in output_row.iter_mut().enumerate() {
                let source_start = source_y * output_width + output_x;
                let mut sum = i32::from(intermediate[source_start]) * weights[0] as i32;
                for (tap, &weight) in weights.iter().enumerate().skip(1) {
                    sum +=
                        i32::from(intermediate[source_start + tap * output_width]) * weight as i32;
                }
                *pixel = fixed_point_to_u8_i32(sum);
            }
        }
    }
}

#[cfg(not(feature = "parallel"))]
pub(super) fn pil_resize_luma_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    let output_len = output_height.checked_mul(output_width)?;
    let mut output = vec![0; output_len];
    pil_resize_luma_i32_into(
        img,
        output_width,
        output_height,
        horizontal,
        vertical,
        &mut output,
    )?;
    Some(output)
}

#[cfg(not(feature = "parallel"))]
fn pil_resize_luma_i32_into(
    img: &DynamicImage,
    output_width: usize,
    output_height: usize,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
    output: &mut [u8],
) -> Option<()> {
    let DynamicImage::ImageLuma8(source) = img else {
        return None;
    };
    let source_width = usize::try_from(source.width()).ok()?;
    let source_height = usize::try_from(source.height()).ok()?;
    if horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !resize_coefficients_fit_source(horizontal, source_width)
        || !resize_coefficients_fit_source(vertical, source_height)
        || !resize_u8_coefficients_fit_i32(horizontal)
        || !resize_u8_coefficients_fit_i32(vertical)
    {
        return None;
    }
    let intermediate_len = source_height.checked_mul(output_width)?;
    let output_len = output_height.checked_mul(output_width)?;
    if source.as_raw().len() != source_width.checked_mul(source_height)?
        || output.len() != output_len
    {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_width)?;
        let output_start = source_y.checked_mul(output_width)?;
        horizontal_pass_luma_i32(
            &source.as_raw()[source_start..source_start + source_width],
            horizontal,
            &mut intermediate[output_start..output_start + output_width],
        );
    }

    vertical_pass_luma_i32(
        &intermediate,
        source_height,
        output_width,
        vertical,
        false,
        output,
    );
    Some(())
}

/// Resize native L bytes directly into a caller-owned full-width output
/// window. ImageOps.pad uses this to avoid materializing the contained image
/// before copying its rows into the final canvas.
#[cfg(not(feature = "parallel"))]
pub(crate) fn pil_resize_luma_i32_into_window(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> bool {
    let DynamicImage::ImageLuma8(_) = img else {
        return false;
    };
    if output_width == 0 || output_height == 0 || img.width() == 0 || img.height() == 0 {
        return false;
    }
    let horizontal = precompute_coeffs(output_width, img.width(), filter);
    let vertical = precompute_coeffs(output_height, img.height(), filter);
    pil_resize_luma_i32_into(
        img,
        output_width as usize,
        output_height as usize,
        &horizontal,
        &vertical,
        output,
    )
    .is_some()
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_la_i32(source_row: &[u8], coeffs: &FilterCoeffs, output_row: &mut [u8]) {
    for (output_x, output_pixel) in output_row.chunks_exact_mut(2).enumerate() {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let source_start = coeffs.xmin[output_x] as usize * 2;
        let (mut luma_sum, mut alpha_sum) = (0i32, 0i32);
        if weights.len() == 4 {
            luma_sum = i32::from(source_row[source_start]) * weights[0] as i32;
            alpha_sum = i32::from(source_row[source_start + 1]) * weights[0] as i32;
            luma_sum += i32::from(source_row[source_start + 2]) * weights[1] as i32;
            alpha_sum += i32::from(source_row[source_start + 3]) * weights[1] as i32;
            luma_sum += i32::from(source_row[source_start + 4]) * weights[2] as i32;
            alpha_sum += i32::from(source_row[source_start + 5]) * weights[2] as i32;
            luma_sum += i32::from(source_row[source_start + 6]) * weights[3] as i32;
            alpha_sum += i32::from(source_row[source_start + 7]) * weights[3] as i32;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                let sample = source_start + tap * 2;
                let weight = weight as i32;
                luma_sum += i32::from(source_row[sample]) * weight;
                alpha_sum += i32::from(source_row[sample + 1]) * weight;
            }
        }
        output_pixel[0] = fixed_point_to_u8_i32(luma_sum);
        output_pixel[1] = fixed_point_to_u8_i32(alpha_sum);
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_la_i32(
    intermediate: &[u8],
    output_width: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width * 2;
    for (output_y, output_row) in output.chunks_exact_mut(output_stride).enumerate() {
        let weights = coeffs.weights_for(output_y);
        if weights.is_empty() {
            continue;
        }
        let source_start = coeffs.xmin[output_y] as usize * output_stride;
        if weights.len() == 4 {
            let row1 = source_start + output_stride;
            let row2 = row1 + output_stride;
            let row3 = row2 + output_stride;
            let (weight0, weight1, weight2, weight3) = (
                weights[0] as i32,
                weights[1] as i32,
                weights[2] as i32,
                weights[3] as i32,
            );
            for (output_x, output_pixel) in output_row.chunks_exact_mut(2).enumerate() {
                let source_pixel = output_x * 2;
                let mut luma_sum = i32::from(intermediate[source_start + source_pixel]) * weight0;
                let mut alpha_sum =
                    i32::from(intermediate[source_start + source_pixel + 1]) * weight0;
                luma_sum += i32::from(intermediate[row1 + source_pixel]) * weight1;
                alpha_sum += i32::from(intermediate[row1 + source_pixel + 1]) * weight1;
                luma_sum += i32::from(intermediate[row2 + source_pixel]) * weight2;
                alpha_sum += i32::from(intermediate[row2 + source_pixel + 1]) * weight2;
                luma_sum += i32::from(intermediate[row3 + source_pixel]) * weight3;
                alpha_sum += i32::from(intermediate[row3 + source_pixel + 1]) * weight3;
                let alpha = fixed_point_to_u8_i32(alpha_sum);
                output_pixel[0] = unpremultiply_channel(fixed_point_to_u8_i32(luma_sum), alpha);
                output_pixel[1] = alpha;
            }
        } else {
            for (output_x, output_pixel) in output_row.chunks_exact_mut(2).enumerate() {
                let source_pixel = output_x * 2;
                let (mut luma_sum, mut alpha_sum) = (0i32, 0i32);
                for (tap, &weight) in weights.iter().enumerate() {
                    let sample = source_start + tap * output_stride + source_pixel;
                    let weight = weight as i32;
                    luma_sum += i32::from(intermediate[sample]) * weight;
                    alpha_sum += i32::from(intermediate[sample + 1]) * weight;
                }
                let alpha = fixed_point_to_u8_i32(alpha_sum);
                output_pixel[0] = unpremultiply_channel(fixed_point_to_u8_i32(luma_sum), alpha);
                output_pixel[1] = alpha;
            }
        }
    }
}

#[cfg(not(feature = "parallel"))]
pub(super) fn pil_resize_la_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let DynamicImage::ImageLumaA8(source) = img else {
        return None;
    };
    let source_width = usize::try_from(source.width()).ok()?;
    let source_height = usize::try_from(source.height()).ok()?;
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    if horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !resize_coefficients_fit_source(horizontal, source_width)
        || !resize_coefficients_fit_source(vertical, source_height)
        || !resize_u8_coefficients_fit_i32(horizontal)
        || !resize_u8_coefficients_fit_i32(vertical)
    {
        return None;
    }

    let source_stride = source_width.checked_mul(2)?;
    let source_len = source_height.checked_mul(source_stride)?;
    let output_stride = output_width.checked_mul(2)?;
    let intermediate_len = source_height.checked_mul(output_stride)?;
    let output_len = output_height.checked_mul(output_stride)?;
    if source.as_raw().len() != source_len {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    let mut premultiplied_row = vec![0; source_stride];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let output_start = source_y.checked_mul(output_stride)?;
        for (source_pixel, output_pixel) in source.as_raw()
            [source_start..source_start + source_stride]
            .chunks_exact(2)
            .zip(premultiplied_row.chunks_exact_mut(2))
        {
            output_pixel[0] = premultiply_channel(source_pixel[0], source_pixel[1]);
            output_pixel[1] = source_pixel[1];
        }
        horizontal_pass_la_i32(
            &premultiplied_row,
            horizontal,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }

    let mut output = vec![0; output_len];
    vertical_pass_la_i32(&intermediate, output_width, vertical, &mut output);
    Some(output)
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_rgba_i32(source_row: &[u8], coeffs: &FilterCoeffs, output_row: &mut [u8]) {
    for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let mut sums = [0i32; 4];
        let mut source_start = coeffs.xmin[output_x] as usize * 4;
        for &weight in weights {
            let weight = weight as i32;
            for (channel, sum) in sums.iter_mut().enumerate() {
                *sum += i32::from(source_row[source_start + channel]) * weight;
            }
            source_start += 4;
        }
        for (output, sum) in output_pixel.iter_mut().zip(sums) {
            *output = fixed_point_to_u8_i32(sum);
        }
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_rgba_i32(
    intermediate: &[u8],
    output_width: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width * 4;
    for (output_y, output_row) in output.chunks_exact_mut(output_stride).enumerate() {
        let weights = coeffs.weights_for(output_y);
        if weights.is_empty() {
            continue;
        }
        let first_source_row = coeffs.xmin[output_y] as usize * output_stride;
        for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
            let source_start = first_source_row + output_x * 4;
            let mut sums = [0i32; 4];
            for (tap, &weight) in weights.iter().enumerate() {
                let pixel_start = source_start + tap * output_stride;
                let weight = weight as i32;
                for (channel, sum) in sums.iter_mut().enumerate() {
                    *sum += i32::from(intermediate[pixel_start + channel]) * weight;
                }
            }
            let alpha = fixed_point_to_u8_i32(sums[3]);
            output_pixel[0] = unpremultiply_channel(fixed_point_to_u8_i32(sums[0]), alpha);
            output_pixel[1] = unpremultiply_channel(fixed_point_to_u8_i32(sums[1]), alpha);
            output_pixel[2] = unpremultiply_channel(fixed_point_to_u8_i32(sums[2]), alpha);
            output_pixel[3] = alpha;
        }
    }
}

#[cfg(not(feature = "parallel"))]
pub(super) fn pil_resize_rgba_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let source_width = usize::try_from(source.width()).ok()?;
    let source_height = usize::try_from(source.height()).ok()?;
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    if horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !resize_coefficients_fit_source(horizontal, source_width)
        || !resize_coefficients_fit_source(vertical, source_height)
        || !resize_u8_coefficients_fit_i32(horizontal)
        || !resize_u8_coefficients_fit_i32(vertical)
    {
        return None;
    }

    let source_stride = source_width.checked_mul(4)?;
    let output_stride = output_width.checked_mul(4)?;
    let intermediate_len = source_height.checked_mul(output_stride)?;
    let output_len = output_height.checked_mul(output_stride)?;
    if source.as_raw().len() != source_height.checked_mul(source_stride)? {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    let mut premultiplied_row = vec![0; source_stride];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let output_start = source_y.checked_mul(output_stride)?;
        for (source_pixel, output_pixel) in source.as_raw()
            [source_start..source_start + source_stride]
            .chunks_exact(4)
            .zip(premultiplied_row.chunks_exact_mut(4))
        {
            let alpha = source_pixel[3];
            output_pixel[0] = premultiply_channel(source_pixel[0], alpha);
            output_pixel[1] = premultiply_channel(source_pixel[1], alpha);
            output_pixel[2] = premultiply_channel(source_pixel[2], alpha);
            output_pixel[3] = alpha;
        }
        horizontal_pass_rgba_i32(
            &premultiplied_row,
            horizontal,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }

    let mut output = vec![0; output_len];
    vertical_pass_rgba_i32(&intermediate, output_width, vertical, &mut output);
    Some(output)
}

#[cfg(not(feature = "parallel"))]
mod cmyk;
#[cfg(not(feature = "parallel"))]
mod hsv;
#[cfg(not(feature = "parallel"))]
pub(super) use cmyk::pil_resize_cmyk_i32;
#[cfg(not(feature = "parallel"))]
pub(super) use hsv::pil_resize_hsv_i32;

#[cfg(all(test, not(feature = "parallel")))]
#[path = "integer_modes_tests.rs"]
mod tests;
