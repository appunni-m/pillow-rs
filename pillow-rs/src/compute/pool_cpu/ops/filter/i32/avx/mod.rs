//! x86 AVX/FMA I-mode Filter5x5 strategy.

mod filter5x5_impl;

pub(super) use filter5x5_impl::execute;
