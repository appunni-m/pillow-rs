//! SIMD adapter orchestration for native Image.merge execution.
//!
//! Keep admission, band ownership, backend evidence, and result construction
//! together here. Architecture-specific byte kernels remain in
//! `ops::native_merge`; CPU merge keeps its independent scalar/parallel paths.

use super::super::native_merge;
use super::{SimdImageShape, SimdLayout, simd_unsupported};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{ColorMode, PipelineOp};
use crate::raster::{DynamicImage, GenericImageView};
use std::sync::Arc;

fn native_merge_channels(mode: &ColorMode) -> Option<usize> {
    match mode {
        ColorMode::L | ColorMode::Mode1 => Some(1),
        ColorMode::LA => Some(2),
        ColorMode::RGB => Some(3),
        ColorMode::RGBA | ColorMode::CMYK => Some(4),
        _ => None,
    }
}

fn native_merge_band_contract(
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
) -> bool {
    // Pillow preserves the raw palette indices when a P band is first in a
    // multi-band merge. It is the only non-L band accepted by Image.merge;
    // later bands must still be L, and a single-band L merge cannot use P.
    let palette_first = mode == Some("P")
        && !matches!(target_mode, ColorMode::L | ColorMode::Mode1)
        && bands
            .first()
            .is_some_and(|band| band.mode().ok().is_some_and(|band_mode| band_mode == "P"));
    if !matches!(mode, None | Some("L")) && !palette_first {
        return false;
    }
    bands.iter().enumerate().all(|(index, band)| {
        let Ok(band_mode) = band.mode() else {
            return false;
        };
        band_mode == "L" || (palette_first && index == 0 && band_mode == "P")
    })
}

fn native_merge_contract_for_image(
    img: &DynamicImage,
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
    logical_mode: &str,
) -> Option<(usize, usize)> {
    // The SIMD interleave kernel only preserves canonical byte modes. Alias
    // targets (especially LAB, whose A/B bytes need a +128 bias) must remain
    // on the scalar path so target-specific encoding is not skipped.
    if !matches!(logical_mode, "L" | "LA" | "RGB" | "RGBA" | "CMYK") {
        return None;
    }
    let channels = native_merge_channels(target_mode)?;
    if !matches!(img, DynamicImage::ImageLuma8(_)) || bands.len() != channels {
        return None;
    }
    if !native_merge_band_contract(target_mode, bands, mode) {
        return None;
    }
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if pixels.checked_mul(channels).is_none() || img.as_bytes().len() != pixels {
        return None;
    }
    if !bands
        .iter()
        .all(|band| band.size().ok() == Some(img.dimensions()))
    {
        return None;
    }
    Some((channels, pixels))
}

fn native_merge_contract_for_shape(
    shape: SimdImageShape,
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
    logical_mode: &str,
) -> Option<(usize, usize)> {
    if !matches!(logical_mode, "L" | "LA" | "RGB" | "RGBA" | "CMYK") {
        return None;
    }
    let channels = native_merge_channels(target_mode)?;
    if shape.layout != SimdLayout::Luma8 || bands.len() != channels {
        return None;
    }
    if !native_merge_band_contract(target_mode, bands, mode) {
        return None;
    }
    let pixels = (shape.width as usize).checked_mul(shape.height as usize)?;
    if pixels.checked_mul(channels).is_none() {
        return None;
    }
    if !bands
        .iter()
        .all(|band| band.size().ok() == Some((shape.width, shape.height)))
    {
        return None;
    }
    Some((channels, pixels))
}

pub(super) fn supports_image_contract(
    img: &DynamicImage,
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
    logical_mode: &str,
) -> bool {
    native_merge_contract_for_image(img, target_mode, bands, mode, logical_mode).is_some()
}

pub(super) fn supports_shape_contract(
    shape: SimdImageShape,
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
    logical_mode: &str,
) -> bool {
    native_merge_contract_for_shape(shape, target_mode, bands, mode, logical_mode).is_some()
}
#[allow(unsafe_code)] // The x86 dispatch checks SSSE3 before calling its target-feature kernel.
pub(super) fn native_merge_interleave(
    width: u32,
    height: u32,
    pixels: usize,
    channels: usize,
    band_bytes: &[&[u8]],
) -> Result<(DynamicImage, u64, u64, &'static str), PilError> {
    let output_len = pixels
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD merge output length overflow".into()))?;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    if channels == 3 && pixels >= 16 {
        let (image, vector_blocks, scalar_tail) =
            native_merge::rgb_interleave_neon(width, height, pixels, output_len, band_bytes)?;
        crate::compute::record_pipeline_allocation(output_len);
        return Ok((image, vector_blocks, scalar_tail, "neon-interleave"));
    }
    #[cfg(target_arch = "x86_64")]
    if channels == 3 && pixels >= 16 && std::is_x86_feature_detected!("ssse3") {
        // SAFETY: the runtime feature probe above guarantees SSSE3 support;
        // native_merge_bytes validates the three equal-length L bands.
        let (image, vector_blocks, scalar_tail) = unsafe {
            native_merge::rgb_interleave_ssse3(width, height, pixels, output_len, band_bytes)?
        };
        crate::compute::record_pipeline_allocation(output_len);
        return Ok((image, vector_blocks, scalar_tail, "ssse3-interleave"));
    }

    let mut output = vec![0u8; output_len];
    let pixels_per_vector = match channels {
        1 => 16,
        2 => 8,
        3 | 4 => 4,
        _ => return Err(simd_unsupported("Merge")),
    };
    let mut pixel = 0usize;
    let mut vector_blocks = 0u64;
    while pixel + pixels_per_vector <= pixels {
        let (block_bytes, block) = native_merge::vector_block(band_bytes, pixel, channels)
            .ok_or_else(|| PilError::InternalError("SIMD merge vector shape mismatch".into()))?;
        let output_start = pixel * channels;
        output[output_start..output_start + block_bytes].copy_from_slice(&block[..block_bytes]);
        vector_blocks = vector_blocks.saturating_add(1);
        pixel += pixels_per_vector;
    }
    for output_pixel in pixel..pixels {
        let output_start = output_pixel * channels;
        for channel in 0..channels {
            output[output_start + channel] = band_bytes[channel][output_pixel];
        }
    }
    let scalar_tail = ((pixels - pixel) * channels) as u64;
    let result =
        crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, channels)?;
    crate::compute::record_pipeline_allocation(output_len);
    Ok((result, vector_blocks, scalar_tail, "wide-shuffle"))
}

