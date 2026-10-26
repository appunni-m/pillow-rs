use super::*;

#[cfg(target_endian = "little")]
mod byte_paste;

struct ValidatedMaskedPasteLayout<'a> {
    width: u32,
    height: u32,
    region_width: u32,
    region_height: u32,
    destination_bytes: &'a [u8],
    source_bytes: &'a [u8],
    mask_bytes: &'a [u8],
}

fn masked_paste_storage_matches(
    logical_mode: &str,
    bytes_per_pixel: u8,
    destination: &DynamicImage,
    source: &DynamicImage,
    mask: &DynamicImage,
) -> bool {
    match (logical_mode, bytes_per_pixel) {
        ("1" | "L" | "P", 1) => matches!(
            (destination, source, mask),
            (
                DynamicImage::ImageLuma8(_),
                DynamicImage::ImageLuma8(_),
                DynamicImage::ImageLuma8(_)
            )
        ),
        ("LA", 2) | ("PA", 2) => matches!(
            (destination, source, mask),
            (
                DynamicImage::ImageLumaA8(_),
                DynamicImage::ImageLumaA8(_),
                DynamicImage::ImageLuma8(_)
            )
        ),
        ("RGB" | "HSV", 3) => matches!(
            (destination, source, mask),
            (
                DynamicImage::ImageRgb8(_),
                DynamicImage::ImageRgb8(_),
                DynamicImage::ImageLuma8(_)
            )
        ),
        // CMYK uses ImageRgba8 only as a four-byte carrier; K remains a
        // channel in the stored-byte blend.
        ("RGBA" | "CMYK", 4) => matches!(
            (destination, source, mask),
            (
                DynamicImage::ImageRgba8(_),
                DynamicImage::ImageRgba8(_),
                DynamicImage::ImageLuma8(_)
            )
        ),
        _ => false,
    }
}

fn validate_masked_paste_layout<'a>(
    destination: &'a DynamicImage,
    source: &'a DynamicImage,
    mask: &'a DynamicImage,
    logical_mode: &str,
    dispatch: NativeMaskedBytePasteDispatch,
) -> Result<ValidatedMaskedPasteLayout<'a>, PilError> {
    if !masked_paste_storage_matches(
        logical_mode,
        dispatch.bytes_per_pixel,
        destination,
        source,
        mask,
    ) {
        return Err(PilError::InternalError(
            "GPU native masked Paste storage variant mismatch".into(),
        ));
    }
    let (width, height) = destination.dimensions();
    let (source_width, source_height) = source.dimensions();
    let (region_width, region_height) = mask.dimensions();
    let destination_bytes = destination.as_bytes();
    let source_bytes = source.as_bytes();
    let mask_bytes = mask.as_bytes();
    let destination_dims = CheckedDims::new(width, height, dispatch.bytes_per_pixel)?;
    let source_dims = CheckedDims::new(source_width, source_height, dispatch.bytes_per_pixel)?;
    let mask_dims = CheckedDims::new(region_width, region_height, 1)?;
    let source_is_broadcast =
        (source_width, source_height) == (1, 1) && (region_width, region_height) != (1, 1);
    if dispatch.source_is_broadcast != source_is_broadcast
        || (!source_is_broadcast && (region_width, region_height) != (source_width, source_height))
        || destination_bytes.len() != dispatch.destination_bytes
        || source_bytes.len() != dispatch.source_bytes
        || mask_bytes.len() != dispatch.mask_bytes
        || destination_dims.total_bytes() != dispatch.destination_bytes
        || source_dims.total_bytes() != dispatch.source_bytes
        || mask_dims.total_bytes() != dispatch.mask_bytes
        || destination_dims.total_pixels() != dispatch.destination_pixels as usize
        || source_dims.total_pixels() != dispatch.source_pixels as usize
    {
        return Err(PilError::InternalError(
            "GPU native masked Paste layout mismatch".into(),
        ));
    }
    Ok(ValidatedMaskedPasteLayout {
        width,
        height,
        region_width,
        region_height,
        destination_bytes,
        source_bytes,
        mask_bytes,
    })
}
