//! Eight-lane bilinear interpolation shared by native Rotate kernels.

use wide::f64x8;

pub(crate) const LANES: usize = 8;

#[derive(Clone, Copy, Default)]
pub(crate) struct RotateSample {
    pub(crate) valid: bool,
    pub(crate) x0: usize,
    pub(crate) x1: usize,
    pub(crate) y0: usize,
    pub(crate) y1: usize,
    pub(crate) fx: f64,
    pub(crate) fy: f64,
}

pub(crate) fn sample_coordinates(
    affine: [f64; 6],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
) -> RotateSample {
    let [a, b, c, d, e, f] = affine;
    let source_x = a.mul_add(x as f64 + 0.5, b * (y as f64 + 0.5)) + c;
    let source_y = d.mul_add(x as f64 + 0.5, e * (y as f64 + 0.5)) + f;
    if source_x < 0.0 || source_x >= width as f64 || source_y < 0.0 || source_y >= height as f64 {
        return RotateSample::default();
    }
    let source_x = (source_x - 0.5).clamp(0.0, width as f64 - 1.0);
    let source_y = (source_y - 0.5).clamp(0.0, height as f64 - 1.0);
    let (x0, y0) = (source_x.floor() as usize, source_y.floor() as usize);
    RotateSample {
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
pub(crate) fn fused_mul_add(a: f64x8, b: f64x8, c: f64x8) -> f64x8 {
    #[cfg(any(target_feature = "fma", all(target_arch = "aarch64", target_feature = "neon")))]
    {
        a.mul_add(b, c)
    }
    #[cfg(not(any(target_feature = "fma", all(target_arch = "aarch64", target_feature = "neon"))))]
    {
        let (a, b, c) = (a.to_array(), b.to_array(), c.to_array());
        f64x8::new(std::array::from_fn(|lane| a[lane].mul_add(b[lane], c[lane])))
    }
}

pub(crate) fn bilinear_vector(
    neighbors: [f64x8; 4],
    fx: f64x8,
    fy: f64x8,
) -> f64x8 {
    let top = fused_mul_add(neighbors[1] - neighbors[0], fx, neighbors[0]);
    let bottom = fused_mul_add(neighbors[3] - neighbors[2], fx, neighbors[2]);
    fused_mul_add(bottom - top, fy, top)
}

pub(crate) fn gather_neighborhood(
    source: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    fill: (u8, u8, u8, u8),
    samples: &[RotateSample; LANES],
    alpha_channel: Option<usize>,
) -> [f64x8; 4] {
    let fill_value = fill_sample(fill, channels, channel) as f64;
    let mut values = [[fill_value; LANES]; 4];
    let mut alphas = [[0.0; LANES]; 4];
    let premultiply = alpha_channel.is_some_and(|alpha| alpha != channel);
    for (lane, sample) in samples.iter().enumerate() {
        if !sample.valid {
            if let Some(alpha_channel) = alpha_channel {
                let alpha_fill = fill_sample(fill, channels, alpha_channel) as f64;
                for neighbor in &mut alphas {
                    neighbor[lane] = alpha_fill;
                }
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
            let premultiplied = (f64x8::new(values[neighbor])
                * f64x8::new(alphas[neighbor])
                / divisor
                + bias)
                .to_array();
            for (lane, value) in premultiplied.into_iter().enumerate() {
                if samples[lane].valid {
                    values[neighbor][lane] = (value as u8) as f64;
                }
            }
        }
    }
    values.map(f64x8::new)
}

pub(crate) fn unpremultiply_vector(
    premultiplied: f64x8,
    alpha: [u8; LANES],
) -> [u8; LANES] {
    let premultiplied_bytes = premultiplied.to_array().map(|value| value as u8);
    let alpha_vector = f64x8::new(alpha.map(f64::from));
    let restored = (f64x8::new(premultiplied_bytes.map(f64::from)) * f64x8::splat(255.0)
        / alpha_vector.max(f64x8::splat(1.0)))
        .to_array();
    std::array::from_fn(|lane| {
        if alpha[lane] == 0 {
            premultiplied_bytes[lane]
        } else {
            restored[lane] as u8
        }
    })
}

fn fill_sample(fill: (u8, u8, u8, u8), channels: usize, channel: usize) -> u8 {
    if channels == 2 && channel == 1 {
        fill.3
    } else {
        match channel {
            0 => fill.0,
            1 => fill.1,
            2 => fill.2,
            _ => fill.3,
        }
    }
}
