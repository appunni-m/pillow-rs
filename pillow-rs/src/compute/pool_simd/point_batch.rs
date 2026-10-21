//! Composition of adjacent SIMD point operations into one lookup table.

use crate::pipeline::PipelineOp;
use crate::raster::{ColorType, DynamicImage};

fn byte_point_mode_allowed(image: &DynamicImage, mode: Option<&str>) -> bool {
    match image {
        DynamicImage::ImageLuma8(_) => matches!(mode, None | Some("L")),
        DynamicImage::ImageLumaA8(_) => matches!(mode, None | Some("LA")),
        DynamicImage::ImageRgb8(_) => matches!(mode, None | Some("RGB")),
        DynamicImage::ImageRgba8(_) => matches!(mode, None | Some("RGBA")),
        _ => false,
    }
}

fn point_band_count(image: &DynamicImage) -> Option<usize> {
    match image.color() {
        ColorType::L8 => Some(1),
        ColorType::La8 => Some(2),
        ColorType::Rgb8 => Some(3),
        ColorType::Rgba8 => Some(4),
        _ => None,
    }
}

fn point_lut(op: &PipelineOp, bands: usize) -> Option<Vec<u8>> {
    match op {
        PipelineOp::Invert => Some(
            (0..bands)
                .flat_map(|_| (0u16..=255).map(|value| 255u8 - value as u8))
                .collect(),
        ),
        PipelineOp::Solarize { threshold } => Some(
            (0..bands)
                .flat_map(|_| {
                    (0u16..=255).map(|value| {
                        let value = value as u8;
                        if value >= *threshold {
                            255 - value
                        } else {
                            value
                        }
                    })
                })
                .collect(),
        ),
        PipelineOp::Posterize { bits } if *bits <= 8 => {
            let mask = !((1u8 << (8 - bits)) - 1);
            Some(
                (0..bands)
                    .flat_map(|_| (0u16..=255).map(|value| value as u8 & mask))
                    .collect(),
            )
        }
        PipelineOp::Eval { lut } if lut.len() == bands * 256 => Some(lut.to_vec()),
        _ => None,
    }
}

pub(super) fn fused(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, Vec<u8>)> {
    if !byte_point_mode_allowed(image, mode) {
        return None;
    }
    let bands = point_band_count(image)?;
    let mut composed: Vec<u8> = (0..bands)
        .flat_map(|_| (0u16..=255).map(|value| value as u8))
        .collect();
    let mut consumed = 0usize;
    for op in ops {
        let next = point_lut(op, bands)?;
        for band in 0..bands {
            let offset = band * 256;
            for value in 0..256usize {
                composed[offset + value] = next[offset + composed[offset + value] as usize];
            }
        }
        consumed += 1;
    }
    (consumed >= 2).then_some((consumed, composed))
}
