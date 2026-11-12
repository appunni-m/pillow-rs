use crate::raster::Mode;

use super::Image;

impl Image {
    /// Selects whether Pillow's `Image.new` must call its P-palette adapter.
    ///
    /// The host provides a lazy sequence inspector because evaluating it can
    /// iterate user objects or raise. `Some(length)` means the host confirmed
    /// a list/tuple whose items are all integers. Core invokes the inspector
    /// only for the typed P mode, then accepts lengths three or four.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn new_palette_lookup_required<E>(
        mode: Option<Mode>,
        inspect_sequence: impl FnOnce() -> Result<Option<usize>, E>,
    ) -> Result<bool, E> {
        if mode != Some(Mode::ImagePalette8) {
            return Ok(false);
        }
        let Some(length) = inspect_sequence()? else {
            return Ok(false);
        };
        Ok(matches!(length, 3 | 4))
    }
}