fn native_merge_luma_band(band: &Image) -> Result<Option<Vec<u8>>, PilError> {
    let materialized = band.materialize_for_ops()?;
    Ok(match materialized {
        DynamicImage::ImageLuma8(image) => Some(image.into_raw()),
        _ => None,
    })
}

pub(super) fn native_merge_rgb_luma_owners(
    img: &DynamicImage,
    target_mode: &ColorMode,
    bands: &[Image],
    logical_mode: &str,
    pixels: usize,
) -> Result<Option<Vec<Arc<DynamicImage>>>, PilError> {
    // The pipeline op keeps the complete channel list, including the first
    // source already supplied as `img`; only later bands need shared owners.
    if !matches!(target_mode, ColorMode::RGB) || logical_mode != "RGB" || bands.len() != 3 {
        return Ok(None);
    }
    let DynamicImage::ImageLuma8(first) = img else {
        return Ok(None);
    };
    if first.as_raw().len() != pixels {
        return Ok(None);
    }

    let mut owners = Vec::with_capacity(2);
    for band in bands.iter().skip(1) {
        let owner = band.materialized_shared_for_ops()?;
        let DynamicImage::ImageLuma8(image) = owner.as_ref() else {
            return Ok(None);
        };
        if image.as_raw().len() != pixels {
            return Ok(None);
        }
        owners.push(owner);
    }
    Ok(Some(owners))
}

/// Merge validated L bands into the target's native byte layout.
///
/// The source bands are necessarily separate buffers, so one output buffer is
/// required for the interleaved result. Apart from that required result
/// allocation, the operation does not widen samples or materialize through a
/// color conversion. Complete channel groups use vector shuffles; only the
/// final incomplete group is scalar.
pub(super) fn native_merge_bytes(
    img: &DynamicImage,
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
    logical_mode: &str,
) -> Result<Option<(DynamicImage, u64, u64, &'static str)>, PilError> {
    let Some((channels, pixels)) =
        native_merge_contract_for_image(img, target_mode, bands, mode, logical_mode)
    else {
        return Ok(None);
    };
    let width = img.width();
    let height = img.height();

    // RGB's three native L inputs are already shared immutable buffers. Keep
    // their owners alive and interleave borrowed slices rather than cloning
    // every complete band before the vector kernel reads it.
    if channels == 3 {
        if let Some(owners) =
            native_merge_rgb_luma_owners(img, target_mode, bands, logical_mode, pixels)?
        {
            if let DynamicImage::ImageLuma8(first) = img {
                let mut band_bytes = Vec::with_capacity(3);
                band_bytes.push(first.as_raw().as_slice());
                for owner in &owners {
                    let DynamicImage::ImageLuma8(image) = owner.as_ref() else {
                        return Err(PilError::InternalError(
                            "native RGB merge band changed storage while borrowed".into(),
                        ));
                    };
                    band_bytes.push(image.as_raw().as_slice());
                }
                let (result, vector_blocks, scalar_tail, path) =
                    native_merge_interleave(width, height, pixels, channels, &band_bytes)?;
                return Ok(Some((result, vector_blocks, scalar_tail, path)));
            }
        }
    }

    let mut band_bytes = Vec::with_capacity(channels);
    band_bytes.push(img.as_bytes().to_vec());
    for band in bands.iter().skip(1) {
        let Some(bytes) = native_merge_luma_band(band)? else {
            return Ok(None);
        };
        if bytes.len() != pixels {
            return Ok(None);
        }
        band_bytes.push(bytes);
    }
    let band_byte_slices: Vec<&[u8]> = band_bytes.iter().map(Vec::as_slice).collect();
    let (result, vector_blocks, scalar_tail, path) =
        native_merge_interleave(width, height, pixels, channels, &band_byte_slices)?;
    Ok(Some((result, vector_blocks, scalar_tail, path)))
}

pub(crate) fn simd_merge(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Merge {
        mode: target_mode,
        bands,
        logical_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected Merge op".into()));
    };
    let Some((result, vector_blocks, scalar_tail, path)) =
        native_merge_bytes(img, target_mode, bands, mode, logical_mode)?
    else {
        return Err(simd_unsupported("Merge"));
    };
    crate::compute::record_pipeline_operation_path(path);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(result)
}

#[cfg(test)]
mod tests;
