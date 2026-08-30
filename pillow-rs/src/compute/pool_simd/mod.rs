//! SIMD worker pool — implements BackendImpl for SIMD-accelerated CPU compute.
//!
//! Uses the `wide` crate for portable SIMD (SSE, AVX, NEON) to process pixels
//! in vectorized chunks. Unsupported operations are routed to another backend
//! before execution; an explicitly locked SIMD pipeline reports an error.
//!
//! ## Architecture
//! - Same mode encoding as GPU: 0=L, 1=LA, 2=RGB, 3=RGBA
//! - Uses native interleaved byte layouts for admitted operations
//! - Priority: 50 (above CPU=0, below GPU=100)
//! - Ops live in `ops/` mirroring `pool_cpu/ops/`

use crate::compute::registry;
use crate::compute::{Backend, BackendImpl};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, TransposeMethod};
use crate::raster::DynamicImage;

pub(crate) mod ops;

// ─── SimdPool ──────────────────────────────────────────────────────────────

/// SIMD compute pool — CPU-accelerated via portable SIMD vectors.
///
/// Executes only operations with a registered SIMD implementation.
pub struct SimdPool;

fn point_band_count(img: &DynamicImage) -> Option<usize> {
    match img.color() {
        crate::raster::ColorType::L8 => Some(1),
        crate::raster::ColorType::La8 => Some(2),
        crate::raster::ColorType::Rgb8 => Some(3),
        crate::raster::ColorType::Rgba8 => Some(4),
        _ => None,
    }
}

fn byte_point_mode_allowed(img: &DynamicImage, mode: Option<&str>) -> bool {
    match img {
        DynamicImage::ImageLuma8(_) => matches!(mode, None | Some("L")),
        DynamicImage::ImageLumaA8(_) => matches!(mode, None | Some("LA")),
        DynamicImage::ImageRgb8(_) => matches!(mode, None | Some("RGB")),
        DynamicImage::ImageRgba8(_) => matches!(mode, None | Some("RGBA")),
        _ => false,
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
        PipelineOp::Eval { lut } | PipelineOp::PointOp { lut } if lut.len() == bands * 256 => {
            Some(lut.to_vec())
        }
        _ => None,
    }
}

fn fused_point_batch(
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, Vec<u8>)> {
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
    (consumed >= 2).then_some((consumed, composed))
}

fn transpose_output_dimensions(method: &TransposeMethod, width: u32, height: u32) -> (u32, u32) {
    match method {
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => (height, width),
        _ => (width, height),
    }
}

