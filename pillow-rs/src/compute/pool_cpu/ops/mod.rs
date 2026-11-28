//! Exact-mode operation families for serial CPU execution.
// TODO(pillow-rs#3ptb): finish the remaining Stage 5 source audit. The 26-file
// explicit-instruction sweep has source crosswalks for support entries, target
// gates, prepared selection, and execution arms; this does not prove compiler
// reachability or emitted instructions. Native AVX-512F leaves cover F32/I32
// and four-byte Reduce, exact four-byte nearest Resize strategies, native-byte
// Invert, F32/I32 rank filters, and raw four-byte axis-swapping Transpose.
// Continue adding suitable AVX-512F-only paths
// with exact-mode kernels, guards, parity inputs, and benchmark workloads;
// `pillow-rs#fvsp` tracks the mode/operation review. AVX-512BW/VBMI-only
// kernels are not AVX-512F strategies. AVX-512F leaves use function-level
// target features; crate-wide `+avx512f` is rejected. Cross-target builds,
// runtime receipts, parity, and latency remain open in
// docs/CPU_REFACTOR_PLAN.md and docs/CPU_REFACTOR_INVENTORY.md.

pub(crate) mod analysis;
pub(crate) mod chops;
pub(crate) mod color;
pub(crate) mod color3dlut;
pub(crate) mod composite;
pub(crate) mod crop;
pub(crate) mod draw;
pub(crate) mod effects;
pub(crate) mod enhance;
pub(crate) mod eval;
pub(crate) mod filter;
pub(crate) mod geometry;
pub(crate) mod grayscale;
pub(crate) mod imageops;
pub(crate) mod paste;
pub(crate) mod put;
pub(crate) mod reduce;
pub(crate) mod resize;
pub(crate) mod rotate;
pub(crate) mod thumbnail;
pub(crate) mod transform;
pub(crate) mod transpose;
