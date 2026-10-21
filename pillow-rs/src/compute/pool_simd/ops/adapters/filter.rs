//! SIMD filter-family admission for narrow mode-specific fast paths.

mod kernels;

use crate::compute::pool_cpu::ops::filter::uniform5x5_parameters_match;
use crate::compute::{
    record_pipeline_operation_path, record_pipeline_operation_scalar_tail,
    record_pipeline_operation_vector_blocks,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

#[inline(always)]
fn finish_native_filter5x5(
    img: &DynamicImage,
    output: Vec<u8>,
    channels: usize,
    vector_blocks: u64,
    scalar_tail: u64,
) -> Result<DynamicImage, PilError> {
    record_pipeline_operation_path("vector");
    if vector_blocks != 0 {
        record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        record_pipeline_operation_scalar_tail(scalar_tail);
    }
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

/// Return the unchanged RGB image when the exact normalized 5x5 box filter
/// receives a large constant image. The source's ordered float evaluation
/// preserves every constant byte for this parameter tuple; the vector scan
/// proves that the whole native RGB buffer has one pixel value.
pub(super) fn uniform_rgb_5x5_identity(
    image: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    if !matches!(mode, None | Some("RGB"))
        || !matches!(image, DynamicImage::ImageRgb8(_))
        || !uniform5x5_parameters_match(kernel, scale, offset)
    {
        return None;
    }

    let width = image.width() as usize;
    let height = image.height() as usize;
    let pixel_count = width.checked_mul(height)?;
    if pixel_count <= kernels::SMALL_IMAGE_LIMIT_PIXELS || width < 5 || height < 5 {
        return None;
    }

    let evidence = kernels::scan_uniform_rgb(image.as_bytes(), width, height)?;
    if evidence.vector_blocks != 0 {
        record_pipeline_operation_path("vector");
        record_pipeline_operation_vector_blocks(evidence.vector_blocks);
    }
    if evidence.scalar_tail_pixels != 0 {
        record_pipeline_operation_scalar_tail(evidence.scalar_tail_pixels);
    }
    evidence.is_uniform.then(|| image.clone())
}

/// Execute Pillow's RGB 5x5 BLUR ring with vectorized vertical sums and
/// channel-interleaved horizontal taps. Only the exact native kernel enters;
/// smaller images keep the existing lower-overhead route.
#[cfg(not(feature = "parallel"))]
pub(super) fn rgb_blur_ring_5x5(
    image: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    if !matches!(mode, None | Some("RGB"))
        || !matches!(image, DynamicImage::ImageRgb8(_))
        || !crate::compute::pool_cpu::ops::filter::rgb_blur_ring5x5_parameters_match(
            kernel, scale, offset,
        )
    {
        return None;
    }

    let width = image.width() as usize;
    let height = image.height() as usize;
    if width.checked_mul(height)? <= kernels::SMALL_IMAGE_LIMIT_PIXELS || width < 5 || height < 5 {
        return None;
    }

    let (output, vector_blocks, scalar_tail) =
        kernels::apply_rgb_blur_ring(image.as_bytes(), width, height)?;
    if vector_blocks != 0 {
        record_pipeline_operation_path("vector");
        record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        record_pipeline_operation_scalar_tail(scalar_tail);
    }
    let result =
        crate::image_utils::raw_bytes_to_image(image.width(), image.height(), output, 3).ok()?;
    Some(crate::image::preserve_mode(image, result))
}

fn simd_filter_5x5_i32(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !super::use_native_i32_convolution_path(img, mode, 2) {
        return Err(super::simd_unsupported("Filter5x5"));
    }
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / scale);
    if crate::compute::pool_cpu::ops::filter::uniform_i32_filter5x5_is_identity(
        img.as_bytes(),
        img.width() as usize,
        img.height() as usize,
        &normalized_kernel,
        offset + 0.5,
    ) {
        crate::compute::record_pipeline_operation_path("scalar-control");
        let output = img.as_bytes().to_vec();
        let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
        return Ok(preserve_mode(img, result));
    }
    let mut output = img.as_bytes().to_vec();
    let (vector_blocks, scalar_tail) = kernels::native_filter_5x5_i32_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        &normalized_kernel,
        offset + 0.5,
    )
    .ok_or_else(|| PilError::InternalError("SIMD Filter5x5 I buffer mismatch".into()))?;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
    Ok(preserve_mode(img, result))
}

fn try_specialized_filter5x5(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if matches!(mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && crate::compute::pool_cpu::ops::filter::la_single_outlier5x5_parameters_match(
            kernel, scale, offset,
        )
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) =
            kernels::native_filter_5x5_single_outlier_la_rows(
                img.as_bytes(),
                &mut output,
                img.width() as usize,
                img.height() as usize,
            )
        {
            return finish_native_filter5x5(img, output, 2, vector_blocks, scalar_tail).map(Some);
        }
    }
    if matches!(mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && crate::compute::pool_cpu::ops::filter::uniform5x5_parameters_match(kernel, scale, offset)
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) = kernels::native_filter_5x5_uniform_luma_rows(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
        ) {
            return finish_native_filter5x5(img, output, 1, vector_blocks, scalar_tail).map(Some);
        }
    }
    if matches!(mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && crate::compute::pool_cpu::ops::filter::binomial5x5_parameters_match(
            kernel, scale, offset,
        )
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) = kernels::native_filter_5x5_binomial_luma_rows(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
        ) {
            return finish_native_filter5x5(img, output, 1, vector_blocks, scalar_tail).map(Some);
        }
    }
    if matches!(mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && crate::compute::pool_cpu::ops::filter::binomial5x5_parameters_match(
            kernel, scale, offset,
        )
    {
        let mut output = img.as_bytes().to_vec();
        if let Some((vector_blocks, scalar_tail)) = kernels::native_filter_5x5_binomial_la_rows(
            img.as_bytes(),
            &mut output,
            img.width() as usize,
            img.height() as usize,
        ) {
            return finish_native_filter5x5(img, output, 2, vector_blocks, scalar_tail).map(Some);
        }
    }
    Ok(None)
}

