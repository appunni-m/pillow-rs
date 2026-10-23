"""Input-only display workflows using a publicly registered capture viewer."""

from __future__ import annotations

from typing import Any


DISPLAY_OPERATIONS = {
    ("PIL.Image.Image", "show"),
    ("PIL.ImageShow", "register"),
    ("PIL.ImageShow", "show"),
}
PLATFORM_VIEWER_CLASSES = (
    "WindowsViewer",
    "MacViewer",
    "XDGViewer",
    "DisplayViewer",
    "GmDisplayViewer",
    "EogViewer",
    "XVViewer",
)


def _literal(value: Any) -> dict[str, Any]:
    return {"kind": "literal", "value": value}


def _binding(step_id: str) -> dict[str, str]:
    return {"kind": "binding", "step_id": step_id}


def _step(step_id, surface, operation, arguments, receiver=None):
    return {
        "step_id": step_id, "surface": surface, "operation": operation,
        "receiver": receiver, "arguments": arguments,
    }


def display_case(
    surface: str,
    operation: str,
    case_id: str,
    covers: list[str],
    *,
    mode: str = "RGB",
    color: Any = (31, 79, 197),
    size: tuple[int, int] = (2, 1),
    title: str | None = None,
    registrations: tuple[dict[str, Any], ...] | None = None,
    options: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Describe public calls; callback results are produced by the live sides."""
    if (surface, operation) not in DISPLAY_OPERATIONS:
        raise ValueError("unknown display operation")
    registrations = registrations or ({},)
    steps = [_step("image", "PIL.Image", "new", {
        "mode": _literal(mode), "size": _literal(list(size)),
        "color": _literal(list(color) if isinstance(color, tuple) else color),
    })]
    register_ids = []
    for index, settings in enumerate(registrations):
        register_id = "call" if operation == "register" and index == 0 else f"register-{index}"
        register_ids.append(register_id)
        descriptor = {
            "protocol": "capture-viewer", "viewer_id": f"viewer-{index}",
            "image_step_id": "image", "format": settings.get("format"),
            "accept": settings.get("accept", 1),
            "registration": settings.get("registration", "instance"),
            "hook": settings.get("hook", "show"),
        }
        steps.append(_step(register_id, "PIL.ImageShow", "register", {
            "viewer": _literal(descriptor), "order": _literal(settings.get("order", 0)),
        }))
    call_id = "display" if operation == "register" else "call"
    if surface == "PIL.Image.Image":
        steps.append(_step(call_id, surface, "show", {"title": _literal(title)}, _binding("image")))
    else:
        steps.append(_step(call_id, "PIL.ImageShow", "show", {
            "image": _binding("image"), "title": _literal(title),
            "options": _literal(options or {}),
        }))
    steps.append(_step("callbacks", "PIL.Image.Image", "info", {}, _binding("image")))
    return {
        "case_id": case_id, "surface": surface, "operation": operation,
        "covers": covers, "assets": [],
        "steps": steps, "observations": [*register_ids, call_id, "callbacks"],
    }


def default_display_case(surface, operation, requirement, case_id):
    """Keep default requirement generation safe for the display family."""
    if requirement["dimension"] == "error_path":
        return platform_viewer_case(
            surface,
            operation["id"],
            case_id,
            requirement["id"],
            "XDGViewer",
            launch_failure="file-not-found",
        )
    registrations = None
    if operation["id"] == "register":
        registrations = ({"order": 1, "registration": "class"},)
    else:
        registrations = ({"hook": "show_file"},)
    return display_case(
        surface, operation["id"], case_id, [requirement["id"]],
        size=(1024, 768) if requirement["dimension"] == "performance" else (2, 1),
        title="capture title \"quotes\" and λ" if ".parameter.title" in requirement["id"] else None,
        registrations=registrations,
        options={"capture_option": "forwarded"}
        if ".parameter.options" in requirement["id"] else None,
    )


def platform_viewer_case(
    surface: str,
    operation: str,
    case_id: str,
    requirement_id: str,
    viewer_class: str,
    *,
    launch_failure: str | None = None,
) -> dict[str, Any]:
    """Exercise a built-in viewer while capturing, never launching, processes."""
    steps = [
        _step(
            "image",
            "PIL.Image",
            "new",
            {
                "mode": _literal("LA"),
                "size": _literal([2, 1]),
                "color": _literal([109, 233]),
            },
        ),
        _step(
            "register",
            "PIL.ImageShow",
            "register",
            {
                "viewer": _literal(
                    {
                        "protocol": "capture-platform-viewer",
                        "viewer_id": "platform-viewer",
                        "image_step_id": "image",
                        "viewer_class": viewer_class,
                        "launch_failure": launch_failure,
                    }
                ),
                "order": _literal(0),
            },
        ),
    ]
    call_id = "call"
    if surface == "PIL.Image.Image":
        steps.append(
            _step(
                call_id,
                surface,
                operation,
                {"title": _literal("viewer title λ")},
                _binding("image"),
            )
        )
    else:
        steps.append(
            _step(
                call_id,
                "PIL.ImageShow",
                "show",
                {
                    "image": _binding("image"),
                    "title": _literal("viewer title λ"),
                    "options": _literal({}),
                },
            )
        )
    observations = ["register", call_id]
    if launch_failure is None:
        steps.append(
            _step("callbacks", "PIL.Image.Image", "info", {}, _binding("image"))
        )
        observations.append("callbacks")
    return {
        "case_id": case_id,
        "surface": surface,
        "operation": operation,
        "covers": [requirement_id],
        "assets": [],
        "steps": steps,
        "observations": observations,
    }


def nuanced_display_cases(surface, operations):
    """Mode selection, callback forwarding and registry ordering stimuli."""
    cases = []
    for operation in ("show", "register"):
        definition = operations.get((surface, operation))
        if (surface, operation) not in DISPLAY_OPERATIONS or definition is None:
            continue
        requirement = f"{surface}.{operation}.behavior.default"
        prefix = f"{surface}.{operation}.capture"
        if operation == "register":
            for order in (-2, 0, 1, 3):
                cases.append(display_case(surface, operation, f"{prefix}.order-{order}", [requirement],
                    registrations=({"order": order, "accept": 0}, {"order": 1, "accept": 1})))
            continue
        cases.append(display_case(
            surface, operation, f"{prefix}.la-file-PNG", [requirement],
            mode="LA", color=(109, 233),
            registrations=({"hook": "show_file", "format": "PNG"},),
        ))
        for mode, color in (
            ("1", 1), ("L", 109), ("LA", (109, 233)), ("P", 1),
            ("PA", (1, 233)), ("RGB", (31, 79, 197)),
            ("RGBA", (31, 79, 197, 233)), ("CMYK", (31, 79, 197, 11)),
            ("YCbCr", (109, 61, 197)), ("HSV", (31, 79, 197)),
            ("I", 109), ("F", 109.5), ("I;16", 1027),
        ):
            for hook, format in (("show", None), ("show_image", None), ("show_image", "PNG")):
                label = f"{mode.lower().replace(';', '')}-{hook}-{format or 'default'}"
                cases.append(display_case(surface, operation, f"{prefix}.{label}", [requirement],
                    mode=mode, color=color, title="callback title λ",
                    registrations=({"hook": hook, "format": format},)))
        for label, registrations in (
            ("prepend-short-circuit", ({"order": 1, "accept": 0}, {"order": 0, "accept": 1}, {"order": 1, "accept": 1})),
            ("append-after-decline", ({"order": 0, "accept": 0}, {"order": 1, "accept": 1})),
            ("all-decline", ({"order": 0, "accept": 0}, {"order": 1, "accept": 0})),
        ):
            cases.append(display_case(surface, operation, f"{prefix}.{label}", [requirement],
                registrations=registrations))
        launch_requirement = f"{surface}.{operation}.error.viewer-launch"
        for viewer_class in PLATFORM_VIEWER_CLASSES:
            viewer_slug = viewer_class.removesuffix("Viewer").lower()
            cases.append(
                platform_viewer_case(
                    surface,
                    operation,
                    f"{prefix}.platform.{viewer_slug}.process",
                    requirement,
                    viewer_class,
                )
            )
            cases.append(
                platform_viewer_case(
                    surface,
                    operation,
                    f"{prefix}.platform.{viewer_slug}.launch-failure",
                    launch_requirement,
                    viewer_class,
                    launch_failure="file-not-found",
                )
            )
    return cases
