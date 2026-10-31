//! Mode-aware CPU operation families.
//!
//! TODO(pillow-rs#3ptb): finish Convert and route Sharpness's remaining
//! compatibility callers through its exact-mode prepared architecture
//! strategies. Sharpness now has per-mode Default and native-byte SSE2, AVX,
//! AVX2, AVX+FMA, AVX2+FMA, NEON, and Wasm SIMD128 implementations; Contrast
//! and ColorSaturation have typed strategies for their queued modes. Eager
//! conversion modes and full support review remain pending. Preserve direct
//! eager conversions before retiring the remaining Pillow-string routes.
//! Paste now has exhaustive mode strategies with Default and shared vector
//! routes; its archived architecture-tuned NEON blends and RGB/RGBA shuffles
//! remain TODOs in the Paste vector helper. Add the remaining owners from
//! `docs/IMPLEMENTATION_STRUCTURE.md` for `transform` and `draw`; migrate
//! Merge, BlendModule, and CompositeModule into the Composite owner.
//! Complete current owners and rewire every registry and direct caller without
//! importing the archive.
//! Resize has exhaustive prepared support and exact-mode dispatch for all 25
//! identities, byte nearest strategies for all 14 native byte modes, filtered
//! Luma8 SSE2/NEON/Wasm SIMD128 strategies, I32/F32 filtered specializations, four I;16 vector
//! strategies, and native typed Default paths for the internal high-depth
//! modes. Remaining byte filtered/boxed specializations, compatibility caller
//! routing, and Rayon extraction are tracked beside the Resize owner.
//! Reduce's remaining architecture and later Parallel CPU follow-ups are
//! documented beside its mode strategies.

pub(crate) mod color3dlut;
pub(crate) mod color;
pub(crate) mod composite;
pub(crate) mod analysis;
pub(crate) mod chops;
pub(crate) mod crop;
pub(crate) mod effects;
pub(crate) mod enhance;
pub(crate) mod eval;
pub(crate) mod filter;
pub(crate) mod geometry;
pub(crate) mod grayscale;
pub(crate) mod imageops;
pub(crate) mod put;
pub(crate) mod paste;
pub(crate) mod reduce;
pub(crate) mod resize;
pub(crate) mod rotate;
pub(crate) mod transpose;
pub(crate) mod thumbnail;
