//! SIMD Flip and Mirror family adapters and native-layout kernels.

use super::{
    SimdImageShape, copy_native_bytes, has_nonempty_byte_data, has_valid_luma16_copy_bytes,
    has_vectorized_mirror_rows, mirror_native_row, native_copy_layout,
    native_luma16_image_from_bytes, preserve_mode, shape_has_nonempty_byte_data,
    shape_has_vectorized_mirror_rows, shape_luma16_copy_layout, shape_native_copy_channels,
    simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let native_luma16_copy =
        has_valid_luma16_copy_bytes(img, mode) && img.width() != 0 && img.height() != 0;
    match op {
        PipelineOp::Flip => {
            native_copy_layout(img, mode)
                .is_some_and(|channels| has_nonempty_byte_data(img, channels))
                || native_luma16_copy
        }
        PipelineOp::Mirror => {
            native_copy_layout(img, mode)
                .is_some_and(|channels| has_vectorized_mirror_rows(img, channels))
                || native_luma16_copy
        }
        _ => false,
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let native_luma16_copy =
        shape_luma16_copy_layout(shape, mode) && shape.width != 0 && shape.height != 0;
    match op {
        PipelineOp::Flip => {
            shape_native_copy_channels(shape, mode)
                .is_some_and(|channels| shape_has_nonempty_byte_data(shape, channels))
                || native_luma16_copy
        }
        PipelineOp::Mirror => {
            shape_native_copy_channels(shape, mode)
                .is_some_and(|channels| shape_has_vectorized_mirror_rows(shape, channels))
                || native_luma16_copy
        }
        _ => false,
    }
}

/// Move complete rows in reverse order through the native byte-copy path.
/// Callers calculate each mode's own row width and storage width first. A
/// vertical flip has no per-sample arithmetic to vectorize, so bulk row copies
/// avoid paying SIMD lane construction and zero-fill costs. Samples remain
/// opaque and are never expanded to another mode.
fn native_flip_rows(
    img: &DynamicImage,
    width: u32,
    height: u32,
    row_len: usize,
    stored_bytes_per_pixel: usize,
) -> Option<(DynamicImage, u64, u64)> {
    let total_len = row_len.checked_mul(height as usize)?;
    let source = img.as_bytes().get(..total_len)?;
    if source.len() != img.as_bytes().len()
        || row_len != (width as usize).checked_mul(stored_bytes_per_pixel)?
    {
        return None;
    }
    // The native DynamicImage flip already reserves exact capacity and copies
    // complete typed rows. Reuse that implementation after this mode/storage
    // proof rather than maintaining a second row-reversal kernel here.
    let result = img.flipv();
    Some((preserve_mode(img, result), 0, 0))
}

fn native_flip_mode1(img: &DynamicImage) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageLuma8(image) = img else {
        return None;
    };
    let (width, height) = image.dimensions();
    let mode1_row_bytes = usize::try_from(width).ok()?;
    native_flip_rows(img, width, height, mode1_row_bytes, 1)
}

fn native_flip_l(img: &DynamicImage) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageLuma8(image) = img else {
        return None;
    };
    let (width, height) = image.dimensions();
    let l_row_bytes = usize::try_from(width).ok()?;
    native_flip_rows(img, width, height, l_row_bytes, 1)
}

fn native_flip_palette(img: &DynamicImage) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageLuma8(image) = img else {
        return None;
    };
    let (width, height) = image.dimensions();
    let palette_index_row_bytes = usize::try_from(width).ok()?;
    native_flip_rows(img, width, height, palette_index_row_bytes, 1)
}

fn native_flip_la(img: &DynamicImage) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageLumaA8(image) = img else {
        return None;
    };
    let (width, height) = image.dimensions();
    let la_row_bytes = usize::try_from(width).ok()?.checked_mul(2)?;
    native_flip_rows(img, width, height, la_row_bytes, 2)
}

fn native_flip_pa(img: &DynamicImage) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageLumaA8(image) = img else {
        return None;
    };
    let (width, height) = image.dimensions();
    let palette_alpha_row_bytes = usize::try_from(width).ok()?.checked_mul(2)?;
    native_flip_rows(img, width, height, palette_alpha_row_bytes, 2)
}

fn native_flip_rgb(img: &DynamicImage) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageRgb8(image) = img else {
        return None;
    };
    let (width, height) = image.dimensions();
    let rgb_row_bytes = usize::try_from(width).ok()?.checked_mul(3)?;
    native_flip_rows(img, width, height, rgb_row_bytes, 3)
}

fn native_flip_rgba(img: &DynamicImage) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageRgba8(image) = img else {
        return None;
    };
    let (width, height) = image.dimensions();
    let rgba_row_bytes = usize::try_from(width).ok()?.checked_mul(4)?;
    native_flip_rows(img, width, height, rgba_row_bytes, 4)
}

