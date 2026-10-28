//! CPU operation facade.
//!
//! Families are migrated out of `legacy/ops` as their mode-aware entry points
//! are completed. Keep the module names stable at this boundary so the pool
//! and registry do not depend on implementation file placement.

#[path = "../legacy/ops/chops.rs"]
pub(crate) mod chops;
#[path = "../legacy/ops/color.rs"]
pub(crate) mod color;
#[path = "../legacy/ops/color3dlut.rs"]
pub(crate) mod color3dlut;
#[path = "../legacy/ops/composite/mod.rs"]
pub(crate) mod composite;
pub(crate) mod crop;
#[path = "../legacy/ops/draw/mod.rs"]
pub(crate) mod draw;
#[path = "../legacy/ops/effects.rs"]
pub(crate) mod effects;
#[path = "../legacy/ops/enhance.rs"]
pub(crate) mod enhance;
#[path = "../legacy/ops/eval/mod.rs"]
pub(crate) mod eval;
#[path = "../legacy/ops/filter/mod.rs"]
pub(crate) mod filter;
pub(crate) mod geometry;
pub(crate) mod grayscale;
#[path = "../legacy/ops/imageops/mod.rs"]
pub(crate) mod imageops;
#[path = "../legacy/ops/paste/mod.rs"]
pub(crate) mod paste;
#[path = "../legacy/ops/put.rs"]
pub(crate) mod put;
pub(crate) mod reduce;
pub(crate) mod resize;
pub(crate) mod rotate;
pub(crate) mod thumbnail;
#[path = "../legacy/ops/transform/mod.rs"]
pub(crate) mod transform;
pub(crate) mod transpose;
