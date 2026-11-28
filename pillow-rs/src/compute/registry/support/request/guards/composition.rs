//! Materialized input guards for image composition operations.

use super::super::{GuardAssessment, RequestFacts, StrategyInputs};
use super::checked_native_bytes_guard;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn alpha_composite_avx512f_guard(
    input: StrategyInputs,
    channels: u8,
    minimum_pixels: u32,
) -> GuardAssessment {
    match alpha_composite_vector_guard(input, channels) {
        GuardAssessment::Satisfied
            if input.width.saturating_mul(input.height) >= minimum_pixels =>
        {
            GuardAssessment::Satisfied
        }
        GuardAssessment::Satisfied => GuardAssessment::Ineligible,
        other => other,
    }
}

pub(crate) fn alpha_composite_vector_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    if channels != 2 && channels != 4 {
        return GuardAssessment::Ineligible;
    }
    let (source, destination_offset, source_offset) = match input.request {
        RequestFacts::AlphaComposite {
            source,
            destination_offset,
            source_offset,
        } => (source, destination_offset, source_offset),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if destination_offset != (0, 0) || source_offset != (0, 0) {
        return GuardAssessment::Ineligible;
    }
    let Some(pixels) = input.width.checked_mul(input.height) else {
        return GuardAssessment::Ineligible;
    };
    if pixels < 8 || (source.width, source.height) != (input.width, input.height) {
        return GuardAssessment::Ineligible;
    }
    let destination_layout = checked_native_bytes_guard(input, channels);
    if !matches!(destination_layout, GuardAssessment::Satisfied) {
        return destination_layout;
    }
    let source_layout =
        crate::checked_dims::CheckedDims::new(source.width, source.height, channels);
    let Ok(source_layout) = source_layout else {
        return GuardAssessment::Ineligible;
    };
    match source.byte_len {
        None => GuardAssessment::Deferred,
        Some(actual) if actual == source_layout.total_bytes() => GuardAssessment::Satisfied,
        Some(_) => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn paste_l_mask_neon_guard(
    input: StrategyInputs,
    mode: Mode,
    channels: u8,
    minimum_clipped_width: u32,
) -> GuardAssessment {
    let exact_channels = match mode {
        Mode::ImageLuma8 => 1,
        Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 => 2,
        Mode::ImageRgb8 | Mode::ImageHsv8 => 3,
        Mode::ImageRgba8 | Mode::ImageCmyk8 => 4,
        _ => return GuardAssessment::Ineligible,
    };
    if exact_channels != channels {
        return GuardAssessment::Ineligible;
    }
    let (source, mask, position, region, mask_alpha) = match input.request {
        RequestFacts::Paste {
            source,
            mask,
            position,
            region,
            mask_alpha,
        } => (source, mask, position, region, mask_alpha),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let Some(mask) = mask else {
        return GuardAssessment::Ineligible;
    };
    if source.mode != mode
        || (source.width, source.height) != region
        || mask.mode != Mode::ImageLuma8
        || (mask.width, mask.height) != region
        || mask_alpha
    {
        return GuardAssessment::Ineligible;
    }
    let (clipped_width, clipped_height) =
        clipped_paste_size(position, region, (input.width, input.height));
    if clipped_width < minimum_clipped_width || clipped_height == 0 {
        return GuardAssessment::Ineligible;
    }
    let destination_layout = checked_native_bytes_guard(input, channels);
    if !matches!(destination_layout, GuardAssessment::Satisfied) {
        return destination_layout;
    }
    let Some(region_pixels) = (region.0 as usize).checked_mul(region.1 as usize) else {
        return GuardAssessment::Ineligible;
    };
    let Some(source_bytes) = region_pixels.checked_mul(usize::from(channels)) else {
        return GuardAssessment::Ineligible;
    };
    match (source.byte_len, mask.byte_len) {
        (Some(source_len), Some(mask_len))
            if source_len == source_bytes && mask_len == region_pixels =>
        {
            GuardAssessment::Satisfied
        }
        (None, _) | (_, None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn clipped_paste_size(
    position: (i64, i64),
    region: (u32, u32),
    destination: (u32, u32),
) -> (u32, u32) {
    let source_left = position.0.saturating_neg().max(0).min(i64::from(region.0)) as u32;
    let source_top = position.1.saturating_neg().max(0).min(i64::from(region.1)) as u32;
    let destination_left = position.0.max(0).min(i64::from(destination.0)) as u32;
    let destination_top = position.1.max(0).min(i64::from(destination.1)) as u32;
    (
        region
            .0
            .saturating_sub(source_left)
            .min(destination.0.saturating_sub(destination_left)),
        region
            .1
            .saturating_sub(source_top)
            .min(destination.1.saturating_sub(destination_top)),
    )
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn merge_rgb_interleave_guard(input: StrategyInputs) -> GuardAssessment {
    let RequestFacts::MergeBands {
        target_mode: Mode::ImageRgb8,
        band_count: 3,
        first_mode,
        remaining_modes_are_luma: true,
        dimensions_match: true,
    } = input.request
    else {
        return if input.request == RequestFacts::Unspecified {
            GuardAssessment::Deferred
        } else {
            GuardAssessment::Ineligible
        };
    };
    if !matches!(
        first_mode,
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8
    ) {
        return GuardAssessment::Ineligible;
    }
    let Some(pixels) = (input.width as usize).checked_mul(input.height as usize) else {
        return GuardAssessment::Ineligible;
    };
    if pixels < 16 {
        return GuardAssessment::Ineligible;
    }
    match input.byte_len {
        None => GuardAssessment::Deferred,
        Some(length) if length == pixels => GuardAssessment::Satisfied,
        Some(_) => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn merge_native_interleave_guard(
    input: StrategyInputs,
    target_mode: Mode,
    channels: u8,
) -> GuardAssessment {
    if !(2..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let RequestFacts::MergeBands {
        target_mode: requested_mode,
        band_count,
        first_mode,
        remaining_modes_are_luma,
        dimensions_match,
    } = input.request
    else {
        return if input.request == RequestFacts::Unspecified {
            GuardAssessment::Deferred
        } else {
            GuardAssessment::Ineligible
        };
    };
    if requested_mode != target_mode
        || band_count != usize::from(channels)
        || !matches!(
            first_mode,
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8
        )
        || !remaining_modes_are_luma
        || !dimensions_match
    {
        return GuardAssessment::Ineligible;
    }
    let Some(pixels) = (input.width as usize).checked_mul(input.height as usize) else {
        return GuardAssessment::Ineligible;
    };
    match input.byte_len {
        None => GuardAssessment::Deferred,
        Some(length) if length == pixels => GuardAssessment::Satisfied,
        Some(_) => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn merge_avx512f_interleave_guard(
    input: StrategyInputs,
    target_mode: Mode,
    channels: u8,
    minimum_pixels: u32,
) -> GuardAssessment {
    if !matches!(channels, 2 | 4) {
        return GuardAssessment::Ineligible;
    }
    match merge_native_interleave_guard(input, target_mode, channels) {
        GuardAssessment::Satisfied
            if input.width.saturating_mul(input.height) >= minimum_pixels =>
        {
            GuardAssessment::Satisfied
        }
        GuardAssessment::Satisfied => GuardAssessment::Ineligible,
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn blend_native_vector_guard(
    input: StrategyInputs,
    channels: u8,
    compatible_modes: &'static [Mode],
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (other, alpha) = match input.request {
        RequestFacts::BlendInputs { other, alpha_bits } => (other, f64::from_bits(alpha_bits)),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if !compatible_modes.contains(&other.mode)
        || (other.width, other.height) != (input.width, input.height)
        || !alpha.is_finite()
        || !(alpha as f32).is_finite()
    {
        return GuardAssessment::Ineligible;
    }
    let Some(pixels) = (input.width as usize).checked_mul(input.height as usize) else {
        return GuardAssessment::Ineligible;
    };
    let Some(expected_bytes) = pixels.checked_mul(usize::from(channels)) else {
        return GuardAssessment::Ineligible;
    };
    if expected_bytes != 0 && expected_bytes < 8 {
        return GuardAssessment::Ineligible;
    }
    match (input.byte_len, other.byte_len) {
        (Some(source), Some(secondary))
            if source == expected_bytes && secondary == expected_bytes =>
        {
            GuardAssessment::Satisfied
        }
        (None, _) | (_, None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn blend_native_avx512f_guard(
    input: StrategyInputs,
    channels: u8,
    compatible_modes: &'static [Mode],
) -> GuardAssessment {
    match blend_native_vector_guard(input, channels, compatible_modes) {
        GuardAssessment::Satisfied if input.byte_len.is_some_and(|length| length >= 64) => {
            GuardAssessment::Satisfied
        }
        GuardAssessment::Satisfied => GuardAssessment::Ineligible,
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn composite_native_byte_vector_guard(
    input: StrategyInputs,
    mode: Mode,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (source_mode, destination, mask) = match input.request {
        RequestFacts::CompositeInputs {
            source_mode,
            destination,
            mask,
            ..
        } => (source_mode, destination, mask),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if source_mode != mode
        || destination.mode != mode
        || (mask.width, mask.height) != (input.width, input.height)
    {
        return GuardAssessment::Ineligible;
    }
    let mask_channels = match mask.mode {
        Mode::One | Mode::ImageLuma8 => 1,
        Mode::ImageLumaA8 => 2,
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 => 4,
        _ => return GuardAssessment::Ineligible,
    };
    let Some(source_pixels) = (input.width as usize).checked_mul(input.height as usize) else {
        return GuardAssessment::Ineligible;
    };
    let Some(destination_pixels) =
        (destination.width as usize).checked_mul(destination.height as usize)
    else {
        return GuardAssessment::Ineligible;
    };
    let Some(source_bytes) = source_pixels.checked_mul(usize::from(channels)) else {
        return GuardAssessment::Ineligible;
    };
    let Some(destination_bytes) = destination_pixels.checked_mul(usize::from(channels)) else {
        return GuardAssessment::Ineligible;
    };
    let Some(mask_bytes) = source_pixels.checked_mul(mask_channels) else {
        return GuardAssessment::Ineligible;
    };
    match (input.byte_len, destination.byte_len, mask.byte_len) {
        (Some(source), Some(destination), Some(mask))
            if source == source_bytes && destination == destination_bytes && mask == mask_bytes =>
        {
            GuardAssessment::Satisfied
        }
        (None, _, _) | (_, None, _) | (_, _, None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn composite_l_mask_neon_guard(
    input: StrategyInputs,
    mode: Mode,
    channels: u8,
    minimum_clipped_width: u32,
) -> GuardAssessment {
    let exact_channels = match mode {
        Mode::ImageLuma8 => 1,
        Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 => 2,
        Mode::ImageRgb8 | Mode::ImageHsv8 => 3,
        Mode::ImageRgba8 | Mode::ImageCmyk8 => 4,
        _ => return GuardAssessment::Ineligible,
    };
    if channels != exact_channels {
        return GuardAssessment::Ineligible;
    }
    let RequestFacts::CompositeInputs {
        source_mode,
        destination,
        mask,
        mask_alpha,
    } = input.request
    else {
        return if input.request == RequestFacts::Unspecified {
            GuardAssessment::Deferred
        } else {
            GuardAssessment::Ineligible
        };
    };
    if source_mode != mode
        || destination.mode != mode
        || mask.mode != Mode::ImageLuma8
        || mask_alpha
        || (mask.width, mask.height) != (input.width, input.height)
    {
        return GuardAssessment::Ineligible;
    }
    // Composite overlays image1 at (0, 0); image2 is the output canvas and can
    // have different dimensions, so the NEON row threshold uses the overlap.
    let clipped_width = input.width.min(destination.width);
    let clipped_height = input.height.min(destination.height);
    if clipped_width < minimum_clipped_width || clipped_height == 0 {
        return GuardAssessment::Ineligible;
    }
    let Some(source_pixels) = (input.width as usize).checked_mul(input.height as usize) else {
        return GuardAssessment::Ineligible;
    };
    let Some(destination_pixels) =
        (destination.width as usize).checked_mul(destination.height as usize)
    else {
        return GuardAssessment::Ineligible;
    };
    let Some(source_bytes) = source_pixels.checked_mul(usize::from(channels)) else {
        return GuardAssessment::Ineligible;
    };
    let Some(destination_bytes) = destination_pixels.checked_mul(usize::from(channels)) else {
        return GuardAssessment::Ineligible;
    };
    match (input.byte_len, destination.byte_len, mask.byte_len) {
        (Some(source), Some(destination), Some(mask))
            if source == source_bytes
                && destination == destination_bytes
                && mask == source_pixels =>
        {
            GuardAssessment::Satisfied
        }
        (None, _, _) | (_, None, _) | (_, _, None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}
