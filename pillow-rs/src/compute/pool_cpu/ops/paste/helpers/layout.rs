use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_pixel_lanes;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, Mode};

/// Exact native input mapping chosen once before entering a pixel loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceLayout {
    Native,
    PhysicalFourBytesToLumaAlpha,
    PhysicalFourBytesToRgb,
    PhysicalFourBytesToFour,
}

/// Exact mask storage location and mask arithmetic selected during preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MaskLayout {
    pub(crate) channels: usize,
    pub(crate) value_index: usize,
    pub(crate) binary: bool,
    pub(crate) premultiplied: bool,
}

pub(crate) fn source_layout(
    destination: Mode,
    source: Mode,
) -> Option<SourceLayout> {
    if destination == source {
        return Some(SourceLayout::Native);
    }
    // Pillow's RGB shortcut and virtual convert() overrides can reach core
    // Paste with different identities when physical pixel sizes match.
    if destination.pillow_pixel_size()? != source.pillow_pixel_size()? {
        return None;
    }
    match (destination.pillow_pixel_size()?, destination.hidden_pixel_lane_count()) {
        (4, Some(2)) => Some(SourceLayout::PhysicalFourBytesToLumaAlpha),
        (4, Some(1)) => Some(SourceLayout::PhysicalFourBytesToRgb),
        (4, None) => Some(SourceLayout::PhysicalFourBytesToFour),
        (1 | 2, None) => Some(SourceLayout::Native),
        _ => None,
    }
}

pub(crate) fn physical_four_byte_source(image: &DynamicImage) -> Option<Vec<u8>> {
    let mode = image.mode();
    if mode.pillow_pixel_size() != Some(4) {
        return None;
    }
    let pixel_count = (image.width() as usize).checked_mul(image.height() as usize)?;
    let output_size = pixel_count.checked_mul(4)?;
    let logical = image.as_bytes();
    let channels = mode.byte_carrier_pixel_width()?;
    if channels == 4 {
        return logical.get(..output_size).map(<[u8]>::to_vec);
    }
    let logical_size = pixel_count.checked_mul(channels)?;
    let logical = logical.get(..logical_size)?;
    let hidden = hidden_lanes(image, mode);
    let mut physical = Vec::with_capacity(output_size);
    let lane_count = mode.hidden_pixel_lane_count()?;
    for (index, pixel) in logical.chunks_exact(channels).enumerate() {
        let fallback = default_hidden_pixel_lanes(mode, pixel[0])?;
        let mut physical_pixel = [0; 4];
        for (byte, &sample) in pixel.iter().enumerate() {
            physical_pixel[mode.byte_carrier_physical_offset(byte)?] = sample;
        }
        for lane in 0..lane_count {
            let value = hidden
                .filter(|lanes| lanes.lane_count() >= lane_count)
                .and_then(|lanes| lanes.for_pixel(index))
                .and_then(|lanes| lanes.get(lane))
                .copied()
                .unwrap_or(fallback[lane]);
            physical_pixel[mode.hidden_pixel_lane_offset(lane)?] = value;
        }
        physical.extend_from_slice(&physical_pixel);
    }
    Some(physical)
}

pub(crate) fn hidden_lanes(image: &DynamicImage, mode: Mode) -> Option<&HiddenPixelLanes> {
    match (mode, image) {
        (Mode::ImageLumaA8, DynamicImage::ImageLumaA8(image)) => image.hidden_pixel_lanes(),
        (Mode::ImageLaPremultiplied8, DynamicImage::ImageLaPremultiplied8(image)) => {
            image.hidden_pixel_lanes()
        }
        (Mode::ImagePaletteAlpha8, DynamicImage::ImagePaletteAlpha8(image)) => {
            image.hidden_pixel_lanes()
        }
        (Mode::ImageRgb8, DynamicImage::ImageRgb8(image)) => image.hidden_pixel_lanes(),
        (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(image)) => image.hidden_pixel_lanes(),
        (Mode::ImageLab8, DynamicImage::ImageLab8(image)) => image.hidden_pixel_lanes(),
        (Mode::ImageHsv8, DynamicImage::ImageHsv8(image)) => image.hidden_pixel_lanes(),
        _ => None,
    }
}

pub(crate) const fn hidden_lane_count(mode: Mode) -> Option<usize> {
    mode.hidden_pixel_lane_count()
}

pub(crate) const fn hidden_lane_offset(mode: Mode, lane: usize) -> Option<usize> {
    mode.hidden_pixel_lane_offset(lane)
}

pub(crate) fn mask_layout(mode: Mode, alpha: bool) -> Option<MaskLayout> {
    match (mode, alpha) {
        (Mode::One, false) => Some(MaskLayout {
            channels: 1,
            value_index: 0,
            binary: true,
            premultiplied: false,
        }),
        (Mode::ImageLuma8, false) => Some(MaskLayout {
            channels: 1,
            value_index: 0,
            binary: false,
            premultiplied: false,
        }),
        (Mode::ImageLumaA8, true) => Some(MaskLayout {
            channels: 2,
            value_index: 1,
            binary: false,
            premultiplied: false,
        }),
        (Mode::ImageRgba8, true) => Some(MaskLayout {
            channels: 4,
            value_index: 3,
            binary: false,
            premultiplied: false,
        }),
        (Mode::ImageRgbPremultiplied8, true) => Some(MaskLayout {
            channels: 4,
            value_index: 3,
            binary: false,
            premultiplied: true,
        }),
        _ => None,
    }
}

pub(crate) const fn byte_carrier_pixel_width(mode: Mode) -> Option<usize> {
    mode.byte_carrier_pixel_width()
}
