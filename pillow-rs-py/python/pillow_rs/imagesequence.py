"""ImageSequence — multi-frame image iteration. Pillow-compatible module."""


class Iterator:
    """Iterate over frames in a multi-frame image."""

    def __init__(self, im):
        if not hasattr(im, "seek"):
            raise AttributeError("im must have seek method")
        self.im = im
        self.position = getattr(self.im, "_min_frame", 0)

    def __iter__(self):
        return self

    def __next__(self):
        try:
            self.im.seek(self.position)
            self.position += 1
            return self.im
        except EOFError as error:
            # Match Pillow's ImageSequence.Iterator: only the image's public
            # end-of-sequence signal becomes StopIteration. Other seek errors
            # remain observable to callers.
            raise StopIteration("end of sequence") from error
