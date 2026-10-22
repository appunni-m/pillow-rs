//! SIMD BoxBlur orchestration and native mode admission.

mod kernels;

#[cfg(target_arch = "aarch64")]
use self::kernels::blur_vertical_rows_radius_seven;
use self::kernels::simd_luma_box_blur_radius_one_rows;
#[cfg(test)]
pub(super) use self::kernels::simd_transpose_interleaved_rows;
pub(super) use self::kernels::{simd_blur_rows, simd_blur_transpose_rows};
pub(super) use self::kernels::{
    simd_luma_horizontal_radius_one_three_passes_rows, simd_luma_vertical_radius_one_rows,
};

use super::{
    CheckedDims, SIMD_BOX_BLUR_SCALE, gaussian_rgba_uniform, native_copy_image_bytes,
    native_small_uniform_byte_image, preserve_mode, simd_native_blur_channels,
    simd_rgb_box_blur_radius_one_rows, simd_rgba_horizontal_radius_one_rows,
    simd_rgba_vertical_radius_one_rows, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

#[derive(Clone, Copy)]
struct BlurAxis {
    radius: usize,
    weight: u32,
    fractional_weight: u32,
}

#[derive(Clone, Copy)]
struct BoxBlurPlan {
    horizontal: BlurAxis,
    vertical: BlurAxis,
    passes: u32,
    gaussian_rgba_candidate: bool,
    use_gaussian_rgba_kernel: bool,
}

pub(super) fn simd_pil_box_blur(
    img: &DynamicImage,
    radius: f32,
    passes: u32,
    channels: usize,
    mode: Option<&str>,
    gaussian_rgba_kernel_candidate: bool,
) -> Result<DynamicImage, PilError> {
    simd_pil_box_blur_xy(
        img,
        radius,
        radius,
        passes,
        channels,
        mode,
        gaussian_rgba_kernel_candidate,
    )
}

pub(super) fn simd_pil_box_blur_xy(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
    channels: usize,
    mode: Option<&str>,
    gaussian_rgba_kernel_candidate: bool,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new(img.width(), img.height(), channels as u8)?;
    if !valid_box_blur_input(img, &dimensions, radius_x, radius_y, passes) {
        return Err(simd_unsupported("BoxBlur"));
    }
    if native_small_uniform_byte_image(img, channels) {
        return native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("BoxBlur"));
    }
    crate::compute::record_pipeline_operation_path("vector");
    let plan = make_box_blur_plan(
        img,
        radius_x,
        radius_y,
        passes,
        channels,
        mode,
        gaussian_rgba_kernel_candidate,
    );
    if let Some(result) = try_large_rgba_identity(img, &dimensions, mode, plan)? {
        return Ok(result);
    }
    if let Some(result) =
        try_small_radius_box_blur(img, &dimensions, mode, channels, radius_x, radius_y, plan)?
    {
        return Ok(result);
    }
    execute_general_box_blur(img, &dimensions, mode, channels, plan)
}

