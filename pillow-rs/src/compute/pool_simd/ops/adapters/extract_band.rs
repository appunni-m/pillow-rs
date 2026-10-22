//! SIMD adapter for native 8-bit ExtractBand/GetChannel operations.
//!
//! This family owns mode/layout admission, the registry-facing wrapper, and
//! direct output-buffer execution. Architecture-specific channel movement is
//! shared with sibling channel operations through the parent native kernel.

use super::super::native_channel as native_channel_kernels;
use super::{SimdImageShape, shape_channels, simd_unsupported};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView, GrayImage};

fn native_extract_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn shape_native_extract_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        super::SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L" | "P")),
        super::SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        super::SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        super::SimdLayout::Rgba8 => {
            matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX"))
        }
        super::SimdLayout::Luma16 | super::SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

pub(super) fn supports_for_image(img: &DynamicImage, index: u8, mode: Option<&str>) -> bool {
    native_extract_layout(img, mode).is_some_and(|channels| {
        usize::from(index) < channels
            && img
                .width()
                .checked_mul(img.height())
                .and_then(|pixels| pixels.checked_mul(channels as u32))
                .is_some_and(|bytes| img.as_bytes().len() == bytes as usize && bytes != 0)
    })
}

pub(super) fn supports_for_shape(shape: SimdImageShape, index: u8, mode: Option<&str>) -> bool {
    let pixel_count = (shape.width as usize).saturating_mul(shape.height as usize);
    shape_native_extract_channels(shape, mode).is_some_and(|channels| {
        usize::from(index) < channels && pixel_count.saturating_mul(channels) != 0
    })
}

fn extract_band_path(_channels: usize, pixel_count: usize, vector_blocks: u64) -> &'static str {
    if pixel_count == 0 {
        "scalar-control"
    } else if vector_blocks != 0 {
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            if matches!(_channels, 2..=4) {
                "neon-deinterleave"
            } else {
                "vector-gather"
            }
        }
        #[cfg(target_arch = "x86_64")]
        {
            "ssse3-gather"
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
        #[cfg(not(target_arch = "x86_64"))]
        {
            "vector-gather"
        }
    } else {
        "scalar-gather"
    }
}

/// Extract one channel from a native 8-bit image without expanding it to
/// packed RGBA storage.  The public `getchannel` operation is a byte-domain
/// copy for L/LA/RGB/RGBA (and CMYK, which uses the four-byte RGBA storage in
/// this crate), so widening it would add a conversion boundary for no gain.
/// Typed samples are deliberately left on the exact CPU implementation and
/// recorded as an internal fallback rather than being mislabeled as SIMD.
pub fn simd_extract_band(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::ExtractBand { index } = op else {
        return Err(PilError::ValueError("expected ExtractBand op".into()));
    };

    let (width, height) = img.dimensions();
    let dimensions = CheckedDims::new_allow_empty(width, height, 1)?;
    let pixel_count = dimensions.total_bytes();
    let channels =
        native_extract_layout(img, mode).ok_or_else(|| simd_unsupported("ExtractBand"))?;
    let (output, vector_blocks, scalar_tail, path) = if channels == 3 {
        let source = img.as_bytes();
        let source_len = pixel_count
            .checked_mul(channels)
            .ok_or_else(|| PilError::InternalError("ExtractBand source length overflow".into()))?;
        if source.len() != source_len {
            return Err(PilError::InternalError(
                "ExtractBand source buffer shape mismatch".into(),
            ));
        }
        let channel = usize::from(*index).min(channels - 1);
        let (output, vector_blocks, scalar_tail) =
            native_channel_kernels::extract_rgb_channel_bytes(source, channel)
                .ok_or_else(|| simd_unsupported("ExtractBand"))?;
        if output.len() != pixel_count {
            return Err(PilError::InternalError(
                "SIMD ExtractBand buffer shape mismatch".into(),
            ));
        }
        crate::compute::record_pipeline_allocation(output.len());
        let path = extract_band_path(channels, pixel_count, vector_blocks);
        (output, vector_blocks, scalar_tail, path)
    } else {
        let mut output = dimensions.alloc_buffer();
        let (vector_blocks, scalar_tail, path) =
            simd_extract_band_into(img, *index, mode, &mut output)?;
        (output, vector_blocks, scalar_tail, path)
    };
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path(path);

    GrayImage::from_raw(width, height, output)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD ExtractBand buffer shape mismatch".into()))
}

