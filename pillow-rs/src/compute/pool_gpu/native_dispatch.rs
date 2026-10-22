//! GPU layout-specific dispatch planning for native byte operations.

use super::*;

pub(super) const PACKED_LUMA_WORKGROUP_SIZE: u64 = 256;
pub(super) const PACKED_LA_WORKGROUP_SIZE: u64 = 256;

/// Plan a one-dimensional dispatch over packed L samples. Each invocation
/// handles four adjacent bytes in one storage word, so the grid follows the
/// compact transfer rather than four-byte RGBA pixels.
pub(super) fn plan_packed_luma_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU packed-luma image is too large".into()))?;
    let words = pixels.div_ceil(4);
    if pixels == 0 || words > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU packed-luma dimensions exceed shader indexing limits".into(),
        ));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }

    let groups = words.div_ceil(PACKED_LUMA_WORKGROUP_SIZE).max(1);
    if groups > u64::from(max_workgroups_per_dimension) {
        return Err(PilError::ValueError(
            "GPU packed-luma dispatch exceeds adapter workgroup limits".into(),
        ));
    }
    Ok((
        u32::try_from(groups)
            .map_err(|_| PilError::ValueError("GPU packed-luma dispatch is too wide".into()))?,
        1,
    ))
}

/// Plan dispatch for native-LA words. Each little-endian u32 stores two
/// adjacent interleaved LA pixels, so its compact word count is ceil(pixels/2).
pub(super) fn plan_packed_la_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU packed-LA image is too large".into()))?;
    let words = pixels.div_ceil(2);
    if pixels == 0 || pixels > u64::from(u32::MAX) || words > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU packed-LA dimensions exceed shader indexing limits".into(),
        ));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }

    let groups = words.div_ceil(PACKED_LA_WORKGROUP_SIZE).max(1);
    if groups > u64::from(max_workgroups_per_dimension) {
        return Err(PilError::ValueError(
            "GPU packed-LA dispatch exceeds adapter workgroup limits".into(),
        ));
    }
    Ok((
        u32::try_from(groups)
            .map_err(|_| PilError::ValueError("GPU packed-LA dispatch is too wide".into()))?,
        1,
    ))
}

/// Plan the row-packed resize kernels used by native-L Pad. Unlike the
/// flattened point kernels, the resize shaders use one 16x16 invocation grid
/// over packed output words per row; the full word range must fit WGSL u32
/// indexing and both adapter dispatch axes.
pub(super) fn plan_packed_luma_resize_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU packed-L resize is too large".into()))?;
    let words_per_row = width.div_ceil(4);
    let words = u64::from(words_per_row)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU packed-L resize is too large".into()))?;
    if width == 0 || height == 0 || pixels > u64::from(u32::MAX) || words > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU packed-L resize dimensions exceed shader indexing limits".into(),
        ));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }
    let groups_x = words_per_row.div_ceil(16);
    let groups_y = height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension || groups_y > max_workgroups_per_dimension {
        return Err(PilError::ValueError(
            "GPU packed-L resize exceeds adapter workgroup limits".into(),
        ));
    }
    Ok((groups_x, groups_y))
}

pub(super) fn plan_packed_point_luma_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    plan_packed_luma_dispatch(width, height, max_workgroups_per_dimension)
}

/// Plan compact one-channel output dispatches such as ExtractBand and
/// Grayscale as a near-square 2D grid. One shader invocation writes one packed
/// word containing four output pixels. The flattened word index uses `u32`, so
/// reject dimensions and padded grids outside that index range.
pub(super) const GRAYSCALE_WORKGROUP_SIZE: u64 = 256;

