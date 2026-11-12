mod dimensions;
pub(crate) mod pad_dimensions;
pub(crate) mod pad_hidden_lanes;
pub(crate) mod pad_native_vector;
pub(crate) mod pad_samples;
pub(crate) mod pad_strategies;

pub(super) use dimensions::{contain_dimensions, cover_dimensions, fit_box};
