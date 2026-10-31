//! Compatibility adapters for GPU I;16 resize metadata.

/// TODO(pillow-rs#3ptb): accept the typed `Mode` from GPU admission and remove
/// this Pillow-string compatibility adapter after the GPU caller is migrated.
pub(crate) fn luma16_resample_big_endian(mode: Option<&str>) -> bool {
    matches!(mode, Some("I;16N" | "I;16B"))
}
