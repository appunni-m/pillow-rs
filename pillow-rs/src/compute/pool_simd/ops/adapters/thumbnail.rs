//! SIMD Thumbnail adapter for native-mode downscaling and resampling.

use super::resize::{
    native_resize_byte_layout_for_image, native_resize_byte_layout_for_shape, simd_resize_i32,
    simd_resize_i32_boxed,
};
use super::resize::{native_resize_supported_for_image, native_resize_supported_for_shape};
use super::{
    SimdImageShape, SimdLayout, native_copy_image_bytes, native_reduce_bytes,
    native_reduce_sparse_rgb_2x2, native_reduce_supported_for_image,
    native_reduce_supported_for_shape, simd_resize_convolution, simd_resize_convolution_boxed,
    simd_resize_convolution_with_zero_row_elision, simd_resize_f, simd_resize_f_boxed,
    simd_resize_nearest, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;

#[path = "thumbnail/kernels/float.rs"]
mod float;
#[path = "thumbnail/kernels/fusion.rs"]
mod fusion;
#[path = "thumbnail/kernels/integer.rs"]
mod integer;
use self::float::simd_thumbnail_reduce_f;
#[cfg(test)]
use self::float::thumbnail_f_reduce_value;
pub(crate) use self::fusion::{
    simd_rgb_putpixel_thumbnail_fusion_supported, simd_rgb_putpixel_thumbnail_prepare,
    simd_rgb_putpixel_thumbnail_resize,
};
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
        .saturating_add(u32::from(!img.width().is_multiple_of(factor_x.max(1))))
        * (img.height() / factor_y.max(1))
            .saturating_add(u32::from(!img.height().is_multiple_of(factor_y.max(1))));
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
        && img.width().is_multiple_of(2)
        && img.height().is_multiple_of(2)
    {
        // Current Apple ARM64 controls for native I 1024x768 are slower than
        // CPU (0.609/0.640 ms SIMD vs 0.425/0.430 ms CPU; see
        // docs/evidence/filter/thumbnail-fi-rank9-arm64-20261022/README.md).
        // Samples cluster in this fused kernel but do not split its stages.
        // Source inspection shows scalar 2x2 reduction and per-tap lane
        // gathers/rounding/packing around the f64x8 work; those costs likely
        // outweigh vector arithmetic at this 256x192 output size. Keep the
        // real vector path while row reuse and gather costs are investigated.
        return simd_thumbnail_i_reduce2x2_resample(
            img,
            output_width,
            output_height,
            effective_filter,
        );
    }
    let (work_img, sparse_rgb_reduction) =
        simd_thumbnail_reduced_source(img, mode, factor_x, factor_y)?;
    match mode {
        Some("F") => simd_thumbnail_resize_f(
            img,
            &work_img,
            output_width,
            output_height,
            factor_x,
            factor_y,
            effective_filter,
        ),
        Some("I") => simd_thumbnail_resize_i(
            img,
            &work_img,
            output_width,
            output_height,
            factor_x,
            factor_y,
            effective_filter,
        ),
        _ => simd_thumbnail_resize_bytes(
            img,
            &work_img,
            mode,
            output_width,
            output_height,
            factor_x,
            factor_y,
            effective_filter,
            sparse_rgb_reduction,
        ),
    }
}

