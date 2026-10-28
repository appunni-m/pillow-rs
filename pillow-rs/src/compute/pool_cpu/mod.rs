//! CPU worker pool — implements BackendImpl for CPU backend.

use crate::compute::registry;
use crate::compute::{Backend, BackendImpl};
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::PipelineOp;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, RgbImage, RgbaImage,
};
use std::sync::Arc;

mod batch_execution;
pub(crate) mod ops;

/// CPU compute pool — processes all operations on the CPU.
/// This is the fallback pool that supports every PipelineOp.
pub struct CpuPool;

/// Return the number of independent byte bands for the point-operation paths
/// that can be safely composed without changing mode semantics.
fn point_band_count(img: &DynamicImage) -> Option<usize> {
    match img.color() {
        crate::raster::ColorType::L8 => Some(1),
        crate::raster::ColorType::La8 => Some(2),
        crate::raster::ColorType::Rgb8 => Some(3),
        crate::raster::ColorType::Rgba8 => Some(4),
        _ => None,
    }
}

/// Return whether a byte-domain point chain can retain its exact mode tag.
///
/// Explicit `L` and `RGB` tags are emitted for ordinary loaded images by the
/// public lazy API. They have the same sample semantics as an untagged native
/// image, so excluding them would make long chains pay one full traversal per
/// operation even though the composed lookup is exact. Other tags retain the
/// conservative fallback because their palette, alpha, or typed-sample rules
/// are not represented by this LUT.
fn byte_point_mode_allowed(img: &DynamicImage, mode: Option<&str>) -> bool {
    match img {
        DynamicImage::ImageLuma8(_) => matches!(mode, None | Some("L")),
        DynamicImage::ImageLumaA8(_) => matches!(mode, None | Some("LA")),
        DynamicImage::ImageRgb8(_) => matches!(mode, None | Some("RGB")),
        DynamicImage::ImageRgba8(_) => matches!(mode, None | Some("RGBA")),
        _ => false,
    }
}

/// Build one 256-entry map per band for a point-like operation.
///
/// The public ImageOps implementations for palette, numeric, and 16-bit modes
/// have mode-specific behavior that is not equivalent to a byte LUT over the
/// whole buffer.  LA/RGBA are safe here only for explicit `Eval`
/// tables; the ImageOps invert/solarize/posterize wrappers remain outside this
/// fusion because Pillow rejects those mode combinations.
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

#[inline]
fn point_lut_is_identity(lut: &[u8], bands: usize) -> bool {
    lut.len() == bands.saturating_mul(256)
        && lut.chunks_exact(256).all(|table| {
            table
                .iter()
                .enumerate()
                .all(|(index, &value)| value == index as u8)
        })
}

fn repeated_invert_lut(bands: usize, inverted: bool) -> Vec<u8> {
    (0..bands)
        .flat_map(|_| {
            (0u16..=255).map(move |value| {
                let value = value as u8;
                if inverted { u8::MAX - value } else { value }
            })
        })
        .collect()
}

/// Compose adjacent point/LUT operations into one lookup-table traversal.
///
/// The returned count is at least two.  A single operation remains on the
/// ordinary registry path, so the fusion is strictly an optimization and does
/// not change dispatch behavior for unsupported or malformed inputs.
fn fused_point_batch(
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, PipelineOp)> {
    if !byte_point_mode_allowed(img, mode) {
        return None;
    }
    let bands = point_band_count(img)?;

    // ImageOps.invert is the same byte-wise involution at every step. The
    // long-point benchmark intentionally repeats it many times; composing
    // that run through the generic per-op LUT builder makes the control work
    // O(steps * bands * 256) even though the result only depends on parity.
    // Pillow's point contract is exact for these native byte layouts, so
    // reduce a consecutive invert run to one map before the general composer.
    let invert_run = ops
        .iter()
        .take_while(|op| matches!(op, PipelineOp::Invert))
        .count();
    if invert_run >= 2 {
        return Some((
            invert_run,
            PipelineOp::Eval {
                lut: repeated_invert_lut(bands, invert_run % 2 == 1).into(),
            },
        ));
    }

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

    (consumed >= 2).then_some((
        consumed,
        PipelineOp::Eval {
            lut: composed.into(),
        },
    ))
}

/// Fuse the common `ImageChops.multiply(other).screen(other)` pair into one
/// native byte traversal.
///
/// Pillow truncates the multiply result before screen consumes it, so the
/// intermediate value must still be calculated per channel. Keeping that
/// truncation explicit preserves the public contract while removing the
/// intermediate image allocation and second materialization of `other`.
fn fused_multiply_screen(
    img: &DynamicImage,
    first_other: &Arc<Image>,
    second_other: &Arc<Image>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if mode.is_some() || !first_other.shares_execution_source(second_other) {
        return Ok(None);
    }

    let other = first_other.materialize_for_ops()?;
    let (channels, width, height) = match (img, &other) {
        (DynamicImage::ImageLuma8(left), DynamicImage::ImageLuma8(_right)) => {
            (1usize, left.width(), left.height())
        }
        (DynamicImage::ImageLumaA8(left), DynamicImage::ImageLumaA8(_right)) => {
            (2usize, left.width(), left.height())
        }
        (DynamicImage::ImageRgb8(left), DynamicImage::ImageRgb8(_right)) => {
            (3usize, left.width(), left.height())
        }
        (DynamicImage::ImageRgba8(left), DynamicImage::ImageRgba8(_right)) => {
            (4usize, left.width(), left.height())
        }
        _ => return Ok(None),
    };
    if img.dimensions() != other.dimensions() {
        return Ok(None);
    }

    let left_bytes = img.as_bytes();
    let right_bytes = other.as_bytes();
    let mut output = vec![0u8; left_bytes.len()];
    for ((destination, &left), &right) in output
        .iter_mut()
        .zip(left_bytes.iter())
        .zip(right_bytes.iter())
    {
        let multiplied = (left as u32 * right as u32 / 255) as u8;
        *destination = (255u32 - ((255 - multiplied as u32) * (255 - right as u32) / 255)) as u8;
    }

    let result =
        match channels {
            1 => DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, output).ok_or_else(
                || PilError::InternalError("fused Chops buffer shape mismatch".into()),
            )?),
            2 => DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(width, height, output).ok_or_else(|| {
                    PilError::InternalError("fused Chops buffer shape mismatch".into())
                })?,
            ),
            3 => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, output).ok_or_else(
                || PilError::InternalError("fused Chops buffer shape mismatch".into()),
            )?),
            4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, output).ok_or_else(
                || PilError::InternalError("fused Chops buffer shape mismatch".into()),
            )?),
            _ => return Ok(None),
        };
    Ok(Some(preserve_mode(img, result)))
}

