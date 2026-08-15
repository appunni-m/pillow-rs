"""The documented Pillow-compatible import namespace for pillow-rs."""

from pillow_rs import *
from pillow_rs import Image as _Image
from pillow_rs import __all__ as _pillow_rs_all
from pillow_rs import active_backends, available_backends

# Pillow exposes ``Image`` as a module, so ``Image.Image`` annotations remain
# valid when callers import Image from RSPIL.
_Image.Image = _Image

__all__ = list(_pillow_rs_all) + ["active_backends", "available_backends"]
