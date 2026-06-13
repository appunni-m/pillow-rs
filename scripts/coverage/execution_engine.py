#!/usr/bin/env python3
"""Shared execution engine — type-driven dispatch, backend-agnostic.

This module is imported by BOTH the fixture generator (PIL backend)
and the test suite (RSPIL backend). The same execute() function works
for any backend that implements the 7 handler methods.

For JS/WASM: port this function + implement a WasmBackend with the
same 7 handler methods. The JSON fixtures need no changes.
"""


def execute(backend, op_def, img, img2=None):
    """Execute an operation against image(s) using the provided backend.

    The op_def dict has: type, module, target, params.
    This function purely routes based on type. All backend-specific
    logic (constructors, type coercion, API differences) lives in
    the backend's handler methods.
    """
    typ = op_def["type"]
    target = op_def["target"]
    module = op_def.get("module", "")
    params = op_def.get("params", {})

    if typ == "method":
        return backend.call_method(img, module, target, params)

    elif typ == "filter":
        return backend.call_filter(img, module, target, params)

    elif typ == "dual":
        return backend.call_dual(module, target, img, img2, params)

    elif typ == "draw":
        return backend.call_draw(img, module, target, params)

    elif typ == "enhance":
        return backend.call_enhance(img, module, target, params)

    elif typ == "classmethod":
        return backend.call_classmethod(module, target, params, img)

    elif typ == "value":
        return backend.call_value(img, module, target, params)

    raise ValueError(f"Unknown operation type: {typ}")
