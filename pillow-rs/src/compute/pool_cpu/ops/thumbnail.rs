//! CPU thumbnail operation and reducing-gap helpers.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
#[cfg(target_arch = "x86_64")]
use crate::ops::pil_resize::X86FmaToken;
#[cfg(not(feature = "parallel"))]
use crate::ops::pil_resize::{F64MulAdd, PortableFma, precompute_coeffs_f64};
use crate::ops::pil_resize::{i32_samples_from_le_bytes, pil_resize, pil_resize_boxed, round_up};
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, GenericImageView};

use super::reduce::{execute_reduce, execute_reduce_rgb_with_sparse_rows};
#[cfg(not(feature = "parallel"))]
use super::resize::{
    i32_sample_bytes_from_native_storage, resample_kernel, resize_i_contiguous_eight_tap_weights,
    resize_i_horizontal_row, resize_i_round_up_to_i32, resize_i_sum_eight, resize_i_sum_general,
};
use super::resize::{resize_f, resize_i, resize_i_boxed};

/// Execute a Thumbnail operation.
/// Computes the scale factor to fit within the given box, preserving aspect ratio.
/// Matches PIL's thumbnail behavior including the reducing_gap optimization
/// (default reducing_gap=2.0) for non-NEAREST filters.
pub(crate) fn execute_thumbnail(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: &ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (cur_w, cur_h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(PilError::ValueError("thumbnail size must be > 0".into()));
    }
    // `Image::thumbnail` performs Pillow's aspect-preserving `round_aspect`
    // calculation before queuing this operation so lazy shape metadata and
    // the eventual pixels agree. The operation therefore carries the final
    // dimensions; do not apply the aspect calculation a second time here.
    let new_w = w.max(1).min(cur_w);
    let new_h = h.max(1).min(cur_h);
    // PIL forces NEAREST for mode "1" and "P" to avoid non-binary/interpolated values
    let effective_filter = match explicit_mode {
        Some("1") | Some("P") => ResampleFilter::Nearest,
        _ => *filter,
    };
    // PIL's thumbnail uses reducing_gap=2.0 by default: first integer-reduce
    // by up to scale/reducing_gap, then resize the rest.
    // This matches PIL's ImagingReduce then ImagingResample two-step.
    // Skip reducing_gap for modes with alpha (LA, RGBA) to avoid premultiply
    // issues. F/I use RGBA storage internally but are scalar modes, and CMYK
    // uses the same four-byte storage without an alpha channel.
    let has_alpha = matches!(
        img.color(),
        crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
    ) && !matches!(explicit_mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"));
    let needs_reduce = !matches!(effective_filter, ResampleFilter::Nearest) && !has_alpha;
    let mut resize_box = None;
    let work_img = if needs_reduce {
        let scale_x = cur_w as f64 / new_w as f64;
        let scale_y = cur_h as f64 / new_h as f64;
        // Image.resize computes these independently for the two axes before
        // calling Image.reduce(factor=(factor_x, factor_y)).
        let factor_x = ((scale_x / 2.0) as u32).max(1);
        let factor_y = ((scale_y / 2.0) as u32).max(1);
        if factor_x > 1 || factor_y > 1 {
            let (rw, rh) = (cur_w.div_ceil(factor_x), cur_h.div_ceil(factor_y));
            // Image.resize keeps the original full-image box after reduce and
            // scales its right/bottom edges by the integer factors. When the
            // source dimensions are not divisible by a factor, that box ends
            // inside the ceil-sized reduced image; using the whole reduced
            // image changes boundary pixels (Pillow's _get_safe_box path).
            resize_box = Some((
                0.0,
                0.0,
                cur_w as f64 / factor_x as f64,
                cur_h as f64 / factor_y as f64,
            ));
            match explicit_mode {
                // Pillow keeps F/I samples in their native scalar domain for
                // the reducing_gap pass. Averaging encoded RGBA bytes would
                // corrupt the representation before resize_f/resize_i runs.
                Some("F") if matches!(img, DynamicImage::ImageRgba8(_)) => {
                    reduce_f_thumbnail(img, rw, rh, factor_x, factor_y)?
                }
                #[cfg(not(feature = "parallel"))]
                Some("I")
                    if matches!(img, DynamicImage::ImageRgba8(_))
                        && factor_x == 2
                        && factor_y == 2
                        && cur_w % 2 == 0
                        && cur_h % 2 == 0 =>
                {
                    return Ok(preserve_mode(
                        img,
                        resize_i_thumbnail_reduce2x2(img, new_w, new_h, effective_filter)?,
                    ));
                }
                Some("I") if matches!(img, DynamicImage::ImageRgba8(_)) => {
                    reduce_i_thumbnail(img, rw, rh, factor_x, factor_y)?
                }
                _ => execute_reduce(img, factor_x, factor_y, explicit_mode)?,
            }
        } else {
            img.clone()
        }
    } else {
        img.clone()
    };
    // Only use F/I thumbnail paths when the image is already stored as Rgba8
    // (4 bytes per pixel), meaning it has been converted to F/I mode already.
    // If the image is still RGB or other format, use normal thumbnail regardless
    // of explicit_mode, because the F/I convert hasn't happened yet in the pipeline.
    let result = match (explicit_mode, &work_img, resize_box) {
        (Some("F"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom)))
            if cur_w % 2 == 0
                && cur_h % 2 == 0
                && cur_w / 2 == work_img.width()
                && cur_h / 2 == work_img.height()
                && left == 0.0
                && top == 0.0
                && right == work_img.width() as f64
                && bottom == work_img.height() as f64
                && new_w < work_img.width()
                && new_h < work_img.height() =>
        {
            resize_f(&work_img, new_w, new_h, &effective_filter)?
        }
        (Some("F"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom))) => {
            // Image.resize adjusts the source box after its reducing-gap pass.
            // Keep that fractional box for F as well; resizing the complete
            // ceil-sized reduction includes partial edge samples that Pillow
            // deliberately excludes from the final convolution.
            pil_resize_boxed(
                &work_img,
                new_w,
                new_h,
                left,
                top,
                right,
                bottom,
                effective_filter,
                explicit_mode,
            )
        }
        (Some("F"), DynamicImage::ImageRgba8(_), None) => {
            resize_f(&work_img, new_w, new_h, &effective_filter)?
        }
        (Some("I"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom))) => {
            resize_i_boxed(
                &work_img,
                new_w,
                new_h,
                left,
                top,
                right,
                bottom,
                effective_filter,
            )?
        }
        (Some("I"), DynamicImage::ImageRgba8(_), None) => {
            resize_i(&work_img, new_w, new_h, &effective_filter)?
        }
        (_, _, Some((left, top, right, bottom))) => pil_resize_boxed(
            &work_img,
            new_w,
            new_h,
            left,
            top,
            right,
            bottom,
            effective_filter,
            explicit_mode,
        ),
        _ => pil_resize(&work_img, new_w, new_h, effective_filter, explicit_mode),
    };
    Ok(preserve_mode(img, result))
}