/// Extract one byte channel directly into the caller's final host buffer.
/// The returned counters prove whether architecture-specific vector blocks or
/// a scalar tail produced the output. Callers must provide exactly one byte
/// for every source pixel.
pub fn simd_extract_band_into(
    img: &DynamicImage,
    index: u8,
    mode: Option<&str>,
    output: &mut [u8],
) -> Result<(u64, u64, &'static str), PilError> {
    let channels =
        native_extract_layout(img, mode).ok_or_else(|| simd_unsupported("ExtractBand"))?;
    let source = img.as_bytes();

    let (width, height) = img.dimensions();
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::InternalError("ExtractBand pixel count overflow".into()))?;
    let source_len = pixel_count
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("ExtractBand source length overflow".into()))?;
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "ExtractBand source buffer shape mismatch".into(),
        ));
    }
    if output.len() != pixel_count {
        return Err(PilError::InternalError(
            "SIMD ExtractBand output length mismatch".into(),
        ));
    }

    // `getchannel` validates the public index before queuing the operation.
    // Keep the CPU operation's defensive clamping for direct internal
    // PipelineOp callers while using the native storage stride here.
    let channel = usize::from(index).min(channels - 1);
    // Keep the stride and selected lane constant. On AArch64, structure loads
    // deinterleave the exact 16-pixel source span; other architectures use the
    // portable vector gather. A scalar tail handles only the final partial
    // block.
    let (vector_blocks, scalar_tail) =
        native_channel_kernels::extract_channel_into(source, output, channels, channel);
    let path = extract_band_path(channels, pixel_count, vector_blocks);
    Ok((vector_blocks, scalar_tail, path))
}

