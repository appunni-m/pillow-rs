//! Native interleaved alpha-mode conversions for Pillow's LA/RGBA layouts.

use super::super::native_arithmetic::{simd_div255, simd_pack_u16x16};
use crate::error::PilError;
use crate::pipeline::ColorMode;
use crate::raster::DynamicImage;
use wide::{u8x16, u16x16, u32x16};

#[derive(Clone, Copy)]
enum AlphaTransform {
    Premultiply,
    Unpremultiply,
    OpaquePad,
}

fn transform_for(
    target: &ColorMode,
    source: Option<&str>,
) -> Option<(usize, usize, AlphaTransform)> {
    match (source, target) {
        (Some("LA"), ColorMode::LaPremultiplied) => Some((2, 2, AlphaTransform::Premultiply)),
        (Some("La"), ColorMode::LA) => Some((2, 2, AlphaTransform::Unpremultiply)),
        (Some("RGBA"), ColorMode::RgbPremultiplied) => Some((4, 4, AlphaTransform::Premultiply)),
        (Some("RGBa"), ColorMode::RGBA) => Some((4, 4, AlphaTransform::Unpremultiply)),
        (Some("RGBa"), ColorMode::RGB) => Some((4, 3, AlphaTransform::Unpremultiply)),
        (Some("RGBa"), ColorMode::LaPremultiplied) => Some((4, 2, AlphaTransform::Unpremultiply)),
        (Some("RGBa"), ColorMode::Rgbx) => Some((4, 4, AlphaTransform::Unpremultiply)),
        (Some("RGBA"), ColorMode::Rgbx) => Some((4, 4, AlphaTransform::OpaquePad)),
        _ => None,
    }
}

pub(super) fn supports_image(
    image: &DynamicImage,
    target: &ColorMode,
    mode: Option<&str>,
    matrix: Option<&[f64]>,
) -> bool {
    if matrix.is_some() {
        return false;
    }
    if matches!((mode, target), (Some("LA"), ColorMode::Rgbx)) {
        return matches!(image, DynamicImage::ImageLumaA8(_));
    }
    let Some((channels, _, _)) = transform_for(target, mode) else {
        return false;
    };
    let layout_matches = match channels {
        2 => matches!(image, DynamicImage::ImageLumaA8(_)),
        4 => matches!(image, DynamicImage::ImageRgba8(_)),
        _ => false,
    };
    layout_matches
        && (image.width() as usize)
            .checked_mul(image.height() as usize)
            .and_then(|pixels| pixels.checked_mul(channels))
            == Some(image.as_bytes().len())
}

pub(super) fn supports_shape(
    layout: super::super::SimdLayout,
    width: u32,
    height: u32,
    target: &ColorMode,
    mode: Option<&str>,
    matrix: Option<&[f64]>,
) -> bool {
    if matrix.is_some() {
        return false;
    }
    let Some((channels, _, _)) = transform_for(target, mode) else {
        return matches!(
            (mode, target, layout),
            (
                Some("LA"),
                ColorMode::Rgbx,
                super::super::SimdLayout::LumaA8
            )
        );
    };
    let layout_matches = match channels {
        2 => layout == super::super::SimdLayout::LumaA8,
        4 => layout == super::super::SimdLayout::Rgba8,
        _ => false,
    };
    layout_matches && (width as usize).checked_mul(height as usize).is_some()
}

