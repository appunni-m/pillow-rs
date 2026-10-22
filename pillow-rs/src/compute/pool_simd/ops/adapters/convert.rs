//! SIMD Convert family adapter helpers.
//!
//! Keep mode and architecture dispatch here; focused kernels perform native
//! byte-layout work and report the path used to the registry-facing adapter.

#[path = "convert/cmyk.rs"]
mod cmyk;
#[cfg(all(test, target_arch = "x86_64"))]
#[path = "convert/cmyk_packing_tests.rs"]
mod cmyk_packing_tests;
#[path = "convert/kernels/mod.rs"]
mod kernels;
use self::cmyk::native_cmyk_to_rgb_bytes;

use self::kernels::native_rgba_to_rgb_bytes;

use super::{
    NativeConvertLayout, grayscale, native_convert_bytes, native_convert_layout,
    native_convert_luma16_bytes, native_convert_supported_for_image, native_convert_target,
    native_rgb_to_rgba_bytes, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp};
use crate::raster::DynamicImage;

#[inline(always)]
fn finish_vector_conversion(
    img: &DynamicImage,
    output: Vec<u8>,
    channels: usize,
    vector_blocks: u64,
    scalar_tail: u64,
    path: &'static str,
    record_zero: bool,
) -> Result<DynamicImage, PilError> {
    if record_zero || vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if record_zero || scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path(path);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
}

fn try_source_special_conversion(
    img: &DynamicImage,
    target: &ColorMode,
    matrix: Option<&[f64]>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if (img.width() == 0 || img.height() == 0)
        && native_convert_supported_for_image(img, target, matrix, mode)
    {
        let Some((channels, ..)) = native_convert_target(target) else {
            return Err(simd_unsupported("Convert"));
        };
        crate::compute::record_pipeline_operation_path("scalar-control");
        return crate::image_utils::raw_bytes_to_image_allow_empty(
            img.width(),
            img.height(),
            Vec::new(),
            channels,
        )
        .map(Some);
    }
    if matches!(target, ColorMode::RGB) && mode == Some("CMYK") {
        if !native_convert_supported_for_image(img, target, matrix, mode) {
            return Err(simd_unsupported("Convert"));
        }
        let (output, vector_blocks, scalar_tail) =
            native_cmyk_to_rgb_bytes(img).ok_or_else(|| simd_unsupported("Convert"))?;
        return finish_vector_conversion(
            img,
            output,
            3,
            vector_blocks,
            scalar_tail,
            "vector",
            false,
        )
        .map(Some);
    }
    if let Some((output, vector_blocks, scalar_tail)) =
        native_convert_luma16_bytes(img, target, mode)
    {
        let channels = match target {
            ColorMode::L => 1,
            ColorMode::LA => 2,
            ColorMode::RGB => 3,
            ColorMode::RGBA | ColorMode::CMYK => 4,
            _ => return Err(simd_unsupported("Convert")),
        };
        return finish_vector_conversion(
            img,
            output,
            channels,
            vector_blocks,
            scalar_tail,
            "vector",
            true,
        )
        .map(Some);
    }
    Ok(None)
}

fn try_layout_special_conversion(
    img: &DynamicImage,
    target: &ColorMode,
    layout: NativeConvertLayout,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if matches!(target, ColorMode::RGB)
        && matches!(mode, None | Some("RGBA"))
        && matches!(img, DynamicImage::ImageRgba8(_))
    {
        if let Some((output, vector_blocks, scalar_tail, path)) = native_rgba_to_rgb_bytes(img) {
            return finish_vector_conversion(
                img,
                output,
                3,
                vector_blocks,
                scalar_tail,
                path,
                false,
            )
            .map(Some);
        }
    }
    if matches!(target, ColorMode::L)
        && layout.source_channels == 3
        && matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(mode, None | Some("RGB"))
    {
        let Some((output, vector_blocks, scalar_tail)) = grayscale::native_grayscale_bytes(img, 3)
        else {
            return Err(simd_unsupported("Convert"));
        };
        return finish_vector_conversion(
            img,
            output,
            1,
            vector_blocks,
            scalar_tail,
            "vector",
            true,
        )
        .map(Some);
    }
    if matches!(target, ColorMode::RGBA)
        && layout.source_channels == 3
        && matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(mode, None | Some("RGB"))
    {
        let Some((output, vector_blocks, scalar_tail)) = native_rgb_to_rgba_bytes(img) else {
            return Err(simd_unsupported("Convert"));
        };
        return finish_vector_conversion(
            img,
            output,
            4,
            vector_blocks,
            scalar_tail,
            "vector",
            true,
        )
        .map(Some);
    }
    Ok(None)
}

pub(crate) fn simd_convert(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Convert {
        mode: target,
        matrix,
        dither: _,
    } = op
    else {
        return Err(PilError::ValueError("expected Convert op".into()));
    };
    if let Some(result) = try_source_special_conversion(img, target, matrix.as_deref(), mode)? {
        return Ok(result);
    }
    let Some(layout) = native_convert_layout(img, target, mode) else {
        return Err(simd_unsupported("Convert"));
    };
    if !native_convert_supported_for_image(img, target, matrix.as_deref(), mode) {
        return Err(simd_unsupported("Convert"));
    }
    if let Some(result) = try_layout_special_conversion(img, target, layout, mode)? {
        return Ok(result);
    }
    let Some((output, vector_blocks, scalar_tail)) = native_convert_bytes(img, layout) else {
        return Err(simd_unsupported("Convert"));
    };
    if vector_blocks == 0 {
        return Err(simd_unsupported("Convert"));
    }
    finish_vector_conversion(
        img,
        output,
        layout.target_channels,
        vector_blocks,
        scalar_tail,
        "vector",
        true,
    )
}

pub(crate) fn simd_convert_lab(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !matches!(op, PipelineOp::ConvertLab { .. }) {
        return Err(PilError::ValueError("expected ConvertLab op".into()));
    }
    if !matches!(img, DynamicImage::ImageRgb8(_)) || !matches!(mode, None | Some("RGB")) {
        return Err(simd_unsupported("ConvertLab"));
    }
    crate::compute::record_pipeline_operation_path("scalar-control");
    crate::lab::convert_rgb_image(img)
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::native_rgba_to_rgb_bytes;
    use crate::raster::{DynamicImage, RgbaImage};

    #[test]
    fn rgba_to_rgb_dispatch_reports_the_ssse3_kernel() {
        if !std::is_x86_feature_detected!("ssse3") {
            return;
        }
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(32, 1, (0..128).map(|byte| byte as u8).collect())
                .expect("RGBA source has four bytes per pixel"),
        );
        let (_, vector_blocks, scalar_tail, path) =
            native_rgba_to_rgb_bytes(&image).expect("SSSE3 conversion is available");

        assert_eq!(path, "ssse3-rgba-to-rgb");
        assert_eq!(vector_blocks, 2);
        assert_eq!(scalar_tail, 0);
    }
}
