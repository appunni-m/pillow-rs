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
/// whole buffer.  LA/RGBA are safe here only for explicit `PointOp`/`Eval`
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
        PipelineOp::Eval { lut } | PipelineOp::PointOp { lut } if lut.len() == bands * 256 => {
            Some(lut.to_vec())
        }
        _ => None,
    }
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
        PipelineOp::PointOp {
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
    let row_stride = width as usize * channels;
    if row_stride != 0 {
        #[cfg(feature = "parallel")]
        crate::par_rows_mut!(
            &mut output,
            row_stride,
            height as usize,
            |row_start, row_end, _y, row| {
                let left_row = &left_bytes[row_start..row_end];
                let right_row = &right_bytes[row_start..row_end];
                for ((destination, &left), &right) in
                    row.iter_mut().zip(left_row.iter()).zip(right_row.iter())
                {
                    let multiplied = (left as u32 * right as u32 / 255) as u8;
                    *destination =
                        (255u32 - ((255 - multiplied as u32) * (255 - right as u32) / 255)) as u8;
                }
            }
        );
        #[cfg(not(feature = "parallel"))]
        for ((destination, &left), &right) in output
            .iter_mut()
            .zip(left_bytes.iter())
            .zip(right_bytes.iter())
        {
            let multiplied = (left as u32 * right as u32 / 255) as u8;
            *destination =
                (255u32 - ((255 - multiplied as u32) * (255 - right as u32) / 255)) as u8;
        }
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
        // Each operation already consumes an immutable input and produces its
        // own output buffer.  Do not clone the source merely to seed the
        // backend's accumulator: the first operation can read `img` directly.
        // The only clone left is the degenerate zero-operation contract at the
        // end of the loop.
        let mut result: Option<DynamicImage> = None;
        let mut resources = crate::compute::host_resource_telemetry(img);
        let mut index = 0usize;
        // A lazy pipeline carries one mode tag for its final result, but each
        // operation sees the concrete output of the preceding operation. Keep
        // that logical mode in lockstep with the owned `DynamicImage`; using
        // the original tag for every step makes a Convert(RGB) -> ... chain
        // incorrectly present the later RGB buffer as the source mode.
        let mut current_mode = mode.map(str::to_owned);
        while index < ops.len() {
            let input = result.as_ref().unwrap_or(img);
            if ops::draw::is_draw_op(&ops[index]) {
                let mut end = index + 1;
                while end < ops.len() && ops::draw::is_draw_op(&ops[end]) {
                    end += 1;
                }
                if end - index >= 2 {
                    for op in &ops[index..end] {
                        crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(
                            op,
                        ));
                    }
                    let op_mode = current_mode.as_deref();
                    let next = match ops::draw::execute_draw_batch(input, &ops[index..end], op_mode)
                    {
                        Ok(next) => next,
                        Err(error) => {
                            for _ in index..end {
                                crate::compute::record_pipeline_operation_path("cpu");
                                crate::compute::finish_pipeline_operation_telemetry();
                            }
                            return Err(error);
                        }
                    };
                    for _ in index..end {
                        crate::compute::record_pipeline_operation_path("cpu");
                        crate::compute::finish_pipeline_operation_telemetry();
                    }
                    crate::compute::account_host_buffer_boundary(&mut resources, input, &next);
                    result = Some(next);
                    for op in &ops[index..end] {
                        current_mode = crate::compute::pool_simd::ops::adapters::simd_mode_after_op(
                            op,
                            current_mode.as_deref(),
                        );
                    }
                    index = end;
                    continue;
                }
            }
            if mode.is_none() && index + 1 < ops.len() {
                if let (
                    PipelineOp::Multiply { other: first_other },
                    PipelineOp::Screen {
                        other: second_other,
                    },
                ) = (&ops[index], &ops[index + 1])
                {
                    if let Some(fused) = fused_multiply_screen(
                        input,
                        first_other,
                        second_other,
                        current_mode.as_deref(),
                    )? {
                        crate::compute::begin_pipeline_operation_telemetry("Multiply");
                        crate::compute::begin_pipeline_operation_telemetry("Screen");
                        crate::compute::record_pipeline_operation_path("cpu");
                        crate::compute::finish_pipeline_operation_telemetry();
                        crate::compute::record_pipeline_operation_path("cpu");
                        crate::compute::finish_pipeline_operation_telemetry();
                        crate::compute::account_host_buffer_boundary(&mut resources, input, &fused);
                        resources.fused_operation_count =
                            resources.fused_operation_count.saturating_add(2);
                        result = Some(fused);
                        current_mode = crate::compute::pool_simd::ops::adapters::simd_mode_after_op(
                            &ops[index],
                            current_mode.as_deref(),
                        );
                        current_mode = crate::compute::pool_simd::ops::adapters::simd_mode_after_op(
                            &ops[index + 1],
                            current_mode.as_deref(),
                        );
                        index += 2;
                        continue;
                    }
                }
            }
            // The helper admits untagged native images and explicit L/LA/RGB/
            // RGBA tags. It rejects P/PA/I/F/CMYK and other mode-sensitive
            // layouts, so keep the fusion attempt outside the old
            // `mode.is_none()` gate and let the image/mode check decide
            // whether it is exact.
            if let Some((consumed, fused)) =
                fused_point_batch(&ops[index..], input, current_mode.as_deref())
            {
                for op in &ops[index..index + consumed] {
                    crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
                }
                let next = match registry::execute_cpu(&fused, input, current_mode.as_deref()) {
                    Ok(next) => next,
                    Err(error) => {
                        for _ in 0..consumed {
                            crate::compute::record_pipeline_operation_path("cpu");
                            crate::compute::finish_pipeline_operation_telemetry();
                        }
                        return Err(error);
                    }
                };
                for _ in 0..consumed {
                    crate::compute::record_pipeline_operation_path("cpu");
                    crate::compute::finish_pipeline_operation_telemetry();
                }
                crate::compute::account_host_buffer_boundary(&mut resources, input, &next);
                resources.fused_operation_count = resources
                    .fused_operation_count
                    .saturating_add(consumed as u64);
                result = Some(next);
                for op in &ops[index..index + consumed] {
                    current_mode = crate::compute::pool_simd::ops::adapters::simd_mode_after_op(
                        op,
                        current_mode.as_deref(),
                    );
                }
                index += consumed;
                continue;
            }
            let op = &ops[index];
            crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
            let next = match registry::execute_cpu(op, input, current_mode.as_deref()) {
                Ok(next) => next,
                Err(error) => {
                    crate::compute::record_pipeline_operation_path("cpu");
                    crate::compute::finish_pipeline_operation_telemetry();
                    return Err(error);
                }
            };
            crate::compute::record_pipeline_operation_path("cpu");
            crate::compute::finish_pipeline_operation_telemetry();
            crate::compute::account_host_buffer_boundary(&mut resources, input, &next);
            result = Some(next);
            current_mode = crate::compute::pool_simd::ops::adapters::simd_mode_after_op(
                op,
                current_mode.as_deref(),
            );
            index += 1;
        }
        crate::compute::record_pipeline_resource_telemetry(resources);
        Ok(result.unwrap_or_else(|| img.clone()))
    }
}
