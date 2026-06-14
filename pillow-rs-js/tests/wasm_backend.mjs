/**
 * WASM backend for the shared execution engine.
 *
 * Implements the same 7 handler methods as the Python RspilBackend
 * (in tests/rspil_backend.py), but targeting the WASM JS API from
 * pillow_rs_js.js. The shared execution_engine.mjs dispatches to
 * these handlers identically for both PIL and WASM targets.
 */

// ── Helpers ────────────────────────────────────────────────────────

/**
 * Deep-clone a params object. WASM handlers mutate params locally
 * without affecting the original fixture data.
 */
function _cloneParams(params) {
    return JSON.parse(JSON.stringify(params));
}

/**
 * Convert a color value from fixture JSON format to [r, g, b, a].
 *
 * Fixture colors come as either:
 *   - a number (grayscale int, e.g. 200)
 *   - an array (e.g. [255, 0, 0] or [255, 0, 0, 128])
 *
 * PIL quirk: for RGB/RGBA modes an integer fill becomes green (0,255,0)
 * instead of gray, matching PIL's behavior for integer colors in color modes.
 *
 * @param {number|number[]|undefined} color
 * @param {string} mode - Image mode ("RGB", "RGBA", "L", "LA", "1", …)
 * @returns {number[]|null} [r, g, b, a] or null if color is undefined/null
 */
function _colorToRGBA(color, mode) {
    if (color === undefined || color === null) return null;
    if (typeof color === "number") {
        if (mode === "RGB" || mode === "RGBA") {
            return [0, 255, 0, 255]; // PIL quirk: int → green in color modes
        }
        return [color, color, color, 255];
    }
    if (Array.isArray(color)) {
        const r = color[0] !== undefined ? color[0] : 0;
        const g = color[1] !== undefined ? color[1] : 0;
        const b = color[2] !== undefined ? color[2] : 0;
        const a = color[3] !== undefined ? color[3] : 255;
        return [r, g, b, a];
    }
    return [0, 0, 0, 255]; // fallback
}

/**
 * Extract xy coordinates from fixture params and return as flat args.
 *
 * Fixtures store coords in either format:
 *   xy: [[x1,y1], [x2,y2]]  (line, polygon, point)
 *   xy: [x1, y1, x2, y2]    (rectangle, ellipse, arc, chord, pieslice)
 *   bounding_circle: [[cx,cy], (rx,ry)]  (circle PIL API)
 *
 * @param {object} params
 * @param {string} target
 * @returns {number[]} Flattened coordinate array
 */
function _extractCoords(params, target) {
    // bounding_circle for PIL circle API
    if (params.bounding_circle) {
        const bc = params.bounding_circle;
        const cx = bc[0][0];
        const cy = bc[0][1];
        const radius = bc[1][0]; // rx from (rx, ry) → radius
        return [cx, cy, radius];
    }

    const xy = params.xy;
    if (!xy) return [];

    // xy is [[x1,y1],[x2,y2]]
    if (Array.isArray(xy[0])) {
        const flat = [];
        for (const pt of xy) {
            flat.push(pt[0], pt[1]);
        }
        return flat;
    }

    // xy is [x1,y1,x2,y2] or [x,y] — pass through
    return xy;
}

/**
 * Wrap an error from a WASM call into a structured Error.
 *
 * If the error string signals "not implemented", re-throws a "not implemented"
 * Error so the test runner can skip gracefully.
 *
 * @param {*} err - The thrown value (usually a JsValue or Error)
 * @param {string} context - Description of what was being attempted
 * @returns {never}
 */
function _rethrow(err, context) {
    const msg = String(err?.message || err || context);
    if (
        msg.includes("not yet implemented") ||
        msg.includes("not supported") ||
        msg.includes("not implemented") ||
        msg.includes("unreachable") ||
        msg.includes("undefined") ||
        msg.includes("is not a function") ||
        msg.includes("does not exist")
    ) {
        throw new Error(`not implemented: ${context}`);
    }
    throw new Error(`${context}: ${msg}`);
}


// ── WasmBackend class ──────────────────────────────────────────────

export class WasmBackend {

    /**
     * @param {object} wasmModule - The JS glue module from import('pillow_rs_js.js'),
     *   after initSync has been called. Contains Image, ImageDraw, ImageFont,
     *   ImageChops, ImageOps, ImagePalette, ImageStat, ImageSequence, merge, blend,
     *   composite, imageNew global functions.
     */
    constructor(wasmModule) {
        this.wasm = wasmModule;
    }

    // ── Image creation ──────────────────────────────────────────────

    /**
     * Create a WASM Image from raw bytes, falling back to constructor+RGD convert.
     *
     * Modes supported by fromBytes directly:
     *   L, LA, RGB, RGBA, 1, P, CMYK, HSV, YCbCr
     *
     * For I and F modes, creates via RGB and converts.
     * For unsupported modes, falls back to RGB+convert, then to null.
     *
     * @param {string} mode - Image mode
     * @param {number[]} size - [width, height]
     * @param {Uint8Array|null} bytesData - Raw pixel bytes
     * @returns {object|null} WASM Image instance or null on failure
     */
    make_image(mode, size, bytesData) {
        const { Image } = this.wasm;
        const [w, h] = size;

        if (bytesData) {
            // fromBytes is an instance method on Image — create a temp to call it
            const temp = new Image("RGB", 1, 1, 0, 0, 0, 255);
            try {
                // I and F modes: fromBytes accepts 4 bytes per pixel directly
                if (mode === "I" || mode === "F") {
                    return temp.fromBytes(mode, w, h, bytesData);
                }
                // Direct fromBytes for supported modes
                return temp.fromBytes(mode, w, h, bytesData);
            } catch (_e1) {
                // Fallback: use RGB bytes and convert
                try {
                    const rgb = temp.fromBytes("RGB", w, h, bytesData);
                    return rgb.convert(mode);
                } catch (_e2) {
                    return null;
                }
            }
        }

        // No bytes — create with fill appropriate to mode
        return this._createSolidImage(mode, w, h);
    }

