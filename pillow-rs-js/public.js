import * as generated from './pkg/core/pillow_rs_js.js';
import { createImageShow } from './host/display/imageshow.js';

export * from './pkg/core/pillow_rs_js.js';
export { default } from './pkg/core/pillow_rs_js.js';

const {
    Image,
    ImageChops,
    ImageDraw: CoreImageDraw,
    ImageOps,
    outlineCurve,
} = generated;
const imageInfoStates = new WeakMap();
const drawImages = new WeakMap();
const metadataWrapper = Symbol.for('pillow-rs.image-info-wrapper');

/** Mutable pixel access returned by Image.load(). */
export class PixelAccess {
    #image;

    constructor(image) {
        this.#image = image;
    }

    getPixel(x, y) {
        return this.#image.getpixelFormatted(Number(x), Number(y));
    }

    putPixel(x, y, value) {
        this.#image.putpixelValue(Number(x), Number(y), value);
    }
}

/** Mutable path builder used by ImageDraw.shape(). */
export class Outline {
    #points = [];

    move(x, y) {
        this.#points = [[Number(x), Number(y)]];
    }

    line(x, y) {
        this.#points.push([Number(x), Number(y)]);
    }

    curve(x1, y1, x2, y2, x3, y3) {
        const current = this.#points.at(-1);
        if (!current) {
            const error = new Error('outline has no current point');
            error.name = 'ValueError';
            throw error;
        }
        const sampled = outlineCurve(
            [current[0], current[1], x1, y1, x2, y2, x3, y3].map(Number),
            32,
        );
        for (let index = 0; index < sampled.length; index += 2) {
            this.#points.push([sampled[index], sampled[index + 1]]);
        }
    }

    close() {
        const first = this.#points[0];
        const last = this.#points.at(-1);
        if (this.#points.length > 2
            && (first[0] !== last[0] || first[1] !== last[1])) {
            this.#points.push([...first]);
        }
    }

    toPoints() {
        return this.#points.map((point) => [...point]);
    }
}

const DRAW_MUTATING_METHODS = [
    'arc', 'arcWithInput', 'bitmap', 'bitmapWithInput', 'chord', 'chordWithInput',
    'circle', 'circleWithInput', 'ellipse', 'ellipseWithInput', 'line',
    'lineWithColorInput', 'lineWithInput', 'pieslice', 'piesliceWithInput',
    'point', 'pointWithInput', 'polygon', 'polygonWithInput', 'rectangle',
    'rectangleWithInput', 'regularPolygonWithInput', 'roundedRectangle',
    'roundedRectangleWithInput', 'shapeWithInput', 'text', 'textWithInput',
    'multilineTextWithInput',
];

/** Public drawing context that writes each completed operation to its image. */
export class ImageDraw extends CoreImageDraw {
    constructor(image, mode = null) {
        super(image, mode);
        drawImages.set(this, image);
    }
}

function syncDrawImage(draw) {
    const image = drawImages.get(draw);
    if (image === undefined) return;
    const updated = draw.image;
    try {
        image.replaceFrom(updated);
    } finally {
        updated.free();
    }
}

for (const name of DRAW_MUTATING_METHODS) {
    const original = CoreImageDraw.prototype[name];
    if (typeof original !== 'function') continue;
    Object.defineProperty(ImageDraw.prototype, name, {
        ...Object.getOwnPropertyDescriptor(CoreImageDraw.prototype, name),
        value(...args) {
            const result = Reflect.apply(original, this, args);
            syncDrawImage(this);
            return result;
        },
    });
}

function cloneSnapshot(value) {
    return structuredClone(value);
}

function stateFor(image) {
    let state = imageInfoStates.get(image);
    if (state === undefined) {
        state = {
            value: {},
            native: null,
            omitted: new Set(),
            rebaseline: false,
            knownEmpty: false,
        };
        imageInfoStates.set(image, state);
    }
    return state;
}

function nativeInfoFor(image) {
    const native = image.compatibilityInfo();
    return native != null && typeof native === 'object' ? { ...native } : {};
}

function sameValue(left, right) {
    return JSON.stringify(left) === JSON.stringify(right);
}

