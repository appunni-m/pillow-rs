"""Backend-neutral workflow contract for deferred image pipeline operations.

The parity input builder uses these sets to add public output observations, and
the runner uses the same sets to require backend execution receipts. Keep
backend selection out of this module: fixtures and cases are shared by CPU,
SIMD, and GPU runs.
"""

from __future__ import annotations

import math
import struct

PIPELINE_ALWAYS_OPS = frozenset(
    {
        ("PIL.Image", "alpha_composite"),
        ("PIL.Image", "blend"),
        ("PIL.Image", "composite"),
        ("PIL.Image", "eval"),
        ("PIL.Image", "merge"),
        ("PIL.Image.Image", "crop"),
        ("PIL.Image.Image", "filter"),
        ("PIL.Image.Image", "getchannel"),
        ("PIL.Image.Image", "point"),
        ("PIL.Image.Image", "reduce"),
        ("PIL.Image.Image", "resize"),
        ("PIL.Image.Image", "rotate"),
        ("PIL.Image.Image", "transform"),
        ("PIL.Image.Image", "transpose"),
        ("PIL.ImageChops", "add"),
        ("PIL.ImageChops", "add_modulo"),
        ("PIL.ImageChops", "blend"),
        ("PIL.ImageChops", "composite"),
        ("PIL.ImageChops", "darker"),
        ("PIL.ImageChops", "difference"),
        ("PIL.ImageChops", "hard_light"),
        ("PIL.ImageChops", "invert"),
        ("PIL.ImageChops", "lighter"),
        ("PIL.ImageChops", "logical_and"),
        ("PIL.ImageChops", "logical_or"),
        ("PIL.ImageChops", "logical_xor"),
        ("PIL.ImageChops", "multiply"),
        ("PIL.ImageChops", "offset"),
        ("PIL.ImageChops", "overlay"),
        ("PIL.ImageChops", "screen"),
        ("PIL.ImageChops", "soft_light"),
        ("PIL.ImageChops", "subtract"),
        ("PIL.ImageChops", "subtract_modulo"),
        ("PIL.ImageEnhance.Brightness", "enhance"),
        ("PIL.ImageEnhance.Color", "enhance"),
        ("PIL.ImageEnhance.Contrast", "enhance"),
        ("PIL.ImageEnhance.Sharpness", "enhance"),
        ("PIL.ImageFilter", "BLUR"),
        ("PIL.ImageFilter", "BoxBlur"),
        ("PIL.ImageFilter", "CONTOUR"),
        ("PIL.ImageFilter", "DETAIL"),
        ("PIL.ImageFilter", "EDGE_ENHANCE"),
        ("PIL.ImageFilter", "EDGE_ENHANCE_MORE"),
        ("PIL.ImageFilter", "EMBOSS"),
        ("PIL.ImageFilter", "FIND_EDGES"),
        ("PIL.ImageFilter", "GaussianBlur"),
        ("PIL.ImageFilter", "Kernel"),
        ("PIL.ImageFilter", "MaxFilter"),
        ("PIL.ImageFilter", "MedianFilter"),
        ("PIL.ImageFilter", "MinFilter"),
        ("PIL.ImageFilter", "ModeFilter"),
        ("PIL.ImageFilter", "RankFilter"),
        ("PIL.ImageFilter", "SHARPEN"),
        ("PIL.ImageFilter", "SMOOTH"),
        ("PIL.ImageFilter", "SMOOTH_MORE"),
        ("PIL.ImageFilter", "UnsharpMask"),
        ("PIL.ImageOps", "autocontrast"),
        ("PIL.ImageOps", "colorize"),
        ("PIL.ImageOps", "contain"),
        ("PIL.ImageOps", "cover"),
        ("PIL.ImageOps", "crop"),
        ("PIL.ImageOps", "equalize"),
        ("PIL.ImageOps", "expand"),
        ("PIL.ImageOps", "fit"),
        ("PIL.ImageOps", "flip"),
        ("PIL.ImageOps", "grayscale"),
        ("PIL.ImageOps", "invert"),
        ("PIL.ImageOps", "mirror"),
        ("PIL.ImageOps", "pad"),
        ("PIL.ImageOps", "posterize"),
        ("PIL.ImageOps", "scale"),
        ("PIL.ImageOps", "solarize"),
        # These draw mutators enqueue Draw* operations on the destination.
        ("PIL.ImageDraw.ImageDraw", "arc"),
        ("PIL.ImageDraw.ImageDraw", "bitmap"),
        ("PIL.ImageDraw.ImageDraw", "chord"),
        ("PIL.ImageDraw.ImageDraw", "circle"),
        ("PIL.ImageDraw.ImageDraw", "ellipse"),
        ("PIL.ImageDraw.ImageDraw", "line"),
        ("PIL.ImageDraw.ImageDraw", "pieslice"),
        ("PIL.ImageDraw.ImageDraw", "point"),
        ("PIL.ImageDraw.ImageDraw", "polygon"),
        ("PIL.ImageDraw.ImageDraw", "rectangle"),
        ("PIL.ImageDraw.ImageDraw", "regular_polygon"),
        ("PIL.ImageDraw.ImageDraw", "rounded_rectangle"),
        ("PIL.ImageDraw.ImageDraw", "shape"),
    }
)

