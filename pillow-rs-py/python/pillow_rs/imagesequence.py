"""ImageSequence — multi-frame image iteration. Pillow-compatible module."""

from . import _core


# The iterator state machine and public error behavior live in Rust. Keep this
# module as the stable Pillow-shaped import path for the extension type.
Iterator = _core.Iterator

__all__ = ["Iterator"]