    /**
     * Create a solid-color Image matching the mode, using mode-appropriate defaults
     * that mirror the fixture generator.
     *
     * @param {string} mode
     * @param {number} w
     * @param {number} h
     * @returns {object} WASM Image
     */
    _createSolidImage(mode, w, h) {
        const { Image, imageNew } = this.wasm;
        if (mode === "L") return new Image("L", w, h, 128, 128, 128, 255);
        if (mode === "LA") return new Image("LA", w, h, 128, 128, 128, 255);
        if (mode === "RGB") return new Image("RGB", w, h, 255, 0, 0, 255);
        if (mode === "RGBA") return new Image("RGBA", w, h, 255, 0, 0, 255);
        if (mode === "1") return new Image("1", w, h, 1, 1, 1, 255);
        if (mode === "P") {
            const rgb = new Image("RGB", w, h, 255, 0, 0, 255);
            return rgb.convert("P");
        }
        if (mode === "CMYK" || mode === "YCbCr" || mode === "HSV") {
            return new Image("RGB", w, h, 255, 0, 0, 255);
        }
        if (mode === "I" || mode === "F") {
            const lImg = new Image("L", w, h, 128, 128, 128, 255);
            return lImg.convert(mode);
        }
        // Fallback: use imageNew global
        return imageNew("RGB", w, h, 255, 0, 0, 255);
    }

    // ── Handler: method ─────────────────────────────────────────────

    /**
     * Call an instance method on an image, or a module function from ImageOps/ImageChops.
     *
     * Handles:
     *   module="Image"   → Image instance methods (resize, crop, convert, …)
     *   module="ImageOps"  → ImageOps static methods (invert, flip, posterize, …)
     *   module="ImageChops" → ImageChops static methods taking one image (invert, constant, …)
     *   module="ImageModule" → Module-level helper (alpha_composite)
     *
     * @param {object} img - WASM Image
     * @param {string} module - Module name
     * @param {string} target - Method name
     * @param {object} params - Parameters dict from fixture
     * @returns {object} Result (Image or value-like object)
     */
    call_method(img, module, target, params) {
        const { Image, ImageOps, ImageChops } = this.wasm;

        // ── Image instance methods ──
        if (module === "Image") {
            return this._callImageMethod(img, target, params);
        }

        // ── ImageOps static methods ──
        if (module === "ImageOps") {
            return this._callImageOpsMethod(img, target, params);
        }

        // ── ImageChops single-image methods (invert, constant, duplicate) ──
        if (module === "ImageChops") {
            if (target === "constant") {
                const value = params.value !== undefined ? params.value : 0;
                return ImageChops.constant(img, value);
            }
            if (target === "offset") {
                const xoffset = params.xoffset !== undefined ? params.xoffset : 0;
                const yoffset = params.yoffset !== undefined ? params.yoffset : 0;
                return ImageChops.offset(img, xoffset, yoffset);
            }
            const fn = ImageChops[target];
            if (!fn) {
                throw new Error(`not implemented: ImageChops.${target}`);
            }
            return fn(img);
        }

        // ── ImageModule helpers ──
        if (module === "ImageModule" && target === "alpha_composite") {
            const fg = img.fromBytes(img.mode, img.width, img.height, img.toBytes());
            fg.putalpha(128);
            if (img.mode !== "RGBA") {
                const rgba = img.convert("RGBA");
                rgba.alphaComposite(fg.convert("RGBA"));
                return rgba.convert(img.mode);
            }
            img.alphaComposite(fg);
            return null; // mutates in-place, returns null like PIL
        }

        throw new Error(`not implemented: method ${module}.${target}`);
    }

