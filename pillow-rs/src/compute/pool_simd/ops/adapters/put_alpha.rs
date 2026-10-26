//! SIMD PutAlpha family admission, adapters, and result construction.

mod kernels;

use super::simd_rows_mut_serial;
use super::{SimdImageShape, SimdLayout, simd_unsupported};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, PixelMode};
use crate::raster::{DynamicImage, GenericImageView};

#[cfg(feature = "parallel")]
use self::kernels::SIMD_PUT_ALPHA_PARALLEL_PIXEL_THRESHOLD;
pub(super) use self::kernels::simd_put_alpha_rgb_row;
use self::kernels::{
    SIMD_PUT_ALPHA_RGB_MASK_VECTOR_THRESHOLD, simd_put_alpha_constant_bytes,
    simd_put_alpha_data_bytes, simd_put_alpha_rgb_constant_bytes, simd_put_alpha_rgb_mask_row,
    simd_put_alpha_rgba_constant_bytes, simd_put_alpha_rgba_mask_row,
};

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let pixels = (img.width() as usize).saturating_mul(img.height() as usize);
    match op {
        PipelineOp::PutAlpha {
            mode: alpha_mode, ..
        } => put_alpha_shape(img, *alpha_mode, mode)
            .is_some_and(|(_, _, pixels_per_vector, _)| pixels >= pixels_per_vector),
        PipelineOp::PutAlphaData {
            mask,
            mode: alpha_mode,
        } => put_alpha_data_shape(img, mask.as_ref(), *alpha_mode, mode)
            .is_some_and(|(_, _, pixels_per_vector, _)| pixels >= pixels_per_vector),
        _ => false,
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let pixels = (shape.width as usize).saturating_mul(shape.height as usize);
    match op {
        PipelineOp::PutAlpha {
            mode: alpha_mode, ..
        } => {
            let pixels_per_vector = match alpha_mode {
                PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA => 8,
                PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK => 4,
                _ => usize::MAX,
            };
            shape_put_alpha_supported(shape, *alpha_mode, mode) && pixels >= pixels_per_vector
        }
        PipelineOp::PutAlphaData {
            mask,
            mode: alpha_mode,
        } => {
            let pixels_per_vector = match alpha_mode {
                PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA => 8,
                PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK => 4,
                _ => usize::MAX,
            };
            shape_put_alpha_data_supported(shape, mask.as_ref(), *alpha_mode, mode)
                && pixels >= pixels_per_vector
        }
        _ => false,
    }
}

pub(super) fn update_shape(shape: &mut SimdImageShape, op: &PipelineOp) {
    let (PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. }) = op else {
        return;
    };
    shape.layout = match mode {
        PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA => SimdLayout::LumaA8,
        PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK => SimdLayout::Rgba8,
        _ => SimdLayout::Unsupported,
    };
}

pub(super) fn output_mode(op: &PipelineOp) -> Option<&'static str> {
    let (PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. }) = op else {
        return None;
    };
    // `mode` identifies the source image. Pillow promotes CMYK to RGBA when
    // alpha is inserted, so later capability checks must see the output mode.
    match mode {
        PixelMode::L | PixelMode::LA => Some("LA"),
        PixelMode::P | PixelMode::PA => Some("PA"),
        PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK => Some("RGBA"),
        PixelMode::Mode1 | PixelMode::YCbCr | PixelMode::HSV | PixelMode::I | PixelMode::F => None,
    }
}

