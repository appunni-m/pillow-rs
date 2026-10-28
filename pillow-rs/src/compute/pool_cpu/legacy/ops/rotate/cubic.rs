//! Pillow-compatible cubic interpolation helpers.

#[inline]
pub(crate) fn rotate_cubic_horizontal_f32(samples: [f32; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = f64::from(v2);
    let p2 = f64::from(v3 - v1);
    let p3 = f64::from((v1 - v2).mul_add(2.0, v3) - v4);
    let p4 = f64::from((v2 - v1 - v3) + v4);
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
pub(crate) fn rotate_cubic_horizontal_f64(samples: [f64; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = v2;
    let p2 = -v1 + v3;
    let p3 = 2.0 * (v1 - v2) + v3 - v4;
    let p4 = -v1 + v2 - v3 + v4;
    // Geometry.c's optimized arm64 build contracts each Horner step into an
    // FMA.  Keep the contraction explicit; the final UINT8 cast can change
    // at a one-ULP boundary when this is evaluated as separate multiply/add
    // operations.
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
pub(crate) fn rotate_cubic_horizontal_i32(samples: [i32; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    // Geometry.c's INT32 BICUBIC coefficients are formed in the source
    // integer type before the Horner chain is promoted to double. Explicit
    // wrapping keeps Rust's defined arithmetic aligned with Pillow's native
    // two's-complement operations for large signed samples.
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
    // As in the UINT8 kernel, the native INT32 path uses an FMA Horner
    // chain after the integer coefficient preparation.
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
pub(crate) fn rotate_cubic_vertical_f64_fma(samples: [f64; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = v2;
    let p2 = -v1 + v3;
    let p3 = (v1 - v2).mul_add(2.0, v3) - v4;
    let p4 = -v1 + v2 - v3 + v4;
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
pub(crate) fn rotate_cubic_vertical_f64(samples: [f64; 4], distance: f64) -> f64 {
    rotate_cubic_horizontal_f64(samples, distance)
}
