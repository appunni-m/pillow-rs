"""ImageSequence — multi-frame image iteration. Pillow-compatible module."""


class Iterator:
    """Iterate over frames in a multi-frame image."""

    def __init__(self, im=None, image=None):
        self._image = im if im is not None else image
        self._frame = 0

    def __iter__(self):
        return self

    def __next__(self):
        # seek() always returns Ok (no-op) — single-frame images only
        # have frame 0. Return image on first call, StopIteration after.
        if self._frame > 0:
            raise StopIteration("end of sequence")
        try:
            self._image.seek(self._frame)
        except Exception:
            raise StopIteration("end of sequence")
        self._frame += 1
        return self._image
