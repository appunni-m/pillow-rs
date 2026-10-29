//! `ImageRgba32F` band semantics and context-selected architecture strategies.

// NO_PILLOW_SUPPORT: native ImageRgba32F band and mutation strategies extend Pillow.
use crate::compute::registry::support::BandIndexRule;
use super::super::helpers::extract_band_strategy::declare_strategies;

declare_strategies! {
    ImageRgba32F, BandIndexRule::ClampToNative { channels: 4 };
}

use super::super::helpers::put_data_strategy::declare_strategies as declare_data_strategies;

declare_data_strategies! {
    ImageRgba32F, 0, &[];
}

use super::super::helpers::put_alpha_strategy::declare_strategies as declare_alpha_strategies;

declare_alpha_strategies! {
    ImageRgba32F, ImageRgba8, 0, 4;
}

use super::super::helpers::put_alpha_data_strategy::declare_strategies as declare_alpha_data_strategies;

declare_alpha_data_strategies! {
    ImageRgba32F, ImageRgba8, 0, 4;
}