/// Map one source coordinate to its output coordinate for a Pillow transpose.
/// All seven methods are affine signed-axis permutations, so the four source
/// corners are sufficient to prove a composition matches a candidate method.
fn transpose_forward(
    method: &TransposeMethod,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> (u32, u32) {
    match method {
        TransposeMethod::FlipLeftRight => (width - 1 - x, y),
        TransposeMethod::FlipTopBottom => (x, height - 1 - y),
        TransposeMethod::Rotate90 => (y, width - 1 - x),
        TransposeMethod::Rotate180 => (width - 1 - x, height - 1 - y),
        TransposeMethod::Rotate270 => (height - 1 - y, x),
        TransposeMethod::Transpose => (y, x),
        TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
    }
}

/// Compose two adjacent transpose operations into one D4 transform.
///
/// The implementation is deliberately independent of pixel storage. It is
/// used by the SIMD executor before native bytes are traversed, so a chain
/// such as `FlipLeftRight → Rotate90` performs one allocation and one pass.
fn compose_transpose_methods(
    first: &TransposeMethod,
    second: &TransposeMethod,
    width: u32,
    height: u32,
) -> Option<TransposeMethod> {
    if width == 0 || height == 0 {
        return None;
    }
    let (middle_width, middle_height) = transpose_output_dimensions(first, width, height);
    let output_dimensions = transpose_output_dimensions(second, middle_width, middle_height);
    let corners = [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
    ];
    let candidates = [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ];
    candidates.into_iter().find(|candidate| {
        if transpose_output_dimensions(candidate, width, height) != output_dimensions {
            return false;
        }
        corners.iter().all(|&(x, y)| {
            let (middle_x, middle_y) = transpose_forward(first, width, height, x, y);
            let expected =
                transpose_forward(second, middle_width, middle_height, middle_x, middle_y);
            let actual = transpose_forward(candidate, width, height, x, y);
            expected == actual
        })
    })
}

fn normalize_palette_result(
    result: DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    match mode {
        Some("P") => {
            let rgba = result.to_rgba8();
            let (width, height) = rgba.dimensions();
            let indices = rgba.pixels().map(|pixel| pixel[0]).collect();
            crate::raster::GrayImage::from_raw(width, height, indices)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| {
                    PilError::InternalError("SIMD P-mode buffer shape mismatch".to_string())
                })
        }
        // ExtractBand changes PA's native two-byte index/alpha samples to a
        // one-byte L result. Only normalize a result that still has PA's
        // two-byte layout; widening an extracted L band back to [sample, 255]
        // changes both its byte contract and its public semantics.
        Some("PA") if !matches!(result, DynamicImage::ImageLumaA8(_)) => Ok(result),
        Some("PA") => {
            let rgba = result.to_rgba8();
            let (width, height) = rgba.dimensions();
            let samples = rgba
                .pixels()
                .flat_map(|pixel| [pixel[0], pixel[3]])
                .collect();
            crate::raster::GrayAlphaImage::from_raw(width, height, samples)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| {
                    PilError::InternalError("SIMD PA-mode buffer shape mismatch".to_string())
                })
        }
        _ => Ok(result),
    }
}

impl BackendImpl for SimdPool {
    fn name(&self) -> Backend {
        Backend::Simd
    }

    fn priority(&self) -> u8 {
        50 // Above CPU (0), below GPU (100)
    }

    fn supports(&self, op: &PipelineOp) -> Result<bool, PilError> {
        registry::simd_supports(op)
    }

