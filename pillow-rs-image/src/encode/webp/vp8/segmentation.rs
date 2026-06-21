//! VP8 macroblock segmentation — RFC 6386 Section 9.3.
//!
//! Segmentation allows different macroblock regions of the frame to use
//! different quantizer offsets, enabling adaptive quality allocation.
//! A basic encoder can leave segmentation disabled (a single segment for
//! all macroblocks).

#![allow(dead_code)]

/// Number of segments supported (VP8 allows up to 4).
pub const NUM_SEGMENTS: usize = 4;

/// VP8 macroblock segment feature data.
///
/// When `enabled` is false, the frame uses a single quantizer for all
/// macroblocks.  When enabled, each macroblock carries a segment ID (0-3)
/// and the decoder applies per-segment feature deltas.
#[derive(Debug, Clone)]
pub struct Segmentation {
    /// Whether segmentation is active.
    pub enabled: bool,
    /// Whether the segment map is updated in this frame.
    pub update_map: bool,
    /// Per-segment quantizer offset (-127 to 127), added to the base
    /// quantizer index.
    pub quantizer_update: [i8; NUM_SEGMENTS],
    /// Whether each segment has an active quantizer offset.
    pub seg_feature_active: [bool; NUM_SEGMENTS],
}

impl Segmentation {
    /// Create a default (disabled) segmentation state.
    ///
    /// All segments have zero offset and are inactive.
    pub fn new() -> Self {
        Self {
            enabled: false,
            update_map: false,
            quantizer_update: [0i8; NUM_SEGMENTS],
            seg_feature_active: [false; NUM_SEGMENTS],
        }
    }

    /// Create a segmentation configuration from a quality hint.
    ///
    /// Lower quality values produce more aggressive segmentation differences.
    /// At the highest quality the feature is effectively disabled.
    ///
    /// The segmentation is `enabled` only when `quality < 80`.
    pub fn from_quality(quality: u8) -> Self {
        if quality >= 80 {
            // High quality — no segmentation needed.
            return Self::new();
        }

        // Lower quality → larger quantizer offsets between segments.
        // Segments 0 and 1 get negative offsets (finer quantisation),
        // segments 2 and 3 get positive offsets (coarser quantisation).
        let base_offset: i8 = ((80 - quality) / 10) as i8;
        let offset0 = -(base_offset * 2).min(127);
        let offset1 = -(base_offset).min(127).max(-127);
        let offset2 = base_offset.min(127);
        let offset3 = (base_offset * 2).min(127);

        Self {
            enabled: true,
            update_map: true,
            quantizer_update: [offset0, offset1, offset2, offset3],
            seg_feature_active: [true, true, true, true],
        }
    }

    /// Get the quantizer offset for a given segment ID (0-3).
    ///
    /// Returns 0 if segmentation is disabled or the segment has no active
    /// quantizer feature.
    pub fn quantizer_offset(&self, segment_id: u8) -> i8 {
        if !self.enabled {
            return 0;
        }
        let idx = (segment_id as usize) % NUM_SEGMENTS;
        if self.seg_feature_active[idx] {
            self.quantizer_update[idx]
        } else {
            0
        }
    }
}

impl Default for Segmentation {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_disabled() {
        let seg = Segmentation::new();
        assert!(!seg.enabled);
        assert!(!seg.update_map);
        assert_eq!(seg.quantizer_offset(0), 0);
        assert_eq!(seg.quantizer_offset(1), 0);
        assert_eq!(seg.quantizer_offset(2), 0);
        assert_eq!(seg.quantizer_offset(3), 0);
    }

    #[test]
    fn test_from_quality_high_disables() {
        let seg = Segmentation::from_quality(100);
        assert!(!seg.enabled, "Quality >= 80 should disable segmentation");
    }

    #[test]
    fn test_from_quality_medium_enables() {
        let seg = Segmentation::from_quality(50);
        assert!(seg.enabled, "Quality < 80 should enable segmentation");
        assert!(seg.update_map);
        // (80-50)/10 = 3
        // offset0 = -6, offset1 = -3, offset2 = 3, offset3 = 6
        assert_eq!(seg.quantizer_update[0], -6);
        assert_eq!(seg.quantizer_update[1], -3);
        assert_eq!(seg.quantizer_update[2], 3);
        assert_eq!(seg.quantizer_update[3], 6);
    }

    #[test]
    fn test_from_quality_low_larger_offsets() {
        let seg = Segmentation::from_quality(10);
        assert!(seg.enabled);
        // (80-10)/10 = 7
        // offset0 = -14, offset1 = -7, offset2 = 7, offset3 = 14
        assert_eq!(seg.quantizer_update[0], -14);
        assert_eq!(seg.quantizer_update[1], -7);
        assert_eq!(seg.quantizer_update[2], 7);
        assert_eq!(seg.quantizer_update[3], 14);
    }

    #[test]
    fn test_from_quality_very_low_clamped() {
        let seg = Segmentation::from_quality(0);
        assert!(seg.enabled);
        // (80-0)/10 = 8
        // offset0 = -16, offset1 = -8, offset2 = 8, offset3 = 16
        assert_eq!(seg.quantizer_update[0], -16);
        assert_eq!(seg.quantizer_update[1], -8);
        assert_eq!(seg.quantizer_update[2], 8);
        assert_eq!(seg.quantizer_update[3], 16);
    }

    #[test]
    fn test_quantizer_offset_disabled_returns_zero() {
        let mut seg = Segmentation::new();
        seg.quantizer_update[0] = 10;
        seg.seg_feature_active[0] = true;
        // Still disabled — should return 0.
        assert_eq!(seg.quantizer_offset(0), 0);
    }

    #[test]
    fn test_quantizer_offset_inactive_returns_zero() {
        let mut seg = Segmentation::new();
        seg.enabled = true;
        seg.quantizer_update[1] = 42;
        // seg_feature_active[1] is false by default.
        assert_eq!(seg.quantizer_offset(1), 0);
    }

    #[test]
    fn test_quantizer_offset_active() {
        let mut seg = Segmentation::new();
        seg.enabled = true;
        seg.quantizer_update[2] = -30;
        seg.seg_feature_active[2] = true;
        assert_eq!(seg.quantizer_offset(2), -30);
    }

    #[test]
    fn test_quantizer_offset_wraps_around() {
        let mut seg = Segmentation::new();
        seg.enabled = true;
        seg.quantizer_update[1] = 5;
        seg.seg_feature_active[1] = true;
        // segment_id 5 should wrap to index 1.
        assert_eq!(seg.quantizer_offset(5), 5);
    }

    #[test]
    fn test_default_trait() {
        let seg: Segmentation = Default::default();
        assert!(!seg.enabled);
    }

    #[test]
    fn test_quantizer_offsets_symmetric() {
        // Offsets should be roughly symmetric around zero.
        let seg = Segmentation::from_quality(50);
        let s0 = seg.quantizer_update[0] as i16;
        let s3 = seg.quantizer_update[3] as i16;
        let s1 = seg.quantizer_update[1] as i16;
        let s2 = seg.quantizer_update[2] as i16;
        assert_eq!(s0, -s3, "Segments 0 and 3 should be symmetric");
        assert_eq!(s1, -s2, "Segments 1 and 2 should be symmetric");
    }
}
