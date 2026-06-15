"""ImageOps — high-level image operations. Pillow-compatible module."""
import re
import struct

from .image import Image
from . import _core


def autocontrast(image: Image, cutoff: float = 0, ignore=None, mask=None,
                 preserve_tone: bool = False) -> Image:
    return Image(_core.ops_autocontrast(image._rust_image, cutoff))


def equalize(image: Image, mask=None) -> Image:
    return Image(_core.ops_equalize(image._rust_image))


def invert(image: Image) -> Image:
    return Image(_core.ops_invert(image._rust_image))


def flip(image: Image) -> Image:
    return Image(_core.ops_flip(image._rust_image))


def mirror(image: Image) -> Image:
    return Image(_core.ops_mirror(image._rust_image))


def posterize(image: Image, bits: int) -> Image:
    return Image(_core.ops_posterize(image._rust_image, bits))


def solarize(image: Image, threshold: int = 128) -> Image:
    return Image(_core.ops_solarize(image._rust_image, threshold))


def grayscale(image: Image) -> Image:
    return Image(_core.ops_grayscale(image._rust_image))


def expand(image: Image, border=0, fill=0) -> Image:
    """Add a border around the image. Delegates to Rust pipeline."""
    if isinstance(border, int):
        border = (border, border, border, border)
    # Match PIL's Image.new mode-specific fill behavior:
    #   int fill -> first channel only, other channels = 0
    #   tuple -> fill channels directly (4-tuple keeps A if given)
    if isinstance(fill, int):
        fill = (fill, 0, 0, 0)
    elif isinstance(fill, tuple) and len(fill) == 3:
        fill = (fill[0], fill[1], fill[2], 0)
    return Image(_core.ops_expand(image._rust_image, max(border), fill))


def crop(image: Image, border: int = 0) -> Image:
    """Crop border off image edges. Delegates to Rust pipeline."""
    return Image(_core.ops_crop_border(image._rust_image, border))


def scale(image: Image, factor: float, resample=None) -> Image:
    """Scale image by factor. Delegates to Rust pipeline."""
    return Image(_core.ops_scale(image._rust_image, factor, resample))


def contain(image: Image, size, method=None) -> Image:
    """Resize to fit within size. Delegates to Rust pipeline."""
    return Image(_core.ops_contain(image._rust_image, (size[0], size[1]), method))


def cover(image: Image, size, method=None) -> Image:
    """Resize to cover size. Delegates to Rust pipeline."""
    return Image(_core.ops_cover(image._rust_image, (size[0], size[1]), method))


def fit(image: Image, size, method=None, bleed=0.0, centering=(0.5, 0.5)):
    """Resize and crop to fit exact dimensions. Delegates to Rust pipeline."""
    return Image(_core.ops_fit(image._rust_image, (size[0], size[1]), method, bleed, centering))


def pad(image: Image, size, method=None, color=None, centering=(0.5, 0.5)):
    """Pad image to given size. Delegates to Rust pipeline."""
    c = (0, 0, 0, 255) if color is None else (
        (color, color, color, 255) if isinstance(color, int) else
        (color[0], color[1], color[2], 255) if len(color) == 3 else color
    )
    return Image(_core.ops_pad(image._rust_image, (size[0], size[1]), method, c, centering))


def colorize(image: Image, black, white, mid=None, blackpoint=0, whitepoint=255, midpoint=127):
    """Colorize grayscale image — delegates to Rust."""
    if image.mode != "L":
        image = image.convert("L")
    if isinstance(black, str):
        from PIL.ImageColor import getrgb
        black = getrgb(black)
    if isinstance(white, str):
        from PIL.ImageColor import getrgb
        white = getrgb(white)
    return Image(_core.ops_colorize(image._rust_image, black[:3], white[:3]))


def _get_exif_orientation(exif_bytes):
    """Extract Orientation tag (0x0112) from raw EXIF bytes. Returns None if not found."""
    if not exif_bytes or len(exif_bytes) < 8:
        return None

    # Skip EXIF header if present
    data = exif_bytes
    if data[:6] == b'Exif\x00\x00':
        data = data[6:]
    if len(data) < 8:
        return None

    # Determine byte order
    endian = data[:2]
    if endian == b'II':
        bo = '<'
    elif endian == b'MM':
        bo = '>'
    else:
        return None

    # Check TIFF magic number
    magic = struct.unpack(bo + 'H', data[2:4])[0]
    if magic != 42:
        return None

    # Get IFD0 offset
    ifd_offset = struct.unpack(bo + 'I', data[4:8])[0]
    if ifd_offset + 2 > len(data):
        return None

    # Number of IFD entries
    num_entries = struct.unpack(bo + 'H', data[ifd_offset:ifd_offset + 2])[0]

    for i in range(num_entries):
        entry_start = ifd_offset + 2 + i * 12
        if entry_start + 12 > len(data):
            break
        tag = struct.unpack(bo + 'H', data[entry_start:entry_start + 2])[0]
        if tag == 0x0112:  # Orientation
            value = struct.unpack(bo + 'H', data[entry_start + 8:entry_start + 10])[0]
            if 1 <= value <= 8:
                return value
            return None

    return None


