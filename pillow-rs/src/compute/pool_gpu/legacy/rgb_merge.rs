use super::*;

#[cfg(target_endian = "little")]
mod execution;

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeRgbMergeDispatch {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixels: u32,
    pub(super) input_bytes: usize,
    pub(super) auxiliary_bytes: usize,
    pub(super) output_bytes: usize,
    pub(super) input_transfer_bytes: u64,
    pub(super) auxiliary_transfer_bytes: u64,
    pub(super) output_transfer_bytes: u64,
    pub(super) output_word_count: u32,
    pub(super) output_pixel_groups: u32,
    pub(super) invocation_row_width: u32,
    pub(super) groups_x: u32,
    pub(super) groups_y: u32,
    pub(super) buffer_words: u32,
}

/// Plan a compact three-L-to-RGB merge. Native L planes and RGB output use
/// four-byte padded transfers; 2D dispatch tiling keeps the per-axis device
/// limit valid even when output groups exceed one axis.
#[cfg(target_endian = "little")]
pub(super) fn plan_gpu_native_rgb_merge(
    width: u32,
    height: u32,
    max_workgroups: u32,
    max_storage_binding_bytes: u32,
    max_buffer_bytes: u64,
) -> Option<NativeRgbMergeDispatch> {
    if width == 0 || height == 0 || max_workgroups == 0 {
        return None;
    }
    let dimensions = CheckedDims::new(width, height, 1).ok()?;
    let pixels = dimensions.total_pixels();
    let input_bytes = pixels;
    let auxiliary_bytes = pixels.checked_mul(2)?;
    let output_bytes = pixels.checked_mul(3)?;
    if pixels > GPU_BUFFER_CAPACITY as usize
        || u32::try_from(pixels).is_err()
        || u32::try_from(auxiliary_bytes).is_err()
        || u32::try_from(output_bytes).is_err()
    {
        return None;
    }
    let transfer = |bytes: usize| {
        u64::try_from(bytes)
            .ok()?
            .checked_add(3)
            .map(|aligned| aligned & !3)
    };
    let input_transfer_bytes = transfer(input_bytes)?;
    let auxiliary_transfer_bytes = transfer(auxiliary_bytes)?;
    let output_transfer_bytes = transfer(output_bytes)?;
    let input_words = u32::try_from(input_transfer_bytes / 4).ok()?;
    let auxiliary_words = u32::try_from(auxiliary_transfer_bytes / 4).ok()?;
    let output_word_count = u32::try_from(output_transfer_bytes / 4).ok()?;
    let output_pixels = u32::try_from(pixels).ok()?;
    let output_pixel_groups = output_pixels.div_ceil(4);
    let required_workgroups = output_pixel_groups.div_ceil(256);
    if required_workgroups == 0 {
        return None;
    }
    let groups_x = required_workgroups.min(max_workgroups);
    let groups_y = required_workgroups.div_ceil(groups_x);
    if groups_y > max_workgroups {
        return None;
    }
    let invocation_row_width = groups_x.checked_mul(16)?;
    let buffer_words = input_words.max(output_word_count);
    let buffer_bytes = u64::from(buffer_words).checked_mul(4)?;
    let binding_limit = u64::from(max_storage_binding_bytes);
    if buffer_words == 0
        || buffer_words > GPU_BUFFER_CAPACITY
        || auxiliary_words > GPU_BUFFER_CAPACITY
        || buffer_bytes > max_buffer_bytes
        || [
            input_transfer_bytes,
            auxiliary_transfer_bytes,
            output_transfer_bytes,
        ]
        .into_iter()
        .any(|bytes| bytes > binding_limit || bytes > max_buffer_bytes)
    {
        return None;
    }
    Some(NativeRgbMergeDispatch {
        width,
        height,
        pixels: output_pixels,
        input_bytes,
        auxiliary_bytes,
        output_bytes,
        input_transfer_bytes,
        auxiliary_transfer_bytes,
        output_transfer_bytes,
        output_word_count,
        output_pixel_groups,
        invocation_row_width,
        groups_x,
        groups_y,
        buffer_words,
    })
}

/// Admit the exact native L,L,L -> RGB merge layout. The compact kernel reads
/// three independent L planes and writes three-byte RGB, so other band modes,
/// logical modes, and operation sequences keep the established shader route.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_rgb_luma_merge_bands<'a>(
    ops: &'a [PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
) -> Option<&'a [crate::image::Image]> {
    if !matches!(mode, None | Some("L")) || !matches!(image, DynamicImage::ImageLuma8(_)) {
        return None;
    }
    let [
        PipelineOp::Merge {
            mode: ColorMode::RGB,
            logical_mode,
            bands,
        },
    ] = ops
    else {
        return None;
    };
    let dimensions = image.dimensions();
    (logical_mode == "RGB"
        && bands.len() == 3
        && bands.iter().all(|band| {
            band.size().ok() == Some(dimensions) && band.mode().ok().as_deref() == Some("L")
        }))
    .then_some(bands.as_ref())
}
