//! Native SIMD thumbnail adapter and typed reduction kernels.

use super::resize_i32::{simd_resize_i32, simd_resize_i32_boxed};
use super::{
    SimdImageShape, SimdLayout, native_copy_image_bytes, native_reduce_bytes,
    native_reduce_sparse_rgb_2x2, native_reduce_supported_for_image,
    native_reduce_supported_for_shape, native_resize_byte_layout_for_image,
    native_resize_byte_layout_for_shape, native_resize_supported_for_image,
    native_resize_supported_for_shape, simd_resize_convolution, simd_resize_convolution_boxed,
    simd_resize_convolution_with_zero_row_elision, simd_resize_f, simd_resize_f_boxed,
    simd_resize_nearest, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, RgbImage};

mod float;
mod integer;
use self::float::simd_thumbnail_reduce_f;
#[cfg(test)]
use self::float::thumbnail_f_reduce_value;
use self::integer::{simd_thumbnail_i_reduce2x2_resample, simd_thumbnail_reduce_i};

/// Validate the final dimensions carried by `Image.thumbnail`.
///
/// The public operation computes Pillow's aspect-preserving `round_aspect`
/// result before queuing the lazy node. Repeating that calculation here would
/// shrink an already-adjusted request a second time (for example `(10, 6)`
/// would become `(9, 6)` for an `11x7` source).
pub(super) fn native_thumbnail_dimensions(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 || target_width == 0 || target_height == 0 {
        return None;
    }
    Some((
        target_width.min(source_width).max(1),
        target_height.min(source_height).max(1),
    ))
}

#[inline]
fn native_thumbnail_filter(mode: Option<&str>, filter: ResampleFilter) -> ResampleFilter {
    if matches!(mode, Some("1" | "P")) {
        ResampleFilter::Nearest
    } else {
        filter
    }
}

#[inline]
fn native_thumbnail_has_alpha(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(
        img,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) && !matches!(mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"))
}

#[inline]
fn native_thumbnail_reduction_factors(
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    has_alpha: bool,
) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 || output_width == 0 || output_height == 0 {
        return None;
    }
    if matches!(filter, ResampleFilter::Nearest) || has_alpha {
        return Some((1, 1));
    }
    let factor_x = ((f64::from(source_width) / f64::from(output_width) / 2.0) as u32).max(1);
    let factor_y = ((f64::from(source_height) / f64::from(output_height) / 2.0) as u32).max(1);
    Some((factor_x, factor_y))
}

fn native_thumbnail_layout_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    if matches!(mode, Some("F" | "I")) {
        return matches!(img, DynamicImage::ImageRgba8(_)).then_some((4, true));
    }
    native_resize_byte_layout_for_image(img, mode).map(|(channels, _)| (channels, false))
}

fn native_thumbnail_layout_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    if matches!(mode, Some("F" | "I")) {
        return (shape.layout == SimdLayout::Rgba8).then_some((4, true));
    }
    native_resize_byte_layout_for_shape(shape, mode).map(|(channels, _)| (channels, false))
}

fn native_thumbnail_typed_reduce_supported_for_image(
    img: &DynamicImage,
    factor_x: u32,
    factor_y: u32,
) -> bool {
    if !matches!(img, DynamicImage::ImageRgba8(_)) || img.width() == 0 || img.height() == 0 {
        return false;
    }
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    let output_pixels = (img.width() / factor_x.max(1))
        .saturating_add(u32::from(img.width() % factor_x.max(1) != 0))
        * (img.height() / factor_y.max(1))
            .saturating_add(u32::from(img.height() % factor_y.max(1) != 0));
    img.as_bytes().len() == expected_bytes && output_pixels != 0
}

fn native_thumbnail_typed_reduce_supported_for_shape(
    shape: SimdImageShape,
    factor_x: u32,
    factor_y: u32,
) -> bool {
    if shape.layout != SimdLayout::Rgba8 || shape.width == 0 || shape.height == 0 {
        return false;
    }
    let output_width = shape.width.div_ceil(factor_x.max(1));
    let output_height = shape.height.div_ceil(factor_y.max(1));
    output_width != 0 && output_height != 0
}

