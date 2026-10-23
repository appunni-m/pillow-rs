use crate::error::PilError;
use crate::image::Image;
use crate::raster::{DynamicImage, GenericImageView};
use std::sync::Arc;

/// Return the native storage width admitted for exact same-mode paste.
///
/// Four-byte storage is shared by alpha, padding, CMYK, and scalar modes, so
/// the logical mode must be part of this check; a matching physical type alone
/// is not enough to establish channel meaning.
pub(super) fn paste_native_channels(mode: &str, image: &DynamicImage) -> Option<usize> {
    match (mode, image) {
        ("1" | "L" | "P", DynamicImage::ImageLuma8(_)) => Some(1),
        ("LA" | "La" | "PA", DynamicImage::ImageLumaA8(_)) => Some(2),
        ("RGB" | "HSV" | "YCbCr" | "LAB", DynamicImage::ImageRgb8(_)) => Some(3),
        ("RGBA" | "RGBa" | "RGBX" | "CMYK" | "I" | "F", DynamicImage::ImageRgba8(_)) => Some(4),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(super) struct PasteMaskLayout {
    pub(super) channels: usize,
    pub(super) value_index: usize,
    pub(super) premultiplied: bool,
}

pub(super) struct PasteMaskPixels {
    pub(super) image: Arc<DynamicImage>,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) layout: PasteMaskLayout,
}

impl PasteMaskPixels {
    #[inline]
    pub(super) fn value(&self, x: u32, y: u32) -> u8 {
        debug_assert!((x as usize) < self.width);
        debug_assert!((y as usize) < self.height);
        let index = ((y as usize * self.width + x as usize) * self.layout.channels)
            + self.layout.value_index;
        self.image.as_bytes()[index]
    }
}

pub(super) fn paste_mask_layout(mode: &str, mask_alpha: bool) -> Option<PasteMaskLayout> {
    match (mode, mask_alpha) {
        ("1" | "L", false) => Some(PasteMaskLayout {
            channels: 1,
            value_index: 0,
            premultiplied: false,
        }),
        ("LA", true) => Some(PasteMaskLayout {
            channels: 2,
            value_index: 1,
            premultiplied: false,
        }),
        ("RGBA", true) => Some(PasteMaskLayout {
            channels: 4,
            value_index: 3,
            premultiplied: false,
        }),
        ("RGBa", true) => Some(PasteMaskLayout {
            channels: 4,
            value_index: 3,
            premultiplied: true,
        }),
        _ => None,
    }
}

pub(super) fn paste_luma16_storage_byte_shift(mode: Option<&str>, byte_offset: u32) -> u32 {
    let high_byte_first = match mode {
        Some("I;16L") => false,
        Some("I;16B") => true,
        Some("I;16" | "I;16N") | None => cfg!(target_endian = "big"),
        _ => cfg!(target_endian = "big"),
    };
    if byte_offset % 2 == 0 {
        if high_byte_first { 8 } else { 0 }
    } else if high_byte_first {
        0
    } else {
        8
    }
}

/// Borrow the validated mask band's native bytes without expanding alpha
/// masks into temporary RGBA images. A missing layout keeps unusual internal
/// pipeline inputs on the existing conversion fallback.
pub(super) fn paste_mask_pixels(
    mask: &Image,
    mask_alpha: bool,
) -> Result<Option<PasteMaskPixels>, PilError> {
    let mode = mask.mode()?;
    let Some(layout) = paste_mask_layout(&mode, mask_alpha) else {
        return Ok(None);
    };
    let image = mask.materialized_shared()?;
    if paste_native_channels(&mode, image.as_ref()) != Some(layout.channels) {
        return Ok(None);
    }
    let (width, height) = image.dimensions();
    let expected_len = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(layout.channels));
    if expected_len != Some(image.as_bytes().len()) {
        return Ok(None);
    }
    Ok(Some(PasteMaskPixels {
        image,
        width: width as usize,
        height: height as usize,
        layout,
    }))
}
