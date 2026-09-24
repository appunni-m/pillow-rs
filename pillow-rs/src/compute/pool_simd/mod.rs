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
#[cfg(feature = "gpu")]
use crate::pipeline::TransposeMethod;
use crate::pipeline::{PipelineOp, TransposeTransform};
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
        PipelineOp::Eval { lut } if lut.len() == bands * 256 => Some(lut.to_vec()),
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

/// Compose methods for the existing GPU caller. Dimensions do not affect D4
/// composition; `None` denotes its exact identity element.
#[cfg(feature = "gpu")]
pub(super) fn compose_transpose_methods(
    first: &TransposeMethod,
    second: &TransposeMethod,
    _width: u32,
    _height: u32,
) -> Option<TransposeMethod> {
    match TransposeTransform::Method(first.clone()).then(second) {
        TransposeTransform::Identity => None,
        TransposeTransform::Method(method) => Some(method),
    }
}

/// An identity run can retain storage only if it is already tightly packed.
/// `ImageBuffer::from_raw` also admits extra trailing samples; an actual
/// transpose discards those samples, so such inputs retain the original path.
pub(super) fn transpose_identity_can_reuse(img: &DynamicImage) -> bool {
    crate::checked_dims::CheckedDims::new_allow_empty(
        img.width(),
        img.height(),
        img.color().bytes_per_pixel(),
    )
    .is_ok_and(|dims| dims.total_bytes() == img.as_bytes().len())
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
        log::debug!(
            "[SIMD] {} op(s) {}x{}: {:?}",
            ops.len(),
            img.width(),
            img.height(),
            ops.iter().map(registry::variant_key).collect::<Vec<_>>()
        );

        // The first operation can read the materialized source directly.  Do
        // not clone the full frame merely to seed the accumulator; each
        // adapter already returns an owned output buffer. Empty batches and
        // borrowed all-identity batches clone only at return.
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
            }
            if let PipelineOp::Transpose { method } = &ops[index] {
                let mut combined = TransposeTransform::Method(method.clone());
                let mut consumed = 1usize;
                while index + consumed < ops.len() {
                    let PipelineOp::Transpose { method: next } = &ops[index + consumed] else {
                        break;
                    };
                    combined = combined.then(next);
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
                    if matches!(combined, TransposeTransform::Identity)
                        && transpose_identity_can_reuse(input)
                    {
                        for op in &ops[index..index + consumed] {
                            crate::compute::begin_pipeline_operation_telemetry(
                                registry::variant_key(op),
                            );
                            crate::compute::record_pipeline_operation_path("scalar-control");
                            crate::compute::finish_pipeline_operation_telemetry();
                        }
                        resources.fused_operation_count = resources
                            .fused_operation_count
                            .saturating_add(consumed as u64);
                        index += consumed;
                        continue;
                    }
                    if let TransposeTransform::Method(combined) = combined {
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
        let current = current.unwrap_or_else(|| {
            let output = img.clone();
            if !ops.is_empty() {
                // Identity admission checked the complete native allocation.
                // An owned result is still required at the public boundary.
                crate::compute::record_pipeline_allocation(output.as_bytes().len());
                crate::compute::account_host_buffer_boundary(&mut resources, img, &output);
            }
            output
        });
        let result = if ops.iter().any(|op| matches!(op, PipelineOp::Merge { .. })) {
            Ok(current)
        } else {
            normalize_palette_result(current, mode)
        }?;
        crate::compute::record_pipeline_resource_telemetry(resources);
        Ok(result)
    }
}

#[cfg(test)]
mod transpose_tests {
    use super::SimdPool;
    use crate::compute::{Backend, BackendImpl, CpuPool, registry};
    use crate::error::PilError;
    use crate::image::Image;
    use crate::image_utils::raw_bytes_to_image_allow_empty;
    use crate::pipeline::{PipelineOp, TransposeMethod};
    use crate::raster::{
        DynamicImage, GenericImageView, ImageBuffer, Luma, Rgb, Rgb32FImage, RgbImage,
    };

    const METHODS: [TransposeMethod; 7] = [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ];

    fn inverse(method: &TransposeMethod) -> TransposeMethod {
        match method {
            TransposeMethod::Rotate90 => TransposeMethod::Rotate270,
            TransposeMethod::Rotate270 => TransposeMethod::Rotate90,
            _ => method.clone(),
        }
    }

    fn transpose_ops(methods: &[TransposeMethod]) -> Vec<PipelineOp> {
        methods
            .iter()
            .map(|method| PipelineOp::Transpose {
                method: method.clone(),
            })
            .collect()
    }

    fn assert_backends_match_sequential(
        source: &DynamicImage,
        mode: Option<&str>,
        methods: &[TransposeMethod],
    ) -> Result<(), PilError> {
        let ops = transpose_ops(methods);
        let expected = ops.iter().try_fold(source.clone(), |input, op| {
            registry::execute_cpu(op, &input, mode)
        })?;
        for backend in [&CpuPool as &dyn BackendImpl, &SimdPool] {
            let actual = backend.execute_batch(&ops, source, mode)?;
            assert_eq!(actual.color(), expected.color(), "{methods:?} {mode:?}");
            assert_eq!(
                actual.dimensions(),
                expected.dimensions(),
                "{methods:?} {mode:?}"
            );
            assert_eq!(
                actual.as_bytes(),
                expected.as_bytes(),
                "{methods:?} {mode:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn transpose_identity_all_pairs_and_continuations_preserve_native_pixels()
    -> Result<(), PilError> {
        for (width, height) in [(0, 0), (0, 7), (7, 0), (1, 1), (1, 7), (9, 1), (13, 19)] {
            for (channels, mode) in [
                (1, "L"),
                (1, "P"),
                (2, "LA"),
                (2, "PA"),
                (3, "RGB"),
                (4, "RGBA"),
                (4, "I"),
                (4, "F"),
            ] {
                let bytes: Vec<u8> = (0..width as usize * height as usize * channels)
                    .map(|index| index.wrapping_mul(113).wrapping_add(index / 7) as u8)
                    .collect();
                let source =
                    raw_bytes_to_image_allow_empty(width, height, bytes.clone(), channels)?;
                for first in &METHODS {
                    for second in &METHODS {
                        assert_backends_match_sequential(
                            &source,
                            Some(mode),
                            &[first.clone(), second.clone()],
                        )?;
                        assert_backends_match_sequential(
                            &source,
                            Some(mode),
                            &[first.clone(), inverse(first), second.clone()],
                        )?;
                    }
                }
                assert_eq!(source.as_bytes(), bytes);
            }
        }
        Ok(())
    }

    #[test]
    fn transpose_identity_preserves_typed_bits_and_native_variant() -> Result<(), PilError> {
        let patterns = [
            0x7f80_0001u32,
            0x7fc1_2345,
            0x8000_0000,
            0,
            0xff80_0000,
            0xffc5_4321,
        ];
        let bytes: Vec<u8> = (0..9 * 31)
            .flat_map(|index| patterns[(index * 3 + index / 7) % patterns.len()].to_ne_bytes())
            .collect();
        let packed = raw_bytes_to_image_allow_empty(9, 31, bytes.clone(), 4)?;
        let samples: Vec<u16> = (0..9 * 31)
            .map(|index| (index * 571 + index / 3 * 19) as u16)
            .collect();
        let sixteen = DynamicImage::ImageLuma16(
            ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(9, 31, samples).unwrap(),
        );
        for first in &METHODS {
            for second in &METHODS {
                for mode in ["I", "F"] {
                    assert_backends_match_sequential(
                        &packed,
                        Some(mode),
                        &[first.clone(), second.clone()],
                    )?;
                }
                for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
                    assert_backends_match_sequential(
                        &sixteen,
                        Some(mode),
                        &[first.clone(), second.clone()],
                    )?;
                }
            }
        }
        assert_eq!(packed.as_bytes(), bytes);
        Ok(())
    }

    #[test]
    fn transpose_identity_does_not_skip_unsupported_simd_admission() {
        let source = DynamicImage::ImageRgb32F(
            Rgb32FImage::from_raw(2, 2, vec![f32::from_bits(0x7fc1_2345); 12]).unwrap(),
        );
        let ops = transpose_ops(&[TransposeMethod::Rotate90, TransposeMethod::Rotate270]);
        assert!(matches!(
            SimdPool.execute_batch(&ops, &source, None),
            Err(PilError::NotImplementedError(_))
        ));
        let empty = DynamicImage::ImageLuma16(
            ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(0, 7, Vec::new()).unwrap(),
        );
        assert!(matches!(
            SimdPool.execute_batch(&ops, &empty, Some("I;16")),
            Err(PilError::NotImplementedError(_))
        ));
        // CPU can preserve the actual float raster variant without interpreting its NaNs.
        let result = CpuPool.execute_batch(&ops, &source, None).unwrap();
        assert_eq!(result.color(), source.color());
        assert_eq!(result.as_bytes(), source.as_bytes());
    }

    #[test]
    fn transpose_identity_preserves_trailing_storage_normalization() -> Result<(), PilError> {
        let source = DynamicImage::ImageRgb8(
            RgbImage::from_raw(3, 5, (0..52).map(|index| (index * 37) as u8).collect()).unwrap(),
        );
        assert!(!super::transpose_identity_can_reuse(&source));
        for first in &METHODS {
            assert_backends_match_sequential(
                &source,
                Some("RGB"),
                &[first.clone(), inverse(first)],
            )?;
        }
        assert_eq!(source.as_bytes().len(), 52);
        Ok(())
    }

    #[test]
    fn transpose_identity_preserves_ownership_and_reports_zero_kernel_work() -> Result<(), PilError>
    {
        struct RestoreTelemetry(bool);
        impl Drop for RestoreTelemetry {
            fn drop(&mut self) {
                Backend::set_pipeline_telemetry_enabled(self.0);
            }
        }
        let bytes: Vec<u8> = (0..9 * 31 * 3)
            .map(|index| (index * 37 + index / 11) as u8)
            .collect();
        let source = raw_bytes_to_image_allow_empty(9, 31, bytes.clone(), 3)?;
        let identity = transpose_ops(&[TransposeMethod::Rotate90, TransposeMethod::Rotate270]);
        let expected_invert = registry::execute_cpu(&PipelineOp::Invert, &source, Some("RGB"))?;
        let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        for backend in [&CpuPool as &dyn BackendImpl, &SimdPool] {
            crate::compute::reset_pipeline_operation_telemetry();
            Backend::reset_pipeline_allocation_telemetry();
            let mut result = backend.execute_batch(&identity, &source, Some("RGB"))?;
            let records = crate::compute::take_pipeline_operation_telemetry();
            assert_eq!(records.len(), 2);
            for record in records {
                assert_eq!(record.operation, "Transpose");
                assert_eq!(record.vector_block_count, 0);
                assert_eq!(record.scalar_tail_count, 0);
                assert_eq!(record.mode_conversion_count, 0);
                assert_eq!(record.handoff_count, 0);
                assert_eq!(
                    record.path,
                    if backend.name() == Backend::Cpu {
                        "cpu"
                    } else {
                        "scalar-control"
                    }
                );
            }
            let resources = crate::compute::take_pipeline_resource_telemetry().unwrap();
            assert_eq!(resources.fused_operation_count, 2);
            assert_eq!(resources.host_buffer_count, 2);
            let allocations = Backend::take_pipeline_allocation_telemetry();
            assert_eq!(allocations.allocation_count, 1);
            assert_eq!(allocations.allocated_bytes, bytes.len() as u64);
            assert_eq!(result.as_bytes(), bytes);
            assert_ne!(result.as_bytes().as_ptr(), source.as_bytes().as_ptr());
            result
                .as_mut_rgb8()
                .unwrap()
                .put_pixel(0, 0, Rgb([1, 2, 3]));
            let retained = result.as_bytes().to_vec();
            assert_eq!(source.as_bytes(), bytes);

            for leading_identity in [false, true] {
                let ops = if leading_identity {
                    vec![identity[0].clone(), identity[1].clone(), PipelineOp::Invert]
                } else {
                    vec![PipelineOp::Invert, identity[0].clone(), identity[1].clone()]
                };
                crate::compute::reset_pipeline_operation_telemetry();
                let actual = backend.execute_batch(&ops, &source, Some("RGB"))?;
                assert_eq!(actual.as_bytes(), expected_invert.as_bytes());
                let records = crate::compute::take_pipeline_operation_telemetry();
                assert_eq!(records.len(), 3);
                for record in records
                    .iter()
                    .filter(|record| record.operation == "Transpose")
                {
                    assert_eq!(record.vector_block_count, 0);
                    assert_eq!(record.scalar_tail_count, 0);
                }
                let resources = crate::compute::take_pipeline_resource_telemetry().unwrap();
                assert_eq!(resources.fused_operation_count, 2);
                assert_eq!(resources.host_buffer_count, 2);
                assert_eq!(result.as_bytes(), retained);
                assert_eq!(source.as_bytes(), bytes);
            }
        }
        Ok(())
    }

    #[test]
    fn transpose_identity_keeps_public_pa_materialization_boundaries() -> Result<(), PilError> {
        let bytes: Vec<u8> = (0..9 * 31 * 2)
            .map(|index| (index * 71 + index / 7) as u8)
            .collect();
        for backend in [Backend::Cpu, Backend::Simd] {
            let source = Image::frombytes("PA", (9, 31), &bytes)?.use_backend(backend);
            let first = source.transpose("ROTATE_90")?;
            let second = first.transpose("ROTATE_270")?;
            let Image::Pipeline {
                source: boundary,
                ops,
                ..
            } = &second
            else {
                panic!("transpose retains a public pipeline result");
            };
            assert_eq!(ops.len(), 1);
            let Image::Pipeline {
                ops: previous_ops, ..
            } = boundary.as_ref()
            else {
                panic!("PA must retain its preceding materialization boundary");
            };
            assert_eq!(previous_ops.len(), 1);
            assert_eq!(second.mode()?, "PA");
            assert_eq!(second.tobytes()?, bytes);
            assert_eq!(source.tobytes()?, bytes);
        }
        Ok(())
    }
}