/// Return whether a validated RGB pixel-write prefix can be folded into a
/// 2×2 non-nearest thumbnail reduction without materializing a full source clone.
pub(crate) fn rgb_putpixel_thumbnail_fusion_supported(
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
                    palette_index: false,
                    ..
                }
            )
        })
    {
        return false;
    }
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) {
        return false;
    }

    let (source_width, source_height) = img.dimensions();
    if source_width < 2
        || source_height < 2
        || source_width % 2 != 0
        || source_height % 2 != 0
        || (source_width as usize).saturating_mul(source_height as usize) < 512 * 512
    {
        return false;
    }
    let (new_width, new_height) = (
        (*w).max(1).min(source_width),
        (*h).max(1).min(source_height),
    );
    if new_width == 0 || new_height == 0 {
        return false;
    }
    let factor_x = ((source_width as f64 / new_width as f64 / 2.0) as u32).max(1);
    let factor_y = ((source_height as f64 / new_height as f64 / 2.0) as u32).max(1);
    factor_x == 2 && factor_y == 2
}

/// Apply validated RGB PutPixel operations at the exact integer-reduction
/// boundary, then run the same boxed resampling filter as `execute_thumbnail`.
/// This avoids cloning the full source image merely to change a few pixels.
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

    // Preserve ordered writes, including repeated coordinates, while keeping
    // only the final native RGB value for each source pixel.
    let mut pixel_overrides = std::collections::BTreeMap::<usize, [u8; 3]>::new();
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
    let affected_blocks = pixel_overrides
        .keys()
        .map(|&pixel_index| {
            let x = pixel_index % source_width as usize;
            let y = pixel_index / source_width as usize;
            (y / 2) * reduced_width as usize + x / 2
        })
        .collect::<std::collections::BTreeSet<_>>();
    let source_bytes = source.as_raw();
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
        {
            if let Some(nonzero_rows) = sparse_nonzero_rows.as_mut() {
                nonzero_rows[block_index / reduced_width as usize] = true;
            }
        }
    }

    let reduced =
        crate::raster::RgbImage::from_raw(reduced_width, reduced_height, reduced_bytes)
            .ok_or_else(|| PilError::InternalError("RGB reduction buffer shape mismatch".into()))?;
    let reduced = DynamicImage::ImageRgb8(reduced);
    let box_right = f64::from(source_width) / 2.0;
    let box_bottom = f64::from(source_height) / 2.0;
    let resized = if let Some(nonzero_rows) = sparse_nonzero_rows.as_deref() {
        crate::ops::pil_resize::pil_resize_boxed_with_sparse_rgb_rows(
            &reduced,
            new_width,
            new_height,
            0.0,
            0.0,
            box_right,
            box_bottom,
            *filter,
            Some("RGB"),
            nonzero_rows,
        )
    } else {
        pil_resize_boxed(
            &reduced,
            new_width,
            new_height,
            0.0,
            0.0,
            box_right,
            box_bottom,
            *filter,
            Some("RGB"),
        )
    };
    Ok(preserve_mode(img, resized))
}