function imageInfo(image) {
    const state = stateFor(image);
    if (state.knownEmpty) return state.value;

    const native = nativeInfoFor(image);
    const previous = state.native;
    if (previous !== null) {
        if (state.rebaseline) {
            for (const name of Object.keys(previous)) {
                if (!Object.hasOwn(native, name)) state.omitted.add(name);
            }
        }
        for (const name of Object.keys(native)) state.omitted.delete(name);
        for (const name of state.omitted) native[name] = previous[name];
    }

    if (previous === null) {
        for (const [name, value] of Object.entries(native)) {
            if (!Object.hasOwn(state.value, name)) state.value[name] = value;
        }
    } else if (state.rebaseline) {
        for (const [name, value] of Object.entries(native)) {
            if (!Object.hasOwn(previous, name) && !Object.hasOwn(state.value, name)) {
                state.value[name] = value;
            }
        }
    } else if (!sameValue(native, previous)) {
        for (const name of Object.keys(previous)) {
            if (!Object.hasOwn(native, name)) delete state.value[name];
        }
        for (const [name, value] of Object.entries(native)) {
            if (!Object.hasOwn(previous, name) || !sameValue(value, previous[name])) {
                state.value[name] = value;
            }
        }
    }

    if (previous === null || !sameValue(native, previous)) {
        state.native = cloneSnapshot(native);
    }
    state.rebaseline = false;
    return state.value;
}

function setImageInfo(image, value) {
    imageInfoStates.set(image, {
        value,
        native: cloneSnapshot(nativeInfoFor(image)),
        omitted: new Set(),
        rebaseline: false,
        knownEmpty: false,
    });
}

function copyImageInfo(source, target) {
    const original = stateFor(source);
    imageInfoStates.set(target, {
        value: { ...original.value },
        native: original.native,
        omitted: new Set(original.omitted),
        rebaseline: true,
        knownEmpty: false,
    });
    return target;
}

function isImage(value) {
    return value instanceof Image;
}

function copyResult(source, result) {
    return isImage(result) ? copyImageInfo(source, result) : result;
}

function copyImageResults(source, result) {
    return Array.isArray(result) ? result.map((image) => copyResult(source, image)) : result;
}

function normalizePutdataValue(value) {
    if (Array.isArray(value)) return value.map(normalizePutdataValue);
    if (value === null || typeof value !== 'object') return value;

    const toPrimitive = value[Symbol.toPrimitive];
    const method = typeof toPrimitive === 'function' ? toPrimitive : value.valueOf;
    if (typeof method !== 'function') return value;
    const primitive = method.call(value, 'number');
    return typeof primitive === 'number' ? primitive : value;
}

function wrapPutdataValues() {
    const original = Image.prototype.putdataValues;
    if (typeof original !== 'function' || original[metadataWrapper]) return;
    const wrapped = function putdataValuesWithOrderedCoercion(data, scale = null, offset = null) {
        if (data instanceof Uint8Array) {
            return Reflect.apply(original, this, [data, scale, offset]);
        }

        if (!Array.isArray(data)) {
            if (data == null || typeof data !== 'object') {
                return Reflect.apply(original, this, [data, scale, offset]);
            }
            const reportedLength = Number(data.length);
            if (!Number.isSafeInteger(reportedLength) || reportedLength < 0) {
                throw new TypeError('argument must be a sequence');
            }
            this.__validatePutdataLength(reportedLength);
            let items;
            try {
                items = Array.from(data);
            } catch (_) {
                throw new TypeError('argument must be a sequence');
            }
            // Pillow materializes generic sequences before writing, then
            // consumes at most the count returned by len(data). This also
            // keeps iterator callbacks outside the WASM mutable borrow.
            data = items.slice(0, reportedLength);
        }

        this.__validatePutdataLength(data.length);
        if (this.__putdataArrayIsSimple(data)) {
            return Reflect.apply(original, this, [data, scale, offset]);
        }

        const normalizedScale = scale == null ? 1 : Number(scale);
        const normalizedOffset = offset == null ? 0 : Number(offset);
        for (let index = 0; index < data.length; index += 1) {
            const value = normalizePutdataValue(data[index]);
            this.__putdataValueAt(index, value, normalizedScale, normalizedOffset);
        }
        return undefined;
    };
    Object.defineProperty(wrapped, metadataWrapper, { value: true });
    Object.defineProperty(Image.prototype, 'putdataValues', {
        ...Object.getOwnPropertyDescriptor(Image.prototype, 'putdataValues'),
        value: wrapped,
    });
}