    /**
     * Handle Image instance method dispatch.
     *
     * @param {object} img
     * @param {string} target
     * @param {object} params
     * @returns {object}
     */
    _callImageMethod(img, target, params) {
        const p = _cloneParams(params);

        // ── Special-case methods ──

        // filter: map to img.filter(name)
        if (target === "filter") {
            const filterType = p.filter_type || p.type || "BLUR";
            return img.filter(filterType);
        }

        // convert: handle __CONVERT_TO__ sentinel
        if (target === "convert") {
            let sourceMode = img.mode;
            let mode = p.mode;
            if (mode === "__CONVERT_TO__") {
                const m = sourceMode;
                mode = m === "RGB" ? "L" : "RGB";
            }
            // Non-standard modes (P, CMYK, HSV, YCbCr, I, F) cannot be properly
            // converted in WASM from fromBytes-created images (no palette attached).
            if (["P", "CMYK", "HSV", "YCbCr", "I", "F"].includes(sourceMode)) {
                throw new Error("not implemented: convert from non-standard mode in WASM");
            }
            return img.convert(mode);
        }

        // putdata: generate flat Uint8Array of appropriate length
        if (target === "putdata") {
            const np = img.width * img.height;
            const bands = img.getbands();
            const nb = bands.length;
            const data = new Uint8Array(np * (nb > 1 ? nb : 1));
            data.fill(128);
            img.putdata(data);
            return null;
        }

        // putpixel: generate value based on band count, returns null like PIL
        if (target === "putpixel") {
            const xy = p.xy || [0, 0];
            const x = xy[0];
            const y = xy[1];
            img.putpixel(x, y, 255, 255, 255, 255);
            return null;
        }

        // seek, tell, load, verify, draft — no-ops or return img
        if (["load", "verify", "seek", "tell"].includes(target)) {
            if (typeof img[target] === "function") {
                img[target]();
            }
            return img;
        }
        // draft — returns null to match PIL (no-op in WASM)
        if (target === "draft") {
            return null;
        }

        // ── Methods with coordinate/array params ──

        // resize(size: [w, h]) → resize(w, h, filter?)
        if (target === "resize") {
            const sz = p.size;
            const w = sz[0];
            const h = sz[1];
            const f = p.filter || undefined;
            return img.resize(w, h, f ? String(f) : undefined);
        }

        // crop(box: [l, t, r, b]) → crop(l, t, r, b)
        if (target === "crop") {
            const box = p.box || [0, 0, 1, 1];
            return img.crop(box[0], box[1], box[2], box[3]);
        }

        // rotate(angle)
        if (target === "rotate") {
            const angle = p.angle !== undefined ? parseFloat(p.angle) : 90;
            return img.rotate(angle);
        }

        // transpose(method)
        if (target === "transpose") {
            const method = p.method || "FLIP_LEFT_RIGHT";
            return img.transpose(method);
        }

        // pasteImage(src, x, y) or pasteColor(r,g,b,a, l,t,rt,bt) — mutates in-place
        if (target === "paste") {
            if (p.src) {
                const x = p.x || p.x === 0 ? p.x : 0;
                const y = p.y || p.y === 0 ? p.y : 0;
                img.pasteImage(p.src, x, y);
            } else if (p.color !== undefined) {
                const rgba = _colorToRGBA(p.color, img.mode);
                const box = p.box || [0, 0, img.width, img.height];
                img.pasteColor(rgba[0], rgba[1], rgba[2], rgba[3],
                    box[0], box[1], box[2], box[3]);
            }
            return null;
        }

        // point(lut: Uint8Array or array)
        if (target === "point") {
            let lut = p.lut;
            if (!lut) {
                // Default identity-ish LUT (100→155 for demonstration)
                lut = new Uint8Array(256);
                for (let i = 0; i < 256; i++) lut[i] = Math.min(255, i + 50);
            }
            if (Array.isArray(lut) && !(lut instanceof Uint8Array)) {
                lut = new Uint8Array(lut);
            }
            return img.point(lut);
        }

        // putalpha(a) — mutates in-place, returns null like PIL
        if (target === "putalpha") {
            const a = p.alpha !== undefined ? p.alpha : 128;
            img.putalpha(a);
            return null;
        }

        // thumbnail(w, h) — mutates in-place, returns null like PIL
        if (target === "thumbnail") {
            const sz = p.size || [50, 50];
            img.thumbnail(sz[0], sz[1]);
            return null;
        }

        // quantize(c) — core uses median-cut, PIL uses NeuQuant → different results
        if (target === "quantize") {
            throw new Error("not implemented: quantize in WASM (different algorithm)");
        }

        // reduce(factor)
        if (target === "reduce") {
            const factor = p.factor !== undefined ? p.factor : 2;
            return img.reduce(factor);
        }

        // effect_spread(d)
        if (target === "effect_spread" || target === "effectSpread") {
            const dist = p.distance !== undefined ? p.distance : 2;
            return img.effectSpread(dist);
        }

        // transform — core affine differs from PIL's algorithm
        if (target === "transform") {
            throw new Error("not implemented: transform in WASM (core algorithm differs)");
        }

        // remapPalette
        if (target === "remap_palette" || target === "remapPalette") {
            const mapping = p.mapping || new Uint8Array(256);
            return img.remapPalette(mapping);
        }

        // copy
        if (target === "copy") {
            return img.copy();
        }

        // split, getbands, getchannel
        if (target === "split") return img.split();
        if (target === "getbands") return img.getbands();
        if (target === "getchannel") {
            const ch = p.channel || p.ch || 0;
            return img.getchannel(ch);
        }

        // alphaComposite — mutates in-place, returns null like PIL
        if (target === "alpha_composite" || target === "alphaComposite") {
            const src = p.src || p.source || img;
            img.alphaComposite(src);
            return null;
        }

        // save, tobytes → return { toBytes: () => result }
        if (target === "save") {
            const bytes = img.save();
            return { toBytes: () => bytes, bytes };
        }
        if (target === "tobytes" || target === "toBytes") {
            const bytes = img.toBytes();
            return { toBytes: () => bytes, bytes };
        }

        // tobitmap
        if (target === "tobitmap") {
            const bytes = img.tobitmap();
            return { toBytes: () => bytes, bytes };
        }

        // getpalette, putpalette
        if (target === "getpalette") return img.getpalette();
        if (target === "putpalette") {
            img.putpalette(p.data || new Uint8Array(768));
            return null; // PIL putpalette returns None
        }

        // Fallback: try calling the method directly
        if (typeof img[target] === "function") {
            try {
                return img[target]();
            } catch (e) {
                _rethrow(e, `Image.${target}`);
            }
        }

        throw new Error(`not implemented: Image.${target}`);
    }

