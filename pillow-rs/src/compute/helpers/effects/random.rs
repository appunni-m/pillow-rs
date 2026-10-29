//! Process-global random sequence and effect control buffers.

use crate::error::PilError;
use std::sync::{Mutex, OnceLock};

// Pillow delegates these effects to process-global libc rand(). The pinned
// macOS/Darwin Pillow 12.2.0 oracle uses the Park-Miller sequence with the
// process-default state equivalent to srand(1). Keep its stream portable.
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

/// Runs a scalar control section on the shared process stream.
pub(crate) fn with_process_rng<T>(f: impl FnOnce(&mut DarwinRand) -> T) -> Result<T, PilError> {
    let mut rng = process_rng()
        .lock()
        .map_err(|_| PilError::InternalError("effect RNG lock poisoned".into()))?;
    Ok(f(&mut rng))
}

/// Builds Pillow's ordered scatter relocation map for GPU gather execution.
pub(crate) fn effect_spread_mapping(
    width: u32,
    height: u32,
    distance: u32,
) -> Result<Vec<u32>, PilError> {
    let indices = effect_spread_indices(width, height, distance)?;
    Ok(indices
        .into_iter()
        .map(|index| u32::try_from(index).unwrap_or(u32::MAX))
        .collect())
}

/// Builds a usize relocation map for native CPU sample gathering.
pub(crate) fn effect_spread_indices(
    width: u32,
    height: u32,
    distance: u32,
) -> Result<Vec<usize>, PilError> {
    let pixels = checked_pixels(width, height, "effect_spread")?;
    let mut mapping = (0..pixels).collect::<Vec<_>>();
    if distance == 0 || pixels == 0 {
        return Ok(mapping);
    }

    let distance_i32 = i32::try_from(distance)
        .map_err(|_| PilError::ValueError("effect_spread distance is too large".into()))?;
    let width_i32 = i32::try_from(width)
        .map_err(|_| PilError::ValueError("effect_spread width is too large".into()))?;
    let height_i32 = i32::try_from(height)
        .map_err(|_| PilError::ValueError("effect_spread height is too large".into()))?;
    let half_distance = distance_i32 / 2;

    if distance == 1 {
        effect_spread_advance_identity_rng(width, height)?;
        return Ok(mapping);
    }

    with_process_rng(|rng| {
        for y in 0..height_i32 {
            for x in 0..width_i32 {
                let source = y as usize * width as usize + x as usize;
                let destination_x = x + (rng.next() as i32 % distance_i32) - half_distance;
                let destination_y = y + (rng.next() as i32 % distance_i32) - half_distance;
                if destination_x >= 0
                    && destination_x < width_i32
                    && destination_y >= 0
                    && destination_y < height_i32
                {
                    let destination = destination_y as usize * width as usize
                        + destination_x as usize;
                    mapping[destination] = source;
                    mapping[source] = destination;
                }
            }
        }
    })?;
    Ok(mapping)
}

/// Advances the shared stream by the two draws consumed per pixel at distance one.
pub(crate) fn effect_spread_advance_identity_rng(
    width: u32,
    height: u32,
) -> Result<(), PilError> {
    let pixels = checked_pixels(width, height, "effect_spread")?;
    let steps = u64::try_from(pixels)
        .ok()
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| PilError::ValueError("effect_spread RNG step count overflow".into()))?;
    with_process_rng(|rng| rng.advance(steps))?;
    Ok(())
}

/// Generates Pillow's L-mode Gaussian-noise samples in stream order.
pub(crate) fn effect_noise_values(
    width: u32,
    height: u32,
    sigma: f64,
) -> Result<Vec<u8>, PilError> {
    let pixels = checked_pixels(width, height, "effect_noise")?;
    let mut values = vec![0u8; pixels];
    let sigma = f64::from(sigma as f32);
    with_process_rng(|rng| {
        const RAND_MAX_F64: f64 = 2_147_483_647.0;
        for pixel in &mut values {
            let (v1, radius) = loop {
                let v1 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
                let v2 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
                let radius = v1 * v1 + v2 * v2;
                if radius < 1.0 {
                    break (v1, radius);
                }
            };
            let factor = (-2.0 * radius.ln() / radius).sqrt();
            let this = factor * v1;
            let value = 128.0 + sigma * this;
            *pixel = if value <= 0.0 {
                0
            } else if value >= 255.0 {
                255
            } else {
                value as u8
            };
        }
    })?;
    Ok(values)
}

fn checked_pixels(width: u32, height: u32, operation: &str) -> Result<usize, PilError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError(format!("{operation} image dimensions overflow")))
}