fn valid_box_blur_input(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> bool {
    img.as_bytes().len() == dimensions.total_bytes()
        && radius_x.is_finite()
        && radius_y.is_finite()
        && radius_x >= 0.0
        && radius_y >= 0.0
        && (radius_x != 0.0 || radius_y != 0.0)
        && passes != 0
}

fn blur_axis(radius: f32) -> BlurAxis {
    let integer_radius = radius as i32 as usize;
    let window_pixels = (2 * integer_radius + 1) as u32;
    let weight = (SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    let fractional_weight =
        SIMD_BOX_BLUR_SCALE.wrapping_sub(window_pixels.wrapping_mul(weight)) / 2;
    BlurAxis {
        radius: integer_radius,
        weight,
        fractional_weight,
    }
}

fn make_box_blur_plan(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
    channels: usize,
    mode: Option<&str>,
    gaussian_rgba_candidate: bool,
) -> BoxBlurPlan {
    let horizontal = blur_axis(radius_x);
    let vertical = blur_axis(radius_y);
    let architecture_supported = {
        #[cfg(target_arch = "aarch64")]
        {
            true
        }
        #[cfg(target_arch = "x86_64")]
        {
            std::is_x86_feature_detected!("avx2")
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            false
        }
    };
    let rgba_layout = channels == 4
        && matches!(img, DynamicImage::ImageRgba8(_))
        && matches!(mode, None | Some("RGBA"));
    let kernel_parameters = passes == 3
        && horizontal.radius == 7
        && vertical.radius == 7
        && horizontal.fractional_weight != 0
        && vertical.fractional_weight != 0;
    BoxBlurPlan {
        horizontal,
        vertical,
        passes,
        gaussian_rgba_candidate,
        use_gaussian_rgba_kernel: gaussian_rgba_candidate
            && architecture_supported
            && rgba_layout
            && kernel_parameters,
    }
}

fn try_large_rgba_identity(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    mode: Option<&str>,
    plan: BoxBlurPlan,
) -> Result<Option<DynamicImage>, PilError> {
    let rgba_layout = dimensions.channels == 4
        && matches!(img, DynamicImage::ImageRgba8(_))
        && matches!(mode, None | Some("RGBA"));
    let parameters = plan.passes == 3
        && plan.horizontal.radius == 7
        && plan.vertical.radius == 7
        && plan.horizontal.fractional_weight != 0
        && plan.vertical.fractional_weight != 0;
    if !(plan.gaussian_rgba_candidate && rgba_layout && parameters) {
        return Ok(None);
    }
    let evidence = gaussian_rgba_uniform::prove_identity(
        img.as_bytes(),
        dimensions.width as usize,
        dimensions.height as usize,
        (
            plan.horizontal.radius,
            plan.horizontal.weight,
            plan.horizontal.fractional_weight,
        ),
        (
            plan.vertical.radius,
            plan.vertical.weight,
            plan.vertical.fractional_weight,
        ),
    );
    if evidence.vector_blocks != 0 {
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(evidence.vector_blocks);
        if evidence.scalar_tail_pixels != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(evidence.scalar_tail_pixels);
        }
    }
    if evidence.identity {
        let result =
            native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("GaussianBlur"))?;
        return Ok(Some(result));
    }
    Ok(None)
}

fn try_small_radius_box_blur(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    mode: Option<&str>,
    channels: usize,
    radius_x: f32,
    radius_y: f32,
    plan: BoxBlurPlan,
) -> Result<Option<DynamicImage>, PilError> {
    let radius_one = plan.passes == 1 && radius_x == 1.0 && radius_y == 1.0;
    if radius_one
        && channels == 3
        && matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(mode, None | Some("RGB"))
    {
        let (width, height) = (dimensions.width as usize, dimensions.height as usize);
        let mut output = dimensions.alloc_buffer();
        simd_rgb_box_blur_radius_one_rows(
            img.as_bytes(),
            &mut output,
            width,
            height,
            plan.horizontal.weight,
            plan.horizontal.fractional_weight,
            plan.vertical.weight,
            plan.vertical.fractional_weight,
        );
        return materialize_box_blur(img, dimensions, output, channels).map(Some);
    }
    if radius_one
        && channels == 1
        && matches!(img, DynamicImage::ImageLuma8(_))
        && matches!(mode, None | Some("L"))
    {
        debug_assert_eq!(plan.horizontal.weight, SIMD_BOX_BLUR_SCALE / 3);
        debug_assert_eq!(plan.horizontal.fractional_weight, 0);
        debug_assert_eq!(plan.vertical.weight, SIMD_BOX_BLUR_SCALE / 3);
        debug_assert_eq!(plan.vertical.fractional_weight, 0);
        let mut output = dimensions.alloc_buffer();
        simd_luma_box_blur_radius_one_rows(
            img.as_bytes(),
            &mut output,
            dimensions.width as usize,
            dimensions.height as usize,
        );
        return materialize_box_blur(img, dimensions, output, channels).map(Some);
    }
    Ok(None)
}