fn put_alpha_data_shape(
    img: &DynamicImage,
    mask: &DynamicImage,
    alpha_mode: PixelMode,
    mode: Option<&str>,
) -> Option<(usize, usize, usize, bool)> {
    if !matches!(mask, DynamicImage::ImageLuma8(_)) || img.dimensions() != mask.dimensions() {
        return None;
    }
    let (source_channels, output_channels, pixels_per_vector, cmyk_source) = match alpha_mode {
        PixelMode::L | PixelMode::P => (1, 2, 8, false),
        PixelMode::LA | PixelMode::PA => (2, 2, 8, false),
        PixelMode::RGB => (3, 4, 4, false),
        PixelMode::RGBA => (4, 4, 4, false),
        // CMYK is stored as four native bytes, but Pillow promotes it to
        // RGBA while inserting an image-backed alpha channel. The data-plane
        // kernel handles that conversion with vector arithmetic below.
        PixelMode::CMYK => (4, 4, 4, true),
        PixelMode::Mode1 | PixelMode::YCbCr | PixelMode::HSV | PixelMode::I | PixelMode::F => {
            return None;
        }
    };
    let mode_matches = match alpha_mode {
        // `Image.putalpha_data` queues a P source with PixelMode::P but
        // `Image::push_op` promotes its resulting logical tag to PA before
        // execution. Accept both names at this byte-layout boundary.
        PixelMode::P => matches!(mode, Some("P") | Some("PA")),
        PixelMode::PA => mode == Some("PA"),
        PixelMode::L => matches!(mode, None | Some("L")),
        PixelMode::LA => matches!(mode, None | Some("LA")),
        PixelMode::RGB => matches!(mode, None | Some("RGB")),
        PixelMode::RGBA => matches!(mode, None | Some("RGBA" | "RGBX")),
        // `push_op` clears the CMYK tag because the result is promoted to
        // ordinary RGBA. The operation's `alpha_mode` remains the source of
        // truth, so a missing execution tag is valid here.
        PixelMode::CMYK => matches!(mode, None | Some("CMYK")),
        _ => false,
    };
    if !mode_matches {
        return None;
    }
    let source_matches = matches!(
        (alpha_mode, img),
        (PixelMode::L | PixelMode::P, DynamicImage::ImageLuma8(_))
            | (PixelMode::LA | PixelMode::PA, DynamicImage::ImageLumaA8(_))
            | (PixelMode::RGB, DynamicImage::ImageRgb8(_))
            | (
                PixelMode::RGBA | PixelMode::CMYK,
                DynamicImage::ImageRgba8(_)
            )
    );
    source_matches.then_some((
        source_channels,
        output_channels,
        pixels_per_vector,
        cmyk_source,
    ))
}

fn put_alpha_shape(
    img: &DynamicImage,
    alpha_mode: PixelMode,
    mode: Option<&str>,
) -> Option<(usize, usize, usize, bool)> {
    let (source_channels, output_channels, pixels_per_vector, cmyk_source) = match alpha_mode {
        PixelMode::L | PixelMode::P => (1, 2, 8, false),
        PixelMode::LA | PixelMode::PA => (2, 2, 8, false),
        PixelMode::RGB => (3, 4, 4, false),
        PixelMode::RGBA => (4, 4, 4, false),
        PixelMode::CMYK => (4, 4, 4, true),
        PixelMode::Mode1 | PixelMode::YCbCr | PixelMode::HSV | PixelMode::I | PixelMode::F => {
            return None;
        }
    };
    let mode_matches = match alpha_mode {
        // A paletted source is represented by one-byte indices before the
        // operation and by a two-byte PA sample layout after it.
        PixelMode::P => matches!(mode, Some("P") | Some("PA")),
        PixelMode::PA => mode == Some("PA"),
        PixelMode::L => matches!(mode, None | Some("L")),
        PixelMode::LA => matches!(mode, None | Some("LA")),
        PixelMode::RGB => matches!(mode, None | Some("RGB")),
        PixelMode::RGBA => matches!(mode, None | Some("RGBA" | "RGBX")),
        // putalpha promotes CMYK storage to ordinary RGBA and clears the
        // logical CMYK tag at the Image boundary.
        PixelMode::CMYK => matches!(mode, None | Some("CMYK")),
        _ => false,
    };
    if !mode_matches {
        return None;
    }
    let source_matches = matches!(
        (alpha_mode, img),
        (PixelMode::L | PixelMode::P, DynamicImage::ImageLuma8(_))
            | (PixelMode::LA | PixelMode::PA, DynamicImage::ImageLumaA8(_))
            | (PixelMode::RGB, DynamicImage::ImageRgb8(_))
            | (
                PixelMode::RGBA | PixelMode::CMYK,
                DynamicImage::ImageRgba8(_)
            )
    );
    source_matches.then_some((
        source_channels,
        output_channels,
        pixels_per_vector,
        cmyk_source,
    ))
}

