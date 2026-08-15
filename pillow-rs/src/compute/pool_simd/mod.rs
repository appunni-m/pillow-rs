//! SIMD worker pool — implements BackendImpl for SIMD-accelerated CPU compute.
//!
//! Uses the `wide` crate for portable SIMD (SSE, AVX, NEON) to process pixels
//! in vectorized chunks. Unsupported operations are routed to another backend
//! before execution; an explicitly locked SIMD pipeline reports an error.
//!
//! ## Architecture
//! - Same mode encoding as GPU: 0=L, 1=LA, 2=RGB, 3=RGBA
//! - Processes RGBA8 packed u32 as 4 independent u8 lanes
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

        let mut current = img.clone();
        let mut resources = crate::compute::host_resource_telemetry(img);
        let mut index = 0usize;
        while index < ops.len() {
            if mode.is_none() {
                if index + 1 < ops.len() {
                    if let (
                        PipelineOp::Multiply { other: first_other },
                        PipelineOp::Screen {
                            other: second_other,
                        },
                    ) = (&ops[index], &ops[index + 1])
                    {
                        if let Some(fused) = ops::adapters::simd_fused_multiply_screen(
                            &current,
                            first_other,
                            second_other,
                            mode,
                        )? {
                            crate::compute::account_host_buffer_boundary(
                                &mut resources,
                                &current,
                                &fused,
                            );
                            resources.fused_operation_count =
                                resources.fused_operation_count.saturating_add(2);
                            current = fused;
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
                            current.width(),
                            current.height(),
                        ) else {
                            break;
                        };
                        combined = composed;
                        consumed += 1;
                    }
                    if consumed > 1 {
                        let fused = PipelineOp::Transpose { method: combined };
                        let key = registry::variant_key(&fused);
                        let entry = registry::registry()?.get(key).ok_or_else(|| {
                            PilError::ValueError(format!("SIMD: unknown op {}", key))
                        })?;
                        let f = entry.simd_fn.ok_or_else(|| {
                            PilError::ValueError(format!("SIMD: no native impl for {}", key))
                        })?;
                        let next = f(&current, &fused, mode)?;
                        crate::compute::account_host_buffer_boundary(
                            &mut resources,
                            &current,
                            &next,
                        );
                        resources.fused_operation_count = resources
                            .fused_operation_count
                            .saturating_add(consumed as u64);
                        current = next;
                        index += consumed;
                        continue;
                    }
                }
            }
            if let Some((consumed, lut)) = fused_point_batch(&ops[index..], &current, mode) {
                let next =
                    if let Some(native) = ops::adapters::native_point_lut(&current, mode, &lut) {
                        native
                    } else {
                        let fused = PipelineOp::Eval { lut: lut.into() };
                        ops::adapters::simd_eval(&current, &fused, mode)?
                    };
                crate::compute::account_host_buffer_boundary(&mut resources, &current, &next);
                resources.fused_operation_count = resources
                    .fused_operation_count
                    .saturating_add(consumed as u64);
                current = next;
                index += consumed;
                continue;
            }
            let op = &ops[index];
            let key = registry::variant_key(op);
            let entry = registry::registry()?
                .get(key)
                .ok_or_else(|| PilError::ValueError(format!("SIMD: unknown op {}", key)))?;
            let f = entry
                .simd_fn
                .ok_or_else(|| PilError::ValueError(format!("SIMD: no native impl for {}", key)))?;
            let next = f(&current, op, mode)?;
            crate::compute::account_host_buffer_boundary(&mut resources, &current, &next);
            current = next;
            index += 1;
        }
        // Image.merge consumes a P source as a raw one-byte band but creates
        // a new multi-band image. Do not run its RGB/LA/RGBA result through
        // the P-mode normalizer, which is reserved for operations that retain
        // the source palette sample layout.
        let result = if ops.iter().any(|op| matches!(op, PipelineOp::Merge { .. })) {
            Ok(current)
        } else {
            normalize_palette_result(current, mode)
        }?;
        crate::compute::record_pipeline_resource_telemetry(resources);
        Ok(result)
    }
}
