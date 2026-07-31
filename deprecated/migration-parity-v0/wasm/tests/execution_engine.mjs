/**
 * Shared execution engine — type-driven dispatch, backend-agnostic.
 *
 * Direct port of scripts/coverage/execution_engine.py.
 * Dispatches to the right handler method on the backend based on op type.
 *
 * Any backend that implements the 7 handler methods (call_method, call_filter,
 * call_dual, call_draw, call_enhance, call_classmethod, call_value) can be
 * driven by this function. The JSON fixtures need no changes.
 *
 * @param {object} backend - A backend instance implementing the 7 handler methods
 * @param {object} opDef - Operation definition with type, module, target, params
 * @param {object} img - Primary image (Image or first of dual)
 * @param {object|null} img2 - Secondary image for dual operations
 * @returns {*} The operation result (Image, value, or null)
 */
export function execute(backend, opDef, img, img2 = null) {
    const typ = opDef.type;
    const target = opDef.target;
    const module = opDef.module || "";
    const params = opDef.params || {};

    if (typ === "method") {
        return backend.call_method(img, module, target, params);
    } else if (typ === "filter") {
        return backend.call_filter(img, module, target, params);
    } else if (typ === "dual") {
        return backend.call_dual(module, target, img, img2, params);
    } else if (typ === "draw") {
        return backend.call_draw(img, module, target, params);
    } else if (typ === "enhance") {
        return backend.call_enhance(img, module, target, params);
    } else if (typ === "classmethod") {
        return backend.call_classmethod(module, target, params, img);
    } else if (typ === "value") {
        return backend.call_value(img, module, target, params);
    }

    throw new Error(`Unknown operation type: ${typ}`);
}
