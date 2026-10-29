//! Pillow's reverse-affine geometry shared by exact-mode Rotate strategies.

use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;

#[derive(Clone, Copy)]
pub(crate) struct Geometry {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) affine: [f64; 6],
}

pub(crate) fn rotate_geometry(width: u32, height: u32, request: RotateRequest) -> Geometry {
    let sw = f64::from(width);
    let sh = f64::from(height);
    let radians = -request.angle.to_radians();
    let a = crate::ops::rotate::round_rotate_coefficient(radians.cos());
    let b = crate::ops::rotate::round_rotate_coefficient(radians.sin());
    let d = crate::ops::rotate::round_rotate_coefficient(-radians.sin());
    let e = a;
    let (center_x, center_y) = request.center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = request.translate.unwrap_or((0.0, 0.0));
    let mut c = a * (-center_x - translate_x) + b * (-center_y - translate_y) + center_x;
    let mut f = d * (-center_x - translate_x) + e * (-center_y - translate_y) + center_y;
    let (min_x, min_y, max_x, max_y) = transformed_bounds(a, b, c, d, e, f, sw, sh);
    let (destination_width, destination_height) = if request.expand {
        ((max_x.ceil() - min_x.floor()) as u32, (max_y.ceil() - min_y.floor()) as u32)
    } else {
        (width, height)
    };
    if request.expand {
        let shift_x = -(f64::from(destination_width) - sw) / 2.0;
        let shift_y = -(f64::from(destination_height) - sh) / 2.0;
        (c, f) = transform(a, b, c, d, e, f, shift_x, shift_y);
    }
    Geometry {
        width: destination_width,
        height: destination_height,
        affine: [a, b, c, d, e, f],
    }
}

fn transformed_bounds(
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
    width: f64,
    height: f64,
) -> (f64, f64, f64, f64) {
    let corners = [(0.0, 0.0), (width, 0.0), (width, height), (0.0, height)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in corners {
        let (rotated_x, rotated_y) = transform(a, b, c, d, e, f, x, y);
        min_x = min_x.min(rotated_x);
        max_x = max_x.max(rotated_x);
        min_y = min_y.min(rotated_y);
        max_y = max_y.max(rotated_y);
    }
    (min_x, min_y, max_x, max_y)
}

#[inline]
fn transform(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, x: f64, y: f64) -> (f64, f64) {
    (a * x + b * y + c, d * x + e * y + f)
}

pub(crate) fn discrete_angle(request: RotateRequest) -> Option<u32> {
    if request.center.is_some() || request.translate.is_some() || !request.angle.is_finite() {
        return None;
    }
    [90.0, 180.0, 270.0]
        .into_iter()
        .find(|angle| request.angle.rem_euclid(360.0) == *angle)
        .map(|angle| angle as u32)
}