/// Choose a direct kernel from the logical mode and concrete native buffer.
/// Modes sharing a raster type still have separate row calculations so an
/// indexed or alpha-bearing mode cannot silently inherit RGBA semantics.
fn native_flip_vertical(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(DynamicImage, u64, u64)> {
    match (mode, img) {
        (Some("1"), DynamicImage::ImageLuma8(_)) => native_flip_mode1(img),
        (Some("L"), DynamicImage::ImageLuma8(_)) => native_flip_l(img),
        (Some("P"), DynamicImage::ImageLuma8(_)) => native_flip_palette(img),
        (Some("LA"), DynamicImage::ImageLumaA8(_)) => native_flip_la(img),
        (Some("PA"), DynamicImage::ImageLumaA8(_)) => native_flip_pa(img),
        (Some("RGB"), DynamicImage::ImageRgb8(_)) => native_flip_rgb(img),
        (Some("HSV" | "YCbCr"), DynamicImage::ImageRgb8(_)) => native_flip_rgb(img),
        (Some("RGBA"), DynamicImage::ImageRgba8(_)) => native_flip_rgba(img),
        (Some("CMYK" | "RGBa" | "RGBX" | "I" | "F"), DynamicImage::ImageRgba8(_)) => {
            native_flip_rgba(img)
        }
        (None, DynamicImage::ImageLuma8(_)) => native_flip_l(img),
        (None, DynamicImage::ImageLumaA8(_)) => native_flip_la(img),
        (None, DynamicImage::ImageRgb8(_)) => native_flip_rgb(img),
        (None, DynamicImage::ImageRgba8(_)) => native_flip_rgba(img),
        _ => None,
    }
}

/// Flip a typed `I;16*` image vertically without converting samples to a
/// color representation. The row order is scalar control; each opaque
/// two-byte sample span is transferred through the native vector copy.
fn native_flip_luma16(img: &DynamicImage, mode: Option<&str>) -> Option<(DynamicImage, u64, u64)> {
    if !has_valid_luma16_copy_bytes(img, mode) || img.width() == 0 || img.height() == 0 {
        return None;
    }
    let (width, height) = img.dimensions();
    let row_len = (width as usize).checked_mul(2)?;
    let total_len = row_len.checked_mul(height as usize)?;
    let source = img.as_bytes().get(..total_len)?;
    let mut output = vec![0u8; total_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_row in 0..height as usize {
        let source_row = height as usize - 1 - output_row;
        let source_start = source_row.checked_mul(row_len)?;
        let output_start = output_row.checked_mul(row_len)?;
        let (blocks, tail) = copy_native_bytes(
            source.get(source_start..source_start + row_len)?,
            output.get_mut(output_start..output_start + row_len)?,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result = native_luma16_image_from_bytes(width, height, output)?;
    Some((result, vector_blocks, scalar_tail))
}

/// Mirror a typed `I;16*` image horizontally with the same native opaque
/// sample kernel used by the byte layouts. Keeping the two-byte group intact
/// prevents a byte-wise reversal from swapping each sample's endianness.
fn native_mirror_luma16(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(DynamicImage, u64, u64)> {
    if !has_valid_luma16_copy_bytes(img, mode) || img.width() == 0 || img.height() == 0 {
        return None;
    }
    let (width, height) = img.dimensions();
    let row_len = (width as usize).checked_mul(2)?;
    let total_len = row_len.checked_mul(height as usize)?;
    let source = img.as_bytes().get(..total_len)?;
    let mut output = vec![0u8; total_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for row in 0..height as usize {
        let start = row.checked_mul(row_len)?;
        let (blocks, tail) = mirror_native_row(
            source.get(start..start + row_len)?,
            output.get_mut(start..start + row_len)?,
            width as usize,
            2,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result = native_luma16_image_from_bytes(width, height, output)?;
    Some((result, vector_blocks, scalar_tail))
}

/// Fast native-layout mirror for ordinary 8-bit byte images.
fn native_mirror(img: &DynamicImage, mode: Option<&str>) -> Option<(DynamicImage, u64, u64)> {
    let (width, height, channels) = match img {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("1" | "L" | "P")) => {
            (image.width(), image.height(), 1)
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA" | "PA")) => {
            (image.width(), image.height(), 2)
        }
        DynamicImage::ImageRgb8(image) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            (image.width(), image.height(), 3)
        }
        DynamicImage::ImageRgba8(image)
            if matches!(
                mode,
                None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")
            ) =>
        {
            (image.width(), image.height(), 4)
        }
        _ => return None,
    };
    let row_len = (width as usize).checked_mul(channels)?;
    let total_len = row_len.checked_mul(height as usize)?;
    let source = img.as_bytes().get(..total_len)?;
    let mut output = vec![0u8; total_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for row in 0..height as usize {
        let start = row.checked_mul(row_len)?;
        let (blocks, tail) = mirror_native_row(
            source.get(start..start + row_len)?,
            output.get_mut(start..start + row_len)?,
            width as usize,
            channels,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result =
        crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, channels).ok()?;
    Some((result, vector_blocks, scalar_tail))
}

pub fn simd_flip(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // ImageOps.flip is the vertical half of transpose. Reuse the native byte
    // mover so ordinary L/LA/RGB/RGBA images do not pay the packed-RGBA
    // conversion and reconstruction cost used by the scalar fallback.
    if let Some((result, vector_blocks, scalar_tail)) = native_flip_vertical(img, mode) {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(result);
    }
    if let Some((result, vector_blocks, scalar_tail)) = native_flip_luma16(img, mode) {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(result);
    }
    Err(simd_unsupported("Flip"))
}

pub fn simd_mirror(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some((result, vector_blocks, scalar_tail)) = native_mirror(img, mode) {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(result);
    }
    if let Some((result, vector_blocks, scalar_tail)) = native_mirror_luma16(img, mode) {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(result);
    }
    Err(simd_unsupported("Mirror"))
}
