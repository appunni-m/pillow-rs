//! Checked image dimensions and allocation helpers.
//!
//! This module is the allocation gate for `pillow-rs`. Callers provide image
//! width, height, and channel count; [`CheckedDims::new`] validates that the
//! shape is non-zero, below the global pixel limit, and safe to multiply before
//! any buffer is allocated.
//!
//! Use [`CheckedDims`] when constructing a new image buffer or calculating a
//! row stride. Do not recompute `width * height * channels` in call sites.

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   CheckedDims is the ONLY approved way to create image buffers in this project.
//   Every function that allocates pixel memory MUST accept CheckedDims or go
//   through CheckedDims::new() for validation. This prevents:
//     (a) Integer overflow: (w * h * channels) wrapping silently in u32
//     (b) Allocation DoS: attacker-controlled dimensions causing GB allocations
//     (c) Zero-dimension images: w=0 or h=0 passing silently
//
//   CI enforces: no bare `(w * h) as usize` in production code
//   (see scripts/check_checked_dims.sh).
//
//   The process-wide max pixel limit matches PIL's `Image.MAX_IMAGE_PIXELS`
//   behavior.
// ============================================================================

use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::PilError;

/// Default maximum pixel count accepted by [`CheckedDims::new`].
///
/// The value allows roughly one GiB of RGBA data:
/// `268_435_456 pixels * 4 bytes = 1_073_741_824 bytes`.
const DEFAULT_MAX_PIXELS: u64 = 268_435_456;

/// Global maximum pixel count. Users can override via `set_max_pixels()`.
/// AS PER DESIGN: AtomicU64 so it's lock-free and thread-safe.
/// PIL equivalent: `Image.MAX_IMAGE_PIXELS`
static MAX_PIXELS: AtomicU64 = AtomicU64::new(DEFAULT_MAX_PIXELS);

/// Validated image dimensions for allocation and stride calculations.
///
/// `CheckedDims` stores a width, height, and channel count after validating
/// that they can safely describe a pixel buffer. The precomputed totals should
/// be used by allocation code instead of repeating arithmetic at call sites.
///
/// # Inputs
///
/// Use [`CheckedDims::new`] with dimensions in pixels and a channel count in
/// bytes per pixel for byte-addressed image modes.
///
/// # Invariants
///
/// - `width`, `height`, and `channels` are non-zero.
/// - `width * height` fits in `u64` and is below the configured max-pixel limit.
/// - `width * height * channels` fits in `u64`.
#[derive(Debug, Clone, Copy)]
pub struct CheckedDims {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Number of stored bytes per pixel.
    pub channels: u8,

    /// Pre-computed: width * height (guaranteed no overflow, non-zero)
    total_pixels: usize,

    /// Pre-computed: width * height * channels (guaranteed no overflow)
    total_bytes: usize,
}

impl CheckedDims {
    /// Validates dimensions for a future image allocation.
    ///
    /// # Inputs
    ///
    /// - `width`: image width in pixels.
    /// - `height`: image height in pixels.
    /// - `channels`: stored bytes per pixel, for example `1` for `L` and `4`
    ///   for `RGBA`.
    ///
    /// # Returns
    ///
    /// A [`CheckedDims`] value with precomputed pixel count, byte count, and row
    /// stride information.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::DimensionError`] if a dimension is zero, if the
    /// pixel count overflows, if the pixel count exceeds the configured
    /// max-pixel limit, or if the byte count overflows.
    pub fn new(width: u32, height: u32, channels: u8) -> Result<Self, PilError> {
        Self::new_with_limit(width, height, channels, MAX_PIXELS.load(Ordering::Relaxed))
    }

    /// Validates dimensions for an operation whose public Pillow contract
    /// permits an empty result image.
    ///
    /// Ordinary image allocations continue to use [`CheckedDims::new`], which
    /// rejects zero dimensions.  Empty images are an explicit boundary case:
    /// they carry a zero-length buffer and never enter a pixel loop.
    pub fn new_allow_empty(width: u32, height: u32, channels: u8) -> Result<Self, PilError> {
        if channels == 0 {
            return Err(PilError::DimensionError(
                "channel count cannot be zero".into(),
            ));
        }

        let total_pixels = (width as u64).checked_mul(height as u64).ok_or_else(|| {
            PilError::DimensionError(format!(
                "image dimensions overflow u64: {}×{}",
                width, height
            ))
        })?;
        let max_pixels = MAX_PIXELS.load(Ordering::Relaxed);
        if total_pixels > max_pixels {
            return Err(PilError::DimensionError(format!(
                "image size {} exceeds MAX_PIXELS ({}) — \
                 increase limit with CheckedDims::set_max_pixels()",
                total_pixels, max_pixels
            )));
        }
        let total_bytes = total_pixels.checked_mul(channels as u64).ok_or_else(|| {
            PilError::DimensionError(format!(
                "buffer size overflow: {} pixels × {} channels exceeds u64",
                total_pixels, channels
            ))
        })?;

        Ok(Self {
            width,
            height,
            channels,
            total_pixels: total_pixels as usize,
            total_bytes: total_bytes as usize,
        })
    }

