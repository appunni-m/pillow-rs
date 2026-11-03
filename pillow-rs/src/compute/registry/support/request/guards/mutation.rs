use super::super::{GuardAssessment, PixelWriteRunFacts, RequestFacts, StrategyGuard, StrategyInputs};
use super::{checked_native_bytes_guard, native_bytes_guard};
use crate::compute::registry::support::PUT_ALPHA_DATA_MASK_MODES;
use crate::pipeline::TransposeMethod;
use crate::raster::Mode;

pub(super) fn alpha_composite_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if channels != 2 && channels != 4 {
        return GuardAssessment::Ineligible;
    }
    let (destination_offset, source_offset) = match input.request {
        RequestFacts::AlphaComposite {
            destination_offset,
            source_offset,
            ..
        } => (destination_offset, source_offset),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if destination_offset != (0, 0) || source_offset != (0, 0) {
        return GuardAssessment::Ineligible;
    }
    if !input
        .width
        .checked_mul(input.height)
        .is_some_and(|pixels| pixels == 0 || pixels >= 8)
    {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, channels)
}

pub(super) fn paste_l_mask_neon_guard(
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
        RequestFacts::Paste { source, mask, position, region, mask_alpha } => {
            (source, mask, position, region, mask_alpha)
        }
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
        region.0.saturating_sub(source_left).min(destination.0.saturating_sub(destination_left)),
        region.1.saturating_sub(source_top).min(destination.1.saturating_sub(destination_top)),
    )
}