def _remove_exif_orientation(exif_bytes):
    """Remove Orientation tag from EXIF bytes by zeroing its tag field."""
    if not exif_bytes or len(exif_bytes) < 14:
        return exif_bytes

    raw = bytearray(exif_bytes)
    header_len = 6 if raw[:6] == b'Exif\x00\x00' else 0

    if len(raw) - header_len < 8:
        return exif_bytes

    endian = raw[header_len:header_len + 2]
    if endian == b'II':
        bo = '<'
    elif endian == b'MM':
        bo = '>'
    else:
        return exif_bytes

    magic = struct.unpack(bo + 'H', raw[header_len + 2:header_len + 4])[0]
    if magic != 42:
        return exif_bytes

    ifd_offset = struct.unpack(bo + 'I', raw[header_len + 4:header_len + 8])[0]
    abs_ifd = header_len + ifd_offset
    if abs_ifd + 2 > len(raw):
        return exif_bytes

    num_entries = struct.unpack(bo + 'H', raw[abs_ifd:abs_ifd + 2])[0]

    for i in range(num_entries):
        entry_start = abs_ifd + 2 + i * 12
        if entry_start + 12 > len(raw):
            break
        tag = struct.unpack(bo + 'H', raw[entry_start:entry_start + 2])[0]
        if tag == 0x0112:  # Orientation
            # Zero out the tag to indicate "no tag"
            raw[entry_start:entry_start + 2] = b'\x00\x00'
            break

    return bytes(raw)


def exif_transpose(image: Image, *, in_place=False):
    """If an image has an EXIF Orientation tag, transpose the image accordingly.

    Matches PIL's exif_transpose behavior.

    :param image: The image to transpose.
    :param in_place: If True, modifies the original image in-place and returns None.
    :returns: A transposed image copy, or None if in_place.
    """
    image.load()
    exif_data = image.getexif()
    orientation = _get_exif_orientation(exif_data) or 1

    # Map EXIF orientation to Transpose method (matches PIL exactly)
    method_map = {
        2: "FLIP_LEFT_RIGHT",
        3: "ROTATE_180",
        4: "FLIP_TOP_BOTTOM",
        5: "TRANSPOSE",
        6: "ROTATE_270",
        7: "TRANSVERSE",
        8: "ROTATE_90",
    }
    method = method_map.get(orientation)

    if method is not None:
        if in_place:
            transposed = image.transpose(method)
            image._rust_image = transposed._rust_image
            image._explicit_mode = transposed._explicit_mode
            result = image
        else:
            result = image.transpose(method)

        # Remove orientation from EXIF (matching PIL behavior)
        if exif_data and exif_data != b'Exif\x00\x00MM\x00*\x00\x00\x00\x08\x00\x00\x00\x00\x00\x00':
            new_exif = _remove_exif_orientation(exif_data)
            # Store modified EXIF back if possible
            if "exif" in result.info:
                result.info["exif"] = new_exif
            # Clean up XMP orientation tags
            for key in ("XML:com.adobe.xmp", "xmp"):
                if key in result.info:
                    value = result.info[key]
                    for pattern in (
                        r'tiff:Orientation="([0-9])"',
                        r"<tiff:Orientation>([0-9])</tiff:Orientation>",
                    ):
                        if isinstance(value, str):
                            value = re.sub(pattern, "", value)
                        elif isinstance(value, tuple):
                            value = tuple(re.sub(pattern.encode(), b"", v) for v in value)
                        else:
                            value = re.sub(pattern.encode(), b"", value)
                    result.info[key] = value

        if not in_place:
            return result
        return None
    elif not in_place:
        return image.copy()
    return None


def deform(image: Image, deformer, resample=None):
    """Deform image using a mesh deformer. Matches PIL error behavior."""
    mesh = deformer.getmesh(image)
    result = image.transform(image.size, "MESH", mesh[0] if mesh else [])
    return result
