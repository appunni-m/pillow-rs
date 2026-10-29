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
use crate::pipeline::PipelineOp;
#[cfg(feature = "gpu")]
use crate::pipeline::TransposeMethod;
#[cfg(feature = "gpu")]
use crate::pipeline::TransposeTransform;
use crate::raster::DynamicImage;

mod batch_execution;
pub(crate) mod ops;
mod point_batch;
mod put_pixel_batch;

// ─── SimdPool ──────────────────────────────────────────────────────────────

/// SIMD compute pool — CPU-accelerated via portable SIMD vectors.
///
/// Executes only operations with a registered SIMD implementation.
pub struct SimdPool;

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
        // Palette kernels operate on native index bytes. When one has already
        // returned the exact tightly packed L/index layout, keep that storage
        // instead of expanding each index to RGBA and gathering red back out.
        // Retain the conversion below for buffers with trailing samples: it
        // canonicalizes those to the logical image dimensions.
        Some("P")
            if matches!(
                &result,
                DynamicImage::ImageLuma8(image)
                    if crate::checked_dims::CheckedDims::new_allow_empty(
                        image.width(),
                        image.height(),
                        1,
                    )
                    .is_ok_and(|dims| dims.total_bytes() == image.as_raw().len())
            ) =>
        {
            Ok(result)
        }
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
        // PA's native ImageLumaA8 storage already is the index/alpha pair.
        // Keep that exact layout; expanding and immediately narrowing it
        // would allocate and copy the full frame without changing a sample.
        // Preserve the established normalization for other result variants.
        Some("PA") if matches!(&result, DynamicImage::ImageLumaA8(_)) => Ok(result),
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
        batch_execution::execute_batch(ops, img, mode)
    }
}