pub(super) fn merge_rgb_interleave_guard(input: StrategyInputs) -> GuardAssessment {
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
    if !matches!(first_mode, Mode::ImageLuma8 | Mode::ImagePalette8) {
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

pub(super) fn merge_native_interleave_guard(
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
        || !matches!(first_mode, Mode::ImageLuma8 | Mode::ImagePalette8)
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

pub(super) fn blend_native_vector_guard(
    input: StrategyInputs,
    channels: u8,
    compatible_modes: &'static [Mode],
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (other, alpha) = match input.request {
        RequestFacts::BlendInputs { other, alpha_bits } => {
            (other, f64::from_bits(alpha_bits))
        }
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

pub(super) fn composite_native_byte_vector_guard(
    input: StrategyInputs,
    mode: Mode,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (source_mode, destination, mask) = match input.request {
        RequestFacts::CompositeInputs { source_mode, destination, mask, .. } => {
            (source_mode, destination, mask)
        }
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
            if source == source_bytes
                && destination == destination_bytes
                && mask == mask_bytes =>
        {
            GuardAssessment::Satisfied
        }
        (None, _, _) | (_, None, _) | (_, _, None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn composite_l_mask_neon_guard(
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

pub(super) fn put_alpha_bytes_guard(
    input: StrategyInputs,
    channels: u8,
    minimum_pixels: usize,
) -> GuardAssessment {
    let pixels = (input.width as usize).checked_mul(input.height as usize);
    if !pixels.is_some_and(|pixels| pixels >= minimum_pixels) {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, channels)
}

pub(super) fn extract_band_bytes_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    let layout = checked_native_bytes_guard(input, channels);
    if matches!(layout, GuardAssessment::Ineligible) {
        return GuardAssessment::Ineligible;
    }
    match input.request {
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        RequestFacts::ExtractBand { index } if index < channels => layout,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn put_data_native_guard(
    input: StrategyInputs,
    channels: u8,
    payloads: &'static [Mode],
) -> GuardAssessment {
    let layout = StrategyGuard::NativeBytes { channels }.assess(input);
    if matches!(layout, GuardAssessment::Ineligible) {
        return layout;
    }
    match input.request {
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        RequestFacts::PutData { payload_mode, .. } if payloads.contains(&payload_mode) => layout,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn effect_spread_bytes_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    match input.request {
        // The archived SIMD adapter admitted distance one. Its selected
        // architecture advances the same process RNG and returns the native
        // identity image without executing pixel-vector blocks.
        RequestFacts::EffectSpread { .. } => (),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    }
    native_bytes_guard(input, channels)
}

pub(super) fn effect_noise_vector_guard(input: StrategyInputs) -> GuardAssessment {
    let sigma = match input.request {
        RequestFacts::EffectNoise { sigma_bits } => f64::from_bits(sigma_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if !sigma.is_finite() || input.width == 0 || input.height == 0 {
        return GuardAssessment::Ineligible;
    }
    if (input.width as usize).checked_mul(input.height as usize).is_some() {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

pub(super) fn effect_mandelbrot_vector_guard(input: StrategyInputs) -> GuardAssessment {
    match input.request {
        RequestFacts::EffectMandelbrot { .. } => (),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    }
    if input.width < 8 || input.height == 0 {
        GuardAssessment::Ineligible
    } else {
        GuardAssessment::Satisfied
    }
}

pub(super) fn expand_vector_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let border = match input.request {
        RequestFacts::Expand { border } => border,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let Some(border) = border.uniform_nonnegative() else {
        return GuardAssessment::Ineligible;
    };
    let Some(expected) = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)))
    else {
        return GuardAssessment::Ineligible;
    };
    match input.byte_len {
        None => return GuardAssessment::Deferred,
        Some(actual) if actual == expected => (),
        Some(_) => return GuardAssessment::Ineligible,
    }
    if channels <= 2 || border == 0 {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

pub(super) fn alpha_mask_guard(input: StrategyInputs, channels: u8, minimum_pixels: usize) -> GuardAssessment {
    let source = StrategyGuard::PutAlphaBytes { channels, minimum_pixels }.assess(input);
    if matches!(source, GuardAssessment::Ineligible) {
        return source;
    }
    let mask = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::PutAlphaData { mask } => mask,
        _ => return GuardAssessment::Ineligible,
    };
    if !PUT_ALPHA_DATA_MASK_MODES.contains(&mask.mode)
        || mask.width != input.width
        || mask.height != input.height
    {
        return GuardAssessment::Ineligible;
    }
    let pixels = (input.width as usize).checked_mul(input.height as usize);
    match (pixels, mask.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(pixels), Some(length)) if pixels == length => source,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

pub(super) fn rgba_transpose_avx2_guard(input: StrategyInputs) -> GuardAssessment {
    let layout = native_transpose_storage_guard(input, 4, true);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    if input.width < 8 || input.height < 8 {
        return GuardAssessment::Ineligible;
    }
    match input.request {
        RequestFacts::Transpose {
            method:
                TransposeMethod::Rotate90
                | TransposeMethod::Rotate270
                | TransposeMethod::Transpose
                | TransposeMethod::Transverse,
        } => layout,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn native_transpose_bytes_guard(
    input: StrategyInputs,
    channels: u8,
    allow_empty: bool,
) -> GuardAssessment {
    let layout = native_transpose_storage_guard(input, channels, allow_empty);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    match input.request {
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        RequestFacts::Transpose { .. } => layout,
        _ => GuardAssessment::Ineligible,
    }
}

/// Transpose uses the logical raster prefix and ignores trailing backing
/// samples. Empty rasters are admitted only when their storage is empty, as in
/// the archived adapter.
fn native_transpose_storage_guard(
    input: StrategyInputs,
    channels: u8,
    allow_empty: bool,
) -> GuardAssessment {
    let expected = crate::checked_dims::CheckedDims::new_allow_empty(
        input.width,
        input.height,
        channels,
    )
    .ok()
    .map(|dimensions| dimensions.total_bytes());
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(0), _) if !allow_empty => GuardAssessment::Ineligible,
        (Some(0), None) => GuardAssessment::Deferred,
        (Some(0), Some(0)) => GuardAssessment::Satisfied,
        (Some(0), Some(_)) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual >= expected => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

pub(super) fn color3dlut_guard(input: StrategyInputs, source_channels: u8) -> GuardAssessment {
    let (target_mode, channels, size, table_len) = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::Color3DLut { target_mode, channels, size, table_len } => {
            (target_mode, channels, size, table_len)
        }
        _ => return GuardAssessment::Ineligible,
    };
    let target_channels = match target_mode {
        Mode::ImageRgb8 => 3,
        Mode::ImageRgba8 | Mode::ImageCmyk8 => 4,
        _ => return GuardAssessment::Ineligible,
    };
    if !(2..=65).contains(&size.0)
        || !(2..=65).contains(&size.1)
        || !(2..=65).contains(&size.2)
        || !matches!(channels, 3 | 4)
        || target_channels < channels as usize
    {
        return GuardAssessment::Ineligible;
    }
    let expected_table = (size.0 as usize)
        .checked_mul(size.1 as usize)
        .and_then(|length| length.checked_mul(size.2 as usize))
        .and_then(|length| length.checked_mul(channels as usize));
    if expected_table != Some(table_len) {
        return GuardAssessment::Ineligible;
    }
    let pixels = (input.width as usize).checked_mul(input.height as usize);
    let expected_bytes = pixels.and_then(|count| count.checked_mul(usize::from(source_channels)));
    match (pixels, expected_bytes, input.byte_len) {
        (Some(pixels), Some(expected), Some(actual))
            if pixels >= 8 && expected == actual => GuardAssessment::Satisfied,
        (Some(pixels), _, _) if pixels < 8 => GuardAssessment::Ineligible,
        (Some(_), Some(_), None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn reduce_native_byte_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    let (x_factor, y_factor) = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::Reduce { x_factor, y_factor } => (x_factor, y_factor),
        _ => return GuardAssessment::Ineligible,
    };
    if x_factor == 0 || y_factor == 0 || (x_factor == 1 && y_factor == 1) {
        return GuardAssessment::Ineligible;
    }
    let Some(area) = x_factor.checked_mul(y_factor) else {
        return GuardAssessment::Ineligible;
    };
    if area > u32::MAX / 256 {
        return GuardAssessment::Ineligible;
    }
    let output_width = input.width.div_ceil(x_factor);
    let output_height = input.height.div_ceil(y_factor);
    let Some(output_bytes) = (output_width as usize)
        .checked_mul(output_height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)))
    else {
        return GuardAssessment::Ineligible;
    };
    if output_bytes == 0 {
        return GuardAssessment::Ineligible;
    }
    let Some(expected_bytes) = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)))
    else {
        return GuardAssessment::Ineligible;
    };
    match input.byte_len {
        None => GuardAssessment::Deferred,
        Some(actual) if actual == expected_bytes => GuardAssessment::Satisfied,
        Some(_) => GuardAssessment::Ineligible,
    }
}

pub(super) fn native_byte_prefix_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)));
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual >= expected => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

pub(super) fn native_mirror_rows_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    if input.width == 0 || input.height == 0 || !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)));
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual >= expected => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

pub(super) fn native_pointwise_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    exact_native_rows_guard(input, channels)
}

pub(super) fn native_pointwise_bytes_including_empty_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)));
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual == expected => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

pub(super) fn exact_native_rows_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    if input.width == 0 || input.height == 0 || !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)));
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual == expected => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

