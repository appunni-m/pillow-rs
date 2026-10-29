//! `ImageRgb16` band semantics and context-selected architecture strategies.

// NO_PILLOW_SUPPORT: native ImageRgb16 band and mutation strategies extend Pillow.
use crate::compute::registry::support::BandIndexRule;
use super::super::helpers::extract_band_strategy::declare_strategies;

declare_strategies! {
    ImageRgb16, BandIndexRule::RgbWithOpaqueAlpha;
}

use super::super::helpers::put_data_strategy::declare_strategies as declare_data_strategies;

declare_data_strategies! {
    ImageRgb16, 0, &[];
}

use super::super::helpers::put_alpha_strategy::declare_strategies as declare_alpha_strategies;

declare_alpha_strategies! {
    ImageRgb16, ImageRgba8, 0, 4;
}

use super::super::helpers::put_alpha_data_strategy::declare_strategies as declare_alpha_data_strategies;

declare_alpha_data_strategies! {
    ImageRgb16, ImageRgba8, 0, 4;
}
