//! CPU operation modules. Each family owns its mode routing and helpers.

// TODO(pillow-rs#3ptb): Audit every registered family below for a complete
// operation-level Mode support declaration and composed backend/architecture
// strategies. Add family-specific TODOs at the owning modules as each path is
// reviewed; module presence alone does not prove the strategy migration.

pub(crate) mod chops;
pub(crate) mod color;
pub(crate) mod color3dlut;
pub(crate) mod composite;
pub(crate) mod crop;
pub(crate) mod draw;
pub(crate) mod effects;
pub(crate) mod expand;
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