PIPELINE_MAYBE_OPS = frozenset(
    {
        ("PIL.Image.Image", "apply_transparency"),
        ("PIL.Image.Image", "convert"),
        ("PIL.Image.Image", "paste"),
        ("PIL.Image.Image", "putalpha"),
        ("PIL.Image.Image", "putpixel"),
        ("PIL.Image.Image", "remap_palette"),
        ("PIL.Image.Image", "thumbnail"),
        ("PIL.ImageOps", "exif_transpose"),
    }
)

PIPELINE_MUTATING_OPS = frozenset(
    {
        ("PIL.Image.Image", "apply_transparency"),
        ("PIL.Image.Image", "paste"),
        ("PIL.Image.Image", "putalpha"),
        ("PIL.Image.Image", "putpixel"),
        ("PIL.Image.Image", "remap_palette"),
        ("PIL.Image.Image", "thumbnail"),
        ("PIL.ImageDraw.ImageDraw", "arc"),
        ("PIL.ImageDraw.ImageDraw", "bitmap"),
        ("PIL.ImageDraw.ImageDraw", "chord"),
        ("PIL.ImageDraw.ImageDraw", "circle"),
        ("PIL.ImageDraw.ImageDraw", "ellipse"),
        ("PIL.ImageDraw.ImageDraw", "line"),
        ("PIL.ImageDraw.ImageDraw", "pieslice"),
        ("PIL.ImageDraw.ImageDraw", "point"),
        ("PIL.ImageDraw.ImageDraw", "polygon"),
        ("PIL.ImageDraw.ImageDraw", "rectangle"),
        ("PIL.ImageDraw.ImageDraw", "regular_polygon"),
        ("PIL.ImageDraw.ImageDraw", "rounded_rectangle"),
        ("PIL.ImageDraw.ImageDraw", "shape"),
    }
)

PIPELINE_RESULT_OPS = PIPELINE_ALWAYS_OPS | PIPELINE_MAYBE_OPS

TERMINAL_OBSERVATION_OPS = frozenset(
    {
        "getbands",
        "getbbox",
        "getcolors",
        "getdata",
        "getextrema",
        "get_flattened_data",
        "getpixel",
        "getprojection",
        "histogram",
        "load",
        "save",
        "tobitmap",
        "tobytes",
        "verify",
        "entropy",
        "extrema",
        "count",
        "sum",
        "sum2",
        "mean",
        "median",
        "rms",
        "var",
        "stddev",
    }
)