fn shape_put_alpha_supported(
    shape: SimdImageShape,
    alpha_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    let (source_layout, mode_matches) = match alpha_mode {
        PixelMode::L | PixelMode::P => (
            SimdLayout::Luma8,
            matches!(mode, None | Some("L" | "P" | "PA")),
        ),
        PixelMode::LA | PixelMode::PA => {
            (SimdLayout::LumaA8, matches!(mode, None | Some("LA" | "PA")))
        }
        PixelMode::RGB => (SimdLayout::Rgb8, matches!(mode, None | Some("RGB"))),
        PixelMode::RGBA => (
            SimdLayout::Rgba8,
            matches!(mode, None | Some("RGBA" | "RGBX")),
        ),
        PixelMode::CMYK => (SimdLayout::Rgba8, matches!(mode, None | Some("CMYK"))),
        _ => (SimdLayout::Unsupported, false),
    };
    mode_matches && shape.layout == source_layout
}

fn shape_put_alpha_data_supported(
    shape: SimdImageShape,
    mask: &DynamicImage,
    alpha_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    if !matches!(mask, DynamicImage::ImageLuma8(_))
        || (shape.width, shape.height) != mask.dimensions()
    {
        return false;
    }
    let (source_layout, mode_matches) = match alpha_mode {
        PixelMode::L | PixelMode::P => (
            SimdLayout::Luma8,
            matches!(mode, None | Some("L" | "P" | "PA")),
        ),
        PixelMode::LA | PixelMode::PA => {
            (SimdLayout::LumaA8, matches!(mode, None | Some("LA" | "PA")))
        }
        PixelMode::RGB => (SimdLayout::Rgb8, matches!(mode, None | Some("RGB"))),
        PixelMode::RGBA => (
            SimdLayout::Rgba8,
            matches!(mode, None | Some("RGBA" | "RGBX")),
        ),
        PixelMode::CMYK => (SimdLayout::Rgba8, matches!(mode, None | Some("CMYK"))),
        _ => (SimdLayout::Unsupported, false),
    };
    mode_matches && shape.layout == source_layout
}

pub fn simd_put_alpha(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutAlpha {
        alpha,
        mode: alpha_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected PutAlpha op".into()));
    };
    let native_two_band_alpha = matches!(
        (*alpha_mode, mode),
        (PixelMode::LA, None | Some("LA")) | (PixelMode::PA, Some("PA"))
    );
    if native_two_band_alpha && let DynamicImage::ImageLumaA8(source) = img {
        // LA stores [luma, alpha] and PA stores [index, alpha]. Both already
        // have the requested two-byte output layout. Clone once and update
        // only byte 1; the generic SIMD packer would rebuild both bands in
        // temporary blocks without doing useful channel work.
        let mut output = source.clone();
        for pixel in output.as_mut().chunks_exact_mut(2) {
            pixel[1] = *alpha;
        }
        return Ok(DynamicImage::ImageLumaA8(output));
    }
    if *alpha_mode == PixelMode::RGBA
        && put_alpha_shape(img, *alpha_mode, mode).is_some()
        && let DynamicImage::ImageRgba8(source) = img
    {
        let (output, vector_blocks) = simd_put_alpha_rgba_constant_bytes(
            source.as_raw(),
            source.width(),
            source.height(),
            *alpha,
        )
        .ok_or_else(|| simd_unsupported("PutAlpha"))?;
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        return crate::image_utils::raw_bytes_to_image(source.width(), source.height(), output, 4);
    }
    if *alpha_mode == PixelMode::RGB
        && put_alpha_shape(img, *alpha_mode, mode).is_some()
        && let DynamicImage::ImageRgb8(source) = img
    {
        let (output, vector_blocks, scalar_tail) = simd_put_alpha_rgb_constant_bytes(
            source.as_raw(),
            source.width(),
            source.height(),
            *alpha,
        )
        .ok_or_else(|| simd_unsupported("PutAlpha"))?;
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return crate::image_utils::raw_bytes_to_image(source.width(), source.height(), output, 4);
    }
    let Some((source_channels, output_channels, _pixels_per_vector, cmyk_source)) =
        put_alpha_shape(img, *alpha_mode, mode)
    else {
        return Err(simd_unsupported("PutAlpha"));
    };
    let pixels = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutAlpha pixel count overflow".into()))?;
    let (output, vector_blocks, scalar_tail) = simd_put_alpha_constant_bytes(
        img.as_bytes(),
        pixels,
        source_channels,
        output_channels,
        cmyk_source,
        *alpha,
    )
    .ok_or_else(|| simd_unsupported("PutAlpha"))?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, output_channels)
}

