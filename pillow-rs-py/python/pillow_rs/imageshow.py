"""Pillow-compatible host-side image display support.

This module owns viewer registration and temporary-file handling. Image mode
conversion and encoding stay on the target image implementation; viewer
programs are only invoked when a caller explicitly asks to display an image.
"""

from __future__ import annotations

import abc
import os
import shutil
import subprocess
import sys
import tempfile
from shlex import quote
from typing import Any

from .image import Image


_viewers: list["Viewer"] = []


def register(viewer: type[Viewer] | Viewer, order: int = 1) -> None:
    """Register a viewer class or instance.

    A zero or negative ``order`` puts the viewer first; a positive order puts
    it last. Viewer classes are instantiated when registered.
    """
    if isinstance(viewer, type) and issubclass(viewer, Viewer):
        viewer = viewer()
    if order > 0:
        _viewers.append(viewer)
    else:
        _viewers.insert(0, viewer)


def show(image: Image, title: str | None = None, **options: Any) -> bool:
    """Show an image with the first registered viewer that accepts it."""
    for viewer in _viewers:
        if viewer.show(image, title=title, **options):
            return True
    return False


_MODE_BASE = {
    # Pillow's ImageMode descriptors. Keep this local so importing the host
    # adapter does not pull in a second PIL namespace.
    "1": "L",
    "L": "L",
    "I": "L",
    "F": "L",
    "P": "P",
    "RGB": "RGB",
    "RGBX": "RGB",
    "RGBA": "RGB",
    "CMYK": "RGB",
    "YCbCr": "RGB",
    "LAB": "RGB",
    "HSV": "RGB",
    "RGBa": "RGB",
    "LA": "L",
    "La": "L",
    "PA": "RGB",
    "I;16": "L",
    "I;16S": "L",
    "I;16L": "L",
    "I;16LS": "L",
    "I;16B": "L",
    "I;16BS": "L",
    "I;16N": "L",
    "I;16NS": "L",
    "I;32": "L",
    "I;32B": "L",
    "I;32L": "L",
    "I;32S": "L",
    "I;32BS": "L",
    "I;32LS": "L",
}


def _getmodebase(mode: str) -> str:
    """Return Pillow's grayscale or color base mode for a known mode."""
    return _MODE_BASE[mode]


def _dump_image(
    image: Image,
    file: str | None = None,
    format: str | None = None,
    **options: Any,
) -> str:
    """Save an image to a temporary file for a registered viewer.

    Pillow's private ``Image._dump`` writes PPM data directly when no format
    is supplied (or ``format == "PPM"``). The core codec set has no PPM
    encoder, so write its simple binary L/RGB representation at this host I/O
    boundary instead of routing through an unavailable Rust format.
    """
    suffix = f".{format}" if format else ""
    if not file:
        descriptor, filename = tempfile.mkstemp(suffix=suffix)
        os.close(descriptor)
    else:
        filename = file
        if not filename.endswith(suffix):
            filename += suffix

    image.load()
    if not format or format == "PPM":
        _save_ppm(image, filename)
    else:
        # PNG compression options flow through Image.save to the Rust encoder.
        # `save_all` is inert for this still-image API.
        image.save(filename, format, **options)
    return filename


def _save_ppm(image: Image, filename: str) -> None:
    """Write the binary PPM variant used by Pillow's display fallback."""
    if image.mode == "1":
        image = image.convert("L")
    elif image.mode in ("P", "RGBA") or image.mode not in ("L", "RGB"):
        image = image.convert("RGB")

    width, height = image.size
    if image.mode == "L":
        header = f"P5\n{width} {height}\n255\n".encode("ascii")
    elif image.mode == "RGB":
        header = f"P6\n{width} {height}\n255\n".encode("ascii")
    else:
        raise ValueError(f"unsupported PPM display mode: {image.mode}")
    with open(filename, "wb") as output:
        output.write(header)
        output.write(image.tobytes())


class Viewer:
    """Base class for image viewers."""

    format: str | None = None
    """The format used when saving an image for this viewer."""

    options: dict[str, Any] = {}
    """Additional save options used by this viewer."""

    def show(self, image: Image, **options: Any) -> int:
        """Prepare the image mode, then pass it to :meth:`show_image`."""
        if not (
            image.mode in ("1", "RGBA")
            or (self.format == "PNG" and image.mode in ("I;16", "LA"))
        ):
            base = _getmodebase(image.mode)
            if image.mode != base:
                image = image.convert(base)
        return self.show_image(image, **options)

    def get_format(self, image: Image) -> str | None:
        """Return a format name, or ``None`` for a PPM fallback file."""
        return self.format

    def get_command(self, file: str, **options: Any) -> str:
        """Return the shell command used to display a saved file."""
        raise NotImplementedError("unavailable in base viewer")

    def save_image(self, image: Image) -> str:
        """Save the image to a temporary file and return its path."""
        return _dump_image(
            image,
            format=self.get_format(image),
            **self.options,
        )

    def show_image(self, image: Image, **options: Any) -> int:
        """Save and display the image."""
        return self.show_file(self.save_image(image), **options)

    def show_file(self, path: str, **options: Any) -> int:
        """Display a saved file using :meth:`get_command`."""
        if not os.path.exists(path):
            raise FileNotFoundError
        os.system(self.get_command(path, **options))  # nosec
        return 1