def workflow_operation_is_proven_noop(
    case: dict[str, object], step: dict[str, object]
) -> bool:
    """Recognize a narrow set of workflow-proven branches that queue no work.

    This is shared by receipt classification and observation-boundary checks.
    It intentionally uses only literal workflow inputs and mirrors explicit
    early returns in the public implementation; unknown sources or arguments
    remain in the backend-proof cohort.
    """

    steps = case.get("steps")
    if not isinstance(steps, list):
        return False
    steps_by_id = {
        candidate.get("step_id"): candidate
        for candidate in steps
        if isinstance(candidate, dict)
        and isinstance(candidate.get("step_id"), str)
    }

    def literal(arguments: object, name: str) -> object | None:
        if not isinstance(arguments, dict):
            return None
        value = arguments.get(name)
        if isinstance(value, dict) and value.get("kind") == "literal":
            return value.get("value")
        return None

    def binding_id(value: object) -> str | None:
        if isinstance(value, dict) and value.get("kind") == "binding":
            step_id = value.get("step_id")
            return step_id if isinstance(step_id, str) else None
        return None

    def argument_source(
        source_step: dict[str, object], name: str
    ) -> dict[str, object] | None:
        arguments = source_step.get("arguments")
        if not isinstance(arguments, dict):
            return None
        step_id = binding_id(arguments.get(name))
        source = steps_by_id.get(step_id) if step_id is not None else None
        return source if isinstance(source, dict) else None

    def receiver_source(source_step: dict[str, object]) -> dict[str, object] | None:
        step_id = binding_id(source_step.get("receiver"))
        source = steps_by_id.get(step_id) if step_id is not None else None
        return source if isinstance(source, dict) else None

    def source_size(source_step: dict[str, object] | None) -> tuple[int, int] | None:
        if source_step is None or (source_step.get("surface"), source_step.get("operation")) not in {
            ("PIL.Image", "new"),
            ("PIL.Image", "fromarray"),
            ("PIL.Image", "frombuffer"),
            ("PIL.Image", "frombytes"),
            ("PIL.Image", "fromstring"),
        }:
            return None
        size = literal(source_step.get("arguments"), "size")
        if not (
            isinstance(size, (list, tuple))
            and len(size) == 2
            and all(isinstance(value, int) and not isinstance(value, bool) for value in size)
        ):
            return None
        return size[0], size[1]

    def source_mode(source_step: dict[str, object] | None) -> str | None:
        if source_step is None:
            return None
        mode = literal(source_step.get("arguments"), "mode")
        return mode if isinstance(mode, str) else None

    key = (step.get("surface"), step.get("operation"))
    arguments = step.get("arguments")

    if key == ("PIL.ImageDraw.ImageDraw", "line"):
        points = literal(arguments, "xy")
        if not isinstance(points, (list, tuple)):
            return False
        return (
            len(points) == 0
            or (
                len(points) == 1
                and isinstance(points[0], (list, tuple))
                and len(points[0]) == 2
            )
            or (
                len(points) == 2
                and all(
                    isinstance(value, (int, float)) and not isinstance(value, bool)
                    for value in points
                )
            )
        )

    if key == ("PIL.ImageDraw.ImageDraw", "shape"):
        shape = literal(arguments, "shape")
        if (
            isinstance(shape, dict)
            and shape.get("protocol") == "outline"
            and shape.get("commands") == []
        ):
            return True
        if isinstance(shape, (list, tuple)) and (
            len(shape) == 0
            or (
                len(shape) == 1
                and isinstance(shape[0], (list, tuple))
                and len(shape[0]) == 2
            )
            or (
                len(shape) == 2
                and all(
                    isinstance(value, (int, float)) and not isinstance(value, bool)
                    for value in shape
                )
            )
        ):
            return True
        draw = receiver_source(step)
        image = argument_source(draw, "im") if draw is not None else None
        mode = source_mode(image)
        if mode not in {"1", "L", "P"} or not isinstance(arguments, dict):
            return False

        def absent_or_none(name: str) -> bool:
            if name not in arguments:
                return True
            value = arguments[name]
            return (
                isinstance(value, dict)
                and value.get("kind") == "literal"
                and value.get("value") is None
            )

        return absent_or_none("fill") and absent_or_none("outline")

    if key == ("PIL.ImageOps", "pad"):
        output_size = literal(arguments, "size")
        image = argument_source(step, "image")
        input_size = source_size(image)
        return (
            input_size is not None
            and isinstance(output_size, (list, tuple))
            and len(output_size) == 2
            and all(isinstance(value, int) and not isinstance(value, bool) for value in output_size)
            and input_size == tuple(output_size)
            and input_size[0] > 0
            and input_size[1] > 0
        )

    if key == ("PIL.Image.Image", "resize"):
        source = receiver_source(step)
        size = literal(arguments, "size")
        box = literal(arguments, "box")
        resample = literal(arguments, "resample")
        if not (
            source_mode(source) == "F"
            and resample in (0, "NEAREST", "nearest")
            and isinstance(size, (list, tuple))
            and len(size) == 2
            and all(isinstance(value, int) and not isinstance(value, bool) for value in size)
            and source_size(source) == tuple(size)
            and all(value > 0 for value in size)
            and isinstance(box, (list, tuple))
            and len(box) == 4
            and all(isinstance(value, (int, float)) and not isinstance(value, bool) for value in box)
        ):
            return False

        def f32(value: float) -> float:
            return struct.unpack("<f", struct.pack("<f", value))[0]

        def axis_maps_identity(source_extent: int, start: float, end: float) -> bool:
            start = f32(start)
            end = f32(end)
            scale = float(f32(f32(end) - f32(start))) / source_extent
            if not math.isfinite(scale):
                return False
            last = source_extent - 1
            coordinate = start + scale * 0.5
            for expected in range(source_extent):
                sample = math.floor(coordinate)
                if min(max(sample, 0), last) != expected:
                    return False
                coordinate += scale
            return True

        try:
            left, top, right, bottom = (float(value) for value in box)
            return axis_maps_identity(size[0], left, right) and axis_maps_identity(
                size[1], top, bottom
            )
        except (OverflowError, ValueError, struct.error):
            return False

    if key == ("PIL.Image.Image", "convert"):
        source = receiver_source(step)
        size = source_size(source)
        target = literal(arguments, "mode")
        matrix = literal(arguments, "matrix")
        return (
            size is not None
            and (size[0] == 0 or size[1] == 0)
            and isinstance(target, str)
            and target in {
                "L", "LA", "La", "RGB", "RGBA", "RGBX", "RGBa", "CMYK",
                "YCbCr", "HSV", "I", "F", "I;16", "I;16L", "I;16B", "I;16N",
            }
            and matrix is None
        )

    return False

