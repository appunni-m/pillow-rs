//! Fixed-point affine eligibility shared by CPU transform and GPU planning.

/// Computes Pillow's 16.16 nearest-affine coefficients when the map is not a
/// pure scale and all output corners fit the fixed-point coordinate range.
#[must_use]
pub(crate) fn affine_nearest_fixed(
    data: [f64; 6],
    width: u32,
    height: u32,
) -> Option<[i64; 6]> {
    let [a, b, c, d, e, f] = data;
    if b == 0.0 && d == 0.0 {
        return None;
    }
    let corners = [
        (0.0, 0.0),
        (f64::from(width), 0.0),
        (0.0, f64::from(height)),
        (f64::from(width), f64::from(height)),
    ];
    if !corners.into_iter().all(|(x, y)| {
        (a.mul_add(x, b * y) + c).abs() < 32768.0
            && (d.mul_add(x, e * y) + f).abs() < 32768.0
    }) {
        return None;
    }
    let fixed = |value: f64| value.mul_add(65536.0, 0.5).floor() as i64;
    Some([
        fixed(a),
        fixed(b),
        fixed(c + a * 0.5 + b * 0.5),
        fixed(d),
        fixed(e),
        fixed(f + d * 0.5 + e * 0.5),
    ])
}