pub(super) fn reduce_f_thumbnail_2x2_samples(
    dst_w: u32,
    dst_h: u32,
    source_width: u32,
    output_bytes: usize,
    mut sample_f32: impl FnMut(usize) -> f32,
) -> Result<DynamicImage, PilError> {
    let source_width = source_width as usize;
    let output_row_bytes = (dst_w as usize)
        .checked_mul(4)
        .ok_or_else(|| PilError::InternalError("F thumbnail row size overflow".into()))?;
    let mut output = vec![0; output_bytes];
    for y in 0..dst_h as usize {
        let top_row = y * 2 * source_width;
        let bottom_row = top_row + source_width;
        let output_row_start = y * output_row_bytes;
        let output_row_end = output_row_start + output_row_bytes;
        let output_row = &mut output[output_row_start..output_row_end];
        for (x, output_sample) in output_row.chunks_exact_mut(4).enumerate() {
            let source_x = x * 2;
            let top_left = sample_f32(top_row + source_x);
            let top_right = sample_f32(top_row + source_x + 1);
            let bottom_left = sample_f32(bottom_row + source_x);
            let bottom_right = sample_f32(bottom_row + source_x + 1);
            // Pillow adds this quartet in f32 order before promoting it to f64.
            let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
            let mut sum = 0.0f64;
            sum += f64::from(quartet);
            let value = (sum * 0.25) as f32;
            output_sample.copy_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, output, 4)
}

pub(super) fn reduce_f_thumbnail(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    factor_x: u32,
    factor_y: u32,
) -> Result<DynamicImage, PilError> {
    // F's four-byte image is a scalar carrier here; borrow it to avoid a copy.
    let (src_w, src_h, source_image) = match img {
        DynamicImage::ImageRgba8(image) => (image.width(), image.height(), image),
        _ => {
            return Err(PilError::ValueError(
                "F-mode thumbnail reduction requires four-byte scalar storage".into(),
            ));
        }
    };
    // `thumbnail` commonly applies a 2×2 reducing-gap pass before its F32
    // resize. Keep that frequent interior case branch-free: read the four
    // packed scalar samples by row offset, preserve Pillow's f32 quartet
    // order, then promote only the completed quartet to the f64 accumulator.
    if factor_x == 2
        && factor_y == 2
        && src_w % 2 == 0
        && src_h % 2 == 0
        && dst_w == src_w / 2
        && dst_h == src_h / 2
    {
        let source_dims = CheckedDims::new_allow_empty(src_w, src_h, 4)?;
        let source_bytes = source_image.as_raw();
        if source_bytes.len() != source_dims.total_bytes() {
            return Err(PilError::InternalError(
                "F-mode thumbnail source buffer shape mismatch".into(),
            ));
        }
        let output_dims = CheckedDims::new_allow_empty(dst_w, dst_h, 4)?;
        let output_bytes = output_dims.total_bytes();
        if let Some(samples) = crate::ops::pil_resize::f32_samples_borrowed_from_le_bytes(
            source_bytes,
            source_dims.total_bytes() / 4,
        ) {
            return reduce_f_thumbnail_2x2_samples(dst_w, dst_h, src_w, output_bytes, |index| {
                samples[index]
            });
        }
        return reduce_f_thumbnail_2x2_samples(dst_w, dst_h, src_w, output_bytes, |index| {
            let byte_index = index * 4;
            f32::from_le_bytes([
                source_bytes[byte_index],
                source_bytes[byte_index + 1],
                source_bytes[byte_index + 2],
                source_bytes[byte_index + 3],
            ])
        });
    }

    let sample_f32 = |x: u32, y: u32| {
        let sample = source_image.get_pixel(x, y);
        f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]])
    };
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);
    let main_width = src_w / factor_x;
    let main_height = src_h / factor_y;
    for y in 0..dst_h {
        let source_y = y * factor_y;
        let block_h = factor_y.min(src_h - source_y);
        for x in 0..dst_w {
            let source_x = x * factor_x;
            let block_w = factor_x.min(src_w - source_x);
            // `ImagingReduceNxN_32bpc` uses a double accumulator, but its
            // interior 2x2 groups are formed by float additions before being
            // promoted to double. The corner helper handles partial right,
            // bottom, and bottom-right blocks as scalar float values. Keep
            // those two paths distinct: a flat f32 sum drifts on constants,
            // while a flat f64 sum differs on heterogeneous blocks.
            let mut sum = 0.0f64;
            if x < main_width && y < main_height {
                let mut dy = 0;
                while dy + 1 < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let value = |offset_x: u32, offset_y: u32| {
                            sample_f32(source_x + offset_x, source_y + offset_y)
                        };
                        let top_left = value(dx, dy);
                        let top_right = value(dx + 1, dy);
                        let bottom_left = value(dx, dy + 1);
                        let bottom_right = value(dx + 1, dy + 1);
                        let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
                        sum += f64::from(quartet);
                        dx += 2;
                    }
                    if dx < block_w {
                        let value = |offset_y: u32| sample_f32(source_x + dx, source_y + offset_y);
                        sum += f64::from(value(dy) + value(dy + 1));
                    }
                    dy += 2;
                }
                if dy < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let value = |offset_x: u32| sample_f32(source_x + offset_x, source_y + dy);
                        sum += f64::from(value(dx) + value(dx + 1));
                        dx += 2;
                    }
                    if dx < block_w {
                        sum += f64::from(sample_f32(source_x + dx, source_y + dy));
                    }
                }
            } else {
                for dy in 0..block_h {
                    for dx in 0..block_w {
                        sum += f64::from(sample_f32(source_x + dx, source_y + dy));
                    }
                }
            }
            let value = (sum * (1.0 / f64::from(block_w * block_h))) as f32;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, out, 4)
}