/// Plan one invocation per four-pixel output word across a bounded 2D grid.
pub(super) fn plan_packed_output_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    workgroup_size: u64,
    operation: &'static str,
) -> Result<(u32, u32), PilError> {
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError(format!("{operation} image is too large")))?;
    if pixel_count == 0 || pixel_count > u64::from(u32::MAX) {
        return Err(PilError::ValueError(format!(
            "{operation} dimensions exceed shader indexing limits"
        )));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }
    if workgroup_size == 0 {
        return Err(PilError::ValueError(
            "GPU packed-output workgroup size must be nonzero".into(),
        ));
    }

    let output_words = pixel_count.div_ceil(4);
    let required_workgroups = output_words.div_ceil(workgroup_size).max(1);
    let mut groups_x = required_workgroups.isqrt();
    let groups_x_square = groups_x
        .checked_mul(groups_x)
        .ok_or_else(|| PilError::ValueError(format!("{operation} dispatch overflows")))?;
    if groups_x_square < required_workgroups {
        groups_x = groups_x
            .checked_add(1)
            .ok_or_else(|| PilError::ValueError(format!("{operation} dispatch overflows")))?;
    }
    groups_x = groups_x.min(u64::from(max_workgroups_per_dimension));
    let groups_y = required_workgroups.div_ceil(groups_x);
    if groups_y > u64::from(max_workgroups_per_dimension) {
        return Err(PilError::ValueError(format!(
            "{operation} exceeds adapter workgroup limits"
        )));
    }

    let padded_word_count = groups_x
        .checked_mul(groups_y)
        .and_then(|groups| groups.checked_mul(workgroup_size))
        .ok_or_else(|| PilError::ValueError(format!("{operation} dispatch overflows")))?;
    let Some(max_output_word) = padded_word_count.checked_sub(1) else {
        return Err(PilError::ValueError(format!(
            "{operation} dispatch exceeds shader indexing limits"
        )));
    };
    if max_output_word > u64::from(u32::MAX) {
        return Err(PilError::ValueError(format!(
            "{operation} dispatch exceeds shader indexing limits"
        )));
    }

    Ok((
        u32::try_from(groups_x)
            .map_err(|_| PilError::ValueError(format!("{operation} dispatch is too wide")))?,
        u32::try_from(groups_y)
            .map_err(|_| PilError::ValueError(format!("{operation} dispatch is too tall")))?,
    ))
}

pub(super) fn plan_extract_band_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    plan_packed_output_dispatch(
        width,
        height,
        max_workgroups_per_dimension,
        64,
        "GPU channel extraction",
    )
}

pub(super) const SHARPNESS_L_WORKGROUP_WIDTH: u64 = 16;
pub(super) const SHARPNESS_L_WORKGROUP_HEIGHT: u64 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SharpnessLDispatch {
    pub(super) groups_x: u32,
    pub(super) groups_y: u32,
    pub(super) row_aligned: bool,
}

