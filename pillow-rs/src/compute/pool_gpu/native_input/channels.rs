use super::{
    CheckedDims, DynamicImage, NativeCoverPackedOutputDispatch, PipelineOp, ResampleFilter,
    TransformMethod, gpu_resize_nearest_uses_coefficients,
    plan_native_cover_packed_output_dispatch,
};
/// Resolve a queued LA PutPixel immediately before Sharpness to the byte
/// patch that must be applied to native input before the GPU dispatch.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn gpu_native_sharpness_la_putpixel(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<(usize, [u8; 2])> {
    if !matches!(logical_mode, None | Some("LA")) {
        return None;
    }
    let [
        PipelineOp::PutPixel {
            x,
            y,
            color,
            palette_index: false,
        },
        PipelineOp::Sharpness { .. },
    ] = ops
    else {
        return None;
    };
    let DynamicImage::ImageLumaA8(la) = image else {
        return None;
    };
    if *x >= image.width() || *y >= image.height() {
        return None;
    }
    let layout = CheckedDims::new(image.width(), image.height(), 2).ok()?;
    let input_bytes = layout.total_bytes();
    if input_bytes == 0 || input_bytes > u32::MAX as usize || la.as_raw().len() != input_bytes {
        return None;
    }
    let pixel_index = u64::from(*y)
        .checked_mul(u64::from(image.width()))?
        .checked_add(u64::from(*x))?;
    let byte_offset = pixel_index.checked_mul(2)?;
    Some((usize::try_from(byte_offset).ok()?, [color.0, color.3]))
}

/// Admit tightly packed L bytes for singleton Sharpness. The shader consumes
/// and emits four samples per storage word so transfers and working data stay
/// compact throughout the GPU operation.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn gpu_native_sharpness_l_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Sharpness { .. }]) || !matches!(logical_mode, None | Some("L")) {
        return false;
    }
    let DynamicImage::ImageLuma8(luma) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && luma.as_raw().len() == input_bytes
}

/// Admit tightly packed LA bytes for singleton Sharpness, optionally folding
/// one queued LA PutPixel into the upload. The shader reads native two-byte
/// pixels and emits the existing packed RGBA transport, which the readback
/// path narrows directly to LA without constructing an RGBA image.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn gpu_native_sharpness_la_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    let standalone = matches!(ops, [PipelineOp::Sharpness { .. }]);
    if !matches!(logical_mode, None | Some("LA"))
        || (!standalone && gpu_native_sharpness_la_putpixel(ops, image, logical_mode).is_none())
    {
        return false;
    }
    let DynamicImage::ImageLumaA8(la) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 2) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && la.as_raw().len() == input_bytes
}

/// Admit tightly packed RGB bytes for a singleton Sharpness dispatch. The
/// shader gathers each three-byte pixel from the storage words and synthesizes
/// the opaque fourth transport byte in registers.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn gpu_native_sharpness_rgb_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Sharpness { .. }]) || !matches!(logical_mode, None | Some("RGB"))
    {
        return false;
    }
    let DynamicImage::ImageRgb8(rgb) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 3) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && rgb.as_raw().len() == input_bytes
}

/// Admit tightly packed RGB bytes for a singleton Reduce dispatch. The
/// shader gathers each three-byte sample from storage words and supplies the
/// opaque alpha byte that the existing RGB averaging path expects.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn gpu_native_reduce_rgb_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(ops, [PipelineOp::Reduce { .. }]) || !matches!(logical_mode, None | Some("RGB")) {
        return false;
    }
    let DynamicImage::ImageRgb8(rgb) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 3) else {
        return false;
    };
    let input_bytes = layout.total_bytes();
    input_bytes > 0 && input_bytes <= u32::MAX as usize && rgb.as_raw().len() == input_bytes
}

