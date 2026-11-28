use super::super::{
    GuardAssessment, PixelWriteRunFacts, RequestFacts, StrategyGuard, StrategyInputs,
};
use super::{checked_native_bytes_guard, native_bytes_guard};
use crate::compute::registry::support::PUT_ALPHA_DATA_MASK_MODES;
use crate::pipeline::TransposeMethod;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn put_alpha_bytes_guard(
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn extract_band_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn put_data_native_guard(
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn effect_spread_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    match input.request {
        // Distance one is a native-identity strategy that advances the same
        // process RNG; other distances use the native vector data plane.
        RequestFacts::EffectSpread { .. } => (),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    }
    native_bytes_guard(input, channels)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn effect_noise_vector_guard(input: StrategyInputs) -> GuardAssessment {
    let sigma = match input.request {
        RequestFacts::EffectNoise { sigma_bits } => f64::from_bits(sigma_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if !sigma.is_finite() || input.width == 0 || input.height == 0 {
        return GuardAssessment::Ineligible;
    }
    if (input.width as usize)
        .checked_mul(input.height as usize)
        .is_some()
    {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn effect_mandelbrot_vector_guard(input: StrategyInputs) -> GuardAssessment {
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn expand_vector_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn alpha_mask_guard(
    input: StrategyInputs,
    channels: u8,
    minimum_pixels: usize,
) -> GuardAssessment {
    let source = StrategyGuard::PutAlphaBytes {
        channels,
        minimum_pixels,
    }
    .assess(input);
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn rgba_transpose_avx2_guard(input: StrategyInputs) -> GuardAssessment {
    match input.request {
        RequestFacts::Transpose {
            method:
                TransposeMethod::Rotate90
                | TransposeMethod::Rotate270
                | TransposeMethod::Transpose
                | TransposeMethod::Transverse,
        } => (),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    }
    let layout = native_transpose_storage_guard(input, 4, true);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    if input.width < 8 || input.height < 8 {
        return GuardAssessment::Ineligible;
    }
    layout
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn rgba_transpose_x86_64_tiled_guard(
    input: StrategyInputs,
    tile_min_dimension: u32,
    minimum_pixels: usize,
    block_size: u32,
) -> GuardAssessment {
    match input.request {
        RequestFacts::Transpose {
            method: TransposeMethod::Rotate270,
        } => (),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    }
    let layout = native_transpose_storage_guard(input, 4, true);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    if block_size == 0
        || input.width < tile_min_dimension
        || input.height < tile_min_dimension
        || (input.width as usize).saturating_mul(input.height as usize) < minimum_pixels
        || input.width % block_size != 0
        || input.height % block_size != 0
    {
        return GuardAssessment::Ineligible;
    }
    layout
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn native_transpose_bytes_guard(
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

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn transpose_avx512f_native32_guard(
    input: StrategyInputs,
    minimum_dimension: u32,
    max_pixels: u64,
) -> GuardAssessment {
    let method = match input.request {
        RequestFacts::Transpose { method } => method,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if !matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    ) || input.width < minimum_dimension
        || input.height < minimum_dimension
        || u64::from(input.width) * u64::from(input.height) > max_pixels
    {
        return GuardAssessment::Ineligible;
    }
    native_transpose_storage_guard(input, 4, false)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
/// Transpose uses the logical raster prefix and ignores trailing backing
/// samples. Empty rasters are admitted only when their storage is empty, as in
/// the archived adapter.
fn native_transpose_storage_guard(
    input: StrategyInputs,
    channels: u8,
    allow_empty: bool,
) -> GuardAssessment {
    let expected =
        crate::checked_dims::CheckedDims::new_allow_empty(input.width, input.height, channels)
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn color3dlut_guard(input: StrategyInputs, source_channels: u8) -> GuardAssessment {
    let (target_mode, channels, size, table_len) = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::Color3DLut {
            target_mode,
            channels,
            size,
            table_len,
        } => (target_mode, channels, size, table_len),
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
        (Some(pixels), Some(expected), Some(actual)) if pixels >= 8 && expected == actual => {
            GuardAssessment::Satisfied
        }
        (Some(pixels), _, _) if pixels < 8 => GuardAssessment::Ineligible,
        (Some(_), Some(_), None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn reduce_native_byte_vector_guard(
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

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn reduce_avx512_byte_2x2_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let factors = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::Reduce { x_factor, y_factor } => (x_factor, y_factor),
        _ => return GuardAssessment::Ineligible,
    };
    if channels != 4
        || factors != (2, 2)
        || !input.width.is_multiple_of(2)
        || !input.height.is_multiple_of(2)
        || !(input.width / 2).is_multiple_of(16)
    {
        return GuardAssessment::Ineligible;
    }
    let source = checked_native_bytes_guard(input, channels);
    if !matches!(source, GuardAssessment::Satisfied) {
        return source;
    }
    let output_width = input.width / 2;
    let output_height = input.height / 2;
    match crate::checked_dims::CheckedDims::new(output_width, output_height, channels) {
        Ok(output) if output.total_bytes() != 0 => GuardAssessment::Satisfied,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn native_byte_prefix_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
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

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn native_byte_avx512_invert_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    match native_byte_prefix_guard(input, channels) {
        GuardAssessment::Satisfied => {}
        GuardAssessment::Ineligible => return GuardAssessment::Ineligible,
        GuardAssessment::Deferred => return GuardAssessment::Deferred,
    }
    if input.width == 0 || input.height == 0 || !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    match (input.width as usize).checked_mul(usize::from(channels)) {
        Some(row_bytes) if row_bytes >= 64 => GuardAssessment::Satisfied,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn native_typed_avx512_invert_guard(
    input: StrategyInputs,
    bytes_per_pixel: u8,
    sample_bytes: u8,
) -> GuardAssessment {
    if bytes_per_pixel == 0 || !matches!(sample_bytes, 2 | 4) || bytes_per_pixel % sample_bytes != 0
    {
        return GuardAssessment::Ineligible;
    }
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(bytes_per_pixel)));
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual))
            if expected >= 64
                && expected == actual
                && expected % usize::from(sample_bytes) == 0 =>
        {
            GuardAssessment::Satisfied
        }
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn native_mirror_rows_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn native_pointwise_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    exact_native_rows_guard(input, channels)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn native_byte_avx512_pointwise_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    match native_pointwise_bytes_guard(input, channels) {
        GuardAssessment::Satisfied if input.byte_len.is_some_and(|length| length >= 64) => {
            GuardAssessment::Satisfied
        }
        GuardAssessment::Satisfied => GuardAssessment::Ineligible,
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn native_pointwise_bytes_including_empty_guard(
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn exact_native_rows_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn native_crop_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
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
    if width == 0 || height == 0 || source_width != input.width || source_height != input.height {
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

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn native_crop_border_bytes_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn pixel_span_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let (x, y) = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::PutPixel { x, y } => (x, y),
        RequestFacts::Draw { .. }
        | RequestFacts::Quantize { .. }
        | RequestFacts::Transform { .. }
        | RequestFacts::SameModeSecondaryImage { .. }
        | RequestFacts::Paste { .. }
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
        | RequestFacts::ConvertMatrix { .. }
        | RequestFacts::MergeBands { .. }
        | RequestFacts::BlendInputs { .. }
        | RequestFacts::CompositeInputs { .. }
        | RequestFacts::ChopsBinary { .. }
        | RequestFacts::CropBorder { .. }
        | RequestFacts::Crop { .. }
        | RequestFacts::Expand { .. } => return GuardAssessment::Ineligible,
    };
    if x >= input.width || y >= input.height {
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn pixel_write_run_guard(input: StrategyInputs) -> GuardAssessment {
    let writes = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::PutPixelBatch { count, writes } => {
            if count < 2 || writes == PixelWriteRunFacts::Ineligible {
                return GuardAssessment::Ineligible;
            }
            writes
        }
        RequestFacts::Draw { .. }
        | RequestFacts::Quantize { .. }
        | RequestFacts::Transform { .. }
        | RequestFacts::SameModeSecondaryImage { .. }
        | RequestFacts::Paste { .. }
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
        | RequestFacts::ConvertMatrix { .. }
        | RequestFacts::MergeBands { .. }
        | RequestFacts::BlendInputs { .. }
        | RequestFacts::CompositeInputs { .. }
        | RequestFacts::ChopsBinary { .. }
        | RequestFacts::CropBorder { .. }
        | RequestFacts::Crop { .. }
        | RequestFacts::Expand { .. } => return GuardAssessment::Ineligible,
    };
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(4));
    match (expected, input.byte_len) {
        (None, _) | (Some(0..=15), _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual != expected => GuardAssessment::Ineligible,
        (Some(_), Some(_)) if writes == PixelWriteRunFacts::Unknown => GuardAssessment::Deferred,
        (Some(_), Some(_)) => GuardAssessment::Satisfied,
    }
}
