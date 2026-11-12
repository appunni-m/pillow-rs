//! `ImageRgb32F` band semantics and context-selected architecture strategies.

// NO_PILLOW_SUPPORT: native ImageRgb32F band and mutation strategies extend Pillow.
use super::super::helpers::extract_band_strategy::declare_strategies;
use crate::compute::registry::support::BandIndexRule;

declare_strategies! {
    ImageRgb32F, BandIndexRule::RgbWithOpaqueAlpha;
}

use super::super::helpers::put_data_strategy::declare_strategies as declare_data_strategies;

declare_data_strategies! {
    ImageRgb32F, 0, &[];
}

use super::super::helpers::put_alpha_strategy::declare_strategies as declare_alpha_strategies;

declare_alpha_strategies! {
    ImageRgb32F, ImageRgba32F, 0, 4;
}

use super::super::helpers::put_alpha_data_strategy::declare_strategies as declare_alpha_data_strategies;

declare_alpha_data_strategies! {
    ImageRgb32F, ImageRgba32F, 0, 4;
}
super::super::helpers::scalar_write_support::declare_scalar_write_support!(
    ImageRgb32F,
    true,
    false,
    false,
);
