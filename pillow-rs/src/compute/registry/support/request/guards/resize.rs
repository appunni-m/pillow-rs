//! Resize-specific implementation eligibility guards.

use super::core::checked_native_bytes_guard;
use super::super::{GuardAssessment, RequestFacts, StrategyInputs};
use crate::compute::pool_cpu::ops::resize::helpers::boxed_crop;
use crate::compute::pool_cpu::ops::resize::helpers::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source,
};
use crate::ops::pil_resize::{precompute_coeffs, precompute_coeffs_boxed_for_filter};

pub(crate) fn resize_filtered_fma_guard(input: StrategyInputs) -> GuardAssessment {
    match input.request {
        RequestFacts::Resize {
            width,
            height,
            filter,
            boxed,
            box_bounds_bits,
        } => {
            let boxed_shortcut = boxed
                && box_bounds_bits.is_some_and(|(left, top, right, bottom)| {
                    boxed_crop::uses_default_shortcut(
                        (input.width, input.height),
                        (width, height),
                        filter,
                        (
                            f64::from_bits(left),
                            f64::from_bits(top),
                            f64::from_bits(right),
                            f64::from_bits(bottom),
                        ),
                        false,
                    )
                });
            let has_filtered_work = !matches!(filter, crate::pipeline::ResampleFilter::Nearest)
                && input.width != 0
                && input.height != 0
                && width != 0
                && height != 0
                && !boxed_shortcut
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


pub(crate) fn resize_luma16_vector_guard(input: StrategyInputs) -> GuardAssessment {
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

pub(crate) fn resize_i32_neon_guard(input: StrategyInputs) -> GuardAssessment {
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

pub(crate) fn resize_byte_nearest_vector_guard(
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

pub(crate) fn resize_byte_filtered_vector_guard(
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

pub(crate) fn resize_byte_native_vector_guard(
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

pub(crate) fn resize_byte_boxed_nearest_vector_guard(
    input: StrategyInputs,
    channels: u8,
) -> GuardAssessment {
    match input.request {
        RequestFacts::Resize {
            filter: crate::pipeline::ResampleFilter::Nearest,
            boxed: true,
            ..
        } => resize_byte_boxed_vector_guard(input, channels, false),
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}

pub(crate) fn resize_byte_boxed_vector_guard(
    input: StrategyInputs,
    channels: u8,
    straight_alpha: bool,
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

    let Some(bounds) = boxed_crop::narrow_valid_bounds(
        (input.width, input.height),
        (
            f64::from_bits(bounds_bits.0),
            f64::from_bits(bounds_bits.1),
            f64::from_bits(bounds_bits.2),
            f64::from_bits(bounds_bits.3),
        ),
    ) else {
        return GuardAssessment::Ineligible;
    };

    if boxed_crop::uses_default_shortcut(
        (input.width, input.height),
        (width, height),
        filter,
        bounds,
        straight_alpha,
    ) {
        return GuardAssessment::Ineligible;
    }

    let horizontal_work = width != input.width
        || bounds.0 != 0.0
        || bounds.2 != f64::from(input.width as f32);
    if horizontal_work
        && u64::from(input.height) > u64::from(input.width) * 100
        && height < input.height
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