DIRECT_CPU_DRAW_OPS = frozenset(
    {
        ("PIL.ImageDraw.ImageDraw", "text"),
        ("PIL.ImageDraw.ImageDraw", "multiline_text"),
    }
)


def workflow_binding_ids(value: object) -> set[str]:
    """Return binding step IDs referenced by a workflow descriptor tree."""

    if isinstance(value, dict):
        if value.get("kind") == "binding":
            step_id = value.get("step_id")
            return {step_id} if isinstance(step_id, str) else set()
        if value.get("kind") == "bindings":
            return {
                step_id
                for step_id in value.get("step_ids", [])
                if isinstance(step_id, str)
            }
        image_step_id = value.get("image_step_id")
        if isinstance(image_step_id, str):
            return {image_step_id}
        result: set[str] = set()
        for child in value.values():
            result.update(workflow_binding_ids(child))
        return result
    if isinstance(value, (list, tuple)):
        result: set[str] = set()
        for child in value:
            result.update(workflow_binding_ids(child))
        return result
    return set()


def _workflow_binding_id(value: object) -> str | None:
    if (
        isinstance(value, dict)
        and value.get("kind") == "binding"
        and isinstance(value.get("step_id"), str)
    ):
        return value["step_id"]
    return None


def _pipeline_image_target(
    step: dict[str, object], steps_by_id: dict[str, dict[str, object]]
) -> str | None:
    key = (step.get("surface"), step.get("operation"))
    if key[0] == "PIL.ImageDraw.ImageDraw":
        draw_step = steps_by_id.get(_workflow_binding_id(step.get("receiver")) or "")
        arguments = draw_step.get("arguments") if draw_step else None
        if isinstance(arguments, dict):
            return _workflow_binding_id(arguments.get("im") or arguments.get("image"))
        return None
    if key in PIPELINE_MUTATING_OPS:
        return _workflow_binding_id(step.get("receiver"))
    if key == ("PIL.ImageOps", "exif_transpose"):
        arguments = step.get("arguments")
        in_place = arguments.get("in_place") if isinstance(arguments, dict) else None
        if (
            isinstance(in_place, dict)
            and in_place.get("kind") == "literal"
            and in_place.get("value") is True
        ):
            return _workflow_binding_id(arguments.get("image"))
    step_id = step.get("step_id")
    return step_id if isinstance(step_id, str) else None


