//! Focused contract tests for SIMD Rotate geometry selection.

use super::rotate_discrete_fast_angle;

#[test]
fn rotate_discrete_fast_path_requires_exact_angle() {
    assert_eq!(rotate_discrete_fast_angle(90.0, None, None), Some(90));
    assert_eq!(rotate_discrete_fast_angle(-90.0, None, None), Some(270));
    assert_eq!(rotate_discrete_fast_angle(450.0, None, None), Some(90));
    for angle in [88.9, 89.9, 90.1, 91.1, 179.9, 180.1, 269.9, 270.1] {
        assert_eq!(
            rotate_discrete_fast_angle(angle, None, None),
            None,
            "fractional angle {angle} must retain affine sampling"
        );
    }
}