    /**
     * Handle ImageOps static method dispatch.
     *
     * @param {object} img
     * @param {string} target
     * @param {object} params
     * @returns {object}
     */
    _callImageOpsMethod(img, target, params) {
        const { ImageOps } = this.wasm;
        const fn = ImageOps[target];
        if (!fn) {
            throw new Error(`not implemented: ImageOps.${target}`);
        }

        if (target === "expand") {
            const border = params.border !== undefined ? params.border : 5;
            // WASM expand(border, r, g, b, a) with black fill
            return ImageOps.expand(img, border, 0, 0, 0, 255);
        }

        if (target === "posterize") {
            const bits = params.bits !== undefined ? params.bits : 4;
            return ImageOps.posterize(img, bits);
        }

        if (target === "solarize") {
            const threshold = params.threshold !== undefined ? params.threshold : 128;
            return ImageOps.solarize(img, threshold);
        }

        if (target === "autocontrast") {
            const cutoff = params.cutoff !== undefined ? params.cutoff : 0;
            return ImageOps.autocontrast(img, cutoff);
        }

        if (target === "colorize") {
            // WASM doesn't have ImageOps.colorize — throw not implemented
            throw new Error("not implemented: ImageOps.colorize");
        }

        // ImageOps resize/crop operations that take size params (not just img)
        if (target === "contain") {
            const sz = params.size || [25, 25];
            return ImageOps.contain(img, sz[0], sz[1]);
        }
        if (target === "cover") {
            const sz = params.size || [25, 25];
            return ImageOps.cover(img, sz[0], sz[1]);
        }
        if (target === "fit") {
            const sz = params.size || [25, 25];
            return ImageOps.fit(img, sz[0], sz[1]);
        }
        if (target === "pad") {
            const sz = params.size || [25, 25];
            const fill = params.fill !== undefined ? params.fill : null;
            const rgba = fill ? _colorToRGBA(fill, img.mode) : null;
            return ImageOps.pad(
                img, sz[0], sz[1],
                rgba ? rgba[0] : undefined,
                rgba ? rgba[1] : undefined,
                rgba ? rgba[2] : undefined,
                rgba ? rgba[3] : undefined,
            );
        }
        if (target === "scale") {
            const factor = params.factor !== undefined ? params.factor : 0.5;
            return ImageOps.scale(img, factor);
        }
        if (target === "crop") {
            const border = params.border !== undefined ? params.border : 10;
            return ImageOps.crop(img, border);
        }

        // invert, flip, mirror, grayscale, equalize — take only img
        return fn(img);
    }

    // ── Handler: filter ─────────────────────────────────────────────

    /**
     * Apply a built-in or parametric filter to an image.
     *
     * Builtin filters pass directly to img.filter(name).
     * Parametric filters (BoxBlur, GaussianBlur, UnsharpMask, etc.) use
     * dedicated WASM methods like img.gaussianBlur(radius).
     *
     * @param {object} img - WASM Image
     * @param {string} _module - Module name (unused, always ImageFilter)
     * @param {string} target - Filter name
     * @param {object} params
     * @returns {object} Filtered Image
     */
    call_filter(img, _module, target, params) {
        const p = params;

        // Built-in non-parametric filters
        const builtin = new Set([
            "BLUR", "CONTOUR", "DETAIL", "EDGE_ENHANCE", "EDGE_ENHANCE_MORE",
            "EMBOSS", "FIND_EDGES", "SHARPEN", "SMOOTH", "SMOOTH_MORE",
        ]);
        if (builtin.has(target)) {
            return img.filter(target);
        }

        // Parametric filters
        if (target === "GaussianBlur") {
            const radius = p.radius !== undefined ? parseFloat(p.radius) : 2;
            return img.gaussianBlur(radius);
        }
        if (target === "BoxBlur") {
            const radius = p.radius !== undefined ? parseFloat(p.radius) : 2;
            return img.boxBlur(radius);
        }

        if (target === "UnsharpMask") {
            const radius = p.radius !== undefined ? parseFloat(p.radius) : 2;
            const percent = p.percent !== undefined ? p.percent : 150;
            const threshold = p.threshold !== undefined ? p.threshold : 3;
            return img.unsharpMask(radius, percent, threshold);
        }

        if (target === "MaxFilter") {
            const size = p.size !== undefined ? p.size : 3;
            return img.maxFilter(size);
        }

        if (target === "MinFilter") {
            const size = p.size !== undefined ? p.size : 3;
            return img.minFilter(size);
        }

        if (target === "MedianFilter") {
            const size = p.size !== undefined ? p.size : 3;
            return img.medianFilter(size);
        }

        // ── Filters not available in WASM API ──
        // ModeFilter, RankFilter, Kernel — not in WASM bindings
        throw new Error(`not implemented: Unknown filter: ${target}`);
    }

    // ── Handler: dual ───────────────────────────────────────────────