pub(super) fn native_thumbnail_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some((channels, typed_scalar)) = native_thumbnail_layout_for_image(img, mode) else {
        return false;
    };
    let Some((output_width, output_height)) =
        native_thumbnail_dimensions(img.width(), img.height(), target_width, target_height)
    else {
        return false;
    };
    let filter = native_thumbnail_filter(mode, filter);
    let Some((factor_x, factor_y)) = native_thumbnail_reduction_factors(
        img.width(),
        img.height(),
        output_width,
        output_height,
        filter,
        native_thumbnail_has_alpha(img, mode),
    ) else {
        return false;
    };
    if factor_x != 1 || factor_y != 1 {
        let reduction_supported = if typed_scalar {
            native_thumbnail_typed_reduce_supported_for_image(img, factor_x, factor_y)
        } else {
            native_reduce_supported_for_image(img, factor_x, factor_y, mode)
        };
        if !reduction_supported {
            return false;
        }
    }
    channels != 0
        && native_resize_supported_for_image(img, output_width, output_height, filter, mode)
}

pub(super) fn native_thumbnail_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some((channels, typed_scalar)) = native_thumbnail_layout_for_shape(shape, mode) else {
        return false;
    };
    let Some((output_width, output_height)) =
        native_thumbnail_dimensions(shape.width, shape.height, target_width, target_height)
    else {
        return false;
    };
    let filter = native_thumbnail_filter(mode, filter);
    let Some((factor_x, factor_y)) = native_thumbnail_reduction_factors(
        shape.width,
        shape.height,
        output_width,
        output_height,
        filter,
        matches!(shape.layout, SimdLayout::LumaA8 | SimdLayout::Rgba8)
            && !matches!(mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX")),
    ) else {
        return false;
    };
    if factor_x != 1 || factor_y != 1 {
        let reduction_supported = if typed_scalar {
            native_thumbnail_typed_reduce_supported_for_shape(shape, factor_x, factor_y)
        } else {
            native_reduce_supported_for_shape(shape, factor_x, factor_y, mode)
        };
        if !reduction_supported {
            return false;
        }
    }
    channels != 0
        && native_resize_supported_for_shape(shape, output_width, output_height, filter, mode)
}

/// Admit an RGB `PutPixel*` prefix that the SIMD executor can fold into the
/// sparse reducing-gap stage of a following thumbnail.
pub(crate) fn simd_rgb_putpixel_thumbnail_fusion_supported(
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
                    x,
                    y,
                    palette_index: false,
                    ..
                } if *x < img.width() && *y < img.height()
            )
        })
    {
        return false;
    }
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return false;
    };
    let (source_width, source_height) = (img.width(), img.height());
    if source_width < 2
        || source_height < 2
        || source_width % 2 != 0
        || source_height % 2 != 0
        || (source_width as usize).saturating_mul(source_height as usize) < 512 * 512
        || matches!(filter, ResampleFilter::Nearest)
    {
        return false;
    }
    let Some((output_width, output_height)) =
        native_thumbnail_dimensions(source_width, source_height, *w, *h)
    else {
        return false;
    };
    let effective_filter = native_thumbnail_filter(mode, *filter);
    native_thumbnail_reduction_factors(
        source_width,
        source_height,
        output_width,
        output_height,
        effective_filter,
        false,
    ) == Some((2, 2))
        && native_thumbnail_supported_for_image(img, *w, *h, effective_filter, mode)
}

