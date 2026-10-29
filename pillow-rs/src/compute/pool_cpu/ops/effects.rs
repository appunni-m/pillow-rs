//! CPU effects and compatibility exports for historically grouped operations.

use crate::error::PilError;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer, RgbImage, RgbaImage,
};
use std::sync::{Mutex, OnceLock};

#[inline(always)]
fn relocate_samples<T: Copy, const CHANNELS: usize>(samples: &[T], mapping: &[u32]) -> Vec<T> {
    let mut output = Vec::with_capacity(samples.len());
    for &source_pixel in mapping {
        let start = source_pixel as usize * CHANNELS;
        output.extend_from_slice(&samples[start..start + CHANNELS]);
    }
    output
}

fn relocate_native_pixels(img: &DynamicImage, mapping: &[u32]) -> Result<DynamicImage, PilError> {
    if mapping.is_empty() {
        return Ok(img.clone());
    }
    let (width, height) = img.dimensions();
    let invalid_buffer = || PilError::ValueError("effect_spread buffer error".into());
    let output = match img {
        DynamicImage::ImageLuma8(image) => DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, relocate_samples::<_, 1>(image.as_raw(), mapping))
                .ok_or_else(invalid_buffer)?,
        ),
        DynamicImage::ImageLumaA8(image) => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(
                width,
                height,
                relocate_samples::<_, 2>(image.as_raw(), mapping),
            )
            .ok_or_else(invalid_buffer)?,
        ),
        DynamicImage::ImageRgb8(image) => DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, relocate_samples::<_, 3>(image.as_raw(), mapping))
                .ok_or_else(invalid_buffer)?,
        ),
        DynamicImage::ImageRgba8(image) => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, relocate_samples::<_, 4>(image.as_raw(), mapping))
                .ok_or_else(invalid_buffer)?,
        ),
        DynamicImage::ImageLuma16(image) => DynamicImage::ImageLuma16(
            ImageBuffer::from_raw(width, height, relocate_samples::<_, 1>(image.as_raw(), mapping))
                .ok_or_else(invalid_buffer)?,
        ),
        // NO_PILLOW_SUPPORT: EffectSpread also relocates the internal LA16,
        // RGB16, RGBA16, RGB32F, and RGBA32F samples without narrowing them.
        DynamicImage::ImageLumaA16(image) => DynamicImage::ImageLumaA16(
            ImageBuffer::from_raw(width, height, relocate_samples::<_, 2>(image.as_raw(), mapping))
                .ok_or_else(invalid_buffer)?,
        ),
        DynamicImage::ImageRgb16(image) => DynamicImage::ImageRgb16(
            ImageBuffer::from_raw(width, height, relocate_samples::<_, 3>(image.as_raw(), mapping))
                .ok_or_else(invalid_buffer)?,
        ),
        DynamicImage::ImageRgba16(image) => DynamicImage::ImageRgba16(
            ImageBuffer::from_raw(width, height, relocate_samples::<_, 4>(image.as_raw(), mapping))
                .ok_or_else(invalid_buffer)?,
        ),
        DynamicImage::ImageRgb32F(image) => DynamicImage::ImageRgb32F(
            crate::raster::Rgb32FImage::from_raw(
                width,
                height,
                relocate_samples::<_, 3>(image.as_raw(), mapping),
            )
            .ok_or_else(invalid_buffer)?,
        ),
        DynamicImage::ImageRgba32F(image) => DynamicImage::ImageRgba32F(
            crate::raster::Rgba32FImage::from_raw(
                width,
                height,
                relocate_samples::<_, 4>(image.as_raw(), mapping),
            )
            .ok_or_else(invalid_buffer)?,
        ),
    };
    Ok(output)
}

pub(super) fn apply_effect_rows<F>(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    transform: F,
) where
    F: Fn(usize, &mut [u8]),
{
    if bytes.is_empty() || width == 0 || height == 0 {
        return;
    }
    let stride = width.saturating_mul(channels);

    for (y, row) in bytes.chunks_exact_mut(stride).take(height).enumerate() {
        transform(y, row);
    }
}

#[inline]
pub(super) fn blend_row(first: &[u8], second: &[u8], output: &mut [u8], alpha: f64) {
    // ImagingBlend accepts float alpha. The selected Pillow build contracts
    // left + alpha * (right - left); weighted f64 terms change byte rounding.
    let alpha = alpha as f32;
    for ((destination, &left), &right) in output.iter_mut().zip(first.iter()).zip(second.iter()) {
        *destination = alpha
            .mul_add(f32::from(right) - f32::from(left), f32::from(left))
            .clamp(0.0, 255.0) as u8;
    }
}

// ── Darwin-compatible PRNG ───────────────────────────────────────────────
//
// Pillow delegates effect_spread/effect_noise randomness to process-global
// libc rand(). The pinned macOS/Darwin Pillow 12.2.0 oracle uses the
// Park-Miller sequence, whose process-default state is the same as srand(1).
// Keep the generator independent of libc so native and WASM builds reproduce
// the oracle without runtime FFI, while retaining the shared state and call
// consumption that the public APIs expose.