#[cfg(not(feature = "parallel"))]
pub(super) fn resize_i_thumbnail_reduce2x2(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    #[cfg(target_arch = "x86_64")]
    if let Some(fma) = X86FmaToken::detect() {
        return resize_i_thumbnail_reduce2x2_with_fma(img, dst_w, dst_h, filter, &fma);
    }
    resize_i_thumbnail_reduce2x2_with_fma(img, dst_w, dst_h, filter, &PortableFma)
}

/// Stream native-I 2×2 reducing-gap rows directly into the existing serial
/// resize ring. Pillow still rounds and stores each reduced sample and each
/// horizontal intermediate as INT32; this only removes the full reduced image
/// buffer between those exact stages.
#[cfg(not(feature = "parallel"))]
pub(super) fn resize_i_thumbnail_reduce2x2_with_fma<F: F64MulAdd>(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    fma: &F,
) -> Result<DynamicImage, PilError> {
    let (source_width, source_height, source_bytes) = i32_sample_bytes_from_native_storage(img)?;
    if source_width % 2 != 0 || source_height % 2 != 0 {
        return Err(PilError::InternalError(
            "streamed I thumbnail reduction requires even source dimensions".into(),
        ));
    }
    let source_pixels =
        CheckedDims::new_allow_empty(source_width, source_height, 4)?.total_pixels();
    let source = i32_samples_from_le_bytes(source_bytes, source_pixels);
    let reduced_width = source_width / 2;
    let reduced_height = source_height / 2;
    let (kernel, support) = resample_kernel(&filter);
    let horizontal = precompute_coeffs_f64(dst_w, reduced_width, kernel, support);
    let vertical = precompute_coeffs_f64(dst_h, reduced_height, kernel, support);
    let fixed_horizontal_weights = resize_i_contiguous_eight_tap_weights(&horizontal.weights);
    let max_vertical_taps = vertical.weights.iter().map(Vec::len).max().unwrap_or(0);
    let ring_rows = max_vertical_taps
        .checked_next_power_of_two()
        .ok_or_else(|| {
            PilError::ValueError("resize_i: vertical coefficient span is too large".into())
        })?;
    let ring_samples = ring_rows
        .checked_mul(dst_w as usize)
        .ok_or_else(|| PilError::ValueError("resize_i: vertical ring is too large".into()))?;
    let mut ring = vec![0_i32; ring_samples];
    let mut reduced_row = vec![0_i32; reduced_width as usize];
    let output_dims = CheckedDims::new(dst_w, dst_h, 4)?;
    let output_stride = output_dims.row_stride();
    let mut output_bytes = output_dims.alloc_buffer();
    let ring_mask = ring_rows - 1;
    let source_width = source_width as usize;
    let mut next_source_row = 0_usize;

    for (output_y, output_row) in output_bytes.chunks_mut(output_stride).enumerate() {
        let first_vertical_row = vertical.xmin[output_y] as usize;
        let vertical_weights = &vertical.weights[output_y];
        let source_end = first_vertical_row + vertical_weights.len();
        while next_source_row < source_end {
            let top_start = next_source_row * 2 * source_width;
            let bottom_start = top_start + source_width;
            let top = &source[top_start..top_start + source_width];
            let bottom = &source[bottom_start..bottom_start + source_width];
            for (output_x, reduced) in reduced_row.iter_mut().enumerate() {
                let source_x = output_x * 2;
                let quartet = top[source_x]
                    .wrapping_add(top[source_x + 1])
                    .wrapping_add(bottom[source_x])
                    .wrapping_add(bottom[source_x + 1]);
                *reduced = round_up(f64::from(quartet) / 4.0) as i32;
            }

            let ring_start = (next_source_row & ring_mask) * dst_w as usize;
            let horizontal_row = &mut ring[ring_start..ring_start + dst_w as usize];
            resize_i_horizontal_row(
                &reduced_row,
                horizontal_row,
                &horizontal,
                fixed_horizontal_weights.as_ref(),
                fma,
            );
            next_source_row += 1;
        }

        if let Ok(weights) = <&[f64; 8]>::try_from(vertical_weights.as_slice()) {
            for (output_x, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = resize_i_sum_eight(
                    weights,
                    first_vertical_row,
                    |source_y| ring[(source_y & ring_mask) * dst_w as usize + output_x],
                    fma,
                );
                output.copy_from_slice(&resize_i_round_up_to_i32(accumulator).to_le_bytes());
            }
        } else {
            for (output_x, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = resize_i_sum_general(
                    vertical_weights,
                    first_vertical_row,
                    |source_y| ring[(source_y & ring_mask) * dst_w as usize + output_x],
                    fma,
                );
                output.copy_from_slice(&resize_i_round_up_to_i32(accumulator).to_le_bytes());
            }
        }
    }

    let output = crate::raster::RgbaImage::from_raw(dst_w, dst_h, output_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))?;
    Ok(DynamicImage::ImageRgba8(output))
}