pub(super) fn try_convert(
    image: &DynamicImage,
    target: &ColorMode,
    mode: Option<&str>,
    matrix: Option<&[f64]>,
) -> Result<Option<DynamicImage>, PilError> {
    if !supports_image(image, target, mode, matrix) {
        return Ok(None);
    }
    if matches!((mode, target), (Some("LA"), ColorMode::Rgbx)) {
        return expand_la_to_rgbx(image).map(Some);
    }
    let Some((source_channels, target_channels, transform)) = transform_for(target, mode) else {
        return Err(super::simd_unsupported("Convert"));
    };
    let source = image.as_bytes();
    let pixels = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .ok_or_else(|| crate::error::PilError::ValueError("alpha image is too large".into()))?;
    let mut output = Vec::with_capacity(pixels.saturating_mul(target_channels));
    let mut vector_blocks = 0u64;
    for start in (0..source.len()).step_by(16) {
        let active_bytes = (source.len() - start).min(16);
        let active_pixels = active_bytes / source_channels;
        let source_lanes =
            std::array::from_fn(|lane| source.get(start + lane).copied().unwrap_or(0));
        let input = u8x16::new(source_lanes);
        let converted = match transform {
            AlphaTransform::Premultiply => premultiply_block(input, source_channels),
            AlphaTransform::Unpremultiply => unpremultiply_block(input, source_channels),
            AlphaTransform::OpaquePad => opaque_alpha(input, source_channels),
        };
        let lanes = converted.to_array();
        match (target, mode) {
            (ColorMode::LaPremultiplied, Some("RGBa")) => {
                for pixel in lanes.chunks_exact(4).take(active_pixels) {
                    let gray = crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]);
                    output.extend_from_slice(&[gray, 255]);
                }
            }
            (ColorMode::LaPremultiplied, Some("LA")) | (ColorMode::LA, Some("La")) => {
                output.extend_from_slice(&lanes[..active_bytes]);
            }
            (ColorMode::RGB, Some("RGBa")) => {
                for pixel in lanes.chunks_exact(4).take(active_pixels) {
                    output.extend_from_slice(&pixel[..3]);
                }
            }
            (ColorMode::Rgbx, Some("RGBa" | "RGBA")) => {
                for pixel in lanes.chunks_exact(4).take(active_pixels) {
                    output.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
                }
            }
            _ => output.extend_from_slice(&lanes[..active_bytes]),
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if vector_blocks == 0 {
        return Err(super::simd_unsupported("Convert"));
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(image.width(), image.height(), output, target_channels)
        .map(Some)
}

fn alpha_indices(channels: usize) -> [u8; 16] {
    std::array::from_fn(|lane| (lane / channels * channels + channels - 1) as u8)
}

fn alpha_lane_mask(channels: usize) -> u8x16 {
    u8x16::new(std::array::from_fn(|lane| {
        if lane % channels == channels - 1 {
            255
        } else {
            0
        }
    }))
}

fn premultiply_block(input: u8x16, channels: usize) -> u8x16 {
    let alpha = input.swizzle_relaxed(u8x16::new(alpha_indices(channels)));
    let values = u16x16::from(input);
    let alpha = u16x16::from(alpha);
    let scaled = simd_div255(values * alpha + u16x16::splat(127));
    let converted = simd_pack_u16x16(scaled);
    alpha_lane_mask(channels)
        .simd_eq(u8x16::ZERO)
        .select(converted, input)
}

fn unpremultiply_block(input: u8x16, channels: usize) -> u8x16 {
    let alpha = input.swizzle_relaxed(u8x16::new(alpha_indices(channels)));
    let alpha_values = alpha.to_array();
    let reciprocals = u32x16::new(std::array::from_fn(|lane| {
        let value = u32::from(alpha_values[lane]);
        65_536_u32.checked_div(value).unwrap_or_default()
    }));
    let alpha = u32x16::from(u16x16::from(alpha));
    let numerator = u32x16::from(u16x16::from(input)) * u32x16::splat(255);
    let estimate = (numerator * reciprocals) >> 16u32;
    let next = (estimate + u32x16::splat(1)) * alpha;
    let round_up = next.simd_le(numerator) & alpha.simd_gt(u32x16::ZERO);
    let exact = estimate + round_up.select(u32x16::splat(1), u32x16::ZERO);
    let bounded = exact
        .simd_gt(u32x16::splat(255))
        .select(u32x16::splat(255), exact);
    let values = u16x16::new(bounded.to_array().map(|value| value as u16));
    let converted = simd_pack_u16x16(values);
    let preserve = alpha_lane_mask(channels).simd_ne(u8x16::ZERO);
    let zero_alpha = input
        .swizzle_relaxed(u8x16::new(alpha_indices(channels)))
        .simd_eq(u8x16::ZERO);
    preserve.select(input, zero_alpha.select(input, converted))
}

fn opaque_alpha(input: u8x16, channels: usize) -> u8x16 {
    alpha_lane_mask(channels)
        .simd_ne(u8x16::ZERO)
        .select(u8x16::splat(255), input)
}

fn expand_la_to_rgbx(image: &DynamicImage) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLumaA8(source) = image else {
        return Err(super::simd_unsupported("Convert"));
    };
    let input = source.as_raw();
    let mut output = Vec::with_capacity(input.len().saturating_mul(2));
    let mut vector_blocks = 0u64;
    const FIRST: [u8; 16] = [0, 0, 0, 0, 2, 2, 2, 0, 4, 4, 4, 0, 6, 6, 6, 0];
    const SECOND: [u8; 16] = [8, 8, 8, 0, 10, 10, 10, 0, 12, 12, 12, 0, 14, 14, 14, 0];
    let pad_mask = u8x16::new([0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
    for start in (0..input.len()).step_by(16) {
        let active_bytes = (input.len() - start).min(16);
        let active_pixels = active_bytes / 2;
        let lanes = std::array::from_fn(|lane| input.get(start + lane).copied().unwrap_or(0));
        let source = u8x16::new(lanes);
        let first = pad_mask
            .simd_ne(u8x16::ZERO)
            .select(u8x16::splat(255), source.swizzle_relaxed(u8x16::new(FIRST)));
        let second = pad_mask.simd_ne(u8x16::ZERO).select(
            u8x16::splat(255),
            source.swizzle_relaxed(u8x16::new(SECOND)),
        );
        let first = first.to_array();
        let second = second.to_array();
        let first_len = active_pixels.min(4) * 4;
        output.extend_from_slice(&first[..first_len]);
        let second_pixels = active_pixels.saturating_sub(4).min(4);
        output.extend_from_slice(&second[..second_pixels * 4]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(image.width(), image.height(), output, 4)
}
