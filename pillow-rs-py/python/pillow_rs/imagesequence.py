"""ImageSequence — multi-frame image iteration. Pillow-compatible stub."""


class Iterator:
    """Iterate over frames in a multi-frame image."""

    def __init__(self, image):
        self._image = image
        self._frame = 0

    def __iter__(self):
        return self

    def __next__(self):
        try:
            self._image.seek(self._frame)
            frame = self._image.copy()
            self._frame += 1
            return frame
        except Exception:
            raise StopIteration