/// Plan one invocation per four-pixel output word. Word-aligned widths map
/// directly to image rows; other dimensions use a bounded flattened 2D grid.
/// The shader's fourth uniform word selects the same layout returned here.
pub(super) fn plan_sharpness_l_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<SharpnessLDispatch, PilError> {
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU Sharpness image is too large".into()))?;
    if pixel_count == 0 || pixel_count > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU Sharpness dimensions exceed shader indexing limits".into(),
        ));
    }
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }

    let max_groups = u64::from(max_workgroups_per_dimension);
    if width % 4 == 0 {
        let words_per_row = u64::from(width) / 4;
        let groups_x = words_per_row.div_ceil(SHARPNESS_L_WORKGROUP_WIDTH);
        let groups_y = u64::from(height).div_ceil(SHARPNESS_L_WORKGROUP_HEIGHT);
        if groups_x <= max_groups && groups_y <= max_groups {
            return Ok(SharpnessLDispatch {
                groups_x: u32::try_from(groups_x).map_err(|_| {
                    PilError::ValueError("GPU Sharpness dispatch is too wide".into())
                })?,
                groups_y: u32::try_from(groups_y).map_err(|_| {
                    PilError::ValueError("GPU Sharpness dispatch is too tall".into())
                })?,
                row_aligned: true,
            });
        }
    }

    let output_words = pixel_count.div_ceil(4);
    let invocations_per_group = SHARPNESS_L_WORKGROUP_WIDTH * SHARPNESS_L_WORKGROUP_HEIGHT;
    let required_groups = output_words.div_ceil(invocations_per_group).max(1);
    let mut groups_x = required_groups.isqrt();
    if groups_x * groups_x < required_groups {
        groups_x = groups_x
            .checked_add(1)
            .ok_or_else(|| PilError::ValueError("GPU Sharpness dispatch overflows".into()))?;
    }
    groups_x = groups_x.min(max_groups);
    let groups_y = required_groups.div_ceil(groups_x);
    if groups_y > max_groups {
        return Err(PilError::ValueError(
            "GPU Sharpness dispatch exceeds adapter workgroup limits".into(),
        ));
    }
    let padded_words = groups_x
        .checked_mul(groups_y)
        .and_then(|groups| groups.checked_mul(invocations_per_group))
        .ok_or_else(|| PilError::ValueError("GPU Sharpness dispatch overflows".into()))?;
    if padded_words.saturating_sub(1) > u64::from(u32::MAX) {
        return Err(PilError::ValueError(
            "GPU Sharpness dispatch exceeds shader indexing limits".into(),
        ));
    }

    Ok(SharpnessLDispatch {
        groups_x: u32::try_from(groups_x)
            .map_err(|_| PilError::ValueError("GPU Sharpness dispatch is too wide".into()))?,
        groups_y: u32::try_from(groups_y)
            .map_err(|_| PilError::ValueError("GPU Sharpness dispatch is too tall".into()))?,
        row_aligned: false,
    })
}

pub(super) const BLUR_WORKGROUP_SIZE: u32 = 16;

/// Plan bounded 2D blur workgroups. Each invocation computes one output pixel;
/// shader grid-stride loops cover dimensions larger than the device grid.
pub(super) fn plan_blur_dispatch(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
) -> Result<(u32, u32), PilError> {
    if max_workgroups_per_dimension == 0 {
        return Err(PilError::ValueError(
            "GPU adapter reports no compute workgroups per dimension".into(),
        ));
    }
    let max_groups = max_workgroups_per_dimension;
    let groups = |dimension: u32| {
        dimension
            .div_ceil(BLUR_WORKGROUP_SIZE)
            .max(1)
            .min(max_groups)
    };
    Ok((groups(width), groups(height)))
}

#[cfg(target_endian = "little")]
pub(super) struct PackedRgbTransposeLayout {
    pub(super) native_bytes: usize,
    pub(super) transfer_bytes: u64,
    pub(super) workgroups: u32,
}

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeByteFlipDispatch {
    pub(super) row_bytes: usize,
    pub(super) words_per_row: u32,
    pub(super) word_count: u32,
    pub(super) transfer_bytes: u64,
    pub(super) groups_x: u32,
    pub(super) groups_y: u32,
}

/// Tile the native Composite shader's packed-word work across the selected
/// adapter's two-dimensional workgroup grid.
pub(super) fn plan_native_composite_dispatch(
    word_count: u32,
    max_workgroups_per_dimension: u32,
) -> Option<(u32, u32)> {
    if word_count == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let total_workgroups = word_count.div_ceil(64);
    let groups_x = total_workgroups.min(max_workgroups_per_dimension);
    let groups_y = total_workgroups.div_ceil(groups_x);
    (groups_y <= max_workgroups_per_dimension).then_some((groups_x, groups_y))
}

#[cfg(target_endian = "little")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeFlipFormat {
    Mode1,
    L,
    Palette,
    LA,
    PA,
    RGB,
    RGBA,
}

