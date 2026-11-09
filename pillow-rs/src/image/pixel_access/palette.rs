use crate::error::PilError;

use super::super::Image;

impl Image {
    pub(super) fn palette_allocation_index(
        &self,
        entries: usize,
        reserved: &[u32],
        reuse_unused: bool,
    ) -> Result<usize, PilError> {
        // TODO(pillow-rs#3ptb): freeze P/PA parity for reserved metadata
        // indices in short palettes and full-palette reuse, including when
        // public Python `Image.info` overrides a retained palette-alpha marker.
        // Pillow skips `background`/`transparency`, but appends at the current
        // palette end even when the returned index is later.
        if let Some(index) = Self::next_palette_index(entries, reserved) {
            return Ok(index);
        }
        if !reuse_unused {
            return Err(PilError::ValueError(
                "cannot allocate more than 256 colors".into(),
            ));
        }
        let mut used = [false; 256];
        for index in self.tobytes()? {
            used[usize::from(index)] = true;
        }
        Self::highest_unused_palette_index(reserved, used.len(), |index| used[index])
            .ok_or_else(|| PilError::ValueError("cannot allocate more than 256 colors".into()))
    }

    /// Selects an ImagePalette allocation slot from its entry count and image
    /// histogram. Python keeps palette mutation and the public error cause;
    /// this method owns the slot-selection algorithm for the base Image path.
    #[doc(hidden)]
    #[must_use]
    pub fn palette_allocation_index_from_histogram(
        entries: usize,
        reserved: &[u32],
        histogram: &[u64],
    ) -> Option<usize> {
        if let Some(index) = Self::next_palette_index(entries, reserved) {
            return Some(index);
        }
        Self::highest_unused_palette_index(reserved, histogram.len(), |index| histogram[index] != 0)
    }

    fn next_palette_index(entries: usize, reserved: &[u32]) -> Option<usize> {
        let mut next = entries;
        while next < 256 && Self::palette_index_is_reserved(next, reserved) {
            next += 1;
        }
        (next < 256).then_some(next)
    }

    fn highest_unused_palette_index(
        reserved: &[u32],
        histogram_len: usize,
        mut is_used: impl FnMut(usize) -> bool,
    ) -> Option<usize> {
        (0..histogram_len)
            .rev()
            .find(|&index| !is_used(index) && !Self::palette_index_is_reserved(index, reserved))
    }

    fn palette_index_is_reserved(index: usize, reserved: &[u32]) -> bool {
        u32::try_from(index).is_ok_and(|index| reserved.contains(&index))
    }
}