pub(super) fn native_crop_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    if channels == 0 || input.width == 0 || input.height == 0 {
        return GuardAssessment::Ineligible;
    }
    let (left, top, width, height, source_width, source_height) = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::Crop {
            left,
            top,
            width,
            height,
            source_width,
            source_height,
        } => (left, top, width, height, source_width, source_height),
        _ => return GuardAssessment::Ineligible,
    };
    if width == 0
        || height == 0
        || source_width != input.width
        || source_height != input.height
    {
        return GuardAssessment::Ineligible;
    }
    let (Some(right), Some(bottom)) = (left.checked_add(width), top.checked_add(height)) else {
        return GuardAssessment::Ineligible;
    };
    if right > source_width || bottom > source_height {
        return GuardAssessment::Ineligible;
    }
    let Some(expected) = (source_width as usize)
        .checked_mul(source_height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)))
    else {
        return GuardAssessment::Ineligible;
    };
    match input.byte_len {
        None => GuardAssessment::Deferred,
        Some(actual) if actual == expected => GuardAssessment::Satisfied,
        Some(_) => GuardAssessment::Ineligible,
    }
}

pub(super) fn native_crop_border_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    if channels == 0 {
        return GuardAssessment::Ineligible;
    }
    let border = match input.request {
        RequestFacts::CropBorder { border } => border,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if border > input.width / 2 || border > input.height / 2 {
        return GuardAssessment::Ineligible;
    }
    let Some(double_border) = border.checked_mul(2) else {
        return GuardAssessment::Ineligible;
    };
    if input.width - double_border == 0 || input.height - double_border == 0 {
        return GuardAssessment::Ineligible;
    }
    exact_native_rows_guard(input, channels)
}