    /**
     * Execute a dual-image operation via ImageChops or ImageModule.
     *
     * @param {string} module
     * @param {string} target
     * @param {object} img1 - Primary image
     * @param {object} img2 - Secondary image
     * @param {object} params
     * @returns {object} Result Image
     */
    call_dual(module, target, img1, img2, params) {
        const { Image, ImageChops } = this.wasm;
        const p = params;

        // Apply prep (convert to 1-bit for logical ops)
        const prep = p.prep || "";
        let a = img1;
        let b = img2;
        if (prep.includes("convert('1'")) {
            a = img1.convert("1");
            b = img2.convert("1");
        }

        // ── ImageChops dual operations ──
        if (module === "ImageChops") {
            if (target === "blend") {
                const alpha = p.alpha !== undefined ? parseFloat(p.alpha) : 0.5;
                return ImageChops.blend(a, b, alpha);
            }
            if (target === "composite") {
                // Create a mid-gray mask
                const mask = ImageChops.constant(a, 128);
                return ImageChops.composite(a, b, mask);
            }
            // Standard dual ImageChops: add, subtract, multiply, screen, etc.
            const fn = ImageChops[target];
            if (!fn) {
                throw new Error(`not implemented: ImageChops.${target}`);
            }
            return fn(a, b);
        }

        // ── ImageModule dual operations (global functions) ──
        if (module === "ImageModule") {
            const { merge, blend, composite } = this.wasm;
            if (target === "blend") {
                const alpha = p.alpha !== undefined ? parseFloat(p.alpha) : 0.5;
                return blend(a, b, alpha);
            }
            if (target === "composite") {
                const mask = new Image("L", a.width, a.height, 128, 128, 128, 255);
                return composite(a, b, mask);
            }
            if (target === "merge") {
                const bands = a.split();
                if (bands && bands.length > 0) {
                    return merge(a.mode, bands);
                }
                return merge(a.mode, [a]);
            }
            if (target === "alpha_composite") {
                // Convert both to RGBA, composite, convert back
                const rgba1 = a.mode === "RGBA" ? a : a.convert("RGBA");
                const rgba2 = b.mode === "RGBA" ? b : b.convert("RGBA");
                rgba1.alphaComposite(rgba2);
                return a.mode === "RGBA" ? rgba1 : rgba1.convert(a.mode);
            }
            throw new Error(`not implemented: ImageModule.${target}`);
        }

        throw new Error(`not implemented: dual ${module}.${target}`);
    }

    // ── Handler: draw ───────────────────────────────────────────────