/// Return the packed native-byte channel count for a singleton Expand batch.
/// When the adapter's bounded output-word grid fits, the shader also writes
/// the result in this compact native layout; otherwise the generic RGBA output
/// path remains available.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn gpu_native_expand_channels(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<u8> {
    let [PipelineOp::Expand { .. }] = ops else {
        return None;
    };
    let channels = match (logical_mode, image) {
        (None | Some("L"), DynamicImage::ImageLuma8(_)) => 1u8,
        (None | Some("LA"), DynamicImage::ImageLumaA8(_)) => 2,
        (None | Some("RGB" | "HSV"), DynamicImage::ImageRgb8(_)) => 3,
        // CMYK's carrier also has four bytes, but keeps C/M/Y/K byte order;
        // singleton Expand copies those samples without color interpretation.
        (None | Some("RGBA" | "CMYK"), DynamicImage::ImageRgba8(_)) => 4,
        _ => return None,
    };
    let input_bytes = u64::from(image.width())
        .checked_mul(u64::from(image.height()))?
        .checked_mul(u64::from(channels))?;
    (input_bytes > 0 && input_bytes <= u64::from(u32::MAX)).then_some(channels)
}

/// Return the native input stride for a singleton Cover whose byte layout
/// would otherwise be widened to the generic RGBA transport. The resize
/// shader still emits its established packed output; this admission only
/// changes how the source samples are read.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn gpu_native_cover_input_channels(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<u8> {
    let [PipelineOp::Cover { .. }] = ops else {
        return None;
    };
    let channels = match (logical_mode, image) {
        (None | Some("L"), DynamicImage::ImageLuma8(_)) => 1u8,
        (None | Some("LA"), DynamicImage::ImageLumaA8(_)) => 2,
        (None | Some("RGB" | "HSV"), DynamicImage::ImageRgb8(_)) => 3,
        _ => return None,
    };
    let input_bytes = u64::from(image.width())
        .checked_mul(u64::from(image.height()))?
        .checked_mul(u64::from(channels))?;
    (input_bytes > 0
        && input_bytes <= u64::from(u32::MAX)
        && usize::try_from(input_bytes).ok()? == image.as_bytes().len())
    .then_some(channels)
}

pub(in crate::compute::pool_gpu) fn gpu_native_cover_packed_output_dispatch(
    ops: &[PipelineOp],
    native_cover_input_channels: Option<u8>,
    logical_mode: Option<&str>,
    source_height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    buffer_capacity_pixels: u32,
) -> Option<NativeCoverPackedOutputDispatch> {
    let [PipelineOp::Resize { w, h, filter }] = ops else {
        return None;
    };
    let Some(channels @ (1 | 2)) = native_cover_input_channels else {
        return None;
    };
    if !matches!(
        (channels, logical_mode),
        (1, None | Some("L")) | (2, None | Some("LA"))
    ) || (matches!(filter, ResampleFilter::Nearest)
        && !gpu_resize_nearest_uses_coefficients(logical_mode))
    {
        return None;
    }
    let buffer_capacity_bytes = u64::from(buffer_capacity_pixels).checked_mul(4)?;
    plan_native_cover_packed_output_dispatch(
        *w,
        *h,
        source_height,
        channels,
        max_workgroups_per_dimension,
        max_storage_buffer_binding_size,
        max_buffer_size,
        buffer_capacity_bytes,
    )
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_cover_input_channels(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> Option<u8> {
    None
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_expand_channels(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> Option<u8> {
    None
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_extract_band_channels(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> Option<u8> {
    None
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_grayscale_rgb_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_grayscale_cmyk_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_sharpness_la_putpixel(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> Option<(usize, [u8; 2])> {
    None
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_sharpness_l_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_sharpness_la_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_sharpness_rgb_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_reduce_rgb_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}

/// Admit one native LA affine-nearest transform. The matching packed shader
/// reads and writes two-byte luma/alpha samples directly, without RGBA staging.
#[cfg(target_endian = "little")]
pub(in crate::compute::pool_gpu) fn gpu_native_la_transform_input(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(
        ops,
        [PipelineOp::Transform {
            method: TransformMethod::Affine,
            filter: ResampleFilter::Nearest,
            ..
        }]
    ) || !matches!(logical_mode, None | Some("LA"))
    {
        return false;
    }
    let DynamicImage::ImageLumaA8(la) = image else {
        return false;
    };
    let Ok(layout) = CheckedDims::new(image.width(), image.height(), 2) else {
        return false;
    };
    let byte_count = layout.total_bytes();
    byte_count > 0 && byte_count <= u32::MAX as usize && la.as_raw().len() == byte_count
}

#[cfg(not(target_endian = "little"))]
pub(in crate::compute::pool_gpu) fn gpu_native_la_transform_input(
    _ops: &[PipelineOp],
    _image: &DynamicImage,
    _logical_mode: Option<&str>,
) -> bool {
    false
}