fn execute_general_box_blur(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    mode: Option<&str>,
    channels: usize,
    plan: BoxBlurPlan,
) -> Result<DynamicImage, PilError> {
    let (width, height) = (dimensions.width as usize, dimensions.height as usize);
    let mut work = img.as_bytes().to_vec();
    let mut scratch = dimensions.alloc_buffer();
    if let Some(result) = try_rgba_radius_one(
        img,
        dimensions,
        mode,
        channels,
        plan,
        &mut work,
        &mut scratch,
    )? {
        return Ok(result);
    }
    run_horizontal_passes(&mut work, &mut scratch, width, height, channels, plan);
    #[cfg(target_arch = "aarch64")]
    if let Some(result) =
        try_neon_rgba_vertical(img, dimensions, channels, plan, &mut work, &mut scratch)?
    {
        return Ok(result);
    }
    if let Some(result) = try_rgba_vertical_radius_one(
        img,
        dimensions,
        mode,
        channels,
        plan,
        &mut work,
        &mut scratch,
    )? {
        return Ok(result);
    }
    run_transposed_vertical_passes(img, dimensions, channels, plan, &mut work, &mut scratch)
}

fn try_rgba_radius_one(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    mode: Option<&str>,
    channels: usize,
    plan: BoxBlurPlan,
    work: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
) -> Result<Option<DynamicImage>, PilError> {
    if channels != 4
        || !matches!(img, DynamicImage::ImageRgba8(_))
        || !matches!(mode, None | Some("RGBA"))
        || plan.horizontal.radius != 1
        || plan.vertical.radius != 1
    {
        return Ok(None);
    }
    let (width, height) = (dimensions.width as usize, dimensions.height as usize);
    for _ in 0..plan.passes {
        simd_rgba_horizontal_radius_one_rows(
            work,
            scratch,
            width,
            height,
            plan.horizontal.weight,
            plan.horizontal.fractional_weight,
        );
        std::mem::swap(work, scratch);
    }
    for _ in 0..plan.passes {
        simd_rgba_vertical_radius_one_rows(
            work,
            scratch,
            width,
            height,
            plan.vertical.weight,
            plan.vertical.fractional_weight,
        );
        std::mem::swap(work, scratch);
    }
    materialize_box_blur(img, dimensions, std::mem::take(work), channels).map(Some)
}

fn run_horizontal_passes(
    work: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
    width: usize,
    height: usize,
    channels: usize,
    plan: BoxBlurPlan,
) {
    for _ in 0..plan.passes {
        if plan.use_gaussian_rgba_kernel {
            simd_blur_rows::<true>(
                work,
                scratch,
                width,
                height,
                channels,
                plan.horizontal.radius,
                plan.horizontal.weight,
                plan.horizontal.fractional_weight,
            );
        } else {
            simd_blur_rows::<false>(
                work,
                scratch,
                width,
                height,
                channels,
                plan.horizontal.radius,
                plan.horizontal.weight,
                plan.horizontal.fractional_weight,
            );
        }
        std::mem::swap(work, scratch);
    }
}

#[cfg(target_arch = "aarch64")]
fn try_neon_rgba_vertical(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    channels: usize,
    plan: BoxBlurPlan,
    work: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
) -> Result<Option<DynamicImage>, PilError> {
    let (width, height) = (dimensions.width as usize, dimensions.height as usize);
    if !plan.use_gaussian_rgba_kernel || width < 64 || height < 64 {
        return Ok(None);
    }
    for _ in 0..plan.passes {
        let (blocks, tail) = blur_vertical_rows_radius_seven(
            work,
            scratch,
            width,
            height,
            plan.vertical.weight,
            plan.vertical.fractional_weight,
        );
        if blocks != 0 {
            crate::compute::record_pipeline_operation_vector_blocks(blocks);
        }
        if tail != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(tail);
        }
        crate::compute::record_pipeline_operation_path("neon-rgba-row-major-vertical");
        std::mem::swap(work, scratch);
    }
    materialize_box_blur(img, dimensions, std::mem::take(work), channels).map(Some)
}