fn simd_thumbnail_reduced_source(
    img: &DynamicImage,
    mode: Option<&str>,
    factor_x: u32,
    factor_y: u32,
) -> Result<(DynamicImage, bool), PilError> {
    if factor_x == 1 && factor_y == 1 {
        return Ok((img.clone(), false));
    }
    let (reduced, vector_blocks, scalar_tail, sparse_rgb_reduction) = match mode {
        Some("F") => {
            let (image, blocks, tail) = simd_thumbnail_reduce_f(img, factor_x, factor_y)?;
            (image, blocks, tail, false)
        }
        Some("I") => {
            let (image, blocks, tail) = simd_thumbnail_reduce_i(img, factor_x, factor_y)?;
            (image, blocks, tail, false)
        }
        _ => simd_thumbnail_reduce_bytes_source(img, mode, factor_x, factor_y)?,
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok((reduced, sparse_rgb_reduction))
}

fn simd_thumbnail_reduce_bytes_source(
    img: &DynamicImage,
    mode: Option<&str>,
    factor_x: u32,
    factor_y: u32,
) -> Result<(DynamicImage, u64, u64, bool), PilError> {
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
    if let Some((output, width, height)) = sparse_rgb {
        return Ok((
            crate::image_utils::raw_bytes_to_image(width, height, output, channels)?,
            0,
            0,
            true,
        ));
    }
    let (output, width, height, vector_blocks, scalar_tail) = native_reduce_bytes(
        img,
        channels,
        native_thumbnail_has_alpha(img, mode),
        factor_x,
        factor_y,
        false,
    )
    .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    Ok((
        crate::image_utils::raw_bytes_to_image(width, height, output, channels)?,
        vector_blocks,
        scalar_tail,
        false,
    ))
}

fn simd_thumbnail_resize_f(
    img: &DynamicImage,
    work_img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    factor_x: u32,
    factor_y: u32,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    if factor_x == 1 && factor_y == 1 {
        return simd_resize_f(work_img, output_width, output_height, &filter);
    }
    // Current Apple ARM64 controls for native F 1024x768 are slower than CPU
    // (0.417/0.437 ms SIMD vs 0.325/0.323 ms CPU; see
    // docs/evidence/filter/thumbnail-fi-rank9-arm64-20261022/README.md).
    // Profiles place most SIMD samples in simd_resize_f, but do not split its
    // stages. Source inspection shows scalar source/coefficient lane gathers
    // feeding ordered f64x8 updates; their overhead likely outweighs vector
    // work on this 256x192 result.
    let box_right = f64::from(img.width()) / f64::from(factor_x);
    let box_bottom = f64::from(img.height()) / f64::from(factor_y);
    // The reducing-gap box is identical to ordinary F resize only when it
    // reaches the reduced image's exact final pixel bounds. Keep ceil-sized
    // partial edges on the boxed reference path.
    if box_right == f64::from(work_img.width()) && box_bottom == f64::from(work_img.height()) {
        return simd_resize_f(work_img, output_width, output_height, &filter);
    }
    simd_resize_f_boxed(
        work_img,
        output_width,
        output_height,
        0.0,
        0.0,
        box_right,
        box_bottom,
        filter,
    )
}

fn simd_thumbnail_resize_i(
    img: &DynamicImage,
    work_img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    factor_x: u32,
    factor_y: u32,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    if factor_x == 1 && factor_y == 1 {
        return simd_resize_i32(work_img, output_width, output_height, &filter);
    }
    let box_right = f64::from(img.width()) / f64::from(factor_x);
    let box_bottom = f64::from(img.height()) / f64::from(factor_y);
    if box_right == f64::from(work_img.width()) && box_bottom == f64::from(work_img.height()) {
        // A complete reduced image has ordinary resize coefficients; retain
        // boxed convolution only for fractional source edges.
        return simd_resize_i32(work_img, output_width, output_height, &filter);
    }
    simd_resize_i32_boxed(
        work_img,
        output_width,
        output_height,
        0.0,
        0.0,
        box_right,
        box_bottom,
        filter,
    )
}

fn simd_thumbnail_resize_bytes(
    img: &DynamicImage,
    work_img: &DynamicImage,
    mode: Option<&str>,
    output_width: u32,
    output_height: u32,
    factor_x: u32,
    factor_y: u32,
    filter: ResampleFilter,
    sparse_rgb_reduction: bool,
) -> Result<DynamicImage, PilError> {
    let (channels, premultiplied_alpha) = native_resize_byte_layout_for_image(work_img, mode)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if factor_x != 1 || factor_y != 1 {
        return simd_thumbnail_resize_boxed_bytes(
            img,
            work_img,
            mode,
            output_width,
            output_height,
            factor_x,
            factor_y,
            filter,
            channels,
            premultiplied_alpha,
            sparse_rgb_reduction,
        );
    }
    match filter {
        ResampleFilter::Nearest => {
            simd_resize_nearest(work_img, output_width, output_height, channels)
        }
        _ => simd_resize_convolution(
            work_img,
            output_width,
            output_height,
            filter,
            channels,
            premultiplied_alpha,
            false,
        ),
    }
}

fn simd_thumbnail_resize_boxed_bytes(
    img: &DynamicImage,
    work_img: &DynamicImage,
    mode: Option<&str>,
    output_width: u32,
    output_height: u32,
    factor_x: u32,
    factor_y: u32,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
    sparse_rgb_reduction: bool,
) -> Result<DynamicImage, PilError> {
    let box_right = f64::from(img.width()) / f64::from(factor_x);
    let box_bottom = f64::from(img.height()) / f64::from(factor_y);
    let aligned_rgb = matches!(work_img, DynamicImage::ImageRgb8(_))
        && matches!(mode, None | Some("RGB"))
        && box_right == f64::from(work_img.width())
        && box_bottom == f64::from(work_img.height());
    if aligned_rgb {
        // An aligned full-source box is exactly the ordinary Resize geometry.
        // Reuse its checked 32-bit RGB convolution path; boxed general work
        // otherwise accumulates byte samples in wider lanes.
        if sparse_rgb_reduction {
            return simd_resize_convolution_with_zero_row_elision(
                work_img,
                output_width,
                output_height,
                filter,
                channels,
                premultiplied_alpha,
                true,
                0,
                false,
                true,
            );
        }
        return simd_resize_convolution(
            work_img,
            output_width,
            output_height,
            filter,
            channels,
            premultiplied_alpha,
            true,
        );
    }
    simd_resize_convolution_boxed(
        work_img,
        output_width,
        output_height,
        0.0,
        0.0,
        box_right,
        box_bottom,
        filter,
        channels,
        premultiplied_alpha,
        0,
    )
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