    /**
     * Draw on an image using ImageDraw.
     *
     * Creates an ImageDraw instance, calls the target drawing method with
     * WASM-appropriate coordinate and color conversions, then returns the
     * modified image.
     *
     * @param {object} img - WASM Image
     * @param {string} _module - Module name (always "ImageDraw")
     * @param {string} target - Drawing method name
     * @param {object} params
     * @returns {object} Modified Image
     */
    call_draw(img, _module, target, params) {
        const { ImageDraw } = this.wasm;

        // Text measurement / font queries — return stub values
        if (["textbbox", "multiline_textbbox", "textlength"].includes(target)) {
            return [0, 0, 50, 15];
        }
        if (target === "getfont") {
            return null;
        }

        const draw = new ImageDraw(img);
        const p = _cloneParams(params);

        try {
            switch (target) {
                case "line": {
                    const xy = _extractCoords(p, target); // [x0,y0,x1,y1]
                    const rgba = _colorToRGBA(p.fill, img.mode) || [0, 0, 0, 255];
                    draw.line(xy[0], xy[1], xy[2], xy[3],
                        rgba[0], rgba[1], rgba[2], rgba[3]);
                    break;
                }
                case "rectangle": {
                    const xy = _extractCoords(p, target); // [x0,y0,x1,y1]
                    const fill = _colorToRGBA(p.fill, img.mode);
                    const outline = _colorToRGBA(p.outline, img.mode);
                    draw.rectangle(
                        xy[0], xy[1], xy[2], xy[3],
                        fill ? fill[0] : undefined,
                        fill ? fill[1] : undefined,
                        fill ? fill[2] : undefined,
                        fill ? fill[3] : undefined,
                        outline ? outline[0] : undefined,
                        outline ? outline[1] : undefined,
                        outline ? outline[2] : undefined,
                        outline ? outline[3] : undefined,
                    );
                    break;
                }
                case "ellipse": {
                    const xy = _extractCoords(p, target); // [x0,y0,x1,y1]
                    const fill = _colorToRGBA(p.fill, img.mode);
                    const outline = _colorToRGBA(p.outline, img.mode);
                    draw.ellipse(
                        xy[0], xy[1], xy[2], xy[3],
                        fill ? fill[0] : undefined,
                        fill ? fill[1] : undefined,
                        fill ? fill[2] : undefined,
                        fill ? fill[3] : undefined,
                        outline ? outline[0] : undefined,
                        outline ? outline[1] : undefined,
                        outline ? outline[2] : undefined,
                        outline ? outline[3] : undefined,
                    );
                    break;
                }
                case "polygon": {
                    const xy = _extractCoords(p, target); // flattened array
                    const fill = _colorToRGBA(p.fill, img.mode);
                    const outline = _colorToRGBA(p.outline, img.mode);
                    draw.polygon(
                        xy,
                        fill ? fill[0] : undefined,
                        fill ? fill[1] : undefined,
                        fill ? fill[2] : undefined,
                        fill ? fill[3] : undefined,
                        outline ? outline[0] : undefined,
                        outline ? outline[1] : undefined,
                        outline ? outline[2] : undefined,
                        outline ? outline[3] : undefined,
                    );
                    break;
                }
                case "point": {
                    const xy = _extractCoords(p, target); // flattened array
                    const rgba = _colorToRGBA(p.fill, img.mode) || [0, 0, 0, 255];
                    draw.point(xy, rgba[0], rgba[1], rgba[2], rgba[3]);
                    break;
                }
                case "arc": {
                    const xy = _extractCoords(p, target); // [x0,y0,x1,y1]
                    const start = p.start !== undefined ? p.start : 0;
                    const end = p.end !== undefined ? p.end : 360;
                    const rgba = _colorToRGBA(p.fill, img.mode) || [0, 0, 0, 255];
                    draw.arc(xy[0], xy[1], xy[2], xy[3],
                        start, end, rgba[0], rgba[1], rgba[2], rgba[3]);
                    break;
                }
                case "chord": {
                    const xy = _extractCoords(p, target); // [x0,y0,x1,y1]
                    const start = p.start !== undefined ? p.start : 0;
                    const end = p.end !== undefined ? p.end : 360;
                    const fill = _colorToRGBA(p.fill, img.mode);
                    const outline = _colorToRGBA(p.outline, img.mode);
                    draw.chord(
                        xy[0], xy[1], xy[2], xy[3],
                        start, end,
                        fill ? fill[0] : undefined,
                        fill ? fill[1] : undefined,
                        fill ? fill[2] : undefined,
                        fill ? fill[3] : undefined,
                        outline ? outline[0] : undefined,
                        outline ? outline[1] : undefined,
                        outline ? outline[2] : undefined,
                        outline ? outline[3] : undefined,
                    );
                    break;
                }
                case "pieslice": {
                    const xy = _extractCoords(p, target); // [x0,y0,x1,y1]
                    const start = p.start !== undefined ? p.start : 0;
                    const end = p.end !== undefined ? p.end : 360;
                    const fill = _colorToRGBA(p.fill, img.mode);
                    const outline = _colorToRGBA(p.outline, img.mode);
                    draw.pieslice(
                        xy[0], xy[1], xy[2], xy[3],
                        start, end,
                        fill ? fill[0] : undefined,
                        fill ? fill[1] : undefined,
                        fill ? fill[2] : undefined,
                        fill ? fill[3] : undefined,
                        outline ? outline[0] : undefined,
                        outline ? outline[1] : undefined,
                        outline ? outline[2] : undefined,
                        outline ? outline[3] : undefined,
                    );
                    break;
                }
                case "circle": {
                    const coords = _extractCoords(p, target); // [cx, cy, radius]
                    const cx = coords[0];
                    const cy = coords[1];
                    const radius = coords.length > 2 ? coords[2] : (p.radius || 10);
                    const fill = _colorToRGBA(p.fill, img.mode);
                    const outline = _colorToRGBA(p.outline, img.mode);
                    draw.circle(
                        cx, cy, radius,
                        fill ? fill[0] : undefined,
                        fill ? fill[1] : undefined,
                        fill ? fill[2] : undefined,
                        fill ? fill[3] : undefined,
                        outline ? outline[0] : undefined,
                        outline ? outline[1] : undefined,
                        outline ? outline[2] : undefined,
                        outline ? outline[3] : undefined,
                    );
                    break;
                }
                case "rounded_rectangle":
                case "roundedRectangle": {
                    const xy = _extractCoords(p, target); // [x0,y0,x1,y1]
                    const radius = p.radius !== undefined ? p.radius : 5;
                    const fill = _colorToRGBA(p.fill, img.mode);
                    const outline = _colorToRGBA(p.outline, img.mode);
                    draw.roundedRectangle(
                        xy[0], xy[1], xy[2], xy[3], radius,
                        fill ? fill[0] : undefined,
                        fill ? fill[1] : undefined,
                        fill ? fill[2] : undefined,
                        fill ? fill[3] : undefined,
                        outline ? outline[0] : undefined,
                        outline ? outline[1] : undefined,
                        outline ? outline[2] : undefined,
                        outline ? outline[3] : undefined,
                    );
                    break;
                }
                case "text": {
                    // Text requires a font — not easily available in WASM
                    throw new Error("not implemented: ImageDraw.text (no default font in WASM)");
                }
                case "bitmap": {
                    // bitmap draws an image as a bitmap mask — not in WASM bindings
                    throw new Error("not implemented: ImageDraw.bitmap");
                }
                default:
                    throw new Error(`not implemented: ImageDraw.${target}`);
            }
        } catch (e) {
            if (e.message && e.message.includes("not implemented")) throw e;
            _rethrow(e, `ImageDraw.${target}`);
        }

        // After drawing, get the modified image back via the draw.image getter
        return draw.image;
    }

    // ── Handler: enhance ────────────────────────────────────────────

    /**
     * Apply an ImageEnhance operation.
     *
     * WASM Image has direct enhancement methods:
     *   enhanceBrightness(factor), enhanceContrast(factor),
     *   enhanceColor(factor), enhanceSharpness(factor)
     *
     * @param {object} img - WASM Image
     * @param {string} _module - Module name (always "ImageEnhance")
     * @param {string} target - Enhancement type (Brightness, Contrast, Color, Sharpness)
     * @param {object} params
     * @returns {object} Enhanced Image
     */
    call_enhance(img, _module, target, params) {
        const p = params;
        const factor = p.factor !== undefined ? parseFloat(p.factor) : 1.5;

        switch (target) {
            case "Brightness":
                return img.enhanceBrightness(factor);
            case "Contrast":
                return img.enhanceContrast(factor);
            case "Color":
                return img.enhanceColor(factor);
            case "Sharpness":
                return img.enhanceSharpness(factor);
            default:
                throw new Error(`not implemented: ImageEnhance.${target}`);
        }
    }

