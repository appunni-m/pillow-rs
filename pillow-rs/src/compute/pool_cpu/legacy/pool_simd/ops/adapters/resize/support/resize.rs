use super::{
    DynamicImage, ResampleFilter, SIMD_RESIZE_NEAREST_BYTES, SimdImageShape, SimdLayout,
    native_pad_round_dimension,
};
/// Return the dimensions Pillow's scalar `ImageOps.scale` validation computes.
///
/// Scale is scalar control work followed by the same native resize data plane
/// as `Image.resize`.  Keeping the ties-to-even rounding here makes SIMD
/// preflight agree with the public operation without moving validation into a
/// CPU adapter.
pub(in crate::compute::pool_simd::ops::adapters) fn native_scale_dimensions(
    source_width: u32,
    source_height: u32,
    factor: f64,
) -> Option<(u32, u32)> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let round = |dimension: u32| {
        native_pad_round_dimension(f64::from(dimension) * factor).filter(|rounded| *rounded > 0)
    };
    Some((round(source_width)?, round(source_height)?))
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_resize_supported_for_dimensions(
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    channels: usize,
) -> bool {
    if !(1..=4).contains(&channels) || output_width == 0 || output_height == 0 {
        return false;
    }
    // Pillow's scalar-domain resize paths produce a zero-filled output when
    // one source dimension is empty.  The result still has a valid positive
    // destination shape, so this is a vector zero-fill data plane rather than
    // a reason to route the operation through CPU.
    if source_width == 0 || source_height == 0 {
        return true;
    }
    if (source_width, source_height) == (output_width, output_height) {
        return source_width
            .checked_mul(source_height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes >= SIMD_RESIZE_NEAREST_BYTES as u32);
    }
    match filter {
        ResampleFilter::Nearest => super::resize_nearest_vectorizable(
            source_width,
            source_height,
            output_width,
            output_height,
            channels,
        ),
        _ => {
            usize::try_from(source_height)
                .ok()
                .and_then(|height| {
                    usize::try_from(output_width)
                        .ok()
                        .and_then(|width| height.checked_mul(width))
                })
                .and_then(|pixels| pixels.checked_mul(channels))
                .is_some()
                && usize::try_from(output_width)
                    .ok()
                    .and_then(|width| usize::try_from(output_height).ok()?.checked_mul(width))
                    .and_then(|pixels| pixels.checked_mul(channels))
                    .is_some()
        }
    }
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_resize_supported_for_image(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    if matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N")) {
        if !matches!(img, DynamicImage::ImageLuma16(_)) {
            return false;
        }
        let Some(expected_bytes) = (img.width() as usize)
            .checked_mul(img.height() as usize)
            .and_then(|pixels| pixels.checked_mul(2))
        else {
            return false;
        };
        return img.as_bytes().len() == expected_bytes
            && native_resize_supported_for_dimensions(
                img.width(),
                img.height(),
                output_width,
                output_height,
                filter,
                2,
            );
    }
    let is_typed_scalar = matches!(mode, Some("I" | "F"));
    if is_typed_scalar && !matches!(img, DynamicImage::ImageRgba8(_)) {
        return false;
    }
    let channels = if is_typed_scalar {
        4
    } else if let Some(channels) = super::resize_native_channels_for_image(img, mode) {
        channels
    } else {
        return false;
    };
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected_bytes
        && native_resize_supported_for_dimensions(
            img.width(),
            img.height(),
            output_width,
            output_height,
            filter,
            channels,
        )
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_resize_supported_for_shape(
    shape: SimdImageShape,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let (channels, valid) = if matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N")) {
        (2, shape.layout == SimdLayout::Luma16)
    } else if matches!(mode, Some("I" | "F")) {
        (4, shape.layout == SimdLayout::Rgba8)
    } else {
        match super::native_resize_byte_layout_for_shape(shape, mode) {
            Some((channels, _)) => (channels, true),
            None => (0, false),
        }
    };
    valid
        && native_resize_supported_for_dimensions(
            shape.width,
            shape.height,
            output_width,
            output_height,
            filter,
            channels,
        )
}