fn try_rgba_vertical_radius_one(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    mode: Option<&str>,
    channels: usize,
    plan: BoxBlurPlan,
    work: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
) -> Result<Option<DynamicImage>, PilError> {
    if channels != 4
        || !matches!(img, DynamicImage::ImageRgba8(_))
        || !matches!(mode, None | Some("RGBA"))
        || plan.vertical.radius != 1
    {
        return Ok(None);
    }
    let (width, height) = (dimensions.width as usize, dimensions.height as usize);
    for _ in 0..plan.passes {
        simd_rgba_vertical_radius_one_rows(
            work,
            scratch,
            width,
            height,
            plan.vertical.weight,
            plan.vertical.fractional_weight,
        );
        std::mem::swap(work, scratch);
    }
    materialize_box_blur(img, dimensions, std::mem::take(work), channels).map(Some)
}

fn run_transposed_vertical_passes(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    channels: usize,
    plan: BoxBlurPlan,
    work: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
) -> Result<DynamicImage, PilError> {
    let (width, height) = (dimensions.width as usize, dimensions.height as usize);
    let transposed_with_avx2 = simd_blur_transpose_rows(
        work,
        scratch,
        width,
        height,
        channels,
        plan.gaussian_rgba_candidate,
        plan.use_gaussian_rgba_kernel,
    );
    for pass in 0..plan.passes {
        if plan.use_gaussian_rgba_kernel {
            simd_blur_rows::<true>(
                scratch,
                work,
                height,
                width,
                channels,
                plan.vertical.radius,
                plan.vertical.weight,
                plan.vertical.fractional_weight,
            );
        } else {
            simd_blur_rows::<false>(
                scratch,
                work,
                height,
                width,
                channels,
                plan.vertical.radius,
                plan.vertical.weight,
                plan.vertical.fractional_weight,
            );
        }
        if pass + 1 < plan.passes {
            std::mem::swap(work, scratch);
        }
    }
    let restored_with_avx2 = simd_blur_transpose_rows(
        work,
        scratch,
        height,
        width,
        channels,
        plan.gaussian_rgba_candidate,
        plan.use_gaussian_rgba_kernel,
    );
    if transposed_with_avx2 && restored_with_avx2 {
        crate::compute::record_pipeline_operation_path("avx2-transpose");
    }
    materialize_box_blur(img, dimensions, std::mem::take(scratch), channels)
}

fn materialize_box_blur(
    img: &DynamicImage,
    dimensions: &CheckedDims,
    output: Vec<u8>,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let result = crate::image_utils::raw_bytes_to_image(
        dimensions.width,
        dimensions.height,
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

pub(crate) fn simd_box_blur(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let channels =
        simd_native_blur_channels(img, mode).ok_or_else(|| simd_unsupported("BoxBlur"))?;
    let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
    if pixel_count == 0 {
        return Err(simd_unsupported("BoxBlur"));
    }
    match op {
        PipelineOp::BoxBlur { radius } if *radius == 0 => {
            native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("BoxBlur"))
        }
        PipelineOp::BoxBlur { radius } => {
            simd_pil_box_blur(img, *radius as f32, 1, channels, mode, false)
        }
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } if *radius_x == 0.0 && *radius_y == 0.0 => {
            native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("BoxBlur"))
        }
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => simd_pil_box_blur_xy(img, *radius_x, *radius_y, *passes, channels, mode, false),
        _ => Err(PilError::ValueError("expected BoxBlur op".into())),
    }
}