    fn supports_for_image(
        &self,
        op: &PipelineOp,
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<bool, PilError> {
        if !self.supports(op)? {
            return Ok(false);
        }
        Ok(ops::adapters::simd_supports_for_image(img, op, mode))
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let op_keys: Vec<&str> = ops.iter().map(|op| registry::variant_key(op)).collect();
        log::debug!(
            "[SIMD] {} op(s) {}x{}: {:?}",
            ops.len(),
            img.width(),
            img.height(),
            op_keys
        );

        // The first operation can read the materialized source directly.  Do
        // not clone the full frame merely to seed the accumulator; each
        // adapter already returns an owned output buffer.  A zero-operation
        // batch (kept for defensive internal callers) clones only at return.
        let mut current: Option<DynamicImage> = None;
        let mut current_mode = ops::adapters::simd_initial_mode(img, ops, mode);
        let mut resources = crate::compute::host_resource_telemetry(img);
        let mut index = 0usize;
        while index < ops.len() {
            let input = current.as_ref().unwrap_or(img);
            let current_op = &ops[index];
            let op_mode = current_mode.as_deref();
            if !ops::adapters::simd_supports_for_image(input, current_op, op_mode) {
                let key = registry::variant_key(current_op);
                crate::compute::record_pipeline_operation_unsupported(key);
                return Err(PilError::NotImplementedError(format!(
                    "SIMD does not support {key} for the current image layout/mode"
                )));
            }
            if current_mode.is_none() {
                if index + 1 < ops.len() {
                    if let (
                        PipelineOp::Multiply { other: first_other },
                        PipelineOp::Screen {
                            other: second_other,
                        },
                    ) = (&ops[index], &ops[index + 1])
                    {
                        if !ops::adapters::simd_supports_for_image(input, &ops[index + 1], op_mode)
                        {
                            let key = registry::variant_key(&ops[index + 1]);
                            crate::compute::record_pipeline_operation_unsupported(key);
                            return Err(PilError::NotImplementedError(format!(
                                "SIMD does not support {key} for the current image layout/mode"
                            )));
                        }
                        if let Some((fused, vector_blocks, scalar_tail)) =
                            ops::adapters::simd_fused_multiply_screen(
                                input,
                                first_other,
                                second_other,
                                op_mode,
                            )?
                        {
                            crate::compute::begin_pipeline_operation_telemetry("Multiply");
                            crate::compute::begin_pipeline_operation_telemetry("Screen");
                            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
                            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
                            crate::compute::finish_pipeline_operation_telemetry();
                            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
                            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
                            crate::compute::finish_pipeline_operation_telemetry();
                            crate::compute::account_host_buffer_boundary(
                                &mut resources,
                                input,
                                &fused,
                            );
                            resources.fused_operation_count =
                                resources.fused_operation_count.saturating_add(2);
                            current = Some(fused);
                            current_mode =
                                ops::adapters::simd_mode_after_op(&ops[index + 1], op_mode);
                            index += 2;
                            continue;
                        }
                    }
                }
                if let PipelineOp::Transpose { method } = &ops[index] {
                    let mut combined = method.clone();
                    let mut consumed = 1usize;
                    while index + consumed < ops.len() {
                        let PipelineOp::Transpose { method: next } = &ops[index + consumed] else {
                            break;
                        };
                        let Some(composed) = compose_transpose_methods(
                            &combined,
                            next,
                            input.width(),
                            input.height(),
                        ) else {
                            break;
                        };
                        combined = composed;
                        consumed += 1;
                    }
                    if consumed > 1 {
                        if let Some(unsupported) = ops[index..index + consumed]
                            .iter()
                            .find(|op| !ops::adapters::simd_supports_for_image(input, op, op_mode))
                        {
                            let key = registry::variant_key(unsupported);
                            crate::compute::record_pipeline_operation_unsupported(key);
                            return Err(PilError::NotImplementedError(format!(
                                "SIMD does not support {key} for the current image layout/mode"
                            )));
                        }
                        let fused = PipelineOp::Transpose { method: combined };
                        let key = registry::variant_key(&fused);
                        let entry = registry::registry()?.get(key).ok_or_else(|| {
                            PilError::ValueError(format!("SIMD: unknown op {}", key))
                        })?;
                        let f = entry.simd_fn.ok_or_else(|| {
                            PilError::ValueError(format!("SIMD: no native impl for {}", key))
                        })?;
                        for op in &ops[index..index + consumed] {
                            crate::compute::begin_pipeline_operation_telemetry(
                                registry::variant_key(op),
                            );
                        }
                        let next = match f(input, &fused, op_mode) {
                            Ok(next) => next,
                            Err(error) => {
                                for _ in 0..consumed {
                                    crate::compute::record_pipeline_operation_path("unsupported");
                                    crate::compute::finish_pipeline_operation_telemetry();
                                }
                                return Err(error);
                            }
                        };
                        for _ in 0..consumed {
                            crate::compute::record_pipeline_operation_path("native-copy");
                            crate::compute::finish_pipeline_operation_telemetry();
                        }
                        crate::compute::account_host_buffer_boundary(&mut resources, input, &next);
                        resources.fused_operation_count = resources
                            .fused_operation_count
                            .saturating_add(consumed as u64);
                        current = Some(next);
                        current_mode =
                            ops::adapters::simd_mode_after_op(&ops[index + consumed - 1], op_mode);
                        index += consumed;
                        continue;
                    }
                }
            }
            if let Some((consumed, lut)) = fused_point_batch(&ops[index..], input, op_mode) {
                if let Some(unsupported) = ops[index..index + consumed]
                    .iter()
                    .find(|op| !ops::adapters::simd_supports_for_image(input, op, op_mode))
                {
                    let key = registry::variant_key(unsupported);
                    crate::compute::record_pipeline_operation_unsupported(key);
                    return Err(PilError::NotImplementedError(format!(
                        "SIMD does not support {key} for the current image layout/mode"
                    )));
                }
                for op in &ops[index..index + consumed] {
                    crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
                }
                let next =
                    if let Some(native) = ops::adapters::native_point_lut(input, op_mode, &lut) {
                        native
                    } else {
                        let fused = PipelineOp::Eval { lut: lut.into() };
                        match ops::adapters::simd_eval(input, &fused, op_mode) {
                            Ok(next) => next,
                            Err(error) => {
                                for _ in 0..consumed {
                                    crate::compute::record_pipeline_operation_path("unsupported");
                                    crate::compute::finish_pipeline_operation_telemetry();
                                }
                                return Err(error);
                            }
                        }
                    };
                for _ in 0..consumed {
                    crate::compute::record_pipeline_operation_path("vector");
                    crate::compute::finish_pipeline_operation_telemetry();
                }
                crate::compute::account_host_buffer_boundary(&mut resources, input, &next);
                resources.fused_operation_count = resources
                    .fused_operation_count
                    .saturating_add(consumed as u64);
                current = Some(next);
                current_mode =
                    ops::adapters::simd_mode_after_op(&ops[index + consumed - 1], op_mode);
                index += consumed;
                continue;
            }
            // Once a SIMD segment has produced an owned intermediate, native
            // byte transforms can reuse that buffer.  The first operation
            // still reads the caller-owned source immutably; only subsequent
            // operations enter this path, so public branch semantics remain
            // unchanged while repeated point/effect work avoids full-frame
            // clones.
            let can_reuse = current.as_ref().is_some_and(|input| {
                ops::adapters::simd_in_place_supported(input, current_op, op_mode)
            });
            if can_reuse {
                let mut owned = current
                    .take()
                    .expect("current.is_some() guarantees an owned image");
                crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(
                    current_op,
                ));
                match ops::adapters::simd_execute_in_place(&mut owned, current_op, op_mode) {
                    Ok(true) => {
                        crate::compute::finish_pipeline_operation_telemetry();
                        current = Some(owned);
                        current_mode = ops::adapters::simd_mode_after_op(current_op, op_mode);
                        index += 1;
                        continue;
                    }
                    Ok(false) => {
                        // `simd_in_place_supported` and the executor are a
                        // single capability contract.  Reaching this arm
                        // means the implementation failed to honor its own
                        // preflight, so report unsupported instead of
                        // silently switching to an allocating/scalar path.
                        crate::compute::record_pipeline_operation_path("unsupported");
                        crate::compute::finish_pipeline_operation_telemetry();
                        return Err(PilError::NotImplementedError(format!(
                            "SIMD does not support {} for the current image layout/mode",
                            registry::variant_key(current_op)
                        )));
                    }
                    Err(error) => {
                        crate::compute::record_pipeline_operation_path("unsupported");
                        crate::compute::finish_pipeline_operation_telemetry();
                        return Err(error);
                    }
                }
            }
            let op = &ops[index];
            let key = registry::variant_key(op);
            let entry = registry::registry()?
                .get(key)
                .ok_or_else(|| PilError::ValueError(format!("SIMD: unknown op {}", key)))?;
            let f = entry
                .simd_fn
                .ok_or_else(|| PilError::ValueError(format!("SIMD: no native impl for {}", key)))?;
            crate::compute::begin_pipeline_operation_telemetry(key);
            let next = match f(input, op, op_mode) {
                Ok(next) => next,
                Err(error) => {
                    crate::compute::record_pipeline_operation_path("unsupported");
                    crate::compute::finish_pipeline_operation_telemetry();
                    return Err(error);
                }
            };
            crate::compute::finish_pipeline_operation_telemetry();
            crate::compute::account_host_buffer_boundary(&mut resources, input, &next);
            current = Some(next);
            current_mode = ops::adapters::simd_mode_after_op(op, op_mode);
            index += 1;
        }
        // Image.merge consumes a P source as a raw one-byte band but creates
        // a new multi-band image. Do not run its RGB/LA/RGBA result through
        // the P-mode normalizer, which is reserved for operations that retain
        // the source palette sample layout.
        let current = current.unwrap_or_else(|| img.clone());
        let result = if ops.iter().any(|op| matches!(op, PipelineOp::Merge { .. })) {
            Ok(current)
        } else {
            normalize_palette_result(current, mode)
        }?;
        crate::compute::record_pipeline_resource_telemetry(resources);
        Ok(result)
    }
}
