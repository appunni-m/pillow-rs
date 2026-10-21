"""Safe, input-driven display helpers for migration parity workflows."""

from __future__ import annotations

from contextlib import contextmanager
from pathlib import Path
from typing import Any, Callable, Iterator


CAPTURE_VIEWER_PROTOCOL = "capture-viewer"
CAPTURE_VIEWER_FIELDS = {
    "protocol",
    "viewer_id",
    "image_step_id",
    "format",
    "accept",
    "registration",
    "hook",
}

_DISPLAY_SURFACE_PREFIX = "PIL.ImageShow"
_DISPLAY_IMAGE_OPERATION = ("PIL.Image.Image", "show")


def _capture_viewer_spec(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != CAPTURE_VIEWER_FIELDS:
        raise ValueError("capture-viewer has an invalid field set")
    if value.get("protocol") != CAPTURE_VIEWER_PROTOCOL:
        raise ValueError("unsupported display helper protocol")
    if not isinstance(value["viewer_id"], str) or not value["viewer_id"]:
        raise ValueError("capture-viewer viewer_id must be a non-empty string")
    if not isinstance(value["image_step_id"], str) or not value["image_step_id"]:
        raise ValueError("capture-viewer image_step_id must be a non-empty string")
    if value["format"] is not None and value["format"] != "PNG":
        raise ValueError("capture-viewer format must be null or 'PNG'")
    if type(value["accept"]) not in {bool, int}:
        raise ValueError("capture-viewer accept must be a bool or int")
    if value["registration"] not in ("class", "instance"):
        raise ValueError("capture-viewer registration must be class or instance")
    if value["hook"] not in ("show", "show_image", "show_file"):
        raise ValueError("capture-viewer hook must be show, show_image, or show_file")
    return value


def build_capture_viewer(
    value: Any,
    *,
    side: str,
    bindings: dict[str, Any],
    import_surface: Callable[[str, str], Any],
) -> Any:
    """Build the same capture subclass against the selected public side."""

    spec = _capture_viewer_spec(value)
    image_step_id = spec["image_step_id"]
    if image_step_id not in bindings:
        raise ValueError(
            "capture-viewer image_step_id must resolve to an earlier image binding"
        )
    image = bindings[image_step_id]
    if not (
        callable(getattr(image, "tobytes", None))
        and hasattr(image, "mode")
        and hasattr(image, "size")
    ):
        raise ValueError("capture-viewer image_step_id does not refer to an image")

    info = image.info
    if not isinstance(info, dict):
        raise ValueError("capture-viewer requires the image info mapping")
    events = info.setdefault("display_callbacks", [])
    if not isinstance(events, list):
        raise ValueError("image.info['display_callbacks'] must be a list")

    image_show = import_surface(side, "PIL.ImageShow")
    viewer_base = getattr(image_show, "Viewer")

    def capture(self: Any, shown_image: Any, **options: Any) -> Any:
        events.append(
            {
                "viewer_id": spec["viewer_id"],
                "title": options.get("title"),
                "mode": str(getattr(shown_image, "mode", "")),
                "size": getattr(shown_image, "size", None),
                "pixels": bytes(shown_image.tobytes()),
                "options": dict(options),
                "save_options": dict(getattr(self, "options", {})),
            }
        )
        return spec["accept"]

    def capture_file(self: Any, path: str, **options: Any) -> Any:
        try:
            events.append(
                {
                    "viewer_id": spec["viewer_id"],
                    "title": options.get("title"),
                    "_encoded_file": Path(path).read_bytes(),
                    "options": dict(options),
                    "save_options": dict(getattr(self, "options", {})),
                }
            )
            return spec["accept"]
        finally:
            # Capture viewers replace the platform process which normally
            # consumes and eventually removes this generated temporary file.
            try:
                Path(path).unlink()
            except FileNotFoundError:
                pass

    override_name, override_callback = {
        "show": ("show", capture),
        "show_image": ("show_image", capture),
        "show_file": ("show_file", capture_file),
    }[spec["hook"]]
    attributes = {"format": spec["format"], override_name: override_callback}
    if spec["format"] == "PNG":
        attributes["options"] = {"compress_level": 1, "save_all": True}
    capture_viewer = type("CaptureViewer", (viewer_base,), attributes)
    if spec["registration"] == "class":
        return capture_viewer
    return capture_viewer()


def _contains_capture_protocol(value: Any) -> bool:
    if isinstance(value, dict):
        if value.get("protocol") == CAPTURE_VIEWER_PROTOCOL:
            return True
        return any(_contains_capture_protocol(item) for item in value.values())
    if isinstance(value, (list, tuple)):
        return any(_contains_capture_protocol(item) for item in value)
    return False


def is_display_workflow(case: dict[str, Any]) -> bool:
    for step in case.get("steps", []):
        surface = step.get("surface")
        operation = step.get("operation")
        if (surface, operation) == _DISPLAY_IMAGE_OPERATION:
            return True
        if isinstance(surface, str) and (
            surface == _DISPLAY_SURFACE_PREFIX
            or surface.startswith(f"{_DISPLAY_SURFACE_PREFIX}.")
        ):
            return True
        if _contains_capture_protocol(step.get("receiver")):
            return True
        if _contains_capture_protocol(step.get("arguments", {})):
            return True
    return False


@contextmanager
def isolated_viewer_registry(
    side: str,
    case: dict[str, Any],
    import_surface: Callable[[str, str], Any],
) -> Iterator[None]:
    """Keep platform viewers out of a controlled display workflow."""

    if not is_display_workflow(case):
        yield
        return

    try:
        image_show = import_surface(side, "PIL.ImageShow")
    except (ImportError, KeyError):
        # Let the actual public workflow operation expose an unavailable
        # target module through the normal parity error result.
        yield
        return

    registry = getattr(image_show, "_viewers", None)
    if type(registry) is not list:
        raise RuntimeError(
            "cannot safely isolate display workflow: "
            "PIL.ImageShow._viewers is unavailable"
        )

    snapshot = list(registry)
    try:
        registry.clear()
    except Exception:
        raise RuntimeError(
            "cannot safely isolate display workflow: "
            "PIL.ImageShow._viewers could not be cleared"
        ) from None

    try:
        yield
    finally:
        registry[:] = snapshot
        if getattr(image_show, "_viewers", None) is not registry:
            image_show._viewers = registry