pub(super) fn pixel_span_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let (x, y) = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::PutPixel { x, y } => (x, y),
        RequestFacts::Draw { .. }
        | RequestFacts::Quantize { .. }
        | RequestFacts::Transform { .. }
        | RequestFacts::SameModeSecondaryImage { .. }
        | RequestFacts::AlphaComposite { .. }
        | RequestFacts::ExtractBand { .. }
        | RequestFacts::PutPixelBatch { .. }
        | RequestFacts::PutData { .. }
        | RequestFacts::PutAlphaData { .. }
        | RequestFacts::ImageOpsMask { .. }
        | RequestFacts::Color3DLut { .. }
        | RequestFacts::Transpose { .. }
        | RequestFacts::Brightness { .. }
        | RequestFacts::Contrast { .. }
        | RequestFacts::Sharpness { .. }
        | RequestFacts::ColorSaturation { .. }
        | RequestFacts::Rotate { .. }
        | RequestFacts::Posterize { .. }
        | RequestFacts::EffectSpread { .. }
        | RequestFacts::EffectNoise { .. }
        | RequestFacts::EffectMandelbrot { .. }
        | RequestFacts::EvalLut { .. }
        | RequestFacts::PointFloatLut { .. }
        | RequestFacts::FilterWindow { .. }
        | RequestFacts::Filter3x3 { .. }
        | RequestFacts::Filter5x5 { .. }
        | RequestFacts::GaussianBlur { .. }
        | RequestFacts::BoxBlur { .. }
        | RequestFacts::BoxBlurXY { .. }
        | RequestFacts::UnsharpMask { .. }
        | RequestFacts::Resize { .. }
        | RequestFacts::Reduce { .. }
        | RequestFacts::Thumbnail { .. }
        | RequestFacts::Contain { .. }
        | RequestFacts::Cover { .. }
        | RequestFacts::Fit { .. }
        | RequestFacts::Pad { .. }
        | RequestFacts::Scale { .. }
        | RequestFacts::ConvertTarget { .. }
        | RequestFacts::MergeBands { .. }
        | RequestFacts::BlendInputs { .. }
        | RequestFacts::CompositeInputs { .. }
        | RequestFacts::ChopsBinary { .. }
        | RequestFacts::CropBorder { .. }
        | RequestFacts::Crop { .. }
        | RequestFacts::Expand { .. } =>
            return GuardAssessment::Ineligible,
    };
    if x >= input.width || y >= input.height {
        return GuardAssessment::Ineligible;
    }
    let expected = (input.width as usize).checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)));
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual >= expected => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

pub(super) fn pixel_write_run_guard(input: StrategyInputs) -> GuardAssessment {
    let writes = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::PutPixelBatch { count, writes } => {
            if count < 2 || writes == PixelWriteRunFacts::Ineligible {
                return GuardAssessment::Ineligible;
            }
            writes
        },
        RequestFacts::Draw { .. }
        | RequestFacts::Quantize { .. }
        | RequestFacts::Transform { .. }
        | RequestFacts::SameModeSecondaryImage { .. }
        | RequestFacts::AlphaComposite { .. }
        | RequestFacts::ExtractBand { .. }
        | RequestFacts::PutPixel { .. }
        | RequestFacts::PutData { .. }
        | RequestFacts::PutAlphaData { .. }
        | RequestFacts::ImageOpsMask { .. }
        | RequestFacts::Color3DLut { .. }
        | RequestFacts::Transpose { .. }
        | RequestFacts::Brightness { .. }
        | RequestFacts::Contrast { .. }
        | RequestFacts::Sharpness { .. }
        | RequestFacts::ColorSaturation { .. }
        | RequestFacts::Rotate { .. }
        | RequestFacts::Posterize { .. }
        | RequestFacts::EffectSpread { .. }
        | RequestFacts::EffectNoise { .. }
        | RequestFacts::EffectMandelbrot { .. }
        | RequestFacts::EvalLut { .. }
        | RequestFacts::PointFloatLut { .. }
        | RequestFacts::FilterWindow { .. }
        | RequestFacts::Filter3x3 { .. }
        | RequestFacts::Filter5x5 { .. }
        | RequestFacts::GaussianBlur { .. }
        | RequestFacts::BoxBlur { .. }
        | RequestFacts::BoxBlurXY { .. }
        | RequestFacts::UnsharpMask { .. }
        | RequestFacts::Resize { .. }
        | RequestFacts::Reduce { .. }
        | RequestFacts::Thumbnail { .. }
        | RequestFacts::Contain { .. }
        | RequestFacts::Cover { .. }
        | RequestFacts::Fit { .. }
        | RequestFacts::Pad { .. }
        | RequestFacts::Scale { .. }
        | RequestFacts::ConvertTarget { .. }
        | RequestFacts::MergeBands { .. }
        | RequestFacts::BlendInputs { .. }
        | RequestFacts::CompositeInputs { .. }
        | RequestFacts::ChopsBinary { .. }
        | RequestFacts::CropBorder { .. }
        | RequestFacts::Crop { .. }
        | RequestFacts::Expand { .. } =>
            return GuardAssessment::Ineligible,
    };
    let expected = (input.width as usize).checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(4));
    match (expected, input.byte_len) {
        (None, _) | (Some(0..=15), _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual != expected => GuardAssessment::Ineligible,
        (Some(_), Some(_)) if writes == PixelWriteRunFacts::Unknown => GuardAssessment::Deferred,
        (Some(_), Some(_)) => GuardAssessment::Satisfied,
    }
}

