//! Cross-backend control data for Pillow-compatible effects.

mod random;

pub(crate) use random::{
    DarwinRand, effect_noise_values, effect_spread_advance_identity_rng, effect_spread_indices,
    effect_spread_mapping, with_process_rng,
};
