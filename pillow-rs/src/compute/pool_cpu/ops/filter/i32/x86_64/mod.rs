//! x86_64 baseline strategy for exact I-mode filtering.

mod filter5x5_impl;

pub(super) use filter5x5_impl::execute;