impl BackendImpl for CpuPool {
    fn name(&self) -> Backend {
        Backend::Cpu
    }

    fn priority(&self) -> u8 {
        0
    }

    fn supports(&self, op: &PipelineOp) -> Result<bool, PilError> {
        registry::cpu_supports(op)
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        batch_execution::execute_batch(ops, img, mode)
    }
}

#[cfg(test)]
mod tests {
    use super::CpuPool;
    use crate::compute::{BackendImpl, registry};
    use crate::error::PilError;
    use crate::image::Image;
    use crate::image_utils::raw_bytes_to_image;
    use crate::pipeline::{PipelineOp, TransposeMethod};
    use crate::raster::GenericImageView;
    use std::sync::Arc;

    #[test]
    fn transpose_batches_preserve_sequential_pixels_and_dimensions() -> Result<(), PilError> {
        let methods = [
            TransposeMethod::FlipLeftRight,
            TransposeMethod::FlipTopBottom,
            TransposeMethod::Rotate90,
            TransposeMethod::Rotate180,
            TransposeMethod::Rotate270,
            TransposeMethod::Transpose,
            TransposeMethod::Transverse,
        ];
        for (width, height) in [(1u32, 7u32), (9, 1), (13, 19)] {
            for (channels, mode) in [(1, "L"), (2, "LA"), (3, "RGB"), (4, "RGBA"), (4, "F")] {
                let bytes: Vec<u8> = (0..width as usize * height as usize * channels)
                    .map(|index| index.wrapping_mul(113).wrapping_add(index / 7) as u8)
                    .collect();
                let source = raw_bytes_to_image(width, height, bytes.clone(), channels)?;
                for first in &methods {
                    for second in &methods {
                        let ops = [
                            PipelineOp::Transpose {
                                method: first.clone(),
                            },
                            PipelineOp::Transpose {
                                method: second.clone(),
                            },
                        ];
                        let intermediate = registry::execute_cpu(&ops[0], &source, Some(mode))?;
                        let expected = registry::execute_cpu(&ops[1], &intermediate, Some(mode))?;
                        let actual = CpuPool.execute_batch(&ops, &source, Some(mode))?;
                        assert_eq!(actual.dimensions(), expected.dimensions());
                        assert_eq!(
                            actual.as_bytes(),
                            expected.as_bytes(),
                            "{mode} {first:?} {second:?}"
                        );
                    }
                }
                assert_eq!(source.as_bytes(), bytes);
            }
        }
        Ok(())
    }

    #[test]
    fn alpha_composite_mirror_fusion_matches_sequential_rgba_for_uniform_and_varied_pixels()
    -> Result<(), PilError> {
        for (width, height, uniform) in [(64u32, 3u32, true), (17, 5, false), (1, 1, true)] {
            let pixel_count = width as usize * height as usize;
            let mut destination_bytes = if uniform {
                [30, 60, 90, 64].repeat(pixel_count)
            } else {
                (0..pixel_count * 4)
                    .map(|index| (index.wrapping_mul(71).wrapping_add(19)) as u8)
                    .collect::<Vec<_>>()
            };
            let mut source_bytes = if uniform {
                [200, 150, 100, 192].repeat(pixel_count)
            } else {
                (0..pixel_count * 4)
                    .map(|index| (index.wrapping_mul(37).wrapping_add(83)) as u8)
                    .collect::<Vec<_>>()
            };
            if !uniform {
                for (pixel, alpha) in source_bytes
                    .chunks_exact_mut(4)
                    .zip([0, 1, 64, 128, 255].into_iter().cycle())
                {
                    pixel[3] = alpha;
                }
                for (pixel, alpha) in destination_bytes
                    .chunks_exact_mut(4)
                    .zip([0, 63, 127, 192, 255].into_iter().cycle())
                {
                    pixel[3] = alpha;
                }
            }

            let destination = raw_bytes_to_image(width, height, destination_bytes, 4)?;
            let source_dynamic = raw_bytes_to_image(width, height, source_bytes, 4)?;
            let source = Arc::new(Image::from_dynamic(source_dynamic, Some("RGBA".to_owned())));
            let operations = [
                PipelineOp::AlphaComposite {
                    source,
                    dest: (0, 0),
                    src: (0, 0),
                },
                PipelineOp::Mirror,
            ];
            let composite = registry::execute_cpu(&operations[0], &destination, Some("RGBA"))?;
            let expected = registry::execute_cpu(&operations[1], &composite, Some("RGBA"))?;
            let actual = CpuPool.execute_batch(&operations, &destination, Some("RGBA"))?;
            assert_eq!(actual.dimensions(), expected.dimensions());
            assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");
        }
        Ok(())
    }
}
