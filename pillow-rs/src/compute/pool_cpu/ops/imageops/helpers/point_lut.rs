//! Composition rules for adjacent native-byte point operations.

use crate::pipeline::PipelineOp;
use crate::raster::Mode;

pub(crate) struct ComposedPointLut {
    pub(crate) operations: usize,
    pub(crate) bands: usize,
    pub(crate) values: Vec<u8>,
}

/// Compose a consecutive point-operation run only for the exact modes whose
/// Pillow semantics match a channel-major byte lookup table.
pub(crate) fn compose_run(ops: &[PipelineOp], mode: Mode) -> Option<ComposedPointLut> {
    let bands = match mode {
        Mode::ImageLuma8 => 1,
        Mode::ImageLumaA8 => 2,
        Mode::ImageRgb8 => 3,
        Mode::ImageRgba8 => 4,
        _ => return None,
    };
    let invert_count = ops
        .iter()
        .take_while(|op| matches!(op, PipelineOp::Invert))
        .count();
    if invert_count >= 2 {
        let values = (0..bands)
            .flat_map(|_| {
                (0u16..=255).map(move |value| {
                    let value = value as u8;
                    if invert_count % 2 == 1 {
                        u8::MAX - value
                    } else {
                        value
                    }
                })
            })
            .collect();
        return Some(ComposedPointLut {
            operations: invert_count,
            bands,
            values,
        });
    }

    let mut values = identity_lut(bands);
    let mut operations = 0;
    for op in ops {
        let next = operation_lut(op, bands)?;
        compose_lut(&mut values, &next, bands);
        operations += 1;
    }
    (operations >= 2).then_some(ComposedPointLut {
        operations,
        bands,
        values,
    })
}

pub(crate) fn is_identity(lut: &[u8], bands: usize) -> bool {
    lut.len() == bands.saturating_mul(256)
        && lut.chunks_exact(256).all(|table| {
            table
                .iter()
                .enumerate()
                .all(|(index, &value)| value == index as u8)
        })
}

fn identity_lut(bands: usize) -> Vec<u8> {
    (0..bands)
        .flat_map(|_| (0u16..=255).map(|value| value as u8))
        .collect()
}

fn operation_lut(op: &PipelineOp, bands: usize) -> Option<Vec<u8>> {
    match op {
        PipelineOp::Invert => Some(
            (0..bands)
                .flat_map(|_| (0u16..=255).map(|value| u8::MAX - value as u8))
                .collect(),
        ),
        PipelineOp::Solarize { threshold } => Some(
            (0..bands)
                .flat_map(|_| {
                    (0u16..=255).map(|value| {
                        let value = value as u8;
                        if value >= *threshold {
                            u8::MAX - value
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

fn compose_lut(composed: &mut [u8], next: &[u8], bands: usize) {
    for band in 0..bands {
        let offset = band * 256;
        for value in 0..256usize {
            composed[offset + value] = next[offset + composed[offset + value] as usize];
        }
    }
}
