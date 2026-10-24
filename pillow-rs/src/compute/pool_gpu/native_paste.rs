use super::*;

mod execution;
#[cfg(target_endian = "little")]
mod masked_execution;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeBytePasteDispatch {
    pub(super) destination_bytes: usize,
    pub(super) source_bytes: usize,
    pub(super) destination_transfer_bytes: u64,
    pub(super) source_transfer_bytes: u64,
    pub(super) destination_pixels: u32,
    pub(super) bytes_per_pixel: u8,
    pub(super) word_count: u32,
    pub(super) workgroups: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeRgbRgbaPasteDispatch {
    pub(super) destination_bytes: usize,
    pub(super) source_bytes: usize,
    pub(super) destination_transfer_bytes: u64,
    pub(super) source_transfer_bytes: u64,
    pub(super) input_transfer_bytes: u64,
    pub(super) destination_pixels: u32,
    pub(super) workgroups_x: u32,
    pub(super) workgroups_y: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeMaskedBytePasteDispatch {
    pub(super) destination_bytes: usize,
    pub(super) source_bytes: usize,
    pub(super) mask_bytes: usize,
    pub(super) destination_transfer_bytes: u64,
    pub(super) source_transfer_bytes: u64,
    pub(super) mask_transfer_bytes: u64,
    pub(super) input_transfer_bytes: u64,
    pub(super) destination_pixels: u32,
    pub(super) source_pixels: u32,
    pub(super) source_is_broadcast: bool,
    pub(super) bytes_per_pixel: u8,
    pub(super) word_count: u32,
    pub(super) workgroups: u32,
    pub(super) workgroups_x: u32,
    pub(super) workgroups_y: u32,
}

pub(super) const GPU_MASKED_RGBA_WORKGROUP_SIZE: u32 = 64;

/// Select a shader by both logical mode and packed byte width. LA and PA use
/// the same two-byte carrier but retain separate pipelines so palette indices
/// are never admitted through an alpha-image mode branch.
pub(super) fn gpu_native_masked_byte_paste_shader(
    logical_mode: &str,
    bytes_per_pixel: u8,
) -> Option<(&'static str, &'static str, &'static str)> {
    match (logical_mode, bytes_per_pixel) {
        ("L", 1) => Some((
            "__internal_paste_native_masked_l",
            "paste_native_masked_l.wgsl",
            include_str!("shaders/paste_native_masked_l.wgsl"),
        )),
        ("LA", 2) => Some((
            "__internal_paste_native_masked_la",
            "paste_native_masked_la.wgsl",
            include_str!("shaders/paste_native_masked_la.wgsl"),
        )),
        ("PA", 2) => Some((
            "__internal_paste_native_masked_pa",
            "paste_native_masked_pa.wgsl",
            include_str!("shaders/paste_native_masked_pa.wgsl"),
        )),
        ("RGB" | "HSV", 3) => Some((
            "__internal_paste_native_masked_rgb",
            "paste_native_masked_rgb.wgsl",
            include_str!("shaders/paste_native_masked_rgb.wgsl"),
        )),
        ("RGBA", 4) => Some((
            "__internal_paste_native_masked_rgba",
            "paste_native_masked_rgba.wgsl",
            include_str!("shaders/paste_native_masked_rgba.wgsl"),
        )),
        // CMYK's four stored bytes are C/M/Y/K, not RGBA with an alpha
        // channel. The shader blends all four bytes independently, so it is
        // exact for CMYK while keeping a distinct logical pipeline identity.
        ("CMYK", 4) => Some((
            "__internal_paste_native_masked_cmyk",
            "paste_native_masked_rgba.wgsl",
            include_str!("shaders/paste_native_masked_rgba.wgsl"),
        )),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeRgbCropDispatch {
    pub(super) source_bytes: usize,
    pub(super) output_bytes: usize,
    pub(super) source_transfer_bytes: u64,
    pub(super) output_transfer_bytes: u64,
    pub(super) output_width: u32,
    pub(super) output_height: u32,
    pub(super) output_word_count: u32,
    pub(super) output_pixel_groups: u32,
    pub(super) buffer_words: u32,
    pub(super) workgroups: u32,
}

/// Plan one exact native RGB crop. Both host transfers stay three bytes per
/// pixel; only the final incomplete storage word is padded for WebGPU.
pub(super) fn plan_gpu_native_rgb_crop(
    source_width: u32,
    source_height: u32,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    max_workgroups: u32,
    max_storage_binding_bytes: u32,
    max_buffer_bytes: u64,
) -> Option<NativeRgbCropDispatch> {
    if source_width == 0
        || source_height == 0
        || left >= right
        || top >= bottom
        || right > source_width
        || bottom > source_height
    {
        return None;
    }
    let output_width = right.checked_sub(left)?;
    let output_height = bottom.checked_sub(top)?;
    let source = CheckedDims::new(source_width, source_height, 3).ok()?;
    let output = CheckedDims::new(output_width, output_height, 3).ok()?;
    if source.total_pixels() > GPU_BUFFER_CAPACITY as usize
        || output.total_pixels() > GPU_BUFFER_CAPACITY as usize
    {
        return None;
    }
    // The byte-indexing shader uses u32 offsets. Prove its row strides and
    // total address ranges before dispatch, including packed RGB byte tails.
    source_width.checked_mul(3)?;
    output_width.checked_mul(3)?;
    let source_bytes = source.total_bytes();
    let output_bytes = output.total_bytes();
    let source_transfer_bytes = u64::try_from(source_bytes).ok()?.checked_add(3)? & !3;
    let output_transfer_bytes = u64::try_from(output_bytes).ok()?.checked_add(3)? & !3;
    let source_word_count = u32::try_from(source_transfer_bytes / 4).ok()?;
    let output_word_count = u32::try_from(output_transfer_bytes / 4).ok()?;
    let output_pixel_count = u32::try_from(output.total_pixels()).ok()?;
    let output_pixel_groups = output_pixel_count.div_ceil(4);
    let buffer_words = source_word_count.max(output_word_count);
    if buffer_words == 0 || buffer_words > GPU_BUFFER_CAPACITY {
        return None;
    }
    let binding_limit = u64::from(max_storage_binding_bytes);
    if [source_transfer_bytes, output_transfer_bytes]
        .into_iter()
        .any(|size| size > binding_limit || size > max_buffer_bytes)
        || u32::try_from(source_transfer_bytes).is_err()
        || u32::try_from(output_transfer_bytes).is_err()
        || u32::try_from(output_bytes).is_err()
    {
        return None;
    }
    let workgroups = output_pixel_groups.div_ceil(64);
    if workgroups == 0 || workgroups > max_workgroups {
        return None;
    }
    Some(NativeRgbCropDispatch {
        source_bytes,
        output_bytes,
        source_transfer_bytes,
        output_transfer_bytes,
        output_width,
        output_height,
        output_word_count,
        output_pixel_groups,
        buffer_words,
        workgroups,
    })
}

/// Plan compact native-byte transport for exact L-, LA-, PA-, RGB-, HSV-, or
/// RGBA-to-same-mode masked Paste with an L mask. RGB/HSV invocations own four
/// pixels and emit three aligned words; RGBA uses one word per pixel and a
/// bounded 2D grid when one dispatch axis cannot represent all groups.
#[cfg(test)]
pub(super) fn plan_gpu_native_masked_l_paste(
    destination_width: u32,
    destination_height: u32,
    source_width: u32,
    source_height: u32,
    mask_width: u32,
    mask_height: u32,
    max_workgroups: u32,
    max_storage_binding_bytes: u32,
    max_buffer_bytes: u64,
) -> Option<NativeMaskedBytePasteDispatch> {
    plan_gpu_native_masked_byte_paste(
        destination_width,
        destination_height,
        source_width,
        source_height,
        mask_width,
        mask_height,
        1,
        max_workgroups,
        max_storage_binding_bytes,
        max_buffer_bytes,
    )
}

pub(super) fn plan_gpu_native_masked_byte_paste(
    destination_width: u32,
    destination_height: u32,
    source_width: u32,
    source_height: u32,
    mask_width: u32,
    mask_height: u32,
    bytes_per_pixel: u8,
    max_workgroups: u32,
    max_storage_binding_bytes: u32,
    max_buffer_bytes: u64,
) -> Option<NativeMaskedBytePasteDispatch> {
    if !matches!(bytes_per_pixel, 1 | 2 | 3 | 4) {
        return None;
    }
    let destination =
        CheckedDims::new(destination_width, destination_height, bytes_per_pixel).ok()?;
    let source = CheckedDims::new(source_width, source_height, bytes_per_pixel).ok()?;
    let mask = CheckedDims::new(mask_width, mask_height, 1).ok()?;
    let source_is_broadcast =
        (source_width, source_height) == (1, 1) && (mask_width, mask_height) != (1, 1);
    if destination_width == 0
        || destination_height == 0
        || source_width == 0
        || source_height == 0
        || (!source_is_broadcast && (mask_width, mask_height) != (source_width, source_height))
        || destination.total_pixels() > GPU_BUFFER_CAPACITY as usize
        || source.total_pixels() > GPU_BUFFER_CAPACITY as usize
        || mask.total_pixels() > GPU_BUFFER_CAPACITY as usize
    {
        return None;
    }
    let destination_bytes = destination.total_bytes();
    let source_bytes = source.total_bytes();
    let mask_bytes = mask.total_bytes();
    let destination_pixels = u32::try_from(destination.total_pixels()).ok()?;
    let source_pixels = u32::try_from(source.total_pixels()).ok()?;
    let transfer_bytes = |bytes: usize| {
        u64::try_from(bytes)
            .ok()?
            .checked_add(3)
            .map(|aligned| aligned & !3)
    };
    let destination_transfer_bytes = transfer_bytes(destination_bytes)?;
    let source_transfer_bytes = transfer_bytes(source_bytes)?;
    let mask_transfer_bytes = transfer_bytes(mask_bytes)?;
    let input_transfer_bytes = source_transfer_bytes.checked_add(mask_transfer_bytes)?;
    let binding_limit = u64::from(max_storage_binding_bytes);
    let transfers = [destination_transfer_bytes, input_transfer_bytes];
    if transfers
        .into_iter()
        .any(|size| size > binding_limit || size > max_buffer_bytes)
        || u32::try_from(destination_transfer_bytes).is_err()
        || u32::try_from(source_transfer_bytes).is_err()
        || u32::try_from(mask_transfer_bytes).is_err()
        || u32::try_from(input_transfer_bytes).is_err()
    {
        return None;
    }

    let word_count = u32::try_from(destination_transfer_bytes / 4).ok()?;
    if word_count == 0 {
        return None;
    }
    let work_items = if bytes_per_pixel == 3 {
        destination_pixels.div_ceil(4)
    } else {
        word_count
    };
    let workgroup_size = if bytes_per_pixel == 4 {
        GPU_MASKED_RGBA_WORKGROUP_SIZE
    } else {
        64
    };
    let required_workgroups = work_items.div_ceil(workgroup_size);
    if required_workgroups == 0 || max_workgroups == 0 {
        return None;
    }
    if bytes_per_pixel != 4 && required_workgroups > max_workgroups {
        return None;
    }
    let workgroups_x = if required_workgroups > max_workgroups {
        ceil_sqrt_u32(required_workgroups).min(max_workgroups)
    } else {
        required_workgroups
    };
    let workgroups_y = required_workgroups.div_ceil(workgroups_x);
    if workgroups_y > max_workgroups {
        return None;
    }
    let workgroups = workgroups_x.checked_mul(workgroups_y)?;
    Some(NativeMaskedBytePasteDispatch {
        destination_bytes,
        source_bytes,
        mask_bytes,
        destination_transfer_bytes,
        source_transfer_bytes,
        mask_transfer_bytes,
        input_transfer_bytes,
        destination_pixels,
        source_pixels,
        source_is_broadcast,
        bytes_per_pixel,
        word_count,
        workgroups,
        workgroups_x,
        workgroups_y,
    })
}

/// Return ceil(sqrt(value)) using division for the comparison so the square
/// never overflows. The value is positive for dispatch planning.
pub(super) fn ceil_sqrt_u32(value: u32) -> u32 {
    debug_assert_ne!(value, 0);
    let mut low = 1u32;
    let mut high = value;
    while low < high {
        let middle = low + (high - low) / 2;
        if value.div_ceil(middle) <= middle {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// Plan compact native L/LA/RGB Paste transport without allocating either image.
/// The shader writes one aligned output word per invocation, so its flat
/// dispatch is bounded by the adapter's workgroup and storage limits.
pub(super) fn plan_gpu_native_byte_paste(
    destination_width: u32,
    destination_height: u32,
    source_width: u32,
    source_height: u32,
    bytes_per_pixel: u8,
    max_workgroups: u32,
    max_storage_binding_bytes: u32,
    max_buffer_bytes: u64,
) -> Option<NativeBytePasteDispatch> {
    if !matches!(bytes_per_pixel, 1 | 2 | 3) {
        return None;
    }
    let destination =
        CheckedDims::new(destination_width, destination_height, bytes_per_pixel).ok()?;
    let source = CheckedDims::new(source_width, source_height, bytes_per_pixel).ok()?;
    let destination_bytes = destination.total_bytes();
    let source_bytes = source.total_bytes();
    u32::try_from(destination_bytes).ok()?;
    u32::try_from(source_bytes).ok()?;
    let destination_pixels = u32::try_from(destination.total_pixels()).ok()?;

    let transfer_bytes = |bytes: usize| {
        u64::try_from(bytes)
            .ok()?
            .checked_add(3)
            .map(|aligned| aligned & !3)
    };
    let destination_transfer_bytes = transfer_bytes(destination_bytes)?;
    let source_transfer_bytes = transfer_bytes(source_bytes)?;
    let binding_limit = u64::from(max_storage_binding_bytes);
    if destination_transfer_bytes > binding_limit
        || source_transfer_bytes > binding_limit
        || destination_transfer_bytes > max_buffer_bytes
        || source_transfer_bytes > max_buffer_bytes
    {
        return None;
    }

    let word_count = u32::try_from(destination_transfer_bytes / 4).ok()?;
    if word_count == 0 {
        return None;
    }
    let workgroups = word_count.div_ceil(64);
    if workgroups > max_workgroups {
        return None;
    }
    Some(NativeBytePasteDispatch {
        destination_bytes,
        source_bytes,
        destination_transfer_bytes,
        source_transfer_bytes,
        destination_pixels,
        bytes_per_pixel,
        word_count,
        workgroups,
    })
}

/// Plan an unmasked native RGB-source to RGBA-destination paste. Output
/// invocations own aligned destination pixels; the packed RGB source may
/// cross u32 boundaries and is indexed as a flat three-byte pixel stream.
pub(super) fn plan_gpu_native_rgb_to_rgba_paste(
    destination_width: u32,
    destination_height: u32,
    source_width: u32,
    source_height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_binding_bytes: u32,
    max_buffer_bytes: u64,
) -> Option<NativeRgbRgbaPasteDispatch> {
    if max_workgroups_per_dimension == 0 {
        return None;
    }
    let destination = CheckedDims::new(destination_width, destination_height, 4).ok()?;
    let source = CheckedDims::new(source_width, source_height, 3).ok()?;
    if destination_width == 0 || destination_height == 0 || source_width == 0 || source_height == 0
    {
        return None;
    }
    let destination_bytes = destination.total_bytes();
    let source_bytes = source.total_bytes();
    let destination_pixels = u32::try_from(destination.total_pixels()).ok()?;
    // Shader byte/pixel indexing is u32. Restrict the complete buffers, not
    // just their dimensions, so every multiplication and final byte offset
    // stays representable without wraparound.
    u32::try_from(destination_bytes).ok()?;
    u32::try_from(source_bytes).ok()?;
    let transfer_bytes = |bytes: usize| {
        u64::try_from(bytes)
            .ok()?
            .checked_add(3)
            .map(|aligned| aligned & !3)
    };
    let destination_transfer_bytes = transfer_bytes(destination_bytes)?;
    let source_transfer_bytes = transfer_bytes(source_bytes)?;
    let input_transfer_bytes = destination_transfer_bytes.checked_add(source_transfer_bytes)?;
    let binding_limit = u64::from(max_storage_binding_bytes);
    if destination_transfer_bytes > binding_limit
        || source_transfer_bytes > binding_limit
        || destination_transfer_bytes > max_buffer_bytes
        || source_transfer_bytes > max_buffer_bytes
    {
        return None;
    }

    let output_groups = destination_pixels.div_ceil(64);
    let workgroups_x = output_groups.min(max_workgroups_per_dimension);
    let workgroups_y = output_groups.div_ceil(workgroups_x);
    if workgroups_y > max_workgroups_per_dimension {
        return None;
    }
    Some(NativeRgbRgbaPasteDispatch {
        destination_bytes,
        source_bytes,
        destination_transfer_bytes,
        source_transfer_bytes,
        input_transfer_bytes,
        destination_pixels,
        workgroups_x,
        workgroups_y,
    })
}

/// Admit only exact unmasked L-to-L, LA-to-LA, or RGB-to-RGB Paste layouts.
/// Palette indices, premultiplied LA, and other storage variants keep their
/// semantic path.
pub(super) fn gpu_native_byte_paste_layout(
    op: &PipelineOp,
    destination: &DynamicImage,
    source: &DynamicImage,
    mode: Option<&str>,
) -> Option<((i32, i32), u8)> {
    let PipelineOp::Paste {
        source: source_image,
        x,
        y,
        w,
        h,
        mask: None,
        ..
    } = op
    else {
        return None;
    };
    let source_mode = source_image.mode().ok()?;
    let bytes_per_pixel = match (mode, source_mode.as_str(), destination, source) {
        (Some("L"), "L", DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(_)) => 1,
        (Some("LA"), "LA", DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(_)) => 2,
        (Some("RGB"), "RGB", DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(_)) => 3,
        _ => return None,
    };
    if *w <= 0
        || *h <= 0
        || (u32::try_from(*w).ok()?, u32::try_from(*h).ok()?) != source.dimensions()
        || CheckedDims::new(destination.width(), destination.height(), bytes_per_pixel)
            .ok()?
            .total_bytes()
            != destination.as_bytes().len()
        || CheckedDims::new(source.width(), source.height(), bytes_per_pixel)
            .ok()?
            .total_bytes()
            != source.as_bytes().len()
    {
        return None;
    }
    Some(((*x, *y), bytes_per_pixel))
}

/// Admit only explicit unmasked RGB-to-RGBA Paste. Other logical modes must
/// retain their established conversion and paste semantics.
pub(super) fn gpu_native_rgb_to_rgba_paste_layout(
    op: &PipelineOp,
    destination: &DynamicImage,
    source: &DynamicImage,
    mode: Option<&str>,
) -> Option<(i32, i32)> {
    let PipelineOp::Paste {
        source: source_image,
        x,
        y,
        w,
        h,
        mask: None,
        ..
    } = op
    else {
        return None;
    };
    if mode != Some("RGBA")
        || source_image.mode().ok()?.as_str() != "RGB"
        || !matches!(destination, DynamicImage::ImageRgba8(_))
        || !matches!(source, DynamicImage::ImageRgb8(_))
        || *w <= 0
        || *h <= 0
        || (u32::try_from(*w).ok()?, u32::try_from(*h).ok()?) != source.dimensions()
    {
        return None;
    }
    if CheckedDims::new(destination.width(), destination.height(), 4)
        .ok()?
        .total_bytes()
        != destination.as_bytes().len()
        || CheckedDims::new(source.width(), source.height(), 3)
            .ok()?
            .total_bytes()
            != source.as_bytes().len()
    {
        return None;
    }
    Some((*x, *y))
}

/// Admit exact same-mode L, LA, PA, RGB, HSV, RGBA, or CMYK Paste with a
/// native L mask. Other logical modes and mask semantics retain the generic
/// path.
pub(super) fn gpu_native_masked_byte_paste_layout(
    op: &PipelineOp,
    destination: &DynamicImage,
    source: &DynamicImage,
    mask: &DynamicImage,
    mode: Option<&str>,
) -> Option<((i32, i32), u8, &'static str)> {
    let PipelineOp::Paste {
        source: source_image,
        x,
        y,
        w,
        h,
        mask: Some(mask_image),
        mask_alpha: false,
        ..
    } = op
    else {
        return None;
    };
    let source_mode = source_image.mode().ok()?;
    if mask_image.mode().ok()?.as_str() != "L" {
        return None;
    }
    let (bytes_per_pixel, logical_mode) =
        match (mode, source_mode.as_str(), destination, source, mask) {
            (
                Some("L"),
                "L",
                DynamicImage::ImageLuma8(_),
                DynamicImage::ImageLuma8(_),
                DynamicImage::ImageLuma8(_),
            ) => (1, "L"),
            (
                Some("LA"),
                "LA",
                DynamicImage::ImageLumaA8(_),
                DynamicImage::ImageLumaA8(_),
                DynamicImage::ImageLuma8(_),
            ) => (2, "LA"),
            (
                Some("PA"),
                "PA",
                DynamicImage::ImageLumaA8(_),
                DynamicImage::ImageLumaA8(_),
                DynamicImage::ImageLuma8(_),
            ) => (2, "PA"),
            (
                Some("RGB"),
                "RGB",
                DynamicImage::ImageRgb8(_),
                DynamicImage::ImageRgb8(_),
                DynamicImage::ImageLuma8(_),
            ) => (3, "RGB"),
            (
                Some("HSV"),
                "HSV",
                DynamicImage::ImageRgb8(_),
                DynamicImage::ImageRgb8(_),
                DynamicImage::ImageLuma8(_),
            ) => (3, "HSV"),
            (
                Some("RGBA"),
                "RGBA",
                DynamicImage::ImageRgba8(_),
                DynamicImage::ImageRgba8(_),
                DynamicImage::ImageLuma8(_),
            ) => (4, "RGBA"),
            (
                Some("CMYK"),
                "CMYK",
                DynamicImage::ImageRgba8(_),
                DynamicImage::ImageRgba8(_),
                DynamicImage::ImageLuma8(_),
            ) => (4, "CMYK"),
            _ => return None,
        };
    let (source_width, source_height) = source.dimensions();
    let (mask_width, mask_height) = mask.dimensions();
    let source_is_broadcast =
        (source_width, source_height) == (1, 1) && (mask_width, mask_height) != (1, 1);
    if *w <= 0
        || *h <= 0
        || (u32::try_from(*w).ok()?, u32::try_from(*h).ok()?) != (mask_width, mask_height)
        || (!source_is_broadcast && (source_width, source_height) != (mask_width, mask_height))
        || (source_is_broadcast && !matches!(logical_mode, "L" | "RGB" | "HSV" | "RGBA" | "CMYK"))
    {
        return None;
    }
    let destination_dims =
        CheckedDims::new(destination.width(), destination.height(), bytes_per_pixel).ok()?;
    let source_dims = CheckedDims::new(source_width, source_height, bytes_per_pixel).ok()?;
    let mask_dims = CheckedDims::new(mask_width, mask_height, 1).ok()?;
    if destination_dims.total_bytes() != destination.as_bytes().len()
        || source_dims.total_bytes() != source.as_bytes().len()
        || mask_dims.total_bytes() != mask.as_bytes().len()
    {
        return None;
    }
    Some(((*x, *y), bytes_per_pixel, logical_mode))
}