pub(crate) struct DarwinRand {
    state: u32,
}

impl Default for DarwinRand {
    fn default() -> Self {
        Self { state: 1 }
    }
}

impl DarwinRand {
    pub(crate) fn next(&mut self) -> u32 {
        const MULTIPLIER: u64 = 16_807;
        const MODULUS: u64 = 2_147_483_647;

        self.state = ((u64::from(self.state) * MULTIPLIER) % MODULUS) as u32;
        self.state
    }

    fn advance(&mut self, steps: u64) {
        const MULTIPLIER: u64 = 16_807;
        const MODULUS: u64 = 2_147_483_647;

        let mut exponent = steps;
        let mut factor = MULTIPLIER;
        let mut multiplier = 1u64;
        while exponent != 0 {
            if exponent & 1 != 0 {
                multiplier = (multiplier * factor) % MODULUS;
            }
            factor = (factor * factor) % MODULUS;
            exponent >>= 1;
        }
        self.state = ((u64::from(self.state) * multiplier) % MODULUS) as u32;
    }
}

static PROCESS_RNG: OnceLock<Mutex<DarwinRand>> = OnceLock::new();

fn process_rng() -> &'static Mutex<DarwinRand> {
    PROCESS_RNG.get_or_init(|| Mutex::new(DarwinRand::default()))
}

/// Run a backend's scalar RNG control section while preserving Pillow's
/// process-global random stream. Pixel consumers may batch the values into a
/// native vector kernel without taking a separate or reordered RNG stream.
pub(crate) fn with_process_rng<T>(f: impl FnOnce(&mut DarwinRand) -> T) -> Result<T, PilError> {
    let mut rng = process_rng()
        .lock()
        .map_err(|_| PilError::InternalError("effect RNG lock poisoned".into()))?;
    Ok(f(&mut rng))
}

// ── EffectSpread ──

pub fn op_effect_spread(img: &DynamicImage, distance: u32) -> Result<DynamicImage, PilError> {
    // Pillow 12.2.0 libImaging/Effects.c:117-159:
    // For image8 (L, P, 1): 1 byte per pixel, SPREAD(UINT8, image8)
    // For image32 (RGB, RGBA, etc): 4 bytes per pixel, SPREAD(INT32, image32)
    // Creates a new output image. For each pixel (x,y) in the input:
    //   Compute (xx,yy) = (x + rand()%d - d/2, y + rand()%d - d/2)
    //   If (xx,yy) is in bounds:
    //     output[yy][xx] = input[y][x]
    //     output[y][x] = input[yy][xx]
    //   Else:
    //     output[y][x] = input[y][x]
    // Input is NEVER modified; output is a new image.
    // Multiple pixels CAN map to the same (xx,yy); last write wins.
    // The C function consumes process-global rand() state. The existing
    // isolated generator below is not claimed as stochastic pixel parity;
    // only seed-independent contracts are exact until a principled oracle
    // replaces the historical fixture-selected seed.
    if distance == 0 {
        return Ok(img.clone());
    }
    // With distance one, each random offset is rand() % 1 == 0. Keep the
    // process-global stream identical by jumping over the same two draws per
    // pixel, then copy the native dynamic image once instead of building a
    // u32 relocation map and gathering every sample.
    if distance == 1 && img.width() != 0 && img.height() != 0 {
        i32::try_from(img.width())
            .map_err(|_| PilError::ValueError("effect_spread width is too large".into()))?;
        i32::try_from(img.height())
            .map_err(|_| PilError::ValueError("effect_spread height is too large".into()))?;
        effect_spread_advance_identity_rng(img.width(), img.height())?;
        return Ok(img.clone());
    }
    let mapping = effect_spread_mapping(img.width(), img.height(), distance)?;
    relocate_native_pixels(img, &mapping)
}

/// Build the exact per-pixel relocation map used by Pillow's spread effect.
/// The map is a control-plane representation of the C scatter loop: every
/// source pixel consumes two values from the process-global Park–Miller stream,
/// and each in-bounds pair updates both affected output locations from the
/// immutable input.  A GPU/other backend can then gather through this map
/// without reproducing the inherently order-dependent scatter writes.
pub(crate) fn effect_spread_mapping(
    width: u32,
    height: u32,
    distance: u32,
) -> Result<Vec<u32>, PilError> {
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or_else(|| PilError::ValueError("effect_spread image dimensions overflow".into()))?;
    let mut mapping = (0..pixels)
        .map(|index| u32::try_from(index).unwrap_or(u32::MAX))
        .collect::<Vec<_>>();
    if distance == 0 || pixels == 0 {
        return Ok(mapping);
    }
    let d = i32::try_from(distance)
        .map_err(|_| PilError::ValueError("effect_spread distance is too large".into()))?;
    let half_d = d / 2;
    let width_i32 = i32::try_from(width)
        .map_err(|_| PilError::ValueError("effect_spread width is too large".into()))?;
    let height_i32 = i32::try_from(height)
        .map_err(|_| PilError::ValueError("effect_spread height is too large".into()))?;
    if distance == 1 {
        let steps = u64::try_from(pixels)
            .ok()
            .and_then(|pixels| pixels.checked_mul(2))
            .ok_or_else(|| PilError::ValueError("effect_spread RNG step count overflow".into()))?;
        with_process_rng(|rng| rng.advance(steps))?;
        return Ok(mapping);
    }
    with_process_rng(|rng| {
        for y in 0..height_i32 {
            for x in 0..width_i32 {
                let source = (y * width_i32 + x) as usize;
                let xx = x + (rng.next() as i32 % d) - half_d;
                let yy = y + (rng.next() as i32 % d) - half_d;
                if xx >= 0 && xx < width_i32 && yy >= 0 && yy < height_i32 {
                    let destination = (yy * width_i32 + xx) as usize;
                    mapping[destination] = u32::try_from(source).unwrap_or(u32::MAX);
                    mapping[source] = u32::try_from(destination).unwrap_or(u32::MAX);
                }
            }
        }
    })?;
    Ok(mapping)
}

