//! Pillow-compatible typed resize filters shared by CPU and SIMD execution.

use crate::ops::pil_resize::pillow_sin_f64;
use crate::pipeline::ResampleFilter;

/// Box / Nearest-neighbor kernel.
pub(crate) fn f_kernel_box(x: f64) -> f64 {
    if x > -0.5 && x <= 0.5 { 1.0 } else { 0.0 }
}

/// Triangle (bilinear) kernel.
pub(crate) fn f_kernel_triangle(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 { 1.0 - a } else { 0.0 }
}

/// Catmull-Rom (bicubic) kernel.
pub(crate) fn f_kernel_catrom(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 {
        // Match Pillow's Resample.c Horner evaluation exactly.  Expanding
        // this polynomial with powi changes the last f64 bits for some
        // heterogeneous F samples, which becomes a visible f32 ULP after
        // ImagingResample stores the accumulated value.
        let t = 1.5f64.mul_add(a, -2.5);
        a.mul_add(t * a, 1.0)
    } else if a < 2.0 {
        let t = (a - 5.0).mul_add(a, 8.0);
        a.mul_add(t, -4.0) * -0.5
    } else {
        0.0
    }
}

/// Lanczos kernel with window `a`.
pub(crate) fn f_kernel_lanczos(x: f64, a: f64) -> f64 {
    // Resample.c uses `-a <= x && x < a`, so the negative support edge is
    // evaluated (and can produce a tiny signed coefficient) while the
    // positive edge is excluded.
    if x < -a || x >= a {
        return 0.0;
    }
    let sinc = |value: f64| {
        if value == 0.0 {
            1.0
        } else {
            let pix = value * std::f64::consts::PI;
            pillow_sin_f64(pix) / pix
        }
    };
    sinc(x) * sinc(x / a)
}

/// Hamming kernel.
pub(crate) fn f_kernel_hamming(x: f64) -> f64 {
    let x = x.abs();
    if x == 0.0 {
        // Resample.c has an exact-zero special case.  Values merely close to
        // zero still run through sin/cos; the float constants then leave a
        // visible residual (0.54f + 0.46f) in cancellation-sensitive F rows.
        return 1.0;
    }
    if x >= 1.0 {
        return 0.0;
    }
    // Keep the numeric F/I paths aligned with Pillow's Hamming windowed-sinc
    // resampler (Resample.c), including its sinc factor.
    let pix = std::f64::consts::PI * x;
    // Resample.c uses sincos and contracts the `0.46f * cos + 0.54f` window
    // expression; preserving those operations matters for exact f32
    // cancellation residuals.
    let (sin, cos) = pix.sin_cos();
    (sin / pix) * cos.mul_add(0.46_f32 as f64, 0.54_f32 as f64)
}

pub(crate) fn f_kernel_lanczos3(x: f64) -> f64 {
    f_kernel_lanczos(x, 3.0)
}

/// Returns the scalar kernel and support used to build Pillow-compatible
/// coefficients. SIMD uses this only for its control-plane coefficient table;
/// pixel accumulation remains in the SIMD adapter.
pub(crate) fn resample_kernel(filter: &ResampleFilter) -> (fn(f64) -> f64, f64) {
    match filter {
        ResampleFilter::Nearest => (f_kernel_box, 0.5),
        ResampleFilter::Bilinear => (f_kernel_triangle, 1.0),
        ResampleFilter::Bicubic => (f_kernel_catrom, 2.0),
        ResampleFilter::Lanczos => (f_kernel_lanczos3, 3.0),
        ResampleFilter::Box => (f_kernel_box, 0.5),
        ResampleFilter::Hamming => (f_kernel_hamming, 1.0),
    }
}
