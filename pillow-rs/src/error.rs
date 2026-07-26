//! Pillow-compatible error categories for core operations.
//!
//! The Python and JavaScript bindings translate these variants into their host
//! exceptions. Core code should return a specific variant instead of using
//! ad-hoc `String` errors so callers can preserve Pillow-like error categories.

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE OR WEAKEN:
//   - Every error variant serves a specific PIL-compatible error category.
//   - New errors MUST be added as PilError variants, never as String or ad-hoc types.
//   - The `#[from]` impls ensure `?` propagation works seamlessly from
//     image_slash_star::ImageError and std::io::Error.
//   - CI enforces: no `Result<_, String>` anywhere in production code
//     (see scripts/lint.sh and scripts/check_error_types.sh).
// ============================================================================

use std::sync::Arc;

use thiserror::Error;

/// Error type returned by fallible `pillow-rs` core APIs.
///
/// Variants intentionally mirror common Pillow/Python exception categories
/// where the Rust core needs to preserve user-visible behavior through
/// bindings.
#[derive(Error, Debug, Clone)]
pub enum PilError {
    /// Pillow-style I/O error message.
    #[error("{0}")]
    IOError(String),

    /// Operating-system error message.
    #[error("{0}")]
    OsError(String),

    /// AS PER DESIGN: Replaces the old empty AssertionError. Every assertion
    /// error MUST carry a descriptive message explaining what invariant failed.
    #[error("assertion failed: {0}")]
    AssertionError(String),

    /// Index lookup failed, usually for image coordinates, bands, or palette entries.
    #[error("{0}")]
    IndexError(String),

    /// Mapping/key lookup failed, used for Pillow paths that raise `KeyError`.
    #[error("{0}")]
    KeyError(String),

    /// Input bytes could not be identified as a supported image format.
    #[error("cannot identify image file '{0}'")]
    UnidentifiedImageError(String),

    /// Input value is syntactically valid Rust but invalid for the Pillow operation.
    #[error("{0}")]
    ValueError(String),

    /// Input bytes do not follow the syntax required by a legacy Pillow format.
    #[error("{0}")]
    SyntaxError(String),

    /// Input type or mode is incompatible with the requested Pillow operation.
    #[error("{0}")]
    TypeError(String),

    /// Pillow-style internal system error surfaced by legacy C extension paths.
    #[error("{0}")]
    SystemError(String),

    /// Error propagated from the underlying image buffer or codec crate.
    #[error("image processing error: {0}")]
    ImageError(#[from] image_slash_star::ImageError),

    /// Pillow-compatible placeholder for APIs that are intentionally incomplete.
    #[error("{0}")]
    NotImplementedError(String),

    /// Image format name or extension is unknown to the format registry.
    #[error("unknown format: {0}")]
    UnknownFormat(String),

    /// Standard library I/O error propagated through core format helpers.
    #[error("IO error: {0}")]
    Io(Arc<std::io::Error>),

    // ============================================================================
    // AS PER DESIGN — DO NOT REMOVE:
    //   Below variants were added per the SYSTEMIC_FIXES.md audit to eliminate
    //   bare `String` error types and dead error variants. Every domain gets a
    //   named, descriptive variant so callers can match on error kind.
    // ============================================================================
    /// Palette-specific errors (color indexing, palette size mismatches, etc.)
    #[error("palette error: {0}")]
    PaletteError(String),

    /// Internal invariant violation — should never happen in correct usage.
    /// Carries a message explaining which invariant was broken.
    #[error("internal error: {0}")]
    InternalError(String),

    /// Dimension validation failure (overflow, exceeds MAX_PIXELS, zero-dimension).
    /// Always use CheckedDims::new() to produce these — never construct directly.
    #[error("invalid dimensions: {0}")]
    DimensionError(String),
}

impl From<std::io::Error> for PilError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(Arc::new(error))
    }
}
