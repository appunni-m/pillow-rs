"""Deprecated compatibility alias for the public :mod:`PIL` namespace.

Use ``from PIL import Image`` for new code.  This package remains a small
source-level bridge for applications that still use the pre-0.1.0 name.
"""

from PIL import *
from PIL import __all__ as _pil_all

__all__ = list(_pil_all)
