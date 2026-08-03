//! Core state for Pillow's public image-sequence iterator.
//!
//! The host binding owns the image object's `seek` protocol and its return
//! handle. Core owns only the frame position, so iterator behavior remains
//! independent of Python objects and other host-language representations.

/// Rust-owned state for Pillow's public multi-frame image iterator.
///
/// The binding advances this state only after a seek succeeds. That preserves
/// Pillow's behavior at the end of a sequence: an EOF does not consume the
/// next position before the host maps it to `StopIteration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageSequenceIterator {
    position: u32,
}

impl ImageSequenceIterator {
    /// Creates an iterator starting at the image's minimum frame.
    pub fn new(min_frame: u32) -> Self {
        Self {
            position: min_frame,
        }
    }

    /// Returns the frame that the binding should seek next.
    pub fn position(&self) -> u32 {
        self.position
    }

    /// Advances after the selected frame was successfully sought.
    pub fn advance(&mut self) {
        self.position = self.position.saturating_add(1);
    }
}
