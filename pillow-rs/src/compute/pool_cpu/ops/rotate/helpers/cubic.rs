//! Pillow's horizontal cubic interpolation equations used by Rotate kernels.

#[inline]
pub(crate) fn horizontal_f32(samples: [f32; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = f64::from(v2);
    let p2 = f64::from(v3 - v1);
    let p3 = f64::from((v1 - v2).mul_add(2.0, v3) - v4);
    let p4 = f64::from((v2 - v1 - v3) + v4);
    distance.mul_add(distance.mul_add(distance.mul_add(p4, p3), p2), p1)
}

#[inline]
pub(crate) fn horizontal_f64(samples: [f64; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = v2;
    let p2 = -v1 + v3;
    let p3 = 2.0 * (v1 - v2) + v3 - v4;
    let p4 = -v1 + v2 - v3 + v4;
    distance.mul_add(distance.mul_add(distance.mul_add(p4, p3), p2), p1)
}

#[inline]
pub(crate) fn horizontal_i32(samples: [i32; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = f64::from(v2);
    let p2 = f64::from(v1.wrapping_neg().wrapping_add(v3));
    let p3 = f64::from(
        v1.wrapping_sub(v2)
            .wrapping_mul(2)
            .wrapping_add(v3)
            .wrapping_sub(v4),
    );
    let p4 = f64::from(
        v1.wrapping_neg()
            .wrapping_add(v2)
            .wrapping_sub(v3)
            .wrapping_add(v4),
    );
    distance.mul_add(distance.mul_add(distance.mul_add(p4, p3), p2), p1)
}

#[inline]
pub(crate) fn vertical_f64_fma(samples: [f64; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = v2;
    let p2 = -v1 + v3;
    let p3 = (v1 - v2).mul_add(2.0, v3) - v4;
    let p4 = -v1 + v2 - v3 + v4;
    distance.mul_add(distance.mul_add(distance.mul_add(p4, p3), p2), p1)
}

#[inline]
pub(crate) fn vertical_f64(samples: [f64; 4], distance: f64) -> f64 {
    horizontal_f64(samples, distance)
}