/// Fold pending RGB writes into the existing sparse 2×2 reduction.
/// `None` leaves dense sources on the ordinary per-operation executor path.
pub(crate) fn simd_rgb_putpixel_thumbnail_prepare(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !simd_rgb_putpixel_thumbnail_fusion_supported(img, putpixel_ops, thumbnail, mode) {
        return Ok(None);
    }
    let DynamicImage::ImageRgb8(source) = img else {
        return Ok(None);
    };
    let PipelineOp::Thumbnail { .. } = thumbnail else {
        return Ok(None);
    };
    let Some((mut reduced, reduced_width, reduced_height)) = native_reduce_sparse_rgb_2x2(img)
    else {
        return Ok(None);
    };

    // Preserve write order: the final value for a repeated coordinate wins.
    // Recompute only affected 2×2 blocks against the original native bytes.
    let source_width = img.width() as usize;
    let reduced_width = reduced_width as usize;
    let mut pixel_overrides = std::collections::BTreeMap::<usize, [u8; 3]>::new();
    for op in putpixel_ops {
        let PipelineOp::PutPixel { x, y, color, .. } = op else {
            return Ok(None);
        };
        let pixel_index = *y as usize * source_width + *x as usize;
        pixel_overrides.insert(pixel_index, [color.0, color.1, color.2]);
    }
    let affected_blocks = pixel_overrides
        .keys()
        .map(|&pixel_index| {
            let x = pixel_index % source_width;
            let y = pixel_index / source_width;
            (y / 2) * reduced_width + x / 2
        })
        .collect::<std::collections::BTreeSet<_>>();
    let source_bytes = source.as_raw();
    for block_index in affected_blocks {
        let block_x = block_index % reduced_width;
        let block_y = block_index / reduced_width;
        let mut sums = [0u32; 3];
        for dy in 0..2usize {
            for dx in 0..2usize {
                let source_pixel_index = (block_y * 2 + dy) * source_width + block_x * 2 + dx;
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
            reduced[output_start + channel] = ((sums[channel] + 2) >> 2) as u8;
        }
    }

    let reduced = RgbImage::from_raw(reduced_width as u32, reduced_height, reduced)
        .ok_or_else(|| PilError::InternalError("RGB thumbnail reduction shape mismatch".into()))?;
    Ok(Some(DynamicImage::ImageRgb8(reduced)))
}

/// Resize the prepared reduced RGB image through the existing sparse SIMD
/// path, preserving the standard thumbnail kernel and vector receipt.
pub(crate) fn simd_rgb_putpixel_thumbnail_resize(
    reduced: &DynamicImage,
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return Err(PilError::ValueError("expected Thumbnail op".into()));
    };
    if !matches!(reduced, DynamicImage::ImageRgb8(_)) || !matches!(mode, None | Some("RGB")) {
        return Err(simd_unsupported("Thumbnail"));
    }
    let (output_width, output_height) =
        native_thumbnail_dimensions(reduced.width(), reduced.height(), *w, *h)
            .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    simd_resize_convolution_with_zero_row_elision(
        reduced,
        output_width,
        output_height,
        native_thumbnail_filter(mode, *filter),
        3,
        false,
        true,
        0,
        false,
        true,
    )
}

