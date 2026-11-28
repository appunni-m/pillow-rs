//! x86 coefficient filters matching the non-FMA Pillow build.

use crate::ops::pil_resize::filters::{kernel_box, kernel_lanczos3, kernel_triangle};
use crate::pipeline::ResampleFilter;

/// Select coefficient functions for the x86 Pillow resampler.
// VERIFY: source=reviewed@2026-11-28; parity=passed@2026-11-28; benchmark=pending
pub(crate) fn filter_from_resample(filter: ResampleFilter) -> (fn(f64) -> f64, f64) {
    match filter {
        ResampleFilter::Nearest | ResampleFilter::Box => (kernel_box, 0.5),
        ResampleFilter::Bilinear => (kernel_triangle, 1.0),
        ResampleFilter::Bicubic => (kernel_bicubic, 2.0),
        ResampleFilter::Lanczos => (kernel_lanczos3, 3.0),
        ResampleFilter::Hamming => (kernel_hamming, 1.0),
        ResampleFilter::Mks2013 => (kernel_mks2013, 2.5),
        ResampleFilter::Mks2021 => (kernel_mks2021, 4.5),
    }
}

// VERIFY: source=reviewed@2026-11-28; parity=passed@2026-11-28; benchmark=pending
fn kernel_bicubic(x: f64) -> f64 {
    let x = x.abs();
    if x < 1.0 {
        let first = std::hint::black_box(1.5 * x) - 2.5;
        let second = std::hint::black_box(first * x);
        std::hint::black_box(second * x) + 1.0
    } else if x < 2.0 {
        let first = std::hint::black_box((x - 5.0) * x) + 8.0;
        let second = std::hint::black_box(first * x) - 4.0;
        std::hint::black_box(second * -0.5)
    } else {
        0.0
    }
}

// VERIFY: source=reviewed@2026-11-28; parity=passed@2026-11-28; benchmark=pending
fn kernel_hamming(x: f64) -> f64 {
    let x = x.abs();
    if x == 0.0 {
        return 1.0;
    }
    if x >= 1.0 {
        return 0.0;
    }
    let pixel = x * std::f64::consts::PI;
    let window = std::hint::black_box(0.46_f32 as f64 * pixel.cos()) + 0.54_f32 as f64;
    pixel.sin() / pixel * window
}

// VERIFY: source=reviewed@2026-11-28; parity=passed@2026-11-28; benchmark=pending
fn kernel_mks2013(x: f64) -> f64 {
    let x = x.abs();
    if x < 0.5 {
        17.0 / 16.0 - std::hint::black_box(7.0 / 4.0 * x.powf(2.0))
    } else if x < 1.5 {
        (1.0 - x) * (7.0 / 4.0 - x)
    } else if x < 2.5 {
        -1.0 / 8.0 * (x - 5.0 / 2.0).powf(2.0)
    } else {
        0.0
    }
}

// VERIFY: source=reviewed@2026-11-28; parity=passed@2026-11-28; benchmark=pending
fn kernel_mks2021(x: f64) -> f64 {
    let x = x.abs();
    if x < 0.5 {
        577.0 / 576.0 - std::hint::black_box(239.0 / 144.0 * x.powf(2.0))
    } else if x < 1.5 {
        35.0 / 36.0 * (x - 1.0) * (x - 239.0 / 140.0)
    } else if x < 2.5 {
        1.0 / 6.0 * (x - 2.0) * (65.0 / 24.0 - x)
    } else if x < 3.5 {
        1.0 / 36.0 * (x - 3.0) * (x - 15.0 / 4.0)
    } else if x < 4.5 {
        -1.0 / 288.0 * (x - 9.0 / 2.0).powf(2.0)
    } else {
        0.0
    }
}