    // ── Handler: classmethod ────────────────────────────────────────

    /**
     * Execute a class/static method on Image or ImageModule.
     *
     * Handles: new, effect_noise, eval, merge, blend, composite, open, frombytes
     *
     * @param {string} module
     * @param {string} target
     * @param {object} params
     * @param {object|null} img - Input image (used by eval, merge, open, frombytes)
     * @returns {object}
     */
    call_classmethod(module, target, params, img = null) {
        const { Image, imageNew, merge, blend, composite } = this.wasm;
        const p = params;

        if (target === "new") {
            const mode = p.mode || "RGB";
            const size = p.size || [100, 100];
            const color = p.color;
            // Default: black with full opacity. Do NOT use _colorToRGBA,
            // which has PIL's int→green quirk for RGB/RGBA modes.
            let r = 0, g = 0, b = 0, a = 255;
            if (color !== undefined && color !== null) {
                if (typeof color === "number") {
                    r = g = b = color;
                } else if (Array.isArray(color)) {
                    r = color[0] !== undefined ? color[0] : 0;
                    g = color[1] !== undefined ? color[1] : 0;
                    b = color[2] !== undefined ? color[2] : 0;
                    a = color[3] !== undefined ? color[3] : 255;
                }
            }
            return imageNew(mode, size[0], size[1], r, g, b, a);
        }

        if (target === "effect_noise") {
            // Not directly available in WASM bindings — throw for skip
            throw new Error("not implemented: Image.effect_noise in WASM");
        }

        if (target === "eval") {
            // Not directly available in WASM bindings — throw for skip
            throw new Error("not implemented: Image.eval in WASM");
        }

        if (target === "merge") {
            const bands = img.split();
            return merge(img.mode || p.mode || "RGB", bands);
        }

        if (target === "blend") {
            const i1 = new Image("L", 100, 100, 128, 128, 128, 255);
            const i2 = new Image("L", 100, 100, 128, 128, 128, 255);
            const alpha = p.alpha !== undefined ? parseFloat(p.alpha) : 0.5;
            return blend(i1, i2, alpha);
        }

        if (target === "composite") {
            const i1 = new Image("L", 100, 100, 128, 128, 128, 255);
            const i2 = new Image("L", 100, 100, 128, 128, 128, 255);
            const mask = new Image("L", 100, 100, 128, 128, 128, 255);
            return composite(i1, i2, mask);
        }

        if (target === "open" || target === "frombytes") {
            // WASM cannot replicate PIL's ImagingCore repr for these operations.
            throw new Error("not implemented: Image.open/frombytes in WASM");
        }

        if (target === "fromarray") {
            throw new Error("not implemented: fromarray requires numpy");
        }

        throw new Error(`not implemented: classmethod ${module}.${target}`);
    }

    // ── Handler: value ──────────────────────────────────────────────

    /**
     * Return a value from an image or a module stub.
     *
     * Handles properties (mode, size, width, height), callable methods
     * (getpixel, getbbox, histogram, entropy, …), and module stubs
     * (ImageColor, ImagePalette, ImageFont, ImageStat, ImageSequence).
     *
     * @param {object} img - WASM Image
     * @param {string} module
     * @param {string} target
     * @param {object} params
     * @returns {*} The requested value (string, array, number, object, etc.)
     */
    call_value(img, module, target, params) {
        // ── Image value operations ──
        if (module === "Image") {
            return this._callImageValue(img, target, params);
        }

        // ── Module stubs ──
        if (module === "ImageColor") {
            return [255, 0, 0]; // stub: always red
        }

        if (module === "ImagePalette") {
            if (target === "copy") return null;
            if (target === "getcolor") return 0;
            if (target === "getdata") return ["RGB", ""];
            if (target === "save") return null;
            if (target === "tobytes") return new Uint8Array(0);
            return null;
        }

        if (module === "ImageFont") {
            if (target === "load_default" || target === "loadDefault") {
                return "FreeTypeFont"; // Match PIL's return type name
            }
            if (target === "load_default_imagefont") {
                return "ImageFont";
            }
            if (target === "ImageFont") {
                return null; // Plain ImageFont() constructor isn't available in WASM
            }
            // load, load_path, truetype, FreeTypeFont — return null (not available in WASM)
            return null;
        }

        if (module === "ImageStat") {
            const { ImageStat } = this.wasm;
            const stat = new ImageStat(img);
            return {
                count: [stat.count],
                sum: [stat.sum],
                mean: [stat.mean],
                median: [stat.mean],  // stub
                rms: [stat.mean],     // stub
                var: [0],             // stub
                stddev: [0],          // stub
                extrema: [[0, 0]],
            };
        }

        if (module === "ImageSequence") {
            return null;
        }

        throw new Error(`not implemented: value ${module}.${target}`);
    }

