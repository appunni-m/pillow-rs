use std::ops::Range;

/// Clipped source/destination intersection for one paste region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PasteRegion {
    pub(crate) source_left: u32,
    pub(crate) source_top: u32,
    pub(crate) destination_left: u32,
    pub(crate) destination_top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// Computes one interleaved byte row range without overflowing its extent.
#[must_use]
pub(crate) fn checked_row_range(
    row: u32,
    left: u32,
    width: u32,
    channels: usize,
    stride: usize,
) -> Option<Range<usize>> {
    let row = usize::try_from(row).ok()?;
    let left = usize::try_from(left).ok()?.checked_mul(channels)?;
    let width = usize::try_from(width).ok()?.checked_mul(channels)?;
    let start = row.checked_mul(stride)?.checked_add(left)?;
    let end = start.checked_add(width)?;
    Some(start..end)
}

impl PasteRegion {
    /// Clips the requested source region against the destination canvas.
    #[must_use]
    pub(crate) fn clipped(
        x: i64,
        y: i64,
        source_size: (u32, u32),
        destination_size: (u32, u32),
    ) -> Self {
        let (source_width, source_height) = source_size;
        let (destination_width, destination_height) = destination_size;
        let source_left = x.saturating_neg().max(0).min(i64::from(source_width)) as u32;
        let source_top = y.saturating_neg().max(0).min(i64::from(source_height)) as u32;
        let destination_left = x.max(0).min(i64::from(destination_width)) as u32;
        let destination_top = y.max(0).min(i64::from(destination_height)) as u32;
        Self {
            source_left,
            source_top,
            destination_left,
            destination_top,
            width: source_width
                .saturating_sub(source_left)
                .min(destination_width.saturating_sub(destination_left)),
            height: source_height
                .saturating_sub(source_top)
                .min(destination_height.saturating_sub(destination_top)),
        }
    }

    #[must_use]
    pub(crate) const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}
