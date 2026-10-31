//! Materialized input guards for declared implementation strategies.

use super::super::{GuardAssessment, RequestFacts, StrategyGuard, StrategyInputs};
use crate::compute::pool_cpu::ops::resize::helpers::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source,
};
use crate::ops::pil_resize::{precompute_coeffs, precompute_coeffs_boxed_for_filter};
use crate::pipeline::TransposeMethod;
use crate::raster::Mode;

pub(super) fn resize_filtered_fma_guard(input: StrategyInputs) -> GuardAssessment {
    match input.request {
        RequestFacts::Resize { width, height, filter, boxed, .. } => {
            let has_filtered_work = !matches!(filter, crate::pipeline::ResampleFilter::Nearest)
                && input.width != 0
                && input.height != 0
                && width != 0
                && height != 0
                && (boxed || (input.width, input.height) != (width, height));
            if has_filtered_work {
                GuardAssessment::Satisfied
            } else {
                GuardAssessment::Ineligible
            }
        }
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn thumbnail_i32_stream_guard(input: StrategyInputs) -> GuardAssessment {
    let (width, height, filter) = match input.request {
        RequestFacts::Thumbnail { width, height, filter } => (width, height, filter),
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
    if !crate::compute::helpers::resampling::thumbnail::uses_even_two_by_two_reduction(
        (input.width, input.height),
        output,
        filter,
    ) {
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

pub(super) fn reduce_f32_vector_guard(input: StrategyInputs) -> GuardAssessment {
    let (x_factor, y_factor) = match input.request {
        RequestFacts::Reduce { x_factor, y_factor } => (x_factor, y_factor),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if x_factor != 2
        || y_factor != 2
        || !input.width.is_multiple_of(2)
        || !input.height.is_multiple_of(2)
        || !(input.width / 2).is_multiple_of(8)
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

pub(super) fn resize_luma16_vector_guard(input: StrategyInputs) -> GuardAssessment {
    let (width, height) = match input.request {
        RequestFacts::Resize { width, height, boxed: false, .. } => (width, height),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if width == 0 || height == 0 {
        return GuardAssessment::Ineligible;
    }
    let layout = checked_native_bytes_guard(input, 2);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let output_width = match usize::try_from(width) {
        Ok(width) => width,
        Err(_) => return GuardAssessment::Ineligible,
    };
    let output_height = match usize::try_from(height) {
        Ok(height) => height,
        Err(_) => return GuardAssessment::Ineligible,
    };
    let source_height = match usize::try_from(input.height) {
        Ok(height) => height,
        Err(_) => return GuardAssessment::Ineligible,
    };
    if output_width.checked_mul(output_height).is_none()
        || source_height.checked_mul(output_width).is_none()
    {
        GuardAssessment::Ineligible
    } else {
        GuardAssessment::Satisfied
    }
}

pub(super) fn resize_i32_neon_guard(input: StrategyInputs) -> GuardAssessment {
    let (width, height) = match input.request {
        RequestFacts::Resize {
            width,
            height,
            boxed: false,
            ..
        } => (width, height),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if input.width == 0
        || input.height == 0
        || width == 0
        || height == 0
        || (input.width, input.height) == (width, height)
    {
        return GuardAssessment::Ineligible;
    }
    let layout = checked_native_bytes_guard(input, 4);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    let output_pixels = match (width as usize).checked_mul(height as usize) {
        Some(pixels) => pixels,
        None => return GuardAssessment::Ineligible,
    };
    let intermediate_samples = (input.height as usize).checked_mul(width as usize);
    if output_pixels.checked_mul(4).is_some() && intermediate_samples.is_some() {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

pub(super) fn resize_byte_nearest_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (width, height) = match input.request {
        RequestFacts::Resize {
            width,
            height,
            filter: crate::pipeline::ResampleFilter::Nearest,
            boxed: false,
            ..
        } => (width, height),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if width == 0
        || height == 0
        || input.width == 0
        || input.height == 0
        || (input.width, input.height) == (width, height)
    {
        return GuardAssessment::Ineligible;
    }
    let layout = checked_native_bytes_guard(input, channels);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    if crate::checked_dims::CheckedDims::new(width, height, channels).is_ok() {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

pub(super) fn resize_byte_filtered_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (width, height, filter) = match input.request {
        RequestFacts::Resize {
            width,
            height,
            filter,
            boxed: false,
            ..
        } => (width, height, filter),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if matches!(filter, crate::pipeline::ResampleFilter::Nearest)
        || width == 0
        || height == 0
        || input.width == 0
        || input.height == 0
        || (input.width, input.height) == (width, height)
    {
        return GuardAssessment::Ineligible;
    }
    let source_layout = checked_native_bytes_guard(input, channels);
    if !matches!(source_layout, GuardAssessment::Satisfied) {
        return source_layout;
    }
    if crate::checked_dims::CheckedDims::new(width, input.height, channels).is_err()
        || crate::checked_dims::CheckedDims::new(width, height, channels).is_err()
    {
        return GuardAssessment::Ineligible;
    }
    let horizontal = precompute_coeffs(width, input.width, filter);
    let vertical = precompute_coeffs(height, input.height, filter);
    let coefficients_fit = match (
        usize::try_from(input.width),
        usize::try_from(input.height),
    ) {
        (Ok(source_width), Ok(source_height)) => {
            byte_coefficients_fit_source(&horizontal, source_width)
                && byte_coefficients_fit_source(&vertical, source_height)
                && byte_coefficients_fit_i32(&horizontal)
                && byte_coefficients_fit_i32(&vertical)
        }
        _ => false,
    };
    if coefficients_fit {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

pub(super) fn resize_byte_native_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    match input.request {
        RequestFacts::Resize {
            filter: crate::pipeline::ResampleFilter::Nearest,
            ..
        } => resize_byte_nearest_vector_guard(input, channels),
        RequestFacts::Resize { .. } => resize_byte_filtered_vector_guard(input, channels),
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn resize_byte_boxed_nearest_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    match input.request {
        RequestFacts::Resize {
            filter: crate::pipeline::ResampleFilter::Nearest,
            boxed: true,
            ..
        } => resize_byte_boxed_vector_guard(input, channels),
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn resize_byte_boxed_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (width, height, filter, bounds_bits) = match input.request {
        RequestFacts::Resize {
            width,
            height,
            filter,
            boxed: true,
            box_bounds_bits: Some(bounds),
        } => (width, height, filter, bounds),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if width == 0 || height == 0 || input.width == 0 || input.height == 0 {
        return GuardAssessment::Ineligible;
    }
    let layout = checked_native_bytes_guard(input, channels);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    if crate::checked_dims::CheckedDims::new(width, height, channels).is_err()
        || crate::checked_dims::CheckedDims::new(width, input.height, channels).is_err()
    {
        return GuardAssessment::Ineligible;
    }

    let bounds = (
        f64::from(f64::from_bits(bounds_bits.0) as f32),
        f64::from(f64::from_bits(bounds_bits.1) as f32),
        f64::from(f64::from_bits(bounds_bits.2) as f32),
        f64::from(f64::from_bits(bounds_bits.3) as f32),
    );
    if !bounds.0.is_finite()
        || !bounds.1.is_finite()
        || !bounds.2.is_finite()
        || !bounds.3.is_finite()
        || bounds.0 < 0.0
        || bounds.1 < 0.0
        || bounds.2 > f64::from(input.width as f32)
        || bounds.3 > f64::from(input.height as f32)
        || bounds.2 < bounds.0
        || bounds.3 < bounds.1
    {
        return GuardAssessment::Ineligible;
    }

    let full_source_identity = input.width == width
        && input.height == height
        && bounds.0 == 0.0
        && bounds.1 == 0.0
        && bounds.2 == f64::from(input.width as f32)
        && bounds.3 == f64::from(input.height as f32);
    if full_source_identity
        || u64::from(input.height) > u64::from(input.width) * 100 && height < input.height
    {
        return GuardAssessment::Ineligible;
    }
    if matches!(filter, crate::pipeline::ResampleFilter::Nearest) {
        return GuardAssessment::Satisfied;
    }

    let horizontal = precompute_coeffs_boxed_for_filter(
        width,
        input.width,
        bounds.0,
        bounds.2,
        filter,
    );
    let vertical = precompute_coeffs_boxed_for_filter(
        height,
        input.height,
        bounds.1,
        bounds.3,
        filter,
    );
    let coefficient_shape_is_valid = match (
        usize::try_from(input.width),
        usize::try_from(input.height),
    ) {
        (Ok(source_width), Ok(source_height)) => {
            byte_coefficients_fit_source(&horizontal, source_width)
                && byte_coefficients_fit_source(&vertical, source_height)
                && byte_coefficients_fit_i32(&horizontal)
                && byte_coefficients_fit_i32(&vertical)
        }
        _ => false,
    };
    if coefficient_shape_is_valid {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

pub(super) fn brightness_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
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

pub(super) fn sharpness_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
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

pub(super) fn color_saturation_vector_guard(
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

pub(super) fn chops_equal_dimensions_guard(input: StrategyInputs) -> GuardAssessment {
    match input.request {
        RequestFacts::ChopsBinary { other, .. }
            if input.width == other.width && input.height == other.height =>
        {
            GuardAssessment::Satisfied
        }
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

pub(super) fn chops_add_subtract_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
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
    let scale = f32::from_bits(scale_bits);
    let offset = f32::from_bits(offset_bits);
    let row_bytes = input.width.checked_mul(u32::from(channels));
    if !scale.is_finite() || !offset.is_finite() {
        GuardAssessment::Ineligible
    } else if input.width == 0 || input.height == 0 {
        GuardAssessment::Satisfied
    } else if (scale == 1.0 && offset == 0.0) || row_bytes.is_some_and(|bytes| bytes >= 8) {
        GuardAssessment::Satisfied
    } else {
        GuardAssessment::Ineligible
    }
}

pub(super) fn filter3x3_native_bytes_vectorizable_guard(
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
        RequestFacts::Filter3x3 { kernel_bits, scale_bits, .. } => (kernel_bits, scale_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let scale = f32::from_bits(scale_bits);
    if input.width < 3
        || input.height < 3
        || !scale.is_finite()
        || scale == 0.0
        || kernel_bits.iter().any(|bits| !f32::from_bits(*bits).is_finite())
    {
        GuardAssessment::Ineligible
    } else {
        GuardAssessment::Satisfied
    }
}

pub(super) fn filter5x5_native_bytes_vectorizable_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (kernel_bits, scale_bits) = match input.request {
        RequestFacts::Filter5x5 { kernel_bits, scale_bits, .. } => (kernel_bits, scale_bits),
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
    if input.width < 5 || input.height < 5 || !valid_parameters {
        GuardAssessment::Ineligible
    } else {
        GuardAssessment::Satisfied
    }
}

pub(super) fn filter_native_byte_blur_guard(
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
                let radius = crate::compute::helpers::filters::gaussian_blur_three_pass_radius(
                    sigma.abs(),
                );
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

pub(super) fn filter_rgba_gaussian_blur_radius_seven_guard(
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
        || input.width < 64
        || input.height < 64
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

pub(super) fn filter_luma_rank_window_guard(input: StrategyInputs) -> GuardAssessment {
    let (size, rank) = match input.request {
        RequestFacts::FilterWindow { size, rank } => (size, rank),
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

pub(super) fn filter_native_byte_extrema_guard(
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

pub(super) fn filter_native_byte_rank_window_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let (size, rank) = match input.request {
        RequestFacts::FilterWindow { size, rank } => (size, rank),
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

pub(super) fn filter_f32_rank_window_guard(input: StrategyInputs) -> GuardAssessment {
    let (size, rank) = match input.request {
        RequestFacts::FilterWindow { size, rank } => (size, rank),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    if size == 0
        || size % 2 == 0
        || size > 9
        || input.width == 0
        || input.height == 0
        || rank.is_some_and(|rank| u64::from(rank) >= u64::from(size) * u64::from(size))
    {
        return GuardAssessment::Ineligible;
    }
    checked_native_bytes_guard(input, 4)
}

pub(super) fn filter_unsharp_rgb150_guard(input: StrategyInputs) -> GuardAssessment {
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

pub(super) fn filter3x3_i32_vectorizable_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    let (kernel_bits, scale_bits) = match input.request {
        RequestFacts::Filter3x3 { kernel_bits, scale_bits, .. } => (kernel_bits, scale_bits),
        RequestFacts::Unspecified => return GuardAssessment::Deferred,
        _ => return GuardAssessment::Ineligible,
    };
    let layout = StrategyGuard::CheckedNativeBytes { channels }.assess(input);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    if input.width < 6 || input.height < 3 {
        return GuardAssessment::Ineligible;
    }
    let scale = f32::from_bits(scale_bits);
    let valid_parameters = scale.is_finite()
        && scale != 0.0
        && kernel_bits.iter().all(|bits| f32::from_bits(*bits).is_finite());
    if valid_parameters { GuardAssessment::Satisfied } else { GuardAssessment::Ineligible }
}

pub(super) fn convert_target_guard(
    input: StrategyInputs,
    target: Mode,
    nonempty: bool,
) -> GuardAssessment {
    match input.request {
        RequestFacts::ConvertTarget { target_mode } if target_mode == target => {
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

