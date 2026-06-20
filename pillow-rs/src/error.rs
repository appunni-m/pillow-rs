// ============================================================================
// AS PER DESIGN — DO NOT REMOVE OR WEAKEN:
//   - Every error variant serves a specific PIL-compatible error category.
//   - New errors MUST be added as PilError variants, never as String or ad-hoc types.
//   - The `#[from]` impls ensure `?` propagation works seamlessly from
//     pillow_rs_image::ImageError and std::io::Error.
//   - CI enforces: no `Result<_, String>` anywhere in production code
//     (see scripts/lint.sh and scripts/check_error_types.sh).
// ============================================================================

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PilError {
    #[error("{0}")]
    IOError(String),

    #[error("{0}")]
    OsError(String),

    /// AS PER DESIGN: Replaces the old empty AssertionError. Every assertion
    /// error MUST carry a descriptive message explaining what invariant failed.
    #[error("assertion failed: {0}")]
    AssertionError(String),

    #[error("{0}")]
    IndexError(String),

    #[error("cannot identify image file '{0}'")]
    UnidentifiedImageError(String),

    #[error("{0}")]
    ValueError(String),

    #[error("{0}")]
    TypeError(String),

    #[error("image processing error: {0}")]
    ImageError(#[from] pillow_rs_image::ImageError),

    #[error("{0}")]
    NotImplementedError(String),

    #[error("unknown format: {0}")]
    UnknownFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

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