/// Advance the shared spread/noise stream exactly as a distance-one operation
/// does, without allocating an identity index map or visiting each pixel.
pub(crate) fn effect_spread_advance_identity_rng(width: u32, height: u32) -> Result<(), PilError> {
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("effect_spread image dimensions overflow".into()))?;
    let steps = u64::try_from(pixels)
        .ok()
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| PilError::ValueError("effect_spread RNG step count overflow".into()))?;
    with_process_rng(|rng| rng.advance(steps))?;
    Ok(())
}

// ── Paste ──

pub fn op_effect_noise(img: &DynamicImage, sigma: f64) -> Result<DynamicImage, PilError> {
    // Pillow 12.2.0 `src/libImaging/Effects.c:75-114` uses a polar
    // Box-Muller transform and always returns L mode. Its `nextok` flag is
    // never set, so every accepted pixel consumes one pair and discards the
    // second deviate.
    let (w, h) = (img.width(), img.height());
    let values = effect_noise_values(w, h, sigma)?;
    let out = GrayImage::from_raw(w, h, values)
        .ok_or_else(|| PilError::ValueError("effect_noise buffer error".into()))?;
    Ok(DynamicImage::ImageLuma8(out))
}

/// Generate the exact L-mode bytes for EffectNoise while consuming the shared
/// Pillow-compatible process RNG in source order.  Keeping this scalar control
/// section separate lets GPU encode the generated bytes as an auxiliary gather
/// buffer without inventing a second random stream.
pub(crate) fn effect_noise_values(
    width: u32,
    height: u32,
    sigma: f64,
) -> Result<Vec<u8>, PilError> {
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or_else(|| PilError::ValueError("effect_noise image dimensions overflow".into()))?;
    let mut values = vec![0u8; pixels];
    let sigma = f64::from(sigma as f32);
    with_process_rng(|rng| {
        // `_effect_noise` parses sigma with PyArg's `f` conversion before passing
        // it to ImagingEffectNoise, so round it to FLOAT32 once at the boundary.
        const RAND_MAX_F64: f64 = 2147483647.0;
        for pixel in &mut values {
            let (v1, radius) = loop {
                // Exact match to PIL:
                //   v1 = rand() * (2.0 / RAND_MAX) - 1.0;
                //   v2 = rand() * (2.0 / RAND_MAX) - 1.0;
                let v1 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
                let v2 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
                let radius = v1 * v1 + v2 * v2;
                if radius < 1.0 {
                    break (v1, radius);
                }
            };
            // factor = sqrt(-2.0 * log(radius) / radius)
            let factor = (-2.0 * radius.ln() / radius).sqrt();
            let this = factor * v1;
            // PIL: CLIP8(128 + sigma * this)
            // CLIP8: (v) <= 0 ? 0 : (v) >= 255.0 ? 255 : (UINT8)(v)
            // Cast truncates toward zero (no rounding).
            let v = 128.0 + sigma * this;
            *pixel = if v <= 0.0 {
                0
            } else if v >= 255.0 {
                255
            } else {
                v as u8
            };
        }
    })?;
    Ok(values)
}

pub(crate) use super::color3dlut::op_color3dlut;
pub(crate) use super::composite::{
    op_alpha_composite, op_alpha_composite_mirror, op_blend_module, op_composite_module, op_merge,
};
pub(crate) use super::eval::op_eval;
pub(crate) use super::paste::op_paste;
pub(crate) use super::put::{
    op_put_alpha, op_put_alpha_data, op_put_data, op_put_pixel, op_put_pixel_batch,
};
pub(crate) use super::transform::{affine_nearest_fixed, op_transform};

#[cfg(test)]
mod tests {
    use super::DarwinRand;

    #[test]
    fn darwin_rand_jump_ahead_matches_sequential_draws() {
        for steps in [0, 1, 2, 3, 16, 1_000_003] {
            let mut sequential = DarwinRand::default();
            let mut jumped = DarwinRand::default();
            for _ in 0..steps {
                sequential.next();
            }
            jumped.advance(steps);
            assert_eq!(jumped.next(), sequential.next(), "after {steps} draws");
        }
    }
}
