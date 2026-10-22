//! Scalar and vector interpolation shared by SIMD Rotate and Transform.

use super::super::SIMD_F64_LANES;
use super::SimdRotateSample;
use wide::f64x8;

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn simd_rotate_sample_coordinates(
    affine: [f64; 6],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
) -> SimdRotateSample {
    let [a, b, c, d, e, f] = affine;
    // Pillow evaluates the center mapping with a contracted multiply/add,
    // rejects samples outside the image, then shifts every mode by half a pixel.
    let source_x = a.mul_add(x as f64 + 0.5, b * (y as f64 + 0.5)) + c;
    let source_y = d.mul_add(x as f64 + 0.5, e * (y as f64 + 0.5)) + f;
    if source_x < 0.0 || source_x >= width as f64 || source_y < 0.0 || source_y >= height as f64 {
        return SimdRotateSample::default();
    }
    let source_x = (source_x - 0.5).clamp(0.0, width as f64 - 1.0);
    let source_y = (source_y - 0.5).clamp(0.0, height as f64 - 1.0);
    let x0 = source_x.floor() as usize;
    let y0 = source_y.floor() as usize;
    SimdRotateSample {
        valid: true,
        x0,
        x1: (x0 + 1).min(width - 1),
        y0,
        y1: (y0 + 1).min(height - 1),
        fx: source_x - x0 as f64,
        fy: source_y - y0 as f64,
    }
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn bilinear_rotate_scalar(
    p00: f64,
    p10: f64,
    p01: f64,
    p11: f64,
    fx: f64,
    fy: f64,
) -> f64 {
    let top = (p10 - p00).mul_add(fx, p00);
    let bottom = (p11 - p01).mul_add(fx, p01);
    (bottom - top).mul_add(fy, top)
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn rotate_fused_mul_add(
    a: f64x8,
    b: f64x8,
    c: f64x8,
) -> f64x8 {
    // wide's mul_add uses two roundings without hardware FMA. Preserve the
    // reference's single rounding on those targets as well.
    #[cfg(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        a.mul_add(b, c)
    }
    #[cfg(not(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let (a, b, c) = (a.to_array(), b.to_array(), c.to_array());
        f64x8::new(std::array::from_fn(|lane| {
            a[lane].mul_add(b[lane], c[lane])
        }))
    }
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn bilinear_rotate_vector(
    p00: f64x8,
    p10: f64x8,
    p01: f64x8,
    p11: f64x8,
    fx: f64x8,
    fy: f64x8,
) -> f64x8 {
    let top = rotate_fused_mul_add(p10 - p00, fx, p00);
    let bottom = rotate_fused_mul_add(p11 - p01, fx, p01);
    rotate_fused_mul_add(bottom - top, fy, top)
}

/// Gather a channel's four bilinear neighbors in native byte storage.
///
/// `alpha_channel` is present only for a color channel in LA/RGBA. Source
/// samples are premultiplied with a vector multiply before the required
/// per-lane byte rounding. Invalid lanes retain the unpremultiplied fill
/// value, matching Pillow's fill-then-unpremultiply sequence.
#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn gather_rotate_neighborhood_vectors(
    source: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    fill: (u8, u8, u8, u8),
    samples: &[SimdRotateSample; SIMD_F64_LANES],
    alpha_channel: Option<usize>,
) -> [f64x8; 4] {
    let fill_value = super::rotate_fill_sample(fill, channels, channel) as f64;
    let mut values = [[fill_value; SIMD_F64_LANES]; 4];
    let mut alphas = [[0.0; SIMD_F64_LANES]; 4];
    let premultiply = alpha_channel.is_some_and(|alpha| alpha != channel);
    for (lane, sample) in samples.iter().enumerate() {
        if !sample.valid {
            if let Some(alpha_channel) = alpha_channel {
                let alpha_fill = super::rotate_fill_sample(fill, channels, alpha_channel) as f64;
                alphas[0][lane] = alpha_fill;
                alphas[1][lane] = alpha_fill;
                alphas[2][lane] = alpha_fill;
                alphas[3][lane] = alpha_fill;
            }
            continue;
        }
        let coordinates = [
            (sample.x0, sample.y0),
            (sample.x1, sample.y0),
            (sample.x0, sample.y1),
            (sample.x1, sample.y1),
        ];
        for (neighbor, &(source_x, source_y)) in coordinates.iter().enumerate() {
            let index = (source_y * width + source_x) * channels;
            values[neighbor][lane] = source[index + channel] as f64;
            if let Some(alpha_channel) = alpha_channel {
                alphas[neighbor][lane] = source[index + alpha_channel] as f64;
            }
        }
    }
    if premultiply {
        let divisor = f64x8::splat(255.0);
        let bias = f64x8::splat(0.5);
        for neighbor in 0..4 {
            let premultiplied =
                (f64x8::new(values[neighbor]) * f64x8::new(alphas[neighbor]) / divisor + bias)
                    .to_array();
            for (lane, value) in premultiplied.into_iter().enumerate() {
                if samples[lane].valid {
                    values[neighbor][lane] = (value as u8) as f64;
                }
            }
        }
    }
    [
        f64x8::new(values[0]),
        f64x8::new(values[1]),
        f64x8::new(values[2]),
        f64x8::new(values[3]),
    ]
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn rotate_unpremultiply_vector(
    premultiplied: f64x8,
    alpha: [u8; SIMD_F64_LANES],
) -> [u8; SIMD_F64_LANES] {
    let premultiplied_bytes = premultiplied.to_array().map(|value| value as u8);
    let alpha_vector = f64x8::new(alpha.map(f64::from));
    let safe_alpha = alpha_vector.max(f64x8::splat(1.0));
    let restored = (f64x8::new(premultiplied_bytes.map(f64::from)) * f64x8::splat(255.0)
        / safe_alpha)
        .to_array();
    std::array::from_fn(|lane| {
        if alpha[lane] == 0 {
            premultiplied_bytes[lane]
        } else {
            restored[lane] as u8
        }
    })
}