/// Execute the nearest-neighbour subset of `Image.thumbnail`.
///
/// Aspect-ratio selection and bound clamping are scalar control work. Every
/// selected pixel is then handled by the native byte resize kernel; this
/// adapter never calls `execute_thumbnail` or retries through the CPU path.
pub(crate) fn simd_thumbnail(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Thumbnail { w, h, filter } = op else {
        return Err(PilError::ValueError("expected Thumbnail op".into()));
    };
    if !native_thumbnail_supported_for_image(img, *w, *h, *filter, mode) {
        return Err(simd_unsupported("Thumbnail"));
    }
    let effective_filter = native_thumbnail_filter(mode, *filter);
    let (output_width, output_height) =
        native_thumbnail_dimensions(img.width(), img.height(), *w, *h)
            .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if (img.width(), img.height()) == (output_width, output_height) {
        return native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("Thumbnail"));
    }
    let has_alpha = native_thumbnail_has_alpha(img, mode);
    let (factor_x, factor_y) = native_thumbnail_reduction_factors(
        img.width(),
        img.height(),
        output_width,
        output_height,
        effective_filter,
        has_alpha,
    )
    .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if mode == Some("I")
        && factor_x == 2
        && factor_y == 2
        && img.width() % 2 == 0
        && img.height() % 2 == 0
    {
        return simd_thumbnail_i_reduce2x2_resample(
            img,
            output_width,
            output_height,
            effective_filter,
        );
    }
    let typed_scalar = matches!(mode, Some("F" | "I"));
    let mut sparse_rgb_reduction = false;
    let work_img = if factor_x != 1 || factor_y != 1 {
        let (reduced, vector_blocks, scalar_tail) = if typed_scalar && mode == Some("F") {
            simd_thumbnail_reduce_f(img, factor_x, factor_y)?
        } else if typed_scalar && mode == Some("I") {
            simd_thumbnail_reduce_i(img, factor_x, factor_y)?
        } else {
            let channels = native_resize_byte_layout_for_image(img, mode)
                .map(|(channels, _)| channels)
                .ok_or_else(|| simd_unsupported("Thumbnail"))?;
            let sparse_rgb = if matches!(img, DynamicImage::ImageRgb8(_))
                && matches!(mode, None | Some("RGB"))
                && factor_x == 2
                && factor_y == 2
            {
                native_reduce_sparse_rgb_2x2(img)
            } else {
                None
            };
            let (output, width, height, vector_blocks, scalar_tail) =
                if let Some((output, width, height)) = sparse_rgb {
                    sparse_rgb_reduction = true;
                    (output, width, height, 0, 0)
                } else {
                    native_reduce_bytes(
                        img,
                        channels,
                        native_thumbnail_has_alpha(img, mode),
                        factor_x,
                        factor_y,
                        false,
                    )
                    .ok_or_else(|| simd_unsupported("Thumbnail"))?
                };
            (
                crate::image_utils::raw_bytes_to_image(width, height, output, channels)?,
                vector_blocks,
                scalar_tail,
            )
        };
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        reduced
    } else {
        img.clone()
    };
    if typed_scalar && mode == Some("F") {
        if factor_x != 1 || factor_y != 1 {
            let box_right = f64::from(img.width()) / f64::from(factor_x);
            let box_bottom = f64::from(img.height()) / f64::from(factor_y);
            // The reducing-gap box is identical to ordinary F resize only
            // when it reaches the reduced image's exact final pixel bounds.
            // Keep ceil-sized partial edges on the boxed reference path.
            if box_right == f64::from(work_img.width())
                && box_bottom == f64::from(work_img.height())
            {
                return simd_resize_f(&work_img, output_width, output_height, &effective_filter);
            }
            return simd_resize_f_boxed(
                &work_img,
                output_width,
                output_height,
                0.0,
                0.0,
                box_right,
                box_bottom,
                effective_filter,
            );
        }
        return simd_resize_f(&work_img, output_width, output_height, &effective_filter);
    }
    if typed_scalar && mode == Some("I") {
        if factor_x != 1 || factor_y != 1 {
            let box_right = f64::from(img.width()) / f64::from(factor_x);
            let box_bottom = f64::from(img.height()) / f64::from(factor_y);
            if box_right == f64::from(work_img.width())
                && box_bottom == f64::from(work_img.height())
            {
                // A complete reduced image has ordinary resize coefficients;
                // retain boxed convolution only for fractional source edges.
                return simd_resize_i32(&work_img, output_width, output_height, &effective_filter);
            }
            return simd_resize_i32_boxed(
                &work_img,
                output_width,
                output_height,
                0.0,
                0.0,
                box_right,
                box_bottom,
                effective_filter,
            );
        }
        return simd_resize_i32(&work_img, output_width, output_height, &effective_filter);
    }
    let (channels, premultiplied_alpha) = native_resize_byte_layout_for_image(&work_img, mode)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if factor_x != 1 || factor_y != 1 {
        let box_right = f64::from(img.width()) / f64::from(factor_x);
        let box_bottom = f64::from(img.height()) / f64::from(factor_y);
        if matches!(work_img, DynamicImage::ImageRgb8(_))
            && matches!(mode, None | Some("RGB"))
            && box_right == f64::from(work_img.width())
            && box_bottom == f64::from(work_img.height())
        {
            // An aligned full-source box is exactly the ordinary Resize
            // geometry. Reuse its proven checked 32-bit RGB convolution path;
            // the boxed general kernel otherwise accumulates these byte
            // samples in wider lanes on the common reducing-gap thumbnail.
            if sparse_rgb_reduction {
                return simd_resize_convolution_with_zero_row_elision(
                    &work_img,
                    output_width,
                    output_height,
                    effective_filter,
                    channels,
                    premultiplied_alpha,
                    true,
                    0,
                    false,
                    true,
                );
            }
            return simd_resize_convolution(
                &work_img,
                output_width,
                output_height,
                effective_filter,
                channels,
                premultiplied_alpha,
                true,
            );
        }
        return simd_resize_convolution_boxed(
            &work_img,
            output_width,
            output_height,
            0.0,
            0.0,
            box_right,
            box_bottom,
            effective_filter,
            channels,
            premultiplied_alpha,
            0,
        );
    }
    match effective_filter {
        ResampleFilter::Nearest => {
            simd_resize_nearest(&work_img, output_width, output_height, channels)
        }
        _ => simd_resize_convolution(
            &work_img,
            output_width,
            output_height,
            effective_filter,
            channels,
            premultiplied_alpha,
            false,
        ),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sparse_rgb_putpixel_thumbnail_fusion_matches_materialized_ordered_writes() {
        use crate::compute::pool_cpu::ops::geometry::execute_thumbnail;
        use crate::pipeline::{PipelineOp, ResampleFilter};
        use crate::raster::{DynamicImage, RgbImage};

        let (width, height) = (512u32, 512u32);
        let mut source = vec![0u8; width as usize * height as usize * 3];
        source[(3 * width as usize + 2) * 3..(3 * width as usize + 2) * 3 + 3]
            .copy_from_slice(&[4, 8, 12]);
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source.clone()).expect("RGB source shape"),
        );
        let writes = [
            PipelineOp::PutPixel {
                x: 2,
                y: 3,
                color: (180, 120, 60, 255),
                palette_index: false,
            },
            PipelineOp::PutPixel {
                x: 2,
                y: 3,
                color: (10, 20, 30, 255),
                palette_index: false,
            },
            PipelineOp::PutPixel {
                x: width - 1,
                y: height - 1,
                color: (7, 129, 251, 255),
                palette_index: false,
            },
        ];
        let filter = ResampleFilter::Bicubic;
        let thumbnail = PipelineOp::Thumbnail {
            w: 128,
            h: 128,
            filter,
        };
        assert!(super::simd_rgb_putpixel_thumbnail_fusion_supported(
            &image,
            &writes,
            &thumbnail,
            Some("RGB"),
        ));
        let reduced =
            super::simd_rgb_putpixel_thumbnail_prepare(&image, &writes, &thumbnail, Some("RGB"))
                .expect("sparse reducer succeeds")
                .expect("sparse source admits the fused path");
        let actual = super::simd_rgb_putpixel_thumbnail_resize(&reduced, &thumbnail, Some("RGB"))
            .expect("SIMD thumbnail resize");

        let mut materialized = source;
        for op in writes {
            let PipelineOp::PutPixel { x, y, color, .. } = op else {
                unreachable!();
            };
            let start = (y as usize * width as usize + x as usize) * 3;
            materialized[start..start + 3].copy_from_slice(&[color.0, color.1, color.2]);
        }
        let materialized = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, materialized).expect("written RGB shape"),
        );
        let expected = execute_thumbnail(&materialized, 128, 128, &filter, Some("RGB"))
            .expect("CPU thumbnail reference");
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }

    #[test]
    fn aligned_rgb_thumbnail_uses_exact_narrow_bicubic_resize() {
        use crate::compute::pool_cpu::ops::geometry::{execute_reduce, execute_thumbnail};
        use crate::pipeline::{PipelineOp, ResampleFilter};
        use crate::raster::{DynamicImage, RgbImage};

        let (width, height) = (64u32, 48u32);
        let source = (0..width as usize * height as usize * 3)
            .map(|index| (index.wrapping_mul(37).wrapping_add(index / 11 * 17 + 29) % 256) as u8)
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source).expect("RGB thumbnail source shape"),
        );
        let filter = ResampleFilter::Bicubic;
        let reduced = execute_reduce(&image, 2, 2, Some("RGB")).expect("RGB reduce stage");
        let wide = super::simd_resize_convolution(&reduced, 16, 12, filter, 3, false, false)
            .expect("wide SIMD RGB resize");
        let narrow = super::simd_resize_convolution(&reduced, 16, 12, filter, 3, false, true)
            .expect("narrow SIMD RGB resize");
        assert_eq!(narrow.as_bytes(), wide.as_bytes());

        let operation = PipelineOp::Thumbnail {
            w: 16,
            h: 12,
            filter,
        };
        let expected =
            execute_thumbnail(&image, 16, 12, &filter, Some("RGB")).expect("CPU RGB thumbnail");
        let actual =
            super::simd_thumbnail(&image, &operation, Some("RGB")).expect("SIMD RGB thumbnail");
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_thumbnail_vector_reduce_matches_scalar_pillow_grouping() {
        use crate::raster::{DynamicImage, RgbaImage};

        let height = 8usize;
        let edge_values = [
            0x0000_0001u32,
            0x8000_0001,
            0x3f80_0001,
            0xbf7f_ffff,
            0x7f7f_ffff,
            0xff7f_ffff,
            0x7f80_0000,
            0xff80_0000,
            0x7fc1_2345,
            0x0000_0000,
            0x8000_0000,
            0x3eaa_aaab,
            0xbeaa_aaaa,
            0x0080_0000,
            0x8080_0000,
            0x3f00_0001,
        ];
        for width in [16usize, 32, 48] {
            let source: Vec<f32> = (0..width * height)
                .map(|index| f32::from_bits(edge_values[index % edge_values.len()]))
                .collect();
            let source_bytes: Vec<u8> = source
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            let image = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(width as u32, height as u32, source_bytes)
                    .expect("F thumbnail source shape"),
            );

            let (actual, vector_blocks, scalar_tail) =
                super::simd_thumbnail_reduce_f(&image, 2, 2).expect("SIMD F thumbnail reduction");
            let mut expected = Vec::with_capacity(width * height);
            for y in 0..height / 2 {
                for x in 0..width / 2 {
                    expected.extend_from_slice(
                        &super::thumbnail_f_reduce_value(&source, width, height, 2, 2, x, y)
                            .to_le_bytes(),
                    );
                }
            }

            assert_eq!(actual.as_bytes(), expected, "width={width}");
            assert_eq!(vector_blocks, (width / 16 * (height / 2)) as u64);
            assert_eq!(scalar_tail, 0);
        }
    }

    #[test]
    fn i_thumbnail_simd_matches_cpu_at_tails_and_edges() {
        use crate::compute::pool_cpu::ops::geometry::execute_thumbnail;
        use crate::pipeline::{PipelineOp, ResampleFilter};

        for (width, height, output_width, output_height) in [
            (37u32, 29u32, 7u32, 5u32),
            (41, 31, 8, 6),
            (34, 27, 9, 7),
            (40, 32, 10, 8),
        ] {
            let mut source = Vec::with_capacity(width as usize * height as usize * 4);
            for index in 0..(width as usize * height as usize) {
                let value = match index % 11 {
                    0 => i32::MIN,
                    1 => i32::MAX,
                    2 => -1_048_573,
                    3 => 1_000_003,
                    _ => (index as i32)
                        .wrapping_mul(73_921)
                        .wrapping_add((index as i32 / 17).wrapping_mul(31_337)),
                };
                source.extend_from_slice(&value.to_le_bytes());
            }
            let image = crate::raster::DynamicImage::ImageRgba8(
                crate::raster::RgbaImage::from_raw(width, height, source)
                    .expect("I-mode test storage dimensions must be valid"),
            );

            for filter in [
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Box,
            ] {
                let expected =
                    execute_thumbnail(&image, output_width, output_height, &filter, Some("I"))
                        .expect("scalar I thumbnail must succeed");
                let operation = PipelineOp::Thumbnail {
                    w: output_width,
                    h: output_height,
                    filter,
                };
                let actual = super::simd_thumbnail(&image, &operation, Some("I"))
                    .expect("SIMD I thumbnail must succeed");

                assert_eq!(
                    actual.as_bytes(),
                    expected.as_bytes(),
                    "I thumbnail {width}x{height} -> {output_width}x{output_height} with {filter:?}"
                );
            }
        }
    }
}
