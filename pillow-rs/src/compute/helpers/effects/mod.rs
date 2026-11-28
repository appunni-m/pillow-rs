//! Cross-backend control data for Pillow-compatible effects.

mod random;

#[cfg(feature = "gpu")]
pub(crate) use random::effect_spread_mapping;
pub(crate) use random::{
    DarwinRand, effect_noise_values, effect_spread_advance_identity_rng, effect_spread_indices,
    with_process_rng,
};