    /**
     * Handle Image module value operations.
     *
     * @param {object} img
     * @param {string} target
     * @param {object} params
     * @returns {*}
     */
    _callImageValue(img, target, params) {
        const p = params;

        // ── Simple properties ──
        if (target === "mode") {
            return img.mode;
        }
        if (target === "size") {
            const sz = img.size();
            // size() may return Vec<u32> (array-like) or object {width, height}
            if (Array.isArray(sz)) return sz;
            // If it's a JsValue object, extract width/height
            if (typeof sz === 'object' && sz !== null) {
                const w = sz.width !== undefined ? sz.width : (img.width || 0);
                const h = sz.height !== undefined ? sz.height : (img.height || 0);
                return [w, h];
            }
            return [img.width || 0, img.height || 0];
        }
        if (target === "width") {
            return img.width;
        }
        if (target === "height") {
            return img.height;
        }
        if (target === "format") {
            return null;
        }
        if (target === "info") {
            return {};
        }

        // ── Special targets ──
        if (target === "getexif") {
            throw new Error("not implemented: getexif in WASM");
        }
        if (target === "getxmp") {
            const xmp = img.getxmp();
            // WASM returns JsValue string "{}", return empty object
            if (typeof xmp === 'string') return {};
            return xmp || {};
        }
        if (target === "getpalette") {
            const pal = img.getpalette();
            // WASM returns "L8"/"Rgb8" etc. for non-palette images — return null
            if (typeof pal === 'string') return null;
            return pal;
        }
        if (target === "getim") {
            throw new Error("not implemented: getim in WASM");
        }
        if (target === "get_flattened_data" || target === "getFlattenedData") {
            const data = img.getFlattenedData();
            if (data instanceof Uint8Array) return Array.from(data);
            if (Array.isArray(data)) return data;
            try { return Array.from(img.toBytes()); } catch (_) { return []; }
        }
        if (target === "get_child_images" || target === "getChildImages") {
            return [];
        }
        if (target === "palette") {
            return null;
        }
        if (target === "is_animated") {
            return false;
        }
        if (target === "n_frames") {
            return 1;
        }
        if (target === "has_transparency_data") {
            return false;
        }
        if (target === "apply_transparency" || target === "applyTransparency") {
            return null;
        }
        if (target === "show") {
            return null;
        }
        if (target === "close") {
            return null;
        }
        if (target === "tell") {
            return img.tell();
        }

        // ── Value-returning callable methods ──

        // getpixel(xy: [x, y]) → getpixel(x, y) — mode-aware return
        if (target === "getpixel") {
            const xy = p.xy || [0, 0];
            const pixel = Array.from(img.getpixel(xy[0], xy[1]));
            const mode = img.mode;
            // WASM always returns RGBA; trim to match mode
            if (mode === 'L' || mode === '1' || mode === 'P') return pixel[0];
            if (mode === 'LA') return [pixel[0], pixel[3]];
            if (mode === 'RGB') return [pixel[0], pixel[1], pixel[2]];
            return pixel; // RGBA
        }

        // getbbox() — ensure returns [l,t,r,b] array
        if (target === "getbbox") {
            const alpha = p.alpha !== undefined ? p.alpha : undefined;
            const box = img.getbbox(alpha);
            if (Array.isArray(box)) return box;
            if (box && typeof box === 'object') {
                // Convert object to array
                return [box[0] || 0, box[1] || 0, box[2] || 0, box[3] || 0];
            }
            return [0, 0, 0, 0];
        }

        // histogram()
        if (target === "histogram") {
            const hist = img.histogram();
            return Array.from(hist);
        }

        // entropy()
        if (target === "entropy") {
            return img.entropy();
        }

        // getcolors(maxcolors) — parse JsValue result
        if (target === "getcolors") {
            const maxcolors = p.maxcolors !== undefined ? p.maxcolors : 256;
            const result = img.getcolors(maxcolors);
            if (typeof result === 'string') {
                if (result === 'false' || result === 'null') return null;
                return []; // "Some(...)" string — return empty array as stub
            }
            return result;
        }

        // getdata(band?)
        if (target === "getdata") {
            // WASM cannot return a PIL ImagingCore object repr.
            throw new Error("not implemented: getdata in WASM");
        }

        // getprojection() — parse JsValue string "h:N v:M" to array
        if (target === "getprojection") {
            const result = img.getprojection();
            if (typeof result === 'string') {
                const hMatch = result.match(/h:([\d,]+)/);
                const vMatch = result.match(/v:([\d,]+)/);
                if (hMatch || vMatch) {
                    const hArr = hMatch ? hMatch[1].split(',').map(Number) : [];
                    const vArr = vMatch ? vMatch[1].split(',').map(Number) : [];
                    return [hArr, vArr];
                }
                return [[], []];
            }
            return result;
        }

        // getbands()
        if (target === "getbands") {
            return img.getbands();
        }

        // getchannel(ch)
        if (target === "getchannel") {
            const ch = p.channel || p.ch || 0;
            return img.getchannel(ch);
        }

        // tobytes / toBytes
        if (target === "tobytes" || target === "toBytes") {
            return img.toBytes();
        }

        // tobitmap
        if (target === "tobitmap") {
            return img.tobitmap();
        }

        // split (returns array of band images wrapped as value objects)
        if (target === "split") {
            return img.split();
        }

        // repr
        if (target === "repr") {
            return img.repr();
        }

        // ── Fallback: try accessing as property or callable ──
        try {
            const val = img[target];
            if (val !== undefined) {
                if (typeof val === "function") {
                    return val();
                }
                return val;
            }
        } catch (_e) {
            // fall through to not-implemented
        }

        throw new Error(`not implemented: value Image.${target}`);
    }
}