def workflow_pipeline_observation_gaps(
    case: dict[str, object],
) -> list[tuple[str, str | None]]:
    """Find candidate image operations without a dependent observed boundary.

    Boundaries are checked per operation target through workflow dependencies,
    including mutations that consume another pending image. Dependency edges
    carry step order so an earlier observation cannot prove later work on the
    same mutable image.
    """

    steps = case.get("steps")
    if not isinstance(steps, list):
        return []
    observed = case.get("observations", [])
    observed_ids = set(observed) if isinstance(observed, list) else set()
    steps_by_id = {
        step["step_id"]: step
        for step in steps
        if isinstance(step, dict) and isinstance(step.get("step_id"), str)
    }
    dependencies: dict[str, list[tuple[int, str]]] = {}
    candidates: list[tuple[int, dict[str, object], str | None]] = []

    for index, step in enumerate(steps):
        if not isinstance(step, dict):
            continue
        step_id = step.get("step_id")
        if not isinstance(step_id, str):
            continue
        referenced = workflow_binding_ids(step.get("receiver"))
        referenced.update(workflow_binding_ids(step.get("arguments", {})))
        dependencies.setdefault(step_id, []).extend(
            (index, dependency) for dependency in referenced
        )

        key = (step.get("surface"), step.get("operation"))
        if workflow_operation_is_proven_noop(case, step):
            continue
        if key in PIPELINE_RESULT_OPS and key[0] != "PIL.ImageFilter":
            target = _pipeline_image_target(step, steps_by_id)
            candidates.append((index, step, target))
        elif key in DIRECT_CPU_DRAW_OPS:
            target = _pipeline_image_target(step, steps_by_id)
            candidates.append((index, step, target))
        else:
            target = None

        if target is not None and (
            key in PIPELINE_MUTATING_OPS or key in DIRECT_CPU_DRAW_OPS
        ):
            for dependency in workflow_binding_ids(step.get("arguments", {})):
                if dependency != target:
                    dependencies.setdefault(target, []).append((index, dependency))

    def depends_on(
        step_id: str, target: str, through_index: int, visited: set[str]
    ) -> bool:
        if step_id == target:
            return True
        if step_id in visited:
            return False
        visited.add(step_id)
        return any(
            index <= through_index
            and depends_on(dependency, target, through_index, visited)
            for index, dependency in dependencies.get(step_id, [])
        )

    def materializes(step: dict[str, object]) -> bool:
        key = (step.get("surface"), step.get("operation"))
        return step.get("operation") in TERMINAL_OBSERVATION_OPS or (
            key in PIPELINE_RESULT_OPS
            and key not in PIPELINE_MUTATING_OPS
            and key[0] != "PIL.ImageFilter"
        )

    gaps: list[tuple[str, str | None]] = []
    for operation_index, operation, target in candidates:
        operation_id = operation.get("step_id")
        if not isinstance(operation_id, str):
            continue
        if target is None:
            gaps.append((operation_id, None))
            continue
        covered = False
        for boundary_index, boundary in enumerate(steps):
            if (
                not isinstance(boundary, dict)
                or boundary.get("step_id") not in observed_ids
                or not materializes(boundary)
            ):
                continue
            boundary_key = (boundary.get("surface"), boundary.get("operation"))
            same_result = (
                boundary_index == operation_index
                and boundary_key in PIPELINE_RESULT_OPS
                and boundary_key not in PIPELINE_MUTATING_OPS
            )
            if boundary_index <= operation_index and not same_result:
                continue
            boundary_id = boundary.get("step_id")
            if isinstance(boundary_id, str) and depends_on(
                boundary_id, target, boundary_index, set()
            ):
                covered = True
                break
        if not covered:
            gaps.append((operation_id, target))
    return gaps