pub(crate) fn simd_filter_5x5(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Filter5x5 {
        kernel,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Filter5x5 op".into()));
    };
    if super::native_filter_identity_supported_for_image(img, mode, 2) {
        return super::simd_filter_identity(img, mode, "Filter5x5");
    }
    if mode == Some("I") {
        return simd_filter_5x5_i32(img, kernel, *scale, *offset, mode);
    }
    if !super::use_native_byte_convolution_path(img, mode, 2) {
        return Err(super::simd_unsupported("Filter5x5"));
    }
    let channels = super::native_filter_byte_layout(img, mode)
        .ok_or_else(|| super::simd_unsupported("Filter5x5"))?;
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / *scale);
    if super::native_small_uniform_convolution_identity(
        img,
        channels,
        &normalized_kernel,
        5,
        *offset + 0.5,
    ) {
        return super::simd_filter_identity(img, mode, "Filter5x5");
    }
    if let Some(result) = uniform_rgb_5x5_identity(img, kernel, *scale, *offset, mode) {
        return Ok(result);
    }
    #[cfg(not(feature = "parallel"))]
    if let Some(result) = rgb_blur_ring_5x5(img, kernel, *scale, *offset, mode) {
        return Ok(result);
    }
    if let Some(result) = try_specialized_filter5x5(img, kernel, *scale, *offset, mode)? {
        return Ok(result);
    }
    let mut output = img.as_bytes().to_vec();
    crate::compute::record_pipeline_operation_path("vector");
    kernels::native_filter_5x5_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        channels,
        &normalized_kernel,
        *offset + 0.5,
    );
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "parallel"))]
    use super::rgb_blur_ring_5x5;
    use super::uniform_rgb_5x5_identity;
    use crate::compute::pool_cpu::ops::filter::execute_filter5x5;
    use crate::raster::{DynamicImage, RgbImage};

    #[cfg(not(feature = "parallel"))]
    const RGB_BLUR_RING: [f32; 25] = [
        1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0,
        0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    ];

    #[test]
    fn uniform_rgb_box_fast_path_matches_cpu_reference_for_byte_edges() {
        const WIDTH: usize = 257;
        const HEIGHT: usize = 256;
        let kernel = [1.0f32; 25];

        for value in [0u8, 1, 2, 17, 127, 128, 253, 254, 255] {
            let pixel = [value, 255 - value, value.wrapping_mul(31)];
            let raw = pixel.repeat(WIDTH * HEIGHT);
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw)
                    .expect("valid uniform RGB image"),
            );
            let expected = execute_filter5x5(&image, &kernel, 25.0, 0.0, Some("RGB"))
                .expect("CPU Filter5x5 reference");
            let actual = uniform_rgb_5x5_identity(&image, &kernel, 25.0, 0.0, Some("RGB"))
                .expect("uniform RGB fast path admission");
            assert_eq!(actual.as_bytes(), expected.as_bytes(), "pixel={pixel:?}");
        }
    }

    #[test]
    fn uniform_rgb_box_fast_path_rejects_other_modes_and_parameters() {
        const WIDTH: usize = 257;
        const HEIGHT: usize = 256;
        let raw = [17u8, 83, 149].repeat(WIDTH * HEIGHT);
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw).expect("valid uniform RGB image"),
        );
        let kernel = [1.0f32; 25];
        assert!(uniform_rgb_5x5_identity(&image, &kernel, 25.0, 0.0, Some("L")).is_none());
        assert!(uniform_rgb_5x5_identity(&image, &kernel, 24.0, 0.0, Some("RGB")).is_none());
        assert!(uniform_rgb_5x5_identity(&image, &kernel, 25.0, 1.0, Some("RGB")).is_none());
    }

    #[test]
    #[cfg(not(feature = "parallel"))]
    fn rgb_blur_ring_simd_path_matches_cpu_for_unaligned_interleaved_tails() {
        const WIDTH: usize = 257;
        const HEIGHT: usize = 257;
        let raw = (0..WIDTH * HEIGHT * 3)
            .map(|index| ((index * 73 + index / 7 * 29 + 17) % 256) as u8)
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(WIDTH as u32, HEIGHT as u32, raw).expect("valid RGB image"),
        );
        let expected = execute_filter5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB"))
            .expect("CPU RGB BLUR ring reference");
        let actual = rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB"))
            .expect("large RGB BLUR ring SIMD admission");
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }

    #[test]
    #[cfg(not(feature = "parallel"))]
    fn rgb_blur_ring_simd_path_declines_small_or_nonmatching_inputs() {
        let raw = [17u8, 83, 149].repeat(256 * 256);
        let image =
            DynamicImage::ImageRgb8(RgbImage::from_raw(256, 256, raw).expect("valid RGB image"));
        assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("RGB")).is_none());
        assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 15.0, 0.0, Some("RGB")).is_none());
        assert!(rgb_blur_ring_5x5(&image, &RGB_BLUR_RING, 16.0, 0.0, Some("L")).is_none());
    }
}
