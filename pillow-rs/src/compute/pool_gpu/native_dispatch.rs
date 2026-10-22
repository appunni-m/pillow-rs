//! GPU layout-specific dispatch planning for native byte operations.

use super::*;

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