class WindowsViewer(Viewer):
    """Use the default Windows application to display a PNG file."""

    format = "PNG"
    options = {"compress_level": 1, "save_all": True}

    def get_command(self, file: str, **options: Any) -> str:
        return (
            f'start "Pillow" /WAIT "{file}" '
            "&& ping -n 4 127.0.0.1 >NUL "
            f'&& del /f "{file}"'
        )

    def show_file(self, path: str, **options: Any) -> int:
        if not os.path.exists(path):
            raise FileNotFoundError
        subprocess.Popen(
            self.get_command(path, **options),
            shell=True,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW"),
        )  # nosec
        return 1


if sys.platform == "win32":
    register(WindowsViewer)


class MacViewer(Viewer):
    """Use macOS Preview to display a PNG file."""

    format = "PNG"
    options = {"compress_level": 1, "save_all": True}

    def get_command(self, file: str, **options: Any) -> str:
        command = "open -a Preview.app"
        return f"({command} {quote(file)}; sleep 20; rm -f {quote(file)})&"

    def show_file(self, path: str, **options: Any) -> int:
        if not os.path.exists(path):
            raise FileNotFoundError
        subprocess.call(["open", "-a", "Preview.app", path])

        pyinstaller = getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS")
        executable = (not pyinstaller and sys.executable) or shutil.which("python3")
        if executable:
            subprocess.Popen(
                [
                    executable,
                    "-c",
                    "import os, sys, time; time.sleep(20); os.remove(sys.argv[1])",
                    path,
                ]
            )
        return 1


if sys.platform == "darwin":
    register(MacViewer)


class UnixViewer(abc.ABC, Viewer):
    """Base for viewers on Unix-like systems."""

    format = "PNG"
    options = {"compress_level": 1, "save_all": True}

    @abc.abstractmethod
    def get_command_ex(self, file: str, **options: Any) -> tuple[str, str]:
        """Return the command prefix and executable name."""
        raise NotImplementedError

    def get_command(self, file: str, **options: Any) -> str:
        command = self.get_command_ex(file, **options)[0]
        return f"{command} {quote(file)}"


class XDGViewer(UnixViewer):
    """Use the freedesktop.org ``xdg-open`` command."""

    def get_command_ex(self, file: str, **options: Any) -> tuple[str, str]:
        return "xdg-open", "xdg-open"

    def show_file(self, path: str, **options: Any) -> int:
        if not os.path.exists(path):
            raise FileNotFoundError
        subprocess.Popen(["xdg-open", path])
        return 1


class DisplayViewer(UnixViewer):
    """Use ImageMagick ``display``; this viewer supports a title."""

    def get_command_ex(
        self, file: str, title: str | None = None, **options: Any
    ) -> tuple[str, str]:
        command = executable = "display"
        if title:
            command += f" -title {quote(title)}"
        return command, executable

    def show_file(self, path: str, **options: Any) -> int:
        if not os.path.exists(path):
            raise FileNotFoundError
        args = ["display"]
        title = options.get("title")
        if title:
            args += ["-title", title]
        args.append(path)
        subprocess.Popen(args)
        return 1


class GmDisplayViewer(UnixViewer):
    """Use the GraphicsMagick ``gm display`` command."""

    def get_command_ex(self, file: str, **options: Any) -> tuple[str, str]:
        return "gm display", "gm"

    def show_file(self, path: str, **options: Any) -> int:
        if not os.path.exists(path):
            raise FileNotFoundError
        subprocess.Popen(["gm", "display", path])
        return 1


class EogViewer(UnixViewer):
    """Use the GNOME Image Viewer ``eog`` command."""

    def get_command_ex(self, file: str, **options: Any) -> tuple[str, str]:
        return "eog -n", "eog"

    def show_file(self, path: str, **options: Any) -> int:
        if not os.path.exists(path):
            raise FileNotFoundError
        subprocess.Popen(["eog", "-n", path])
        return 1


class XVViewer(UnixViewer):
    """Use the X Viewer ``xv`` command; this viewer supports a title."""

    def get_command_ex(
        self, file: str, title: str | None = None, **options: Any
    ) -> tuple[str, str]:
        command = executable = "xv"
        if title:
            command += f" -name {quote(title)}"
        return command, executable

    def show_file(self, path: str, **options: Any) -> int:
        if not os.path.exists(path):
            raise FileNotFoundError
        args = ["xv"]
        title = options.get("title")
        if title:
            args += ["-name", title]
        args.append(path)
        subprocess.Popen(args)
        return 1


if sys.platform not in ("win32", "darwin"):
    if shutil.which("xdg-open"):
        register(XDGViewer)
    if shutil.which("display"):
        register(DisplayViewer)
    if shutil.which("gm"):
        register(GmDisplayViewer)
    if shutil.which("eog"):
        register(EogViewer)
    if shutil.which("xv"):
        register(XVViewer)


class IPythonViewer(Viewer):
    """Display images in an IPython frontend."""

    def show_image(self, image: Image, **options: Any) -> int:
        ipython_display(image)
        return 1


try:
    from IPython.display import display as ipython_display
except ImportError:
    pass
else:
    register(IPythonViewer)


__all__ = [
    "DisplayViewer",
    "EogViewer",
    "GmDisplayViewer",
    "IPythonViewer",
    "MacViewer",
    "UnixViewer",
    "Viewer",
    "WindowsViewer",
    "XDGViewer",
    "XVViewer",
    "register",
    "show",
]