#[cfg(target_endian = "little")]
pub(super) enum NativeGeometryDispatchOutcome {
    NotApplicable,
    ExceedsLimits(&'static str),
    Executed(DynamicImage),
}

#[cfg(target_endian = "little")]
impl NativeFlipFormat {
    pub(super) fn channels(self) -> u8 {
        match self {
            Self::Mode1 | Self::L | Self::Palette => 1,
            Self::LA | Self::PA => 2,
            Self::RGB => 3,
            Self::RGBA => 4,
        }
    }
}

/// Keep the logical image mode attached to its concrete byte layout before
/// selecting the native Flip kernels. Mode 1 and P use one stored byte per
/// pixel; PA and LA use two; RGB and RGBA retain their own packed kernels.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_flip_format(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<NativeFlipFormat> {
    if !matches!(ops, [PipelineOp::Flip]) {
        return None;
    }
    match (logical_mode, image) {
        (Some("1"), DynamicImage::ImageLuma8(_)) => Some(NativeFlipFormat::Mode1),
        (Some("L") | None, DynamicImage::ImageLuma8(_)) => Some(NativeFlipFormat::L),
        (Some("P"), DynamicImage::ImageLuma8(_)) => Some(NativeFlipFormat::Palette),
        (Some("LA") | None, DynamicImage::ImageLumaA8(_)) => Some(NativeFlipFormat::LA),
        (Some("PA"), DynamicImage::ImageLumaA8(_)) => Some(NativeFlipFormat::PA),
        (Some("RGB") | None, DynamicImage::ImageRgb8(_)) => Some(NativeFlipFormat::RGB),
        (Some("RGBA") | None, DynamicImage::ImageRgba8(_)) => Some(NativeFlipFormat::RGBA),
        _ => None,
    }
}

/// Plan one native one- or two-byte-per-pixel Flip. Each GPU row is padded to
/// a four-byte boundary at upload, so each shader invocation can copy one
/// complete word and owns exactly one output word. Reject any shape whose
/// products, adapter limits, working set, or flattened two-dimensional grid
/// cannot be represented by the shader's u32 indexing.
#[cfg(target_endian = "little")]
pub(super) fn plan_native_byte_flip_dispatch(
    width: u32,
    height: u32,
    channels: u8,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    buffer_capacity_words: u32,
) -> Option<NativeByteFlipDispatch> {
    if width == 0 || height == 0 || !matches!(channels, 1 | 2) || max_workgroups_per_dimension == 0
    {
        return None;
    }
    let dimensions = CheckedDims::new(width, height, channels).ok()?;
    let row_bytes = usize::try_from(width)
        .ok()?
        .checked_mul(usize::from(channels))?;
    let native_bytes = dimensions.total_bytes();
    if u32::try_from(native_bytes).is_err() || row_bytes > u32::MAX as usize {
        return None;
    }
    let words_per_row = u32::try_from(row_bytes.div_ceil(4)).ok()?;
    let word_count = u64::from(words_per_row).checked_mul(u64::from(height))?;
    let transfer_bytes = word_count.checked_mul(4)?;
    if word_count == 0
        || word_count > u64::from(u32::MAX)
        || word_count > u64::from(buffer_capacity_words)
        || transfer_bytes > u64::from(max_storage_buffer_binding_size)
        || transfer_bytes > max_buffer_size
        || usize::try_from(transfer_bytes).is_err()
    {
        return None;
    }
    let required_workgroups = word_count.div_ceil(64);
    let groups_x = required_workgroups.min(u64::from(max_workgroups_per_dimension));
    let groups_y = required_workgroups.div_ceil(groups_x);
    if groups_y > u64::from(max_workgroups_per_dimension) {
        return None;
    }
    Some(NativeByteFlipDispatch {
        row_bytes,
        words_per_row,
        word_count: u32::try_from(word_count).ok()?,
        transfer_bytes,
        groups_x: u32::try_from(groups_x).ok()?,
        groups_y: u32::try_from(groups_y).ok()?,
    })
}

#[cfg(target_endian = "little")]
pub(super) enum NativeTransposeInput<'a> {
    Rgb(&'a RgbImage, &'a PackedRgbTransposeLayout),
    Rgba(&'a RgbaImage),
}