fn finish_put_alpha_data_native(
    img: &DynamicImage,
    mask: &[u8],
    pixels: usize,
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
) -> Result<DynamicImage, PilError> {
    let (output, vector_blocks, scalar_tail) = simd_put_alpha_data_bytes(
        img.as_bytes(),
        mask,
        pixels,
        source_channels,
        output_channels,
        cmyk_source,
    )
    .ok_or_else(|| simd_unsupported("PutAlphaData"))?;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, output_channels)
}

pub fn simd_put_alpha_data(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutAlphaData {
        mask,
        mode: alpha_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected PutAlphaData op".into()));
    };
    let mask = mask.as_ref();
    let Some((source_channels, output_channels, _pixels_per_vector, cmyk_source)) =
        put_alpha_data_shape(img, mask, *alpha_mode, mode)
    else {
        return Err(simd_unsupported("PutAlphaData"));
    };
    let DynamicImage::ImageLuma8(mask) = mask else {
        return Err(simd_unsupported("PutAlphaData"));
    };
    let pixels = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData pixel count overflow".into()))?;
    if *alpha_mode == PixelMode::RGBA
        && matches!(mode, None | Some("RGBA"))
        && let DynamicImage::ImageRgba8(source) = img
        && source.dimensions() == mask.dimensions()
        && source.as_raw().len() == mask.as_raw().len().saturating_mul(4)
    {
        // Preserve the already-RGBA image in one output clone and overwrite
        // only alpha bytes. The generic SIMD packer rebuilds every color byte
        // even though Pillow's RGBA putalpha(mask) leaves them unchanged.
        let mut output = source.clone();
        #[cfg(feature = "parallel")]
        if pixels >= SIMD_PUT_ALPHA_PARALLEL_PIXEL_THRESHOLD {
            let width = usize::try_from(img.width())
                .map_err(|_| PilError::ValueError("SIMD PutAlphaData width overflow".into()))?;
            let height = usize::try_from(img.height())
                .map_err(|_| PilError::ValueError("SIMD PutAlphaData height overflow".into()))?;
            let row_stride = width
                .checked_mul(4)
                .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData row overflow".into()))?;
            simd_rows_mut_serial!(
                output.as_mut(),
                row_stride,
                height,
                |_row_start, _row_end, y, row| {
                    let mask_start = y as usize * width;
                    simd_put_alpha_rgba_mask_row(
                        row,
                        &mask.as_raw()[mask_start..mask_start + width],
                    );
                }
            );
        } else {
            simd_put_alpha_rgba_mask_row(output.as_mut(), mask.as_raw());
        }
        #[cfg(not(feature = "parallel"))]
        simd_put_alpha_rgba_mask_row(output.as_mut(), mask.as_raw());
        return Ok(DynamicImage::ImageRgba8(output));
    }
    if *alpha_mode == PixelMode::RGB
        && pixels >= SIMD_PUT_ALPHA_RGB_MASK_VECTOR_THRESHOLD
        && let DynamicImage::ImageRgb8(source) = img
    {
        // This route is architecture-vectorized but row-serial. Keep it
        // available in the default SIMD build; the `parallel` feature is not
        // a prerequisite for the u8x16 interleave and must not add Rayon here.
        let width = usize::try_from(img.width())
            .map_err(|_| PilError::ValueError("SIMD PutAlphaData width overflow".into()))?;
        let height = usize::try_from(img.height())
            .map_err(|_| PilError::ValueError("SIMD PutAlphaData height overflow".into()))?;
        let source_stride = width
            .checked_mul(3)
            .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData row overflow".into()))?;
        let output_stride = width
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData row overflow".into()))?;
        let output_length = pixels
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData output overflow".into()))?;
        let mut output = vec![0u8; output_length];
        simd_rows_mut_serial!(
            &mut output,
            output_stride,
            height,
            |_row_start, _row_end, y, row| {
                let source_start = y as usize * source_stride;
                let mask_start = y as usize * width;
                simd_put_alpha_rgb_mask_row(
                    &source.as_raw()[source_start..source_start + source_stride],
                    &mask.as_raw()[mask_start..mask_start + width],
                    row,
                    width,
                );
            }
        );
        crate::compute::record_pipeline_operation_vector_blocks((width / 4 * height) as u64);
        crate::compute::record_pipeline_operation_scalar_tail((width % 4 * height) as u64);
        return crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4);
    }
    finish_put_alpha_data_native(
        img,
        mask.as_raw(),
        pixels,
        source_channels,
        output_channels,
        cmyk_source,
    )
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "parallel")]
    #[test]
    fn native_rgb_putalpha_mask_interleave_covers_vector_blocks_and_tails() {
        for width in [1, 3, 4, 5, 17] {
            let source: Vec<u8> = (0..width * 3)
                .map(|index| (index * 37 + 11) as u8)
                .collect();
            let mask: Vec<u8> = (0..width).map(|index| (index * 53 + 7) as u8).collect();
            let mut actual = vec![0u8; width * 4];
            super::simd_put_alpha_rgb_mask_row(&source, &mask, &mut actual, width);
            let expected: Vec<u8> = source
                .chunks_exact(3)
                .zip(&mask)
                .flat_map(|(pixel, alpha)| [pixel[0], pixel[1], pixel[2], *alpha])
                .collect();
            assert_eq!(actual, expected, "width {width}");
        }
    }

    #[test]
    fn native_rgba_putalpha_mask_replaces_only_alpha_samples() {
        for pixels in [0, 1, 3, 4, 5, 17] {
            let original: Vec<u8> = (0..pixels * 4)
                .map(|index| (index * 29 + 3) as u8)
                .collect();
            let mask: Vec<u8> = (0..pixels).map(|index| (index * 47 + 5) as u8).collect();
            let mut actual = original.clone();
            super::simd_put_alpha_rgba_mask_row(&mut actual, &mask);
            let mut expected = original.clone();
            for (pixel, &alpha) in expected.chunks_exact_mut(4).zip(&mask) {
                pixel[3] = alpha;
            }
            assert_eq!(actual, expected, "pixel count {pixels}");
        }
    }

    #[test]
    fn native_rgb_putalpha_interleave_covers_vector_blocks_and_tails() {
        for (width, height) in [(1, 1), (3, 2), (4, 1), (5, 3), (17, 4)] {
            let source: Vec<u8> = (0..width * height * 3)
                .map(|index| (index * 37 + 11) as u8)
                .collect();
            let (actual, vector_blocks, scalar_tail) =
                super::simd_put_alpha_rgb_constant_bytes(&source, width, height, 173)
                    .expect("valid RGB storage should interleave");
            let expected: Vec<u8> = source
                .chunks_exact(3)
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 173])
                .collect();
            let pixels = u64::from(width) * u64::from(height);
            assert_eq!(actual, expected, "{width}x{height}");
            assert_eq!(vector_blocks, pixels / 4, "{width}x{height}");
            assert_eq!(scalar_tail, pixels % 4, "{width}x{height}");
        }
        assert!(super::simd_put_alpha_rgb_constant_bytes(&[1, 2], 1, 1, 173).is_none());
    }

    #[test]
    fn native_rgba_putalpha_constant_preserves_rgb_and_replaces_alpha() {
        for width in [1usize, 3, 4, 5, 17] {
            let height = 3;
            let original: Vec<u8> = (0..width * height * 4)
                .map(|index| (index * 31 + 7) as u8)
                .collect();
            let source = original.clone();
            let mut expected = original;
            for pixel in expected.chunks_exact_mut(4) {
                pixel[3] = 173;
            }

            let (actual, vector_count) = super::simd_put_alpha_rgba_constant_bytes(
                &source,
                width as u32,
                height as u32,
                173,
            )
            .expect("valid RGBA storage should replace alpha");

            assert_eq!(vector_count, (width * height).div_ceil(4) as u64);
            assert_eq!(actual, expected, "width {width}");
        }
        assert!(super::simd_put_alpha_rgba_constant_bytes(&[0, 1], 1, 1, 173).is_none());
    }
}