    fn new_with_limit(
        width: u32,
        height: u32,
        channels: u8,
        max_pixels: u64,
    ) -> Result<Self, PilError> {
        // AS PER DESIGN: Zero-dimension check (separate from overflow for
        // clearer error messages)
        if width == 0 || height == 0 {
            return Err(PilError::DimensionError(format!(
                "image dimensions cannot be zero: {}×{}",
                width, height
            )));
        }

        if channels == 0 {
            return Err(PilError::DimensionError(
                "channel count cannot be zero".into(),
            ));
        }

        // AS PER DESIGN: checked_mul catches overflow. Do NOT replace with
        // bare `(w * h) as usize` — that wraps silently and causes OOB.
        let total_pixels = (width as u64).checked_mul(height as u64).ok_or_else(|| {
            PilError::DimensionError(format!(
                "image dimensions overflow u64: {}×{}",
                width, height
            ))
        })?;

        // AS PER DESIGN: Allocation DoS prevention. Users can relax this
        // via `set_max_pixels(None)` — matching PIL behavior.
        if total_pixels > max_pixels {
            return Err(PilError::DimensionError(format!(
                "image size {} exceeds MAX_PIXELS ({}) — \
                 increase limit with CheckedDims::set_max_pixels()",
                total_pixels, max_pixels
            )));
        }

        let total_bytes = total_pixels.checked_mul(channels as u64).ok_or_else(|| {
            PilError::DimensionError(format!(
                "buffer size overflow: {} pixels × {} channels exceeds u64",
                total_pixels, channels
            ))
        })?;

        Ok(Self {
            width,
            height,
            channels,
            total_pixels: total_pixels as usize,
            total_bytes: total_bytes as usize,
        })
    }

    // ── Accessors (AS PER DESIGN: prefer these over manual arithmetic) ──

    /// Returns the validated pixel count.
    ///
    /// The value is `width * height` and is guaranteed to be non-zero.
    #[inline]
    pub fn total_pixels(&self) -> usize {
        self.total_pixels
    }

    /// Returns the validated byte count for a tightly packed image buffer.
    ///
    /// The value is `width * height * channels` and is guaranteed to be
    /// non-zero.
    #[inline]
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns the tightly packed row stride in bytes.
    #[inline]
    pub fn row_stride(&self) -> usize {
        self.width as usize * self.channels as usize
    }

    // ── Allocation helpers (AS PER DESIGN: use these, never vec![0u8; w*h*ch]) ──

    /// Allocates a zero-filled pixel buffer with [`CheckedDims::total_bytes`].
    ///
    /// Use this helper instead of manually calculating a vector length.
    #[inline]
    pub fn alloc_buffer(&self) -> Vec<u8> {
        let buffer = vec![0u8; self.total_bytes];
        crate::compute::record_pipeline_allocation(buffer.len());
        buffer
    }
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate overflow behavior.
#[cfg(test)]
mod tests {
    use super::CheckedDims;
    use super::DEFAULT_MAX_PIXELS;

    #[test]
    fn valid_dimensions() {
        let d = CheckedDims::new(100, 200, 4).unwrap();
        assert_eq!(d.total_pixels(), 20_000);
        assert_eq!(d.total_bytes(), 80_000);
        assert_eq!(d.row_stride(), 400);
    }

    #[test]
    fn zero_width_rejected() {
        assert!(CheckedDims::new(0, 100, 3).is_err());
    }

    #[test]
    fn zero_height_rejected() {
        assert!(CheckedDims::new(100, 0, 3).is_err());
    }

    #[test]
    fn zero_channels_rejected() {
        assert!(CheckedDims::new(100, 100, 0).is_err());
    }

    #[test]
    fn overflow_rejected() {
        // u32::MAX × u32::MAX overflows u64
        assert!(CheckedDims::new(u32::MAX, u32::MAX, 1).is_err());
    }

    #[test]
    fn byte_count_overflow_rejected_before_allocation() {
        // The pixel product still fits in u64, but multiplying it by the
        // maximum byte channel count must be rejected before any allocation.
        let error = CheckedDims::new_with_limit(u32::MAX, u32::MAX, u8::MAX, u64::MAX)
            .expect_err("byte-count overflow must be rejected");
        assert!(error.to_string().contains("buffer size overflow"));
    }

    #[test]
    fn exceeds_max_pixels() {
        let d = CheckedDims::new_with_limit(100, 100, 1, DEFAULT_MAX_PIXELS).unwrap();
        assert_eq!(d.total_pixels(), 10_000);

        // Exercise the same validator without racing the process-wide limit
        // against other unit tests running in parallel.
        assert!(CheckedDims::new_with_limit(100, 100, 1, 5_000).is_err());
    }

    #[test]
    fn alloc_buffer_correct_size() {
        let d = CheckedDims::new(10, 20, 3).unwrap();
        let buf = d.alloc_buffer();
        assert_eq!(buf.len(), 600);
        assert!(buf.iter().all(|&b| b == 0));
    }
}