fn finish_simd_batch(
    img: &DynamicImage,
    ops: &[PipelineOp],
    current: Option<DynamicImage>,
    current_mode: Option<&str>,
    mut resources: crate::compute::PipelineResourceTelemetry,
) -> Result<DynamicImage, PilError> {
    let current = current.unwrap_or_else(|| {
        let output = img.clone();
        if !ops.is_empty() {
            crate::compute::record_pipeline_allocation(output.as_bytes().len());
            crate::compute::account_host_buffer_boundary(&mut resources, img, &output);
        }
        output
    });
    let result = if ops.iter().any(|op| matches!(op, PipelineOp::Merge { .. })) {
        Ok(current)
    } else {
        normalize_palette_result(current, current_mode)
    }?;
    crate::compute::record_pipeline_resource_telemetry(resources);
    Ok(result)
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

    struct RestoreTelemetry(bool);

    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
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
        let bytes: Vec<u8> = (0..9 * 31 * 3)
            .map(|index| (index * 37 + index / 11) as u8)
            .collect();
        let source = raw_bytes_to_image_allow_empty(9, 31, bytes.clone(), 3)?;
        let identity = transpose_ops(&[TransposeMethod::Rotate90, TransposeMethod::Rotate270]);
        let expected_invert = registry::execute_cpu(&PipelineOp::Invert, &source, Some("RGB"))?;
        let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
        for backend in [&CpuPool as &dyn BackendImpl, &SimdPool] {
            let (result, retained) =
                assert_transpose_identity_owned_output(backend, &identity, &source, &bytes)?;
            assert_transpose_identity_invert_continuations(
                backend,
                &identity,
                &source,
                &expected_invert,
                &result,
                &retained,
                &bytes,
            )?;
        }
        Ok(())
    }

    fn assert_transpose_identity_owned_output(
        backend: &dyn BackendImpl,
        identity: &[PipelineOp],
        source: &DynamicImage,
        bytes: &[u8],
    ) -> Result<(DynamicImage, Vec<u8>), PilError> {
        crate::compute::reset_pipeline_operation_telemetry();
        Backend::reset_pipeline_allocation_telemetry();
        let mut result = backend.execute_batch(identity, source, Some("RGB"))?;
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
        Ok((result, retained))
    }

    fn assert_transpose_identity_invert_continuations(
        backend: &dyn BackendImpl,
        identity: &[PipelineOp],
        source: &DynamicImage,
        expected_invert: &DynamicImage,
        result: &DynamicImage,
        retained: &[u8],
        bytes: &[u8],
    ) -> Result<(), PilError> {
        for leading_identity in [false, true] {
            let ops = if leading_identity {
                vec![identity[0].clone(), identity[1].clone(), PipelineOp::Invert]
            } else {
                vec![PipelineOp::Invert, identity[0].clone(), identity[1].clone()]
            };
            crate::compute::reset_pipeline_operation_telemetry();
            let actual = backend.execute_batch(&ops, source, Some("RGB"))?;
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

#[cfg(test)]
mod palette_normalization_tests {
    use crate::compute::BackendImpl;
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage};

    #[test]
    fn palette_indices_keep_tightly_packed_luma_storage() {
        let image = GrayImage::from_raw(3, 1, vec![7, 129, 244]).unwrap();
        let result =
            super::normalize_palette_result(DynamicImage::ImageLuma8(image), Some("P")).unwrap();

        let DynamicImage::ImageLuma8(result) = result else {
            panic!("P-mode output must retain its one-byte index storage");
        };
        assert_eq!(result.as_raw(), &[7, 129, 244]);
    }

    #[test]
    fn palette_indices_with_trailing_storage_are_canonicalized() {
        let image = GrayImage::from_raw(2, 1, vec![7, 129, 244]).unwrap();
        let result =
            super::normalize_palette_result(DynamicImage::ImageLuma8(image), Some("P")).unwrap();

        let DynamicImage::ImageLuma8(result) = result else {
            panic!("P-mode output must use one-byte index storage");
        };
        assert_eq!(result.dimensions(), (2, 1));
        assert_eq!(result.as_raw(), &[7, 129]);
    }

    #[test]
    fn pa_extract_band_returns_l_samples_without_reapplying_pa_layout() {
        let source_bytes: Vec<u8> = (0..16 * 16).flat_map(|pixel| [pixel as u8, 192]).collect();
        let source = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(16, 16, source_bytes).expect("PA source dimensions"),
        );

        for (index, expected) in [
            (0, (0..256).map(|pixel| pixel as u8).collect::<Vec<_>>()),
            (1, vec![192; 256]),
        ] {
            let actual = super::SimdPool
                .execute_batch(&[PipelineOp::ExtractBand { index }], &source, Some("PA"))
                .expect("SIMD PA channel extraction");
            let DynamicImage::ImageLuma8(actual) = actual else {
                panic!("ExtractBand must return one-band L storage");
            };
            assert_eq!(actual.as_raw(), &expected, "PA channel {index}");
        }
    }
}

#[cfg(test)]
mod alpha_composite_mirror_tests {
    use super::SimdPool;
    use crate::compute::BackendImpl;
    use crate::error::PilError;
    use crate::image::Image;
    use crate::image_utils::raw_bytes_to_image;
    use crate::pipeline::PipelineOp;
    use std::sync::Arc;

    #[test]
    fn rgba_alpha_composite_mirror_fusion_matches_sequential_with_tails() -> Result<(), PilError> {
        for (width, height, uniform) in [
            (64u32, 3u32, true),
            (17, 5, false),
            (9, 2, false),
            (8, 1, false),
            (1, 8, false),
        ] {
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
            let source_image = raw_bytes_to_image(width, height, source_bytes, 4)?;
            let source = Arc::new(Image::from_dynamic(source_image, Some("RGBA".to_owned())));
            let operations = [
                PipelineOp::AlphaComposite {
                    source,
                    dest: (0, 0),
                    src: (0, 0),
                },
                PipelineOp::Mirror,
            ];
            let composite = SimdPool.execute_batch(&operations[..1], &destination, Some("RGBA"))?;
            let expected = SimdPool.execute_batch(&operations[1..], &composite, Some("RGBA"))?;
            let actual = SimdPool.execute_batch(&operations, &destination, Some("RGBA"))?;
            assert_eq!(
                (actual.width(), actual.height()),
                (expected.width(), expected.height())
            );
            assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");
        }
        Ok(())
    }
}
