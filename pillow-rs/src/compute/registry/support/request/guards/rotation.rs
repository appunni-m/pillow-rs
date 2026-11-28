//! Materialized request guards for Rotate's vector strategies.

use super::super::{GuardAssessment, RequestFacts, StrategyGuard, StrategyInputs};

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn rotate_vector_bytes_guard(
    input: StrategyInputs,
    channels: u8,
    force_nearest: bool,
    bilinear: bool,
) -> GuardAssessment {
    if !(1..=4).contains(&channels) {
        return GuardAssessment::Ineligible;
    }
    let empty_input = input.width == 0 || input.height == 0;
    let RequestFacts::Rotate {
        angle_bits,
        expand,
        center_bits,
        translate_bits,
        affine_bits,
        output_size: _,
        filter,
        nearest,
        ..
    } = input.request
    else {
        return if input.request == RequestFacts::Unspecified {
            GuardAssessment::Deferred
        } else {
            GuardAssessment::Ineligible
        };
    };
    let angle = f64::from_bits(angle_bits);
    let finite_pair = |bits: Option<(u64, u64)>| {
        bits.is_none_or(|(x, y)| f64::from_bits(x).is_finite() && f64::from_bits(y).is_finite())
    };
    let finite_affine = affine_bits.is_none_or(|values| {
        values
            .into_iter()
            .all(|value| f64::from_bits(value).is_finite())
    });
    if !angle.is_finite()
        || !finite_pair(center_bits)
        || !finite_pair(translate_bits)
        || !finite_affine
    {
        return GuardAssessment::Ineligible;
    }
    // Empty-source behavior depends on the exact Default fast path: archived
    // discrete paths preserve some empty outputs, while affine paths may
    // reject or fill an expanded image. Keep that case on Default instead of
    // a generic vector shortcut.
    // TODO(pillow-rs#3ptb): verify empty outputs and architecture receipts in
    // the frozen CPU parity matrix.
    if empty_input {
        return GuardAssessment::Ineligible;
    }
    let layout = StrategyGuard::CheckedNativeBytes { channels }.assess(input);
    if !matches!(layout, GuardAssessment::Satisfied) {
        return layout;
    }
    if affine_bits.is_none()
        && angle.rem_euclid(360.0).abs() <= f64::EPSILON
        && center_bits.is_none()
        && translate_bits.is_none()
    {
        return layout;
    }
    if affine_bits.is_none() && center_bits.is_none() && translate_bits.is_none() {
        let normalized = angle.rem_euclid(360.0);
        let discrete = [90.0, 180.0, 270.0]
            .into_iter()
            .find(|value| normalized == *value);
        if discrete.is_some_and(|degree| degree == 180.0 || expand || input.width == input.height) {
            return layout;
        }
    }
    let nearest = force_nearest || nearest;
    if !nearest && (!bilinear || filter != 1) {
        return GuardAssessment::Ineligible;
    }
    if !rotate_output_geometry_supported(input, channels) {
        return GuardAssessment::Ineligible;
    }
    layout
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn rotate_output_geometry_supported(input: StrategyInputs, channels: u8) -> bool {
    let width = input.width;
    let height = input.height;
    if width == 0 || height == 0 {
        return false;
    }
    let RequestFacts::Rotate {
        angle_bits,
        expand,
        center_bits,
        translate_bits,
        affine_bits,
        output_size,
        ..
    } = input.request
    else {
        return false;
    };
    if let Some(affine_bits) = affine_bits {
        let Some((output_width, output_height)) = output_size else {
            return false;
        };
        return affine_bits
            .into_iter()
            .all(|value| f64::from_bits(value).is_finite())
            && output_width != 0
            && output_height != 0
            && output_width
                .checked_mul(output_height)
                .and_then(|pixels| pixels.checked_mul(u32::from(channels)))
                .is_some_and(|output_bytes| output_bytes != 0)
            && crate::checked_dims::CheckedDims::new(output_width, output_height, channels)
                .is_ok();
    }
    let angle = f64::from_bits(angle_bits);
    let (sw, sh) = (f64::from(width), f64::from(height));
    let radians = -angle.to_radians();
    let a = crate::ops::rotate::round_rotate_coefficient(radians.cos());
    let b = crate::ops::rotate::round_rotate_coefficient(radians.sin());
    let d = crate::ops::rotate::round_rotate_coefficient(-radians.sin());
    let e = a;
    let center = center_bits.map(|(x, y)| (f64::from_bits(x), f64::from_bits(y)));
    let translate = translate_bits.map(|(x, y)| (f64::from_bits(x), f64::from_bits(y)));
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
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
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
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
