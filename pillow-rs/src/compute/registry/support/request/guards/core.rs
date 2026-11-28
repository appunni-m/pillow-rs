//! Materialized input guards for declared implementation strategies.

use super::super::{
    GuardAssessment, RequestFacts, SecondaryInputFacts, StrategyGuard, StrategyInputs,
};
use crate::pipeline::ResampleFilter;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn thumbnail_i32_stream_guard(input: StrategyInputs) -> GuardAssessment {
    let (width, height, filter, reducing_gap_bits) = match input.request {
        RequestFacts::Thumbnail {
            width,
            height,
            filter,
            reducing_gap_bits,
        } => (width, height, filter, reducing_gap_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if width == 0 || height == 0 {
        return GuardAssessment::Ineligible;
    }
    let output = (
        width.max(1).min(input.width),
        height.max(1).min(input.height),
    );
    let reducing_gap = reducing_gap_bits.map(f64::from_bits);
    let Ok(factors) = crate::compute::helpers::resampling::thumbnail::reduce_factors_with_gap(
        (input.width, input.height),
        output,
        reducing_gap,
    ) else {
        return GuardAssessment::Ineligible;
    };
    if matches!(filter, ResampleFilter::Nearest) || factors != (2, 2) {
        return GuardAssessment::Ineligible;
    }
    let source_layout = checked_native_bytes_guard(input, 4);
    if !matches!(source_layout, GuardAssessment::Satisfied) {
        return source_layout;
    }
    if crate::checked_dims::CheckedDims::new(output.0, output.1, 4).is_err() {
        GuardAssessment::Ineligible
    } else {
        GuardAssessment::Satisfied
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn thumbnail_i32_avx512f_guard(input: StrategyInputs) -> GuardAssessment {
    match thumbnail_i32_stream_guard(input) {
        GuardAssessment::Satisfied
            if input.width >= 32
                && input.width.is_multiple_of(2)
                && input.height.is_multiple_of(2) =>
        {
            GuardAssessment::Satisfied
        }
        GuardAssessment::Satisfied => GuardAssessment::Ineligible,
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn reduce_f32_vector_guard(input: StrategyInputs, lanes: u8) -> GuardAssessment {
    reduce_32bit_vector_guard(input, lanes)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn reduce_i32_vector_guard(input: StrategyInputs, lanes: u8) -> GuardAssessment {
    reduce_32bit_vector_guard(input, lanes)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn reduce_32bit_vector_guard(input: StrategyInputs, lanes: u8) -> GuardAssessment {
    let (x_factor, y_factor) = match input.request {
        RequestFacts::Reduce { x_factor, y_factor } => (x_factor, y_factor),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if x_factor != 2
        || y_factor != 2
        || !input.width.is_multiple_of(2)
        || !input.height.is_multiple_of(2)
        || !(input.width / 2).is_multiple_of(u32::from(lanes))
    {
        return GuardAssessment::Ineligible;
    }
    let source = checked_native_bytes_guard(input, 4);
    if !matches!(source, GuardAssessment::Satisfied) {
        return source;
    }
    let output_width = input.width.div_ceil(x_factor);
    let output_height = input.height.div_ceil(y_factor);
    match crate::checked_dims::CheckedDims::new(output_width, output_height, 4) {
        Ok(output) if output.total_bytes() > 0 => GuardAssessment::Satisfied,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn brightness_vector_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let factor_bits = match input.request {
        RequestFacts::Brightness { factor_bits } => factor_bits,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let expected = input
        .width
        .checked_mul(input.height)
        .and_then(|pixels| pixels.checked_mul(u32::from(channels)))
        .and_then(|bytes| usize::try_from(bytes).ok());
    if !f64::from_bits(factor_bits).is_finite() {
        return GuardAssessment::Ineligible;
    }
    match (expected, input.byte_len) {
        (Some(expected), Some(actual)) if expected != 0 && actual == expected => {
            GuardAssessment::Satisfied
        }
        (Some(_), None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn contrast_vector_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let factor_bits = match input.request {
        RequestFacts::Contrast { factor_bits } => factor_bits,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let factor = f64::from_bits(factor_bits);
    if !factor.is_finite() || !(factor as f32).is_finite() {
        return GuardAssessment::Ineligible;
    }
    let expected = input
        .width
        .checked_mul(input.height)
        .and_then(|pixels| pixels.checked_mul(u32::from(channels)))
        .and_then(|bytes| usize::try_from(bytes).ok());
    match (expected, input.byte_len) {
        (Some(expected), Some(actual)) if expected >= 8 && actual == expected => {
            GuardAssessment::Satisfied
        }
        (Some(_), None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn sharpness_vector_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let factor_bits = match input.request {
        RequestFacts::Sharpness { factor_bits } => factor_bits,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if !f64::from_bits(factor_bits).is_finite() || input.width < 3 || input.height < 3 {
        return GuardAssessment::Ineligible;
    }
    let expected = input
        .width
        .checked_mul(input.height)
        .and_then(|pixels| pixels.checked_mul(u32::from(channels)))
        .and_then(|bytes| usize::try_from(bytes).ok());
    match (expected, input.byte_len) {
        (Some(expected), Some(actual)) if expected != 0 && actual == expected => {
            GuardAssessment::Satisfied
        }
        (Some(_), None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn color_saturation_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    let factor_bits = match input.request {
        RequestFacts::ColorSaturation { factor_bits } => factor_bits,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let factor = f64::from_bits(factor_bits);
    if !factor.is_finite() || !(factor as f32).is_finite() {
        return GuardAssessment::Ineligible;
    }
    let expected = input
        .width
        .checked_mul(input.height)
        .and_then(|pixels| pixels.checked_mul(u32::from(channels)))
        .and_then(|bytes| usize::try_from(bytes).ok());
    match (expected, input.byte_len) {
        (Some(expected), Some(actual)) if expected >= 8 && actual == expected => {
            GuardAssessment::Satisfied
        }
        (Some(_), None) => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn color_saturation_avx512f_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    match color_saturation_vector_guard(input, channels) {
        GuardAssessment::Satisfied if input.byte_len.is_some_and(|length| length >= 16) => {
            GuardAssessment::Satisfied
        }
        GuardAssessment::Satisfied => GuardAssessment::Ineligible,
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
// Scope: equal-dimension byte-vector Chops require matching native byte layouts.
pub(crate) fn chops_equal_dimensions_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    match input.request {
        RequestFacts::ChopsBinary { other, .. }
            if input.width == other.width && input.height == other.height =>
        {
            // Chops applies raw per-byte arithmetic to compatible layouts;
            // three-band LAB uses the same RgbImage carrier as RGB/YCbCr/HSV.
            chops_native_bytes_guard(input, other, channels)
        }
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
// Scope: ChopsAdd/Sub vector eligibility checks shape, finite parameters, and row threshold.
pub(crate) fn chops_add_subtract_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let (other, scale_bits, offset_bits) = match input.request {
        RequestFacts::ChopsBinary {
            other,
            affine_scale_bits: Some(scale_bits),
            affine_offset_bits: Some(offset_bits),
        } => (other, scale_bits, offset_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if input.width != other.width || input.height != other.height {
        return GuardAssessment::Ineligible;
    }
    let scale = f64::from_bits(scale_bits);
    let offset = f64::from_bits(offset_bits);
    let row_bytes = input.width.checked_mul(u32::from(channels));
    if !scale.is_finite() || !offset.is_finite() {
        return GuardAssessment::Ineligible;
    }
    if scale == 1.0 && offset == 0.0 {
        return chops_native_bytes_guard(input, other, channels);
    }
    if input.width == 0 || input.height == 0 {
        return chops_native_bytes_guard(input, other, channels);
    }
    if !row_bytes.is_some_and(|bytes| bytes >= 8) {
        return GuardAssessment::Ineligible;
    }
    chops_native_bytes_guard(input, other, channels)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn chops_offset_native_bytes_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    // Preserve the archived capability check's checked-u32 shape arithmetic.
    // On very large images it deliberately fell back when the byte count did
    // not fit u32, even if the host's usize could represent the allocation.
    let expected = input
        .width
        .checked_mul(u32::from(channels))
        .and_then(|row_bytes| row_bytes.checked_mul(input.height));
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(0), Some(0)) => GuardAssessment::Satisfied,
        (Some(expected), Some(actual))
            if expected >= 16 && usize::try_from(expected).ok() == Some(actual) =>
        {
            GuardAssessment::Satisfied
        }
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn chops_offset_native_i16_guard(input: StrategyInputs) -> GuardAssessment {
    // Match the archived I;16 capability predicate's checked-u32 byte count.
    // The row-wise typed SIMD kernel needs a complete 8-sample row block,
    // except that an empty image is a valid no-op. Narrow rows use the exact
    // mode's serial Default strategy regardless of total image height.
    let expected = input
        .width
        .checked_mul(input.height)
        .and_then(|pixels| pixels.checked_mul(2));
    match (expected, input.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(0), Some(0)) => GuardAssessment::Satisfied,
        (Some(expected), Some(actual))
            if input.width >= 8
                && expected >= 16
                && usize::try_from(expected).ok() == Some(actual) =>
        {
            GuardAssessment::Satisfied
        }
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-27; parity=pass@cpu-full-neon-20261127/batch-08-lab-secondary-rerun; benchmark=pending
// Scope: LAB secondary Chops inputs use the centered Default path.
fn chops_native_bytes_guard(
    input: StrategyInputs,
    other: SecondaryInputFacts,
    channels: u8,
) -> GuardAssessment {
    // The three-band Default Chops route applies Pillow's centered LAB
    // carrier adjustment per operand. The byte-vector kernel currently works
    // on serialized channels, so a LAB secondary must use that Default path.
    if other.mode == crate::raster::Mode::ImageLab8 {
        return GuardAssessment::Ineligible;
    }
    let primary = native_bytes_guard(input, channels);
    if !matches!(primary, GuardAssessment::Satisfied) {
        return primary;
    }
    let expected = (other.width as usize)
        .checked_mul(other.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)));
    match (expected, other.byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if actual == expected => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter3x3_native_bytes_vectorizable_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let layout = StrategyGuard::CheckedNativeBytes { channels }.assess(input);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let (kernel_bits, scale_bits) = match input.request {
        RequestFacts::Filter3x3 {
            kernel_bits,
            scale_bits,
            ..
        } => (kernel_bits, scale_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let scale = f32::from_bits(scale_bits);
    if input.width == 0
        || input.height == 0
        || !scale.is_finite()
        || scale == 0.0
        || kernel_bits
            .iter()
            .any(|bits| !f32::from_bits(*bits).is_finite())
    {
        GuardAssessment::Ineligible
    } else {
        GuardAssessment::Satisfied
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter5x5_native_bytes_vectorizable_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (kernel_bits, scale_bits) = match input.request {
        RequestFacts::Filter5x5 {
            kernel_bits,
            scale_bits,
            ..
        } => (kernel_bits, scale_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let layout = StrategyGuard::CheckedNativeBytes { channels }.assess(input);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let scale = f32::from_bits(scale_bits);
    let valid_parameters = scale.is_finite()
        && scale != 0.0
        && kernel_bits
            .iter()
            .all(|bits| f32::from_bits(*bits).is_finite());
    if input.width == 0 || input.height == 0 || !valid_parameters {
        GuardAssessment::Ineligible
    } else {
        GuardAssessment::Satisfied
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter5x5_i32_binomial_fma_guard(input: StrategyInputs) -> GuardAssessment {
    let (kernel_bits, scale_bits, offset_bits) = match input.request {
        RequestFacts::Filter5x5 {
            kernel_bits,
            scale_bits,
            offset_bits,
        } => (kernel_bits, scale_bits, offset_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let scale = f32::from_bits(scale_bits);
    if !scale.is_finite() || scale == 0.0 || f32::from_bits(offset_bits) != 0.0 {
        return GuardAssessment::Ineligible;
    }
    let layout = checked_native_bytes_guard(input, 4);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    if input.width == 0 || input.height == 0 {
        return GuardAssessment::Ineligible;
    }

    let factors = [1.0f32, 4.0, 6.0, 4.0, 1.0];
    let exact_binomial = kernel_bits.iter().enumerate().all(|(index, bits)| {
        let coefficient = f32::from_bits(*bits);
        if !coefficient.is_finite() {
            return false;
        }
        let normalized = coefficient / scale;
        let expected = factors[index % 5] * factors[index / 5] / 256.0;
        normalized.to_bits() == expected.to_bits()
    });
    if exact_binomial {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter5x5_i32_vectorizable_guard(input: StrategyInputs) -> GuardAssessment {
    let (kernel_bits, scale_bits) = match input.request {
        RequestFacts::Filter5x5 {
            kernel_bits,
            scale_bits,
            ..
        } => (kernel_bits, scale_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let layout = checked_native_bytes_guard(input, 4);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let scale = f32::from_bits(scale_bits);
    let valid_parameters = scale.is_finite()
        && scale != 0.0
        && kernel_bits
            .iter()
            .all(|bits| f32::from_bits(*bits).is_finite());
    if input.width != 0 && input.height != 0 && valid_parameters {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter_native_byte_blur_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) || input.width == 0 || input.height == 0 {
        return GuardAssessment::Ineligible;
    }
    let (active_blur, identity_copy) = match input.request {
        RequestFacts::GaussianBlur { sigma_bits } => {
            let sigma = f32::from_bits(sigma_bits);
            if !sigma.is_finite() {
                (false, false)
            } else if sigma == 0.0 {
                (false, true)
            } else {
                let radius =
                    crate::compute::helpers::filters::gaussian_blur_three_pass_radius(sigma.abs());
                (radius.is_finite() && radius > 0.0, false)
            }
        }
        RequestFacts::BoxBlur { radius } => (radius != 0, radius == 0),
        RequestFacts::BoxBlurXY {
            radius_x_bits,
            radius_y_bits,
            passes,
        } => {
            let radius_x = f32::from_bits(radius_x_bits);
            let radius_y = f32::from_bits(radius_y_bits);
            let valid = passes != 0
                && radius_x.is_finite()
                && radius_y.is_finite()
                && radius_x >= 0.0
                && radius_y >= 0.0;
            (
                valid && (radius_x != 0.0 || radius_y != 0.0),
                valid && radius_x == 0.0 && radius_y == 0.0,
            )
        }
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let identity_copy_is_vectorizable = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)))
        .is_some_and(|bytes| bytes >= 16);
    if !active_blur && !(identity_copy && identity_copy_is_vectorizable) {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, channels)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter_rgba_gaussian_blur_radius_seven_guard(
    input: StrategyInputs,
) -> GuardAssessment {
    let sigma = match input.request {
        RequestFacts::GaussianBlur { sigma_bits } => f32::from_bits(sigma_bits).abs(),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let radius = crate::compute::helpers::filters::gaussian_blur_three_pass_radius(sigma);
    if !sigma.is_finite()
        || sigma == 0.0
        || !(7.0..8.0).contains(&radius)
        || input.width == 0
        || input.height == 0
    {
        return GuardAssessment::Ineligible;
    }
    let (_, _, fractional_weight) =
        crate::compute::helpers::filters::gaussian_blur_box_parameters(sigma);
    // The archived RGBA radius-seven fast path excludes integral weights.
    if fractional_weight == 0 {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, 4)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn filter_rgba_gaussian_blur_radius_seven_avx512f_guard(
    input: StrategyInputs,
) -> GuardAssessment {
    let eligibility = filter_rgba_gaussian_blur_radius_seven_guard(input);
    if !matches!(eligibility, GuardAssessment::Satisfied) {
        return eligibility;
    }
    if input.width < 32 || input.height < 32 {
        return GuardAssessment::Ineligible;
    }
    GuardAssessment::Satisfied
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter_luma_rank_window_guard(input: StrategyInputs) -> GuardAssessment {
    let (size, rank) = match input.request {
        RequestFacts::FilterWindow { size, rank, .. } => (size, rank),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let area = u64::from(size) * u64::from(size);
    if size == 0
        || size.is_multiple_of(2)
        || size > 15
        || input.width == 0
        || input.height == 0
        || rank.is_some_and(|rank| u64::from(rank) >= area)
    {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, 1)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter_native_byte_extrema_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let size = match input.request {
        RequestFacts::FilterWindow { size, .. } => size,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if size == 0 || size.is_multiple_of(2) || input.width == 0 || input.height == 0 {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, channels)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter_native_byte_rank_window_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (size, rank) = match input.request {
        RequestFacts::FilterWindow { size, rank, .. } => (size, rank),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let area = u64::from(size) * u64::from(size);
    if size == 0
        || size.is_multiple_of(2)
        || size > 15
        || input.width == 0
        || input.height == 0
        || rank.is_some_and(|rank| u64::from(rank) >= area)
    {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, channels)
}

// VERIFY: source=reviewed@2026-11-24; parity=pass@migration-parity-de042d8a4480460c8d2be1f284c38af6; benchmark=pending
pub(crate) fn filter_f32_rank_window_guard(
    input: StrategyInputs,
    max_size: u32,
) -> GuardAssessment {
    let (size, rank, compatibility) = match input.request {
        RequestFacts::FilterWindow {
            size,
            rank,
            f32_rank_compatibility,
        } => (size, rank, f32_rank_compatibility),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if size == 0
        || size % 2 == 0
        || size > max_size
        || input.width == 0
        || input.height == 0
        || rank.is_some_and(|rank| u64::from(rank) >= u64::from(size) * u64::from(size))
    {
        return GuardAssessment::Ineligible;
    }
    match checked_native_bytes_guard(input, 4) {
        GuardAssessment::Satisfied => match compatibility {
            Some(super::super::F32RankCompatibility::VectorOrdering) => GuardAssessment::Satisfied,
            Some(super::super::F32RankCompatibility::DefaultComparator) => {
                GuardAssessment::Ineligible
            }
            None => GuardAssessment::Deferred,
        },
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn filter_f32_avx512_rank_window_guard(
    input: StrategyInputs,
    max_size: u32,
    minimum_width: u32,
    max_pixels: u64,
) -> GuardAssessment {
    let rank_window = filter_f32_rank_window_guard(input, max_size);
    if !matches!(rank_window, GuardAssessment::Satisfied) {
        return rank_window;
    }
    let size = match input.request {
        RequestFacts::FilterWindow { size, .. } => size,
        _ => return GuardAssessment::Ineligible,
    };
    let pixels = u64::from(input.width) * u64::from(input.height);
    if size == 1 || input.width < minimum_width || pixels > max_pixels {
        GuardAssessment::Ineligible
    } else {
        GuardAssessment::Satisfied
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn filter_i32_avx512_rank_window_guard(
    input: StrategyInputs,
    max_size: u32,
    minimum_width: u32,
    max_pixels: u64,
) -> GuardAssessment {
    let (size, rank) = match input.request {
        RequestFacts::FilterWindow { size, rank, .. } => (size, rank),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if size == 0
        || size == 1
        || size % 2 == 0
        || size > max_size
        || input.width < minimum_width
        || input.height == 0
        || rank.is_some_and(|rank| u64::from(rank) >= u64::from(size) * u64::from(size))
    {
        return GuardAssessment::Ineligible;
    }
    match checked_native_bytes_guard(input, 4) {
        GuardAssessment::Satisfied => {
            if u64::from(input.width) * u64::from(input.height) <= max_pixels {
                GuardAssessment::Satisfied
            } else {
                GuardAssessment::Ineligible
            }
        }
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter_unsharp_rgb150_guard(input: StrategyInputs) -> GuardAssessment {
    match input.request {
        RequestFacts::UnsharpMask { percent: 150, .. } => (),
        RequestFacts::UnsharpMask { .. } => return GuardAssessment::Ineligible,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    }
    if input.width == 0 || input.height == 0 {
        return GuardAssessment::Ineligible;
    }
    match checked_native_bytes_guard(input, 3) {
        GuardAssessment::Satisfied if input.byte_len.is_some_and(|length| length >= 16) => {
            GuardAssessment::Satisfied
        }
        GuardAssessment::Satisfied => GuardAssessment::Ineligible,
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn filter3x3_i32_vectorizable_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    let (kernel_bits, scale_bits) = match input.request {
        RequestFacts::Filter3x3 {
            kernel_bits,
            scale_bits,
            ..
        } => (kernel_bits, scale_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let layout = StrategyGuard::CheckedNativeBytes { channels }.assess(input);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let scale = f32::from_bits(scale_bits);
    let valid_parameters = scale.is_finite()
        && scale != 0.0
        && kernel_bits
            .iter()
            .all(|bits| f32::from_bits(*bits).is_finite());
    if valid_parameters && input.width != 0 && input.height != 0 {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn filter3x3_i32_avx512f_guard(input: StrategyInputs) -> GuardAssessment {
    let (kernel_bits, scale_bits) = match input.request {
        RequestFacts::Filter3x3 {
            kernel_bits,
            scale_bits,
            ..
        } => (kernel_bits, scale_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let layout = checked_native_bytes_guard(input, 4);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let scale = f32::from_bits(scale_bits);
    let valid_parameters = scale.is_finite()
        && scale != 0.0
        && kernel_bits
            .iter()
            .all(|bits| f32::from_bits(*bits).is_finite());
    if valid_parameters && input.width >= 18 && input.height >= 3 {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn filter5x5_i32_avx512f_guard(input: StrategyInputs) -> GuardAssessment {
    let (kernel_bits, scale_bits) = match input.request {
        RequestFacts::Filter5x5 {
            kernel_bits,
            scale_bits,
            ..
        } => (kernel_bits, scale_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let layout = checked_native_bytes_guard(input, 4);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let scale = f32::from_bits(scale_bits);
    let valid_parameters = scale.is_finite()
        && scale != 0.0
        && kernel_bits
            .iter()
            .all(|bits| f32::from_bits(*bits).is_finite());
    if valid_parameters && input.width >= 20 && input.height >= 5 {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn convert_target_guard(
    input: StrategyInputs,
    target: Mode,
    nonempty: bool,
) -> GuardAssessment {
    match input.request {
        RequestFacts::ConvertTarget { target_mode }
        | RequestFacts::ConvertMatrix { target_mode, .. }
            if target_mode == target =>
        {
            if !nonempty
                || (input.width as usize)
                    .checked_mul(input.height as usize)
                    .is_some_and(|pixels| pixels > 0)
            {
                GuardAssessment::Satisfied
            } else {
                GuardAssessment::Ineligible
            }
        }
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn transform_byte_kernels_guard(
    input: StrategyInputs,
    channels: u8,
    affine_nearest: bool,
    affine_bilinear: bool,
    projective_nearest: bool,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let facts = match input.request {
        RequestFacts::Transform { .. } => input.request,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let RequestFacts::Transform {
        method,
        data_len,
        filter,
        coefficients_finite,
        output_width,
        output_height,
    } = facts
    else {
        return GuardAssessment::Ineligible;
    };
    let supported_request = match method {
        0 if filter == crate::pipeline::ResampleFilter::Nearest => affine_nearest && data_len == 6,
        0 if filter == crate::pipeline::ResampleFilter::Bilinear => {
            affine_bilinear && data_len == 6 && input.width != 0 && input.height != 0
        }
        1 | 2 if filter == crate::pipeline::ResampleFilter::Nearest => {
            projective_nearest && data_len == 8 && input.width != 0 && input.height != 0
        }
        _ => false,
    };
    if !supported_request
        || !coefficients_finite
        || output_width == 0
        || output_height == 0
        || crate::checked_dims::CheckedDims::new(output_width, output_height, channels).is_err()
    {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, channels)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn transform_i16_affine_guard(
    input: StrategyInputs,
    nearest: bool,
    bilinear: bool,
) -> GuardAssessment {
    let facts = match input.request {
        RequestFacts::Transform { .. } => input.request,
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let RequestFacts::Transform {
        method,
        data_len,
        filter,
        coefficients_finite,
        output_width,
        output_height,
    } = facts
    else {
        return GuardAssessment::Ineligible;
    };
    let supported_filter = match filter {
        crate::pipeline::ResampleFilter::Nearest => nearest,
        crate::pipeline::ResampleFilter::Bilinear => bilinear,
        _ => false,
    };
    if method != 0
        || data_len != 6
        || !supported_filter
        || !coefficients_finite
        || input.width == 0
        || input.height == 0
        || output_width == 0
        || output_height == 0
        || crate::checked_dims::CheckedDims::new(input.width, input.height, 2).is_err()
        || crate::checked_dims::CheckedDims::new(output_width, output_height, 2).is_err()
    {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, 2)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn native_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)));
    exact_byte_layout(input.byte_len, expected)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn checked_native_bytes_guard(input: StrategyInputs, channels: u8) -> GuardAssessment {
    let expected = crate::checked_dims::CheckedDims::new(input.width, input.height, channels)
        .ok()
        .map(|dims| dims.total_bytes());
    exact_byte_layout(input.byte_len, expected)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn get_extrema_i32_avx512f_guard(input: StrategyInputs) -> GuardAssessment {
    match checked_native_bytes_guard(input, 4) {
        GuardAssessment::Satisfied if input.byte_len.is_some_and(|length| length >= 64) => {
            GuardAssessment::Satisfied
        }
        GuardAssessment::Satisfied => GuardAssessment::Ineligible,
        other => other,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn exact_byte_layout(byte_len: Option<usize>, expected: Option<usize>) -> GuardAssessment {
    match (expected, byte_len) {
        (None, _) => GuardAssessment::Ineligible,
        (Some(_), None) => GuardAssessment::Deferred,
        (Some(expected), Some(actual)) if expected == actual => GuardAssessment::Satisfied,
        (Some(_), Some(_)) => GuardAssessment::Ineligible,
    }
}
