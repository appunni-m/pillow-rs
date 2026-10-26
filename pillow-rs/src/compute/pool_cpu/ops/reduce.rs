//! CPU image reduction operation and mode-specialized kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::{DynamicImage, GenericImageView};

use super::thumbnail::{reduce_f_thumbnail, reduce_i_thumbnail};

/// Reduce native RGB blocks without the generic per-channel mode checks.
///
/// Keep Pillow's 24-bit reciprocal-and-amend rounding unchanged. Interior,
/// right-edge, bottom-edge, and corner blocks each retain their own divisor.
pub(super) fn execute_reduce_rgb(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    execute_reduce_rgb_with_sparse_rows(img, x_factor, y_factor, explicit_mode)
        .map(|reduced| reduced.map(|(image, _)| image))
}

fn try_reduce_sparse_2x2(
    img: &DynamicImage,
    factors: (u32, u32),
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> Result<Option<(DynamicImage, Option<Vec<bool>>)>, PilError> {
    let ((fx, fy), (width, height), (new_width, new_height)) =
        (factors, source_dimensions, output_dimensions);
    if fx != 2 || fy != 2 || width % 2 != 0 || height % 2 != 0 {
        return Ok(None);
    }
    Ok(
        execute_reduce_rgb_sparse_2x2_with_rows(img, new_width, new_height)?
            .map(|(image, nonzero_rows)| (image, Some(nonzero_rows))),
    )
}

/// Reduce RGB while retaining exact active rows when the sparse 2×2 path is
/// selected. Thumbnail can use that proof to skip zero rows in both resample
/// passes without rescanning the reduced image.
pub(super) fn execute_reduce_rgb_with_sparse_rows(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<Option<(DynamicImage, Option<Vec<bool>>)>, PilError> {
    if explicit_mode != Some("RGB") || !matches!(img, DynamicImage::ImageRgb8(_)) {
        return Ok(None);
    }

    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let Some(full_divider) = fx.checked_mul(fy) else {
        return Ok(None);
    };
    // Keep both the channel sums plus amend and their fixed-point multiply
    // within u32 for the hot path. Larger factors retain the generic u64 code.
    if full_divider > u32::MAX / 256 {
        return Ok(None);
    }
    let (width, height) = img.dimensions();
    let new_width = width.div_ceil(fx);
    let new_height = height.div_ceil(fy);
    let source_stride = (width as usize).checked_mul(3);
    let expected_source_len = source_stride.and_then(|stride| stride.checked_mul(height as usize));
    let source = img.as_bytes();
    if expected_source_len != Some(source.len()) {
        return Ok(None);
    }

    if let Some(reduced) =
        try_reduce_sparse_2x2(img, (fx, fy), (width, height), (new_width, new_height))?
    {
        return Ok(Some(reduced));
    }

    let mut output = CheckedDims::new(new_width, new_height, 3)?.alloc_buffer();
    if new_width == 0 || new_height == 0 {
        return raw_bytes_to_image(new_width, new_height, output, 3)
            .map(|image| Some((image, None)));
    }

    let division_multiplier = |divider: u32| -> u32 {
        // Pillow's division_UINT32 reciprocal: 2^32 / (256 * divider).
        ((1u128 << 32) / (u128::from(divider) * 256)) as u32
    };
    let main_width = width / fx;
    let main_height = height / fy;
    let right_width = width % fx;
    let bottom_height = height % fy;
    let full_multiplier = division_multiplier(full_divider);
    let full_amend = full_divider / 2;
    let right_divider = right_width * fy;
    let right_multiplier = division_multiplier(right_divider.max(1));
    let right_amend = right_divider / 2;
    let bottom_divider = fx * bottom_height;
    let bottom_multiplier = division_multiplier(bottom_divider.max(1));
    let bottom_amend = bottom_divider / 2;
    let corner_divider = right_width * bottom_height;
    let corner_multiplier = division_multiplier(corner_divider.max(1));
    let corner_amend = corner_divider / 2;
    let source_stride = source_stride.expect("RGB source stride was checked");

    // Pillow's reducing-gap thumbnail path commonly reduces RGB by 2×2
    // before its final resample. Avoid four dynamic loops and per-block
    // coordinate work for this dense interior case; keep odd dimensions on
    // the general path so their partial right/bottom divisors are unchanged.
    if fx == 2 && fy == 2 && width % 2 == 0 && height % 2 == 0 {
        let output_stride = new_width as usize * 3;
        let process_row = |output_y: usize, output_row: &mut [u8]| {
            let source_top = output_y * 2 * source_stride;
            let source_bottom = source_top + source_stride;
            let top_row = &source[source_top..source_top + source_stride];
            let bottom_row = &source[source_bottom..source_bottom + source_stride];

            for (block_x, output_pixel) in output_row.chunks_exact_mut(3).enumerate() {
                let source_x = block_x * 6;
                let top_right = source_x + 3;
                for channel in 0..3 {
                    let sum = u32::from(top_row[source_x + channel])
                        + u32::from(top_row[top_right + channel])
                        + u32::from(bottom_row[source_x + channel])
                        + u32::from(bottom_row[top_right + channel]);
                    // For a 2×2 block Pillow's reciprocal is exactly
                    // `(sum + 2) / 4`; keep the rounding bias and replace the
                    // general per-channel reciprocal multiply with a shift.
                    #[expect(
                        clippy::arithmetic_side_effects,
                        reason = "four byte samples plus the Pillow rounding bias fit u32"
                    )]
                    let average = (sum + 2) >> 2;
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "the Pillow 2x2 average is bounded to one byte"
                    )]
                    {
                        output_pixel[channel] = average as u8;
                    }
                }
            }
        };

        #[cfg(feature = "parallel")]
        {
            const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
            let input_pixels = (width as usize).saturating_mul(height as usize);
            if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
                crate::par_rows_mut!(
                    &mut output,
                    output_stride,
                    new_height as usize,
                    |_row_start, _row_end, y, row| { process_row(y as usize, row) }
                );
            } else {
                for y in 0..new_height as usize {
                    let start = y * output_stride;
                    process_row(y, &mut output[start..start + output_stride]);
                }
            }
        }

        #[cfg(not(feature = "parallel"))]
        for y in 0..new_height as usize {
            let start = y * output_stride;
            process_row(y, &mut output[start..start + output_stride]);
        }

        return raw_bytes_to_image(new_width, new_height, output, 3)
            .map(|image| Some((image, None)));
    }

    let write_rgb_block = |row: &mut [u8],
                           output_x: usize,
                           source_x: usize,
                           source_y: usize,
                           block_width: usize,
                           block_height: usize,
                           multiplier: u32,
                           amend: u32| {
        let mut red_sum = 0u32;
        let mut green_sum = 0u32;
        let mut blue_sum = 0u32;
        let mut source_row_start = source_y * source_stride + source_x * 3;
        for _ in 0..block_height {
            let mut red_row_sum = 0u32;
            let mut green_row_sum = 0u32;
            let mut blue_row_sum = 0u32;
            let mut source_index = source_row_start;
            for _ in 0..block_width {
                red_row_sum += u32::from(source[source_index]);
                green_row_sum += u32::from(source[source_index + 1]);
                blue_row_sum += u32::from(source[source_index + 2]);
                source_index += 3;
            }
            red_sum += red_row_sum;
            green_sum += green_row_sum;
            blue_sum += blue_row_sum;
            source_row_start += source_stride;
        }

        let destination_index = output_x * 3;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
        )]
        let red = ((red_sum + amend) * multiplier) >> 24;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
        )]
        let green = ((green_sum + amend) * multiplier) >> 24;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
        )]
        let blue = ((blue_sum + amend) * multiplier) >> 24;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Pillow's RGB box average is an 8-bit result"
        )]
        let red = red as u8;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Pillow's RGB box average is an 8-bit result"
        )]
        let green = green as u8;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Pillow's RGB box average is an 8-bit result"
        )]
        let blue = blue as u8;
        row[destination_index] = red;
        row[destination_index + 1] = green;
        row[destination_index + 2] = blue;
    };

    let process_row = |y: u32, row: &mut [u8]| {
        let full_y = y < main_height;
        let source_y = if full_y {
            (y * fy) as usize
        } else {
            (main_height * fy) as usize
        };
        let block_height = if full_y {
            fy as usize
        } else {
            bottom_height as usize
        };
        let (interior_multiplier, interior_amend) = if full_y {
            (full_multiplier, full_amend)
        } else {
            (bottom_multiplier, bottom_amend)
        };
        for x in 0..main_width {
            write_rgb_block(
                row,
                x as usize,
                (x * fx) as usize,
                source_y,
                fx as usize,
                block_height,
                interior_multiplier,
                interior_amend,
            );
        }
        if right_width != 0 {
            let (edge_multiplier, edge_amend) = if full_y {
                (right_multiplier, right_amend)
            } else {
                (corner_multiplier, corner_amend)
            };
            write_rgb_block(
                row,
                main_width as usize,
                (main_width * fx) as usize,
                source_y,
                right_width as usize,
                block_height,
                edge_multiplier,
                edge_amend,
            );
        }
    };

    let output_stride = (new_width as usize) * 3;
    #[cfg(feature = "parallel")]
    {
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (width as usize).saturating_mul(height as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                &mut output,
                output_stride,
                new_height as usize,
                |_row_start, _row_end, y, row| {
                    process_row(y, row);
                }
            );
        } else {
            for y in 0..new_height {
                let start = y as usize * output_stride;
                process_row(y, &mut output[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_height {
        let start = y as usize * output_stride;
        process_row(y, &mut output[start..start + output_stride]);
    }

    raw_bytes_to_image(new_width, new_height, output, 3).map(|image| Some((image, None)))
}

/// Reduce large sparse RGB sources by updating only blocks with nonzero input.
/// Dense sources stop at the density bound and use the ordinary row kernel.
pub(super) fn execute_reduce_rgb_sparse_2x2_with_rows(
    img: &DynamicImage,
    new_width: u32,
    new_height: u32,
) -> Result<Option<(DynamicImage, Vec<bool>)>, PilError> {
    let (width, height) = img.dimensions();
    let source = img.as_bytes();
    let source_pixels = source.len() / 3;
    if source_pixels < 512 * 512 || width == 0 || height == 0 {
        return Ok(None);
    }
    // Scanning remains worthwhile only when very few input pixels contribute
    // nonzero samples. Stop early on denser images so they pay only a bounded
    // prefix scan before continuing through the established reducer.
    let maximum_nonzero_pixels = source_pixels / 1024;
    let mut nonzero_pixels = 0usize;
    let mut block_sums = std::collections::HashMap::<usize, [u16; 3]>::new();
    let mut accumulate_pixel = |source_index: usize, pixel: &[u8]| {
        if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
            return true;
        }
        nonzero_pixels += 1;
        if nonzero_pixels > maximum_nonzero_pixels {
            return false;
        }

        let source_x = source_index % width as usize;
        let source_y = source_index / width as usize;
        let block_index = (source_y / 2) * new_width as usize + source_x / 2;
        let sums = block_sums.entry(block_index).or_insert([0; 3]);
        sums[0] += u16::from(pixel[0]);
        sums[1] += u16::from(pixel[1]);
        sums[2] += u16::from(pixel[2]);
        true
    };

    // The common sparse case contains long runs of black RGB pixels. Check
    // three native words at a time, which covers eight complete pixels, before
    // entering the per-pixel accounting loop. Keep the pixel loop for any
    // nonzero block so channel order, density bounds, and edge coordinates
    // remain identical to the ordinary sparse scan.
    let grouped_pixels = source.len() / 24 * 8;
    for block_index in 0..source.len() / 24 {
        let byte_start = block_index * 24;
        let block = &source[byte_start..byte_start + 24];
        let words_are_zero = [0usize, 8, 16].into_iter().all(|offset| {
            u64::from_ne_bytes(
                block[offset..offset + 8]
                    .try_into()
                    .expect("each sparse RGB scan word has eight bytes"),
            ) == 0
        });
        if words_are_zero {
            continue;
        }
        for (pixel_offset, pixel) in block.chunks_exact(3).enumerate() {
            let source_index = block_index * 8 + pixel_offset;
            if !accumulate_pixel(source_index, pixel) {
                return Ok(None);
            }
        }
    }
    for (pixel_offset, pixel) in source[grouped_pixels * 3..].chunks_exact(3).enumerate() {
        if !accumulate_pixel(grouped_pixels + pixel_offset, pixel) {
            return Ok(None);
        }
    }

    let mut output = CheckedDims::new(new_width, new_height, 3)?.alloc_buffer();
    let mut nonzero_rows = vec![false; new_height as usize];
    for (block_index, sums) in block_sums {
        let output_index = block_index * 3;
        let pixel = [
            ((sums[0] + 2) >> 2) as u8,
            ((sums[1] + 2) >> 2) as u8,
            ((sums[2] + 2) >> 2) as u8,
        ];
        output[output_index..output_index + 3].copy_from_slice(&pixel);
        if pixel != [0; 3] {
            nonzero_rows[block_index / new_width as usize] = true;
        }
    }

    raw_bytes_to_image(new_width, new_height, output, 3).map(|image| Some((image, nonzero_rows)))
}

#[cfg(test)]
pub(super) fn execute_reduce_rgb_sparse_2x2(
    img: &DynamicImage,
    new_width: u32,
    new_height: u32,
) -> Result<Option<DynamicImage>, PilError> {
    execute_reduce_rgb_sparse_2x2_with_rows(img, new_width, new_height)
        .map(|reduced| reduced.map(|(image, _)| image))
}

/// Sum one RGBA reduction block after Pillow's per-input premultiplication.
///
/// Keep the four stored RGBA channels explicit here: the generic byte-mode
/// loop checks the channel count and alpha condition for every sample, and
/// reloads alpha while visiting each color channel. Alpha is read once per
/// pixel and the RGB multiply/divide is performed directly on the native
/// four-byte carrier.
#[inline]
pub(super) fn reduce_rgba_premultiplied_block_sums(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
    block_width: usize,
    block_height: usize,
) -> [u64; 4] {
    let mut sums = [0u64; 4];
    for dy in 0..block_height {
        let mut source_index = ((source_y + dy) * width + source_x) * 4;
        for _ in 0..block_width {
            let alpha = u32::from(source[source_index + 3]);
            let red = u32::from(source[source_index]);
            let green = u32::from(source[source_index + 1]);
            let blue = u32::from(source[source_index + 2]);
            sums[0] += u64::from((red * alpha + 127) / 255);
            sums[1] += u64::from((green * alpha + 127) / 255);
            sums[2] += u64::from((blue * alpha + 127) / 255);
            sums[3] += u64::from(alpha);
            source_index += 4;
        }
    }
    sums
}

/// Execute a Reduce operation matching Pillow's `Reduce.c`.
///
/// Pillow computes ceil(w/xscale) x ceil(h/yscale) output pixels, averages
/// complete xscale×yscale blocks for the inner region, then fills the right
/// column, bottom row, and bottom-right corner from partial blocks using the
/// same multiplier/amend rounding (`(sum + amend) * multiplier >> 24`).
pub(crate) fn execute_reduce(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(img.clone());
    }
    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let (w, h) = (img.width(), img.height());
    if matches!(img, DynamicImage::ImageRgba8(_)) && explicit_mode == Some("I") {
        return reduce_i_thumbnail(img, w.div_ceil(fx), h.div_ceil(fy), fx, fy);
    }
    if matches!(img, DynamicImage::ImageRgba8(_)) && explicit_mode == Some("F") {
        return reduce_f_thumbnail(img, w.div_ceil(fx), h.div_ceil(fy), fx, fy);
    }
    if let Some(output) = execute_reduce_rgb(img, fx, fy, explicit_mode)? {
        return Ok(output);
    }
    let channels = img.color().channel_count() as usize;
    let new_w = w.div_ceil(fx);
    let new_h = h.div_ceil(fy);
    let raw = img.as_bytes();
    let premultiplied_alpha = matches!(
        img.color(),
        crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
    ) && !matches!(
        explicit_mode,
        Some("CMYK" | "RGBa" | "La" | "PA" | "RGBX" | "F" | "I")
    );
    let native_rgba_reduce = premultiplied_alpha
        && explicit_mode == Some("RGBA")
        && matches!(img, DynamicImage::ImageRgba8(_));
    let mut out = CheckedDims::new(new_w, new_h, channels as u8)?.alloc_buffer();
    if new_w == 0 || new_h == 0 {
        return raw_bytes_to_image(new_w, new_h, out, channels);
    }

    let division_multiplier = |divider: u32| -> u64 {
        // division_UINT32(divider, 8): 2^32 / (256 * divider), truncated.
        ((1u128 << 32) / (u128::from(divider) * 256)) as u64
    };

    // Every reduced output row reads immutable source pixels and owns a
    // disjoint destination slice. Keep the partial right/bottom blocks in the
    // same row function as the full blocks so the parallel and serial lanes
    // share one exact rounding order.
    let main_w = w / fx;
    let main_h = h / fy;
    let right_width = w % fx;
    let bottom_height = h % fy;
    let full_divider = fx * fy;
    let full_multiplier = division_multiplier(full_divider);
    let full_amend = full_divider / 2;
    let right_divider = right_width * fy;
    let right_multiplier = division_multiplier(right_divider.max(1));
    let right_amend = right_divider / 2;
    let bottom_divider = fx * bottom_height;
    let bottom_multiplier = division_multiplier(bottom_divider.max(1));
    let bottom_amend = bottom_divider / 2;
    let corner_divider = right_width * bottom_height;
    let corner_multiplier = division_multiplier(corner_divider.max(1));
    let corner_amend = corner_divider / 2;
    let process_row = |y: u32, row: &mut [u8]| {
        let full_y = y < main_h;
        let y_count = if full_y { fy } else { bottom_height };
        let source_y = if y < main_h { y * fy } else { main_h * fy };
        for x in 0..new_w {
            let full_x = x < main_w;
            let x_count = if full_x { fx } else { right_width };
            let source_x = if x < main_w { x * fx } else { main_w * fx };
            let (multiplier, amend) = match (full_x, full_y) {
                (true, true) => (full_multiplier, full_amend),
                (false, true) => (right_multiplier, right_amend),
                (true, false) => (bottom_multiplier, bottom_amend),
                (false, false) => (corner_multiplier, corner_amend),
            };
            let dst_idx = x as usize * channels;
            if native_rgba_reduce {
                let sums = reduce_rgba_premultiplied_block_sums(
                    raw,
                    w as usize,
                    source_x as usize,
                    source_y as usize,
                    x_count as usize,
                    y_count as usize,
                );
                let average = |sum: u64| (((sum + u64::from(amend)) * multiplier) >> 24) as u8;
                let alpha = average(sums[3]);
                let mut red = average(sums[0]);
                let mut green = average(sums[1]);
                let mut blue = average(sums[2]);
                if alpha != 0 {
                    red = (u16::from(red) * 255 / u16::from(alpha)) as u8;
                    green = (u16::from(green) * 255 / u16::from(alpha)) as u8;
                    blue = (u16::from(blue) * 255 / u16::from(alpha)) as u8;
                }
                row[dst_idx] = red;
                row[dst_idx + 1] = green;
                row[dst_idx + 2] = blue;
                row[dst_idx + 3] = alpha;
            } else {
                let mut sums = [0u64; 4];
                for dy in 0..y_count {
                    for dx in 0..x_count {
                        let src_idx = ((source_y + dy) * w + source_x + dx) as usize * channels;
                        for c in 0..channels {
                            let sample = if premultiplied_alpha && c + 1 < channels {
                                ((u16::from(raw[src_idx + c])
                                    * u16::from(raw[src_idx + channels - 1])
                                    + 127)
                                    / 255) as u8
                            } else {
                                raw[src_idx + c]
                            };
                            sums[c] += u64::from(sample);
                        }
                    }
                }
                for c in 0..channels {
                    let mut value = (((sums[c] + u64::from(amend)) * multiplier) >> 24) as u8;
                    if premultiplied_alpha && c + 1 < channels {
                        let alpha =
                            (((sums[channels - 1] + u64::from(amend)) * multiplier) >> 24) as u8;
                        if alpha != 0 {
                            value = (u16::from(value) * 255 / u16::from(alpha)) as u8;
                        }
                    }
                    row[dst_idx + c] = value;
                }
            }
        }
    };

    let output_stride = new_w as usize * channels;
    #[cfg(feature = "parallel")]
    {
        // Pillow's Reduce.c has no row-task boundary; each destination row is
        // independent.  Match the other CPU geometry kernels by keeping tiny
        // reductions serial: Rayon setup costs more than the complete 32x24
        // benchmark reduction, while large images still use row-level
        // parallelism.  Use source pixels for the guard so a large source
        // remains parallel even when its reduced output is smaller than 512².
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (w as usize).saturating_mul(h as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                &mut out,
                output_stride,
                new_h as usize,
                |_row_start, _row_end, y, row| {
                    process_row(y, row);
                }
            );
        } else {
            for y in 0..new_h {
                let start = y as usize * output_stride;
                process_row(y, &mut out[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_h {
        let start = y as usize * output_stride;
        process_row(y, &mut out[start..start + output_stride]);
    }

    let result = raw_bytes_to_image(new_w, new_h, out, channels)?;
    Ok(result)
}

#[cfg(test)]
#[path = "reduce/tests.rs"]
mod tests;