function wrapGetPalette() {
    const original = Image.prototype.getpalette;
    if (typeof original !== 'function' || original[metadataWrapper]) return;
    const wrapped = function getpaletteWithPillowDefault(rawmode = 'RGB') {
        return Reflect.apply(original, this, [rawmode]);
    };
    Object.defineProperty(wrapped, metadataWrapper, { value: true });
    Object.defineProperty(Image.prototype, 'getpalette', {
        ...Object.getOwnPropertyDescriptor(Image.prototype, 'getpalette'),
        value: wrapped,
    });
}

function installImageHostMethods() {
    const originalLoad = Image.prototype.load;
    if (typeof originalLoad === 'function' && !originalLoad[metadataWrapper]) {
        const wrappedLoad = function loadWithPixelAccess(...args) {
            Reflect.apply(originalLoad, this, args);
            return new PixelAccess(this);
        };
        Object.defineProperty(wrappedLoad, metadataWrapper, { value: true });
        Object.defineProperty(Image.prototype, 'load', {
            ...Object.getOwnPropertyDescriptor(Image.prototype, 'load'),
            value: wrappedLoad,
        });
    }

    Object.defineProperty(Image.prototype, 'draft', {
        configurable: true,
        writable: true,
        value(mode, size) {
            const [width, height] = size ?? [];
            return this.draftFn(String(mode), Number(width), Number(height));
        },
    });
    Object.defineProperty(Image.prototype, 'getim', {
        configurable: true,
        writable: true,
        value() {
            // The WASM Image is itself the live Rust-owned image handle.
            return this;
        },
    });
}

function wrapImageMethod(name, { array = false, convert = false, loadInfo = false } = {}) {
    const original = Image.prototype[name];
    if (typeof original !== 'function' || original[metadataWrapper]) return;
    const wrapped = function imageInfoAwareMethod(...args) {
        if (loadInfo && this.format === 'WEBP') {
            this.load();
            this.info;
        }
        const result = name === 'point' && typeof args[0] === 'function'
            ? this.pointWithCallback(args[0], args[1] ?? null)
            : Reflect.apply(original, this, args);
        const derived = array
            ? copyImageResults(this, result)
            : copyResult(this, result);
        if (convert && isImage(derived)) {
            Object.assign(derived.info, this.convertedCompatibilityInfo(derived.mode));
        }
        return derived;
    };
    Object.defineProperty(wrapped, metadataWrapper, { value: true });
    Object.defineProperty(Image.prototype, name, { ...Object.getOwnPropertyDescriptor(Image.prototype, name), value: wrapped });
}

if (!Object.getOwnPropertyDescriptor(Image.prototype, 'info')) {
    Object.defineProperty(Image.prototype, 'info', {
        configurable: true,
        enumerable: true,
        get() {
            return imageInfo(this);
        },
        set(value) {
            setImageInfo(this, value);
        },
    });
}

wrapPutdataValues();
wrapGetPalette();
installImageHostMethods();

for (const name of [
    'resize', 'resizeWithInput', 'crop', 'cropWithInput', 'rotate', 'rotateWithInput',
    'transpose', 'filter', 'copy', 'quantize', 'quantizeWithInput', 'quantizeWithPaletteInput',
    'getchannel', 'reduce', 'reduceWithInput', 'point',
    'pointWithMode', 'pointWithTransform', 'effectSpread', 'transform', 'transformWithInput',
    'remapPalette', 'colorDegenerate', 'enhanceSharpness',
]) {
    wrapImageMethod(name, name === 'transpose' ? { loadInfo: true } : {});
}
wrapImageMethod('split', { array: true });
wrapImageMethod('convert', { convert: true });
wrapImageMethod('convertWithInput', { convert: true });