#[cfg(target_endian = "little")]
impl NativeTransposeInput<'_> {
    pub(super) fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Rgb(image, _) => image.dimensions(),
            Self::Rgba(image) => image.dimensions(),
        }
    }

    pub(super) fn channels(&self) -> u8 {
        match self {
            Self::Rgb(..) => 3,
            Self::Rgba(_) => 4,
        }
    }

    pub(super) fn bytes(&self) -> &[u8] {
        match self {
            Self::Rgb(image, _) => image.as_raw(),
            Self::Rgba(image) => image.as_raw(),
        }
    }
}

#[cfg(target_endian = "little")]
pub(super) fn mapped_rgba_transpose_is_supported(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
    shared_metal: bool,
) -> bool {
    shared_metal
        && matches!(mode, None | Some("RGBA"))
        && matches!(ops, [PipelineOp::Transpose { .. }])
        && matches!(image, DynamicImage::ImageRgba8(_))
}

#[cfg(target_endian = "little")]
pub(super) fn packed_rgb_transpose_layout(
    width: u32,
    height: u32,
    max_workgroups: u32,
) -> Option<PackedRgbTransposeLayout> {
    let dimensions = CheckedDims::new(width, height, 3).ok()?;
    // The shader indexes bytes and computes ceil(3 * pixels / 4) in u32.
    // Validate the addition as well as the multiplication before admission.
    let native_bytes = u32::try_from(dimensions.total_bytes()).ok()?;
    let transfer_bytes = native_bytes.checked_add(3)? & !3;
    let pixels = u32::try_from(dimensions.total_pixels()).ok()?;
    // transpose_rgb.wgsl has 128 invocations, each owning four output pixels.
    let workgroups = pixels.div_ceil(512);
    (workgroups <= max_workgroups).then_some(PackedRgbTransposeLayout {
        native_bytes: dimensions.total_bytes(),
        transfer_bytes: u64::from(transfer_bytes),
        workgroups,
    })
}

#[cfg(target_endian = "little")]
pub(super) fn packed_rgb_transpose_tiled_dispatch(
    method: &TransposeMethod,
    width: u32,
    height: u32,
    max_workgroups: u32,
) -> Option<(u32, u32)> {
    // Each lane writes four adjacent RGB pixels as three complete words.
    // Aligned output rows make those words exclusive to that lane, including
    // partial 16x16 tiles. Other layouts keep the general packed shader.
    if width == 0
        || height == 0
        || !width.is_multiple_of(4)
        || !matches!(
            method,
            TransposeMethod::Rotate90
                | TransposeMethod::Rotate270
                | TransposeMethod::Transpose
                | TransposeMethod::Transverse
        )
    {
        return None;
    }
    let groups_x = width.div_ceil(16);
    let groups_y = height.div_ceil(16);
    (groups_x <= max_workgroups && groups_y <= max_workgroups).then_some((groups_x, groups_y))
}

pub(super) fn compact_luma8_transfer_bytes(width: u32, height: u32) -> Result<u64, PilError> {
    let pixel_count = CheckedDims::new(width, height, 1)?.total_pixels();
    let transfer_bytes = pixel_count
        .div_ceil(std::mem::size_of::<u32>())
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU luma readback is too large".into()))?;
    u64::try_from(transfer_bytes)
        .map_err(|_| PilError::ValueError("GPU luma readback is too large".into()))
}

pub(super) fn compact_native_channel_transfer_bytes(
    width: u32,
    height: u32,
    channels: u8,
) -> Result<u64, PilError> {
    let native_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
    let transfer_bytes = native_bytes
        .div_ceil(std::mem::size_of::<u32>())
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU native channel transfer is too large".into()))?;
    u64::try_from(transfer_bytes)
        .map_err(|_| PilError::ValueError("GPU native channel transfer is too large".into()))
}
