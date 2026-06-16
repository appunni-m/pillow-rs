//! SIMD-accelerated operation implementations.
//!
//! Platform dispatch via compile-time cfg:
//! - x86/x86_64 → `x86.rs` (SSE4.1 + AVX2 stubs, delegates to scalar)
//! - aarch64 → `arm.rs` (NEON stubs, delegates to scalar)
//! - fallback → `scalar.rs` (portable, auto-vectorization friendly)
//!
//! `scalar` is always compiled as the reference + fallback.

pub mod adapters;
mod scalar; // always available — reference implementation // SIMD → registry adapter wrappers

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use x86::*;

#[cfg(target_arch = "aarch64")]
mod arm;
#[cfg(target_arch = "aarch64")]
pub use arm::*;

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
pub use scalar::*;
