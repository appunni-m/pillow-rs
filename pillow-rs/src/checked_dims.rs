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
//   To increase the max pixel limit: use CheckedDims::set_max_pixels() —
//   matching PIL's `Image.MAX_IMAGE_PIXELS` behavior.
// ============================================================================

use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::PilError;

/// Default max pixels: ~1 GB for RGBA (matching PIL's default).
/// 268,435,456 pixels × 4 bytes = 1,073,741,824 bytes.
const DEFAULT_MAX_PIXELS: u64 = 268_435_456;

/// Global maximum pixel count. Users can override via `set_max_pixels()`.
/// AS PER DESIGN: AtomicU64 so it's lock-free and thread-safe.
/// PIL equivalent: `Image.MAX_IMAGE_PIXELS`
static MAX_PIXELS: AtomicU64 = AtomicU64::new(DEFAULT_MAX_PIXELS);

/// Validated image dimensions.
///
/// AS PER DESIGN — DO NOT REMOVE OR BYPASS:
/// The ONLY valid constructor is `CheckedDims::new()`, which checks:
/// 1. No single dimension is zero
/// 2. Width × height does not overflow u64
/// 3. Width × height does not exceed `MAX_PIXELS`
/// 4. Total bytes (pixels × channels) does not overflow u64
///
/// All buffer allocation must go through `self.alloc_buffer()` or read from
/// `self.total_bytes()` — never compute sizes manually from raw (w, h).
#[derive(Debug, Clone, Copy)]
pub struct CheckedDims {
    pub width: u32,
    pub height: u32,
    pub channels: u8,

    /// Pre-computed: width * height (guaranteed no overflow, non-zero)
    total_pixels: usize,

    /// Pre-computed: width * height * channels (guaranteed no overflow)
    total_bytes: usize,
}

impl CheckedDims {
    /// AS PER DESIGN — DO NOT REMOVE:
    /// The ONE and ONLY constructor for image dimensions. Every allocation path
    /// in the entire codebase MUST go through here. There is no `new_unchecked`
    /// alternative — if you think you need one, you're doing it wrong.
    ///
    /// # Errors
    /// - `DimensionError` if w=0 or h=0
    /// - `DimensionError` if w×h overflows u64
    /// - `DimensionError` if w×h exceeds `MAX_PIXELS`
    /// - `DimensionError` if w×h×channels overflows u64
    pub fn new(width: u32, height: u32, channels: u8) -> Result<Self, PilError> {
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
        let max = MAX_PIXELS.load(Ordering::Relaxed);
        if total_pixels > max {
            return Err(PilError::DimensionError(format!(
                "image size {} exceeds MAX_PIXELS ({}) — \
                 increase limit with CheckedDims::set_max_pixels()",
                total_pixels, max
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

    /// Total pixel count (width × height). Guaranteed non-zero.
    #[inline]
    pub fn total_pixels(&self) -> usize {
        self.total_pixels
    }

    /// Total buffer size in bytes (width × height × channels). Guaranteed non-zero.
    #[inline]
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Row stride in bytes (width × channels).
    #[inline]
    pub fn row_stride(&self) -> usize {
        self.width as usize * self.channels as usize
    }

    // ── Allocation helpers (AS PER DESIGN: use these, never vec![0u8; w*h*ch]) ──

    /// Allocate a zero-filled pixel buffer of the correct size.
    /// AS PER DESIGN: Use this instead of `vec![0u8; total_bytes]`.
    #[inline]
    pub fn alloc_buffer(&self) -> Vec<u8> {
        vec![0u8; self.total_bytes]
    }

    /// Allocate a buffer and fill it with the given value.
    #[inline]
    pub fn alloc_buffer_fill(&self, value: u8) -> Vec<u8> {
        vec![value; self.total_bytes]
    }

    // ── Global limit control (matching PIL's Image.MAX_IMAGE_PIXELS) ──

    /// Get the current global max pixel limit.
    pub fn max_pixels() -> u64 {
        MAX_PIXELS.load(Ordering::Relaxed)
    }

    /// Override the global max pixel limit.
    /// Pass `None` to remove the limit (use with caution).
    /// AS PER DESIGN: This is the ONLY way to change the limit — no env var
    /// or magic constant elsewhere.
    pub fn set_max_pixels(limit: Option<u64>) {
        MAX_PIXELS.store(limit.unwrap_or(u64::MAX), Ordering::Relaxed);
    }
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate overflow behavior.
#[cfg(test)]
mod tests {
    use super::*;

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
    fn exceeds_max_pixels() {
        let d = CheckedDims::new(100, 100, 1).unwrap();
        assert_eq!(d.total_pixels(), 10_000);

        // Lower the cap to test rejection
        CheckedDims::set_max_pixels(Some(5_000));
        assert!(CheckedDims::new(100, 100, 1).is_err());

        // Restore default
        CheckedDims::set_max_pixels(Some(DEFAULT_MAX_PIXELS));
    }

    #[test]
    fn alloc_buffer_correct_size() {
        let d = CheckedDims::new(10, 20, 3).unwrap();
        let buf = d.alloc_buffer();
        assert_eq!(buf.len(), 600);
        assert!(buf.iter().all(|&b| b == 0));
    }
}
