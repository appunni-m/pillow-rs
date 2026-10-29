//! Materialized input guards for declared implementation strategies.

use super::{GuardAssessment, PixelWriteRunFacts, RequestFacts, StrategyGuard, StrategyInputs};
use crate::raster::Mode;

pub(super) fn rotate_vector_bytes_guard(
    input: StrategyInputs,
    channels: u8,
    force_nearest: bool,
    bilinear: bool,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let layout = StrategyGuard::CheckedNativeBytes { channels }.assess(input);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let (angle_bits, expand, center_bits, translate_bits, filter, nearest) = match input.request {
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        RequestFacts::Rotate {
            angle_bits, expand, center_bits, translate_bits, filter, nearest, ..
        } => (angle_bits, expand, center_bits, translate_bits, filter, nearest),
        _ => return GuardAssessment::Ineligible,
    };
    let nearest = force_nearest || nearest;
    if !nearest && (!bilinear || filter != 1) {
        return GuardAssessment::Ineligible;
    }
    let angle = f64::from_bits(angle_bits);
    let finite_pair = |bits: Option<(u64, u64)>| {
        bits.is_none_or(|(x, y)| f64::from_bits(x).is_finite() && f64::from_bits(y).is_finite())
    };
    if !angle.is_finite() || !finite_pair(center_bits) || !finite_pair(translate_bits) {
        return GuardAssessment::Ineligible;
    }
    let center = center_bits.map(|(x, y)| (f64::from_bits(x), f64::from_bits(y)));
    let translate = translate_bits.map(|(x, y)| (f64::from_bits(x), f64::from_bits(y)));
    if center_bits.is_none() && translate_bits.is_none() {
        let normalized = angle.rem_euclid(360.0);
        if normalized.abs() <= f64::EPSILON {
            return GuardAssessment::Ineligible;
        }
        let discrete = [90.0, 180.0, 270.0].into_iter().find(|value| normalized == *value);
        if discrete.is_some_and(|degree| degree == 180.0 || expand || input.width == input.height) {
            return GuardAssessment::Ineligible;
        }
    }
    if !rotate_nearest_geometry_supported(
        input.width,
        input.height,
        channels,
        angle,
        expand,
        center,
        translate,
    ) {
        return GuardAssessment::Ineligible;
    }
    layout
}

fn rotate_nearest_geometry_supported(
    width: u32,
    height: u32,
    channels: u8,
    angle: f64,
    expand: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    let (sw, sh) = (f64::from(width), f64::from(height));
    let radians = -angle.to_radians();
    let a = crate::ops::rotate::round_rotate_coefficient(radians.cos());
    let b = crate::ops::rotate::round_rotate_coefficient(radians.sin());
    let d = crate::ops::rotate::round_rotate_coefficient(-radians.sin());
    let e = a;
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let c = a * (-center_x - translate_x) + b * (-center_y - translate_y) + center_x;
    let f = d * (-center_x - translate_x) + e * (-center_y - translate_y) + center_y;
    let Some((output_width, output_height)) =
        rotate_destination_dimensions(a, b, c, d, e, f, width, height, expand)
    else {
        return false;
    };
    let Some(output_bytes) = output_width
        .checked_mul(output_height)
        .and_then(|pixels| pixels.checked_mul(u32::from(channels)))
    else {
        return false;
    };
    output_bytes != 0
        && crate::checked_dims::CheckedDims::new(output_width, output_height, channels).is_ok()
}

fn rotate_destination_dimensions(
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
    width: u32,
    height: u32,
    expand: bool,
) -> Option<(u32, u32)> {
    let (sw, sh) = (f64::from(width), f64::from(height));
    let transform = |x: f64, y: f64| (a * x + b * y + c, d * x + e * y + f);
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) =
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in corners {
        let (rotated_x, rotated_y) = transform(x, y);
        min_x = min_x.min(rotated_x);
        max_x = max_x.max(rotated_x);
        min_y = min_y.min(rotated_y);
        max_y = max_y.max(rotated_y);
    }
    let (output_width, output_height) = if expand {
        (max_x.ceil() - min_x.floor(), max_y.ceil() - min_y.floor())
    } else {
        (sw, sh)
    };
    if !output_width.is_finite()
        || !output_height.is_finite()
        || output_width < 0.0
        || output_height < 0.0
        || output_width > f64::from(u32::MAX)
        || output_height > f64::from(u32::MAX)
    {
        return None;
    }
    Some((output_width as u32, output_height as u32))
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
        RequestFacts::PutData { mode, .. } if payloads.contains(&mode) => layout,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn effect_spread_bytes_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    match input.request {
        RequestFacts::EffectSpread { distance } if distance != 1 => (),
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

pub(super) fn native_bytes_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)));
    exact_byte_layout(input.byte_len, expected)
}

pub(super) fn checked_native_bytes_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    let expected = crate::checked_dims::CheckedDims::new(input.width, input.height, channels)
        .ok()
        .map(|dims| dims.total_bytes());
    exact_byte_layout(input.byte_len, expected)
}

fn exact_byte_layout(byte_len: Option<usize>, expected: Option<usize>) -> GuardAssessment {
    match (expected, byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if expected == actual => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
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
    if mask.mode != Mode::ImageLuma8 || mask.width != input.width || mask.height != input.height {
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
    let layout = StrategyGuard::CheckedNativeBytes { channels: 4 }.assess(input);
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

pub(super) fn native_transpose_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let layout = StrategyGuard::CheckedNativeBytes { channels }.assess(input);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    match input.request {
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        RequestFacts::Transpose { .. } => layout,
        _ => GuardAssessment::Ineligible,
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
        RequestFacts::ExtractBand { .. } | RequestFacts::PutPixelBatch { .. }
        | RequestFacts::PutData { .. } | RequestFacts::PutAlphaData { .. }
        | RequestFacts::ImageOpsMask { .. }
        | RequestFacts::Color3DLut { .. } | RequestFacts::Transpose { .. }
        | RequestFacts::Rotate { .. }
        | RequestFacts::Posterize { .. }
        | RequestFacts::EffectSpread { .. }
        | RequestFacts::EffectNoise { .. }
        | RequestFacts::EvalLut { .. }
        | RequestFacts::Reduce { .. }
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
        RequestFacts::ExtractBand { .. } | RequestFacts::PutPixel { .. }
        | RequestFacts::PutData { .. } | RequestFacts::PutAlphaData { .. }
        | RequestFacts::ImageOpsMask { .. }
        | RequestFacts::Color3DLut { .. } | RequestFacts::Transpose { .. }
        | RequestFacts::Rotate { .. }
        | RequestFacts::Posterize { .. }
        | RequestFacts::EffectSpread { .. }
        | RequestFacts::EffectNoise { .. }
        | RequestFacts::EvalLut { .. }
        | RequestFacts::Reduce { .. }
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

