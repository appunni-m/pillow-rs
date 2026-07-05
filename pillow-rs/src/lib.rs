// AS PER DESIGN — DO NOT REMOVE:
//   These allows are explicitly deferred. pillow-rs must eventually:
//   1. Replace unwrap() with expect() explaining invariants
//   2. Either use InfallibleExt::because() or Result propagation
//   3. Replace .map().unwrap_or() with .map_or()
//   4. Remove redundant clones
//   5. Replace manual loop indexing with iter()/enumerate()
//   Tracked at CODEBASE_AUDIT.md Fix 2. Remove each when addressed.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::needless_range_loop)]
// PIL API compatibility — many functions have 8+ parameters matching PIL signatures
#![allow(clippy::too_many_arguments)]

#[cfg(feature = "parallel")]
use rayon as _;

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE THESE MODULES:
//   Each module below is a foundational piece of the systemic fix architecture
//   (see SYSTEMIC_FIXES.md). Removing any of them breaks the guarantees enforced
//   by CI and creates entire classes of bugs.
//
//   - checked_dims:  Only way to allocate image buffers (no overflow, no DoS)
//   - pixel_format:  Named enum replacing bare 0/1/2/3 mode codes
//   - par:           Approved parallelization macros (no raw rayon; native default)
//   - image_utils:   Canonical buffer conversion (no duplicate copies)
//   - compute::op_def: Declarative op registration (no parallel match arms)
// ============================================================================

pub mod bitmap_font;
pub mod checked_dims;
pub mod color;
pub mod compute;
pub mod draw;
pub mod error;
pub mod font;
pub mod format;
pub mod formats;
pub mod image;
pub mod image_utils;
pub mod infallible;
pub mod ops;
#[cfg(feature = "parallel")]
pub mod par;
pub mod pipeline;
pub mod pixel_format;

pub use crate::checked_dims::CheckedDims;
pub use crate::image::Image;
pub use draw::Draw;
pub use error::PilError;
pub use font::Font;
pub use infallible::InfallibleExt;
pub use pixel_format::PixelFormat;
