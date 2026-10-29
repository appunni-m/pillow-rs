//! Mode-aware CPU operation families.
//!
//! TODO(pillow-rs#3ptb): add these remaining active owners from
//! `docs/IMPLEMENTATION_STRUCTURE.md`: `resize`, `thumbnail`, `color`, `filter`,
//! `geometry`, `chops`, `enhance`, `paste`, `composite`, `transform`, and `draw`.
//! Rewire their registry and direct callers without importing the archive.
//! Reduce's remaining architecture and later Parallel CPU follow-ups are
//! documented beside its mode strategies.

pub(crate) mod color3dlut;
pub(crate) mod crop;
pub(crate) mod effects;
pub(crate) mod eval;
pub(crate) mod grayscale;
pub(crate) mod imageops;
pub(crate) mod put;
pub(crate) mod reduce;
pub(crate) mod rotate;
pub(crate) mod transpose;