pub(super) fn reduce_i_thumbnail(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    factor_x: u32,
    factor_y: u32,
) -> Result<DynamicImage, PilError> {
    // I's four-byte image is a scalar carrier here; borrow it to avoid a copy.
    let (src_w, src_h, source_image) = match img {
        DynamicImage::ImageRgba8(image) => (image.width(), image.height(), image),
        _ => {
            return Err(PilError::ValueError(
                "I-mode thumbnail reduction requires four-byte scalar storage".into(),
            ));
        }
    };
    let source_bytes = source_image.as_raw();
    let source_samples = i32_samples_from_le_bytes(
        source_bytes,
        source_bytes.len() / std::mem::size_of::<i32>(),
    );
    let source_stride = src_w as usize;
    let sample_i32 = |x: u32, y: u32| source_samples[y as usize * source_stride + x as usize];
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);

    // Pillow's common native-I thumbnail reducing-gap uses complete 2×2
    // quartets. Traverse each pair of source rows contiguously and preserve
    // Reduce.c's wrapping INT32 additions before the f64 average. Partial
    // blocks and other factors stay on the generic path below.
    if factor_x == 2 && factor_y == 2 && src_w > 0 && src_h > 0 && src_w % 2 == 0 && src_h % 2 == 0
    {
        let row_pair_len = source_stride
            .checked_mul(2)
            .ok_or_else(|| PilError::InternalError("I thumbnail row size overflow".into()))?;
        for rows in source_samples.chunks_exact(row_pair_len) {
            let (top, bottom) = rows.split_at(source_stride);
            for (top_pair, bottom_pair) in top.chunks_exact(2).zip(bottom.chunks_exact(2)) {
                let quartet = top_pair[0]
                    .wrapping_add(top_pair[1])
                    .wrapping_add(bottom_pair[0])
                    .wrapping_add(bottom_pair[1]);
                let value = round_up(f64::from(quartet) / 4.0) as i32;
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        return raw_bytes_to_image(dst_w, dst_h, out, 4);
    }

    let main_width = src_w / factor_x;
    let main_height = src_h / factor_y;
    for y in 0..dst_h {
        let source_y = y * factor_y;
        let block_h = factor_y.min(src_h - source_y);
        for x in 0..dst_w {
            let source_x = x * factor_x;
            let block_w = factor_x.min(src_w - source_x);
            // ImagingReduceNxN_32bpc adds pairs/quartets while the samples
            // are still INT32, so each intermediate addition wraps at 32
            // bits before the result is promoted to the double accumulator.
            // Its corner helper instead adds each partial-edge sample
            // directly to the double accumulator. Preserve those two paths
            // separately; summing every sample as i64 changes overflow cases.
            let mut sum = 0.0f64;
            if x < main_width && y < main_height {
                let value = |offset_x: u32, offset_y: u32| {
                    sample_i32(source_x + offset_x, source_y + offset_y)
                };
                let mut dy = 0;
                while dy + 1 < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let quartet = value(dx, dy)
                            .wrapping_add(value(dx + 1, dy))
                            .wrapping_add(value(dx, dy + 1))
                            .wrapping_add(value(dx + 1, dy + 1));
                        sum += f64::from(quartet);
                        dx += 2;
                    }
                    if dx < block_w {
                        let pair = value(dx, dy).wrapping_add(value(dx, dy + 1));
                        sum += f64::from(pair);
                    }
                    dy += 2;
                }
                if dy < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let pair = value(dx, dy).wrapping_add(value(dx + 1, dy));
                        sum += f64::from(pair);
                        dx += 2;
                    }
                    if dx < block_w {
                        sum += f64::from(value(dx, dy));
                    }
                }
            } else {
                for dy in 0..block_h {
                    for dx in 0..block_w {
                        sum += f64::from(sample_i32(source_x + dx, source_y + dy));
                    }
                }
            }
            let value = round_up(sum / f64::from(block_w * block_h)) as i32;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, out, 4)
}

#[cfg(test)]
#[path = "thumbnail/tests.rs"]
mod tests;