#[cfg(test)]
mod tests {
    use super::simd_extract_band;
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayAlphaImage, RgbImage, RgbaImage};

    #[test]
    fn extract_band_rgba_gather_matches_packed_bytes() {
        let pixel_count = 10;
        let raw: Vec<u8> = (0..pixel_count)
            .flat_map(|pixel| {
                let base = pixel * 4;
                [
                    (base + 3) as u8,
                    (base + 17) as u8,
                    (base + 31) as u8,
                    (base + 47) as u8,
                ]
            })
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(5, 2, raw).expect("RGBA source shape must be valid"),
        );

        for mode in ["RGBA", "CMYK", "RGBa", "RGBX"] {
            for index in 0..4u8 {
                let output =
                    simd_extract_band(&image, &PipelineOp::ExtractBand { index }, Some(mode))
                        .expect("SIMD RGBA-family ExtractBand must succeed");
                let expected: Vec<u8> = (0..pixel_count)
                    .map(|pixel| {
                        let base = pixel * 4;
                        [
                            (base + 3) as u8,
                            (base + 17) as u8,
                            (base + 31) as u8,
                            (base + 47) as u8,
                        ][usize::from(index)]
                    })
                    .collect();
                assert_eq!(
                    output.as_bytes(),
                    expected.as_slice(),
                    "mode={mode} index={index}"
                );
            }
        }
    }

    #[test]
    fn extract_band_vector_gathers_match_full_blocks_and_tail() {
        let pixel_count = 37;
        for (mode, channels) in [("LA", 2usize), ("RGB", 3), ("RGBA", 4)] {
            let raw: Vec<u8> = (0..pixel_count * channels)
                .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
                .collect();
            let image = match mode {
                "LA" => DynamicImage::ImageLumaA8(
                    GrayAlphaImage::from_raw(pixel_count as u32, 1, raw)
                        .expect("LA source shape must be valid"),
                ),
                "RGB" => DynamicImage::ImageRgb8(
                    RgbImage::from_raw(pixel_count as u32, 1, raw)
                        .expect("RGB source shape must be valid"),
                ),
                "RGBA" => DynamicImage::ImageRgba8(
                    RgbaImage::from_raw(pixel_count as u32, 1, raw)
                        .expect("RGBA source shape must be valid"),
                ),
                _ => unreachable!(),
            };

            for channel in 0..channels {
                let output = simd_extract_band(
                    &image,
                    &PipelineOp::ExtractBand {
                        index: channel as u8,
                    },
                    Some(mode),
                )
                .expect("SIMD ExtractBand vector gather must succeed");
                let expected: Vec<u8> = image
                    .as_bytes()
                    .chunks_exact(channels)
                    .map(|pixel| pixel[channel])
                    .collect();
                assert_eq!(
                    output.as_bytes(),
                    expected.as_slice(),
                    "mode={mode} channel={channel}"
                );
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn simd_extract_band_rgb_dispatches_to_ssse3_gather() {
        if !std::is_x86_feature_detected!("ssse3") {
            return;
        }

        let pixel_count = 37;
        let raw: Vec<u8> = (0..pixel_count * 3)
            .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(pixel_count as u32, 1, raw).expect("RGB source shape must be valid"),
        );

        let previous = crate::compute::Backend::set_pipeline_telemetry_enabled(true);
        crate::compute::begin_pipeline_operation_telemetry("ExtractBand");
        let output = simd_extract_band(&image, &PipelineOp::ExtractBand { index: 1 }, Some("RGB"))
            .expect("SIMD ExtractBand must use the SSSE3 gather");
        crate::compute::finish_pipeline_operation_telemetry();
        let telemetry = crate::compute::Backend::take_pipeline_operation_telemetry();
        crate::compute::Backend::set_pipeline_telemetry_enabled(previous);

        let operation = telemetry
            .first()
            .expect("SIMD ExtractBand must publish operation telemetry");
        assert_eq!(operation.path, "ssse3-gather");
        assert_eq!(operation.vector_block_count, 2);
        assert_eq!(operation.scalar_tail_count, 5);
        assert_eq!(
            output.as_bytes(),
            image
                .as_bytes()
                .chunks_exact(3)
                .map(|pixel| pixel[1])
                .collect::<Vec<_>>()
                .as_slice()
        );
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[test]
    fn simd_extract_band_rgb_dispatches_to_neon_deinterleave() {
        let pixel_count = 37;
        let raw: Vec<u8> = (0..pixel_count * 3)
            .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(pixel_count as u32, 1, raw).expect("RGB source shape must be valid"),
        );

        let previous = crate::compute::Backend::set_pipeline_telemetry_enabled(true);
        crate::compute::begin_pipeline_operation_telemetry("ExtractBand");
        let output = simd_extract_band(&image, &PipelineOp::ExtractBand { index: 1 }, Some("RGB"))
            .expect("SIMD ExtractBand must succeed");
        crate::compute::finish_pipeline_operation_telemetry();
        let telemetry = crate::compute::Backend::take_pipeline_operation_telemetry();
        crate::compute::Backend::set_pipeline_telemetry_enabled(previous);

        let operation = telemetry
            .first()
            .expect("SIMD ExtractBand must publish operation telemetry");
        assert_eq!(operation.path, "neon-deinterleave");
        assert_eq!(operation.vector_block_count, 2);
        assert_eq!(operation.scalar_tail_count, 5);
        assert_eq!(
            output.as_bytes(),
            image
                .as_bytes()
                .chunks_exact(3)
                .map(|pixel| pixel[1])
                .collect::<Vec<_>>()
                .as_slice()
        );
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[test]
    fn simd_extract_band_la_and_rgba_dispatch_to_neon_deinterleave() {
        let pixel_count = 37;
        let previous = crate::compute::Backend::set_pipeline_telemetry_enabled(true);

        for (channels, mode) in [(2usize, "LA"), (4usize, "RGBA")] {
            let raw: Vec<u8> = (0..pixel_count * channels)
                .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
                .collect();
            let image = match channels {
                2 => DynamicImage::ImageLumaA8(
                    GrayAlphaImage::from_raw(pixel_count as u32, 1, raw)
                        .expect("LA source shape must be valid"),
                ),
                4 => DynamicImage::ImageRgba8(
                    RgbaImage::from_raw(pixel_count as u32, 1, raw)
                        .expect("RGBA source shape must be valid"),
                ),
                _ => unreachable!(),
            };

            for channel in 0..channels {
                crate::compute::begin_pipeline_operation_telemetry("ExtractBand");
                let output = simd_extract_band(
                    &image,
                    &PipelineOp::ExtractBand {
                        index: channel as u8,
                    },
                    Some(mode),
                )
                .expect("SIMD ExtractBand must use the NEON structure load");
                crate::compute::finish_pipeline_operation_telemetry();
                let telemetry = crate::compute::Backend::take_pipeline_operation_telemetry();
                let operation = telemetry
                    .first()
                    .expect("SIMD ExtractBand must publish operation telemetry");

                assert_eq!(
                    operation.path, "neon-deinterleave",
                    "{mode} channel {channel}"
                );
                assert_eq!(operation.vector_block_count, 2, "{mode} channel {channel}");
                assert_eq!(operation.scalar_tail_count, 5, "{mode} channel {channel}");
                assert_eq!(
                    output.as_bytes(),
                    image
                        .as_bytes()
                        .chunks_exact(channels)
                        .map(|pixel| pixel[channel])
                        .collect::<Vec<_>>()
                        .as_slice(),
                    "{mode} channel {channel}"
                );
            }
        }

        crate::compute::Backend::set_pipeline_telemetry_enabled(previous);
    }
}