function wrapStatic(target, name, sourceIndex = 0, { clear = false, pad = false } = {}) {
    const original = target[name];
    if (typeof original !== 'function' || original[metadataWrapper]) return;
    const wrapped = function imageInfoAwareStatic(...args) {
        const result = Reflect.apply(original, this, args);
        if (!isImage(result) || clear) return result;
        const source = args[sourceIndex];
        if (pad) {
            const [targetWidth, targetHeight] = [Number(args[1]), Number(args[2])];
            const [sourceWidth, sourceHeight] = Array.from(source.size());
            if (sourceWidth > 0 && sourceHeight > 0 && targetWidth > 0 && targetHeight > 0) {
                const scale = Math.min(targetWidth / sourceWidth, targetHeight / sourceHeight);
                const containedWidth = Math.max(1, Math.round(sourceWidth * scale));
                const containedHeight = Math.max(1, Math.round(sourceHeight * scale));
                if (containedWidth !== targetWidth || containedHeight !== targetHeight) return result;
            }
        }
        return copyImageInfo(source, result);
    };
    Object.defineProperty(wrapped, metadataWrapper, { value: true });
    Object.defineProperty(target, name, { ...Object.getOwnPropertyDescriptor(target, name), value: wrapped });
}

for (const name of [
    'invert', 'flip', 'mirror', 'grayscale', 'posterize', 'solarize', 'solarizeWithComparisons',
    'equalize', 'equalizeWithInput', 'autocontrast', 'autocontrastWithMask',
    'autocontrastInvalidMask', 'contain', 'containWithInput', 'cover', 'coverWithInput',
    'fit', 'fitWithInput', 'crop', 'scale', 'scaleWithInput', 'colorize',
]) {
    wrapStatic(ImageOps, name);
}
wrapStatic(ImageOps, 'expand', 0, { clear: true });
wrapStatic(ImageOps, 'padWithInput', 0, { pad: true });
wrapStatic(ImageOps, 'pad', 0, { pad: true });

for (const name of [
    'add', 'subtract', 'multiply', 'screen', 'darker', 'lighter', 'difference', 'invert',
    'duplicate', 'addModulo', 'subtractModulo', 'blend', 'offset', 'logicalAnd', 'logicalOr',
    'logicalXor', 'hardLight', 'softLight', 'overlay',
]) {
    wrapStatic(ImageChops, name);
}
wrapStatic(ImageChops, 'composite', 1);
wrapStatic(ImageChops, 'constant', 0, { clear: true });

function wrapExportedImageResult(original, sourceIndex = 0) {
    return function imageInfoAwareExport(...args) {
        return copyResult(args[sourceIndex], Reflect.apply(original, this, args));
    };
}

export const alphaCompositeFn = wrapExportedImageResult(generated.alphaCompositeFn);
export const blend = wrapExportedImageResult(generated.blend);
export const composite = wrapExportedImageResult(generated.composite, 1);

export function merge(mode, bands) {
    const result = generated.merge(mode, bands);
    return bands.length > 0 ? copyResult(bands[0], result) : result;
}

export function mergeWithInput(mode, bands, bandCount, invalidType) {
    const result = generated.mergeWithInput(mode, bands, bandCount, invalidType);
    return bands.length > 0 ? copyResult(bands[0], result) : result;
}

export function colorizeFn(image, ...args) {
    return copyResult(image, generated.colorizeFn(image, ...args));
}

export function exifTransposeFn(image, inPlace) {
    image.info;
    const before = nativeInfoFor(image);
    const result = generated.exifTransposeFn(image, inPlace);
    if (result == null) {
        image.info;
        return result;
    }
    const derived = copyResult(image, result);
    const after = nativeInfoFor(derived);
    if (!sameValue(before.exif, after.exif)) {
        if (Object.hasOwn(after, 'exif')) derived.info.exif = after.exif;
        else delete derived.info.exif;
    }
    return derived;
}

const {
    ImageShow, Viewer, WindowsViewer, MacViewer, UnixViewer,
    XDGViewer, DisplayViewer, GmDisplayViewer, EogViewer, XVViewer,
} = createImageShow(Image);

if (!Image.prototype[metadataWrapper]) {
    const originalShow = Image.prototype.show;
    const wrappedShow = function showWithRegisteredViewer(title = null) {
        ImageShow.show(this, title);
        return undefined;
    };
    Object.defineProperty(wrappedShow, metadataWrapper, { value: originalShow });
    Object.defineProperty(Image.prototype, 'show', {
        ...Object.getOwnPropertyDescriptor(Image.prototype, 'show'),
        value: wrappedShow,
    });
}

export { Image, ImageChops, ImageOps, ImageShow, Viewer, WindowsViewer, MacViewer, UnixViewer, XDGViewer, DisplayViewer, GmDisplayViewer, EogViewer, XVViewer };
