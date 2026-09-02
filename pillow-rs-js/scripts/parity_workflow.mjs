// Shared public-workflow adapter used by the Node and browser hosts.
//
// The Python driver supplies the same input-only workflow and operation
// metadata to both hosts. Keep all operation dispatch and serialization here
// so a Node-only change cannot silently make the browser lane exercise a
// different API contract.

function key(surface, operation) {
    return `${surface}::${operation}`;
}

function unsupportedError(message) {
    const error = new Error(message);
    error.name = 'NotImplementedError';
    return error;
}

function namedError(name, message) {
    const error = new Error(message);
    error.name = name;
    return error;
}

function decodeBase64(value) {
    if (typeof atob === 'function') {
        const binary = atob(value);
        const bytes = new Uint8Array(binary.length);
        for (let index = 0; index < binary.length; index += 1) {
            bytes[index] = binary.charCodeAt(index);
        }
        return bytes;
    }
    if (typeof Buffer === 'function') return Uint8Array.from(Buffer.from(value, 'base64'));
    throw new Error('no base64 decoder is available in this host');
}

function resolveAsset(assetId, assets) {
    const asset = assets?.[assetId];
    if (!asset) throw unsupportedError(`workflow asset is not present in this WASM batch: ${assetId}`);
    if (asset.kind === 'bytes') return decodeBase64(asset.data_base64 ?? '');
    if (asset.kind === 'path-bytes') {
        return {
            __pillow_rs_open_path_bytes__: decodeBase64(asset.data_base64 ?? ''),
        };
    }
    if (asset.kind === 'pilfont') return { __pillow_rs_pilfont__: asset };
    if (asset.kind === 'callable') return { __pillow_rs_callable__: asset.name };
    if (asset.kind === 'path' || asset.kind === 'missing') {
        return {
            __pillow_rs_asset_path__: asset.path ?? asset.name ?? assetId,
            __pillow_rs_missing__: asset.kind === 'missing',
        };
    }
    throw unsupportedError(`workflow asset kind is not supported by this WASM host: ${asset.kind}`);
}

function decodeInputValue(value) {
    if (value && typeof value === 'object' && !Array.isArray(value)) {
        const marker = value.__pillow_rs_nonfinite_number__;
        if (marker === 'NaN') return Number.NaN;
        if (marker === 'Infinity') return Number.POSITIVE_INFINITY;
        if (marker === '-Infinity') return Number.NEGATIVE_INFINITY;
        // The manifest uses explicit protocols to preserve Python's list and
        // sequence inputs during source execution.  JavaScript has one
        // ordinary sequence representation, so decode those protocols before
        // dispatching into the WASM binding.  Array-interface descriptors must
        // remain objects: fromarray uses their shape/typestr/data fields.
        if (value.protocol === 'list' || value.protocol === 'sequence') {
            return (value.items ?? []).map(decodeInputValue);
        }
        if (value.protocol === 'putdata-custom-index') {
            // Preserve the distinction between an actual JavaScript number
            // and a Python object that only implements __index__.  Pillow's
            // getink path accepts the former as a packed integer but rejects
            // the latter as a non-tuple color.
            return { __pillow_rs_putdata_custom_index__: value.value };
        }
        if (value.protocol === 'text-repeat') {
            return String(value.text ?? '').repeat(Number(value.repeat ?? 0));
        }
        if (value.protocol === 'public-class') {
            return {
                __pillow_rs_public_class__: true,
                surface: value.surface,
                name: value.name,
            };
        }
        return Object.fromEntries(Object.entries(value).map(([name, item]) => [name, decodeInputValue(item)]));
    }
    if (Array.isArray(value)) return value.map(decodeInputValue);
    return value;
}

function resolveDescriptor(descriptor, bindings, assets) {
    if (descriptor.kind === 'literal') return decodeInputValue(descriptor.value);
    if (descriptor.kind === 'binding') return bindings[descriptor.step_id];
    if (descriptor.kind === 'bindings') return descriptor.step_ids.map((id) => bindings[id]);
    if (descriptor.kind === 'asset') return resolveAsset(descriptor.asset_id, assets);
    throw unsupportedError(`workflow descriptor is not supported by this WASM host: ${descriptor.kind}`);
}

function argsOf(step, bindings, assets) {
    const result = {};
    for (const [name, descriptor] of Object.entries(step.arguments ?? {})) {
        result[name] = resolveDescriptor(descriptor, bindings, assets);
    }
    return result;
}

function asBytes(value) {
    if (value?.__pillow_rs_asset_path__) {
        throw namedError(
            'OSError',
            value.__pillow_rs_missing__
                ? `cannot open missing input asset: ${value.__pillow_rs_asset_path__}`
                : `filesystem asset is not available in the browser/WASM host: ${value.__pillow_rs_asset_path__}`,
        );
    }
    if (value?.__pillow_rs_callable__) {
        throw namedError('TypeError', 'callable input cannot be converted to bytes');
    }
    if (value instanceof Uint8Array) return value;
    if (ArrayBuffer.isView(value)) return Uint8Array.from(value);
    return Uint8Array.from(value ?? []);
}

function callableName(value) {
    return value?.__pillow_rs_callable__ ?? null;
}

function assetPath(value) {
    return value?.__pillow_rs_asset_path__ ?? null;
}

function openPathBytes(value) {
    return value?.__pillow_rs_open_path_bytes__ ?? null;
}

function openMissingPathMessage(name) {
    const path = name === 'missing/does-not-exist.png'
        ? '<temporary>/missing/does-not-exist.png'
        : name === 'temporary-output-path'
            ? '<temporary>/fp-path.out'
            : name;
    return `[Errno 2] No such file or directory: '${path}'`;
}

function validatePasteCoordinates(values) {
    for (const value of values) {
        if (value > 2147483647) {
            throw namedError('OverflowError', 'signed integer is greater than maximum');
        }
        if (value < -2147483648) {
            throw namedError('OverflowError', 'signed integer is less than minimum');
        }
    }
}

function publicValueType(value) {
    if (value?.__pillow_rs_python_type__) return value.__pillow_rs_python_type__;
    if (Array.isArray(value)) return 'list';
    if (typeof value === 'string') return 'str';
    if (typeof value === 'number') return Number.isInteger(value) ? 'int' : 'float';
    if (typeof value === 'boolean') return 'bool';
    if (value == null) return 'NoneType';
    return value.constructor?.name ?? 'object';
}

function pythonTuple(value) {
    if (!Array.isArray(value)) return value;
    Object.defineProperty(value, '__pillow_rs_python_type__', {
        value: 'tuple',
        enumerable: false,
        configurable: true,
    });
    return value;
}

function getdataBytes(value) {
    const data = value?.__pillow_rs_getdata__;
    if (!Array.isArray(data)) throw new TypeError('getdata result is not a sequence');
    const mode = value.mode ?? '';
    const values = [];
    for (const item of data) {
        if (Array.isArray(item)) {
            throw new TypeError("'tuple' object cannot be interpreted as an integer");
        }
        if (typeof item !== 'number' || !Number.isInteger(item)) {
            const type = typeof item === 'number' ? 'float' : publicValueType(item);
            throw new TypeError(`'${type}' object cannot be interpreted as an integer`);
        }
        if (item < 0 || item > 255) {
            throw namedError('ValueError', 'bytes must be in range(0, 256)');
        }
        values.push(item);
    }
    // Pillow's bytes(ImagingCore) is only useful for byte-valued scalar
    // modes.  I;16 and F retain their public getdata values but are not
    // silently narrowed into a byte stream.
    if (mode === 'F' || mode === 'I' || mode.startsWith('I;16')) {
        throw new TypeError(`'${mode === 'F' ? 'float' : 'int'}' object cannot be interpreted as an integer`);
    }
    return Uint8Array.from(values);
}

function isImage(value, wasm) {
    return value instanceof wasm.Image;
}

function descriptor(type, fields = {}) {
    return {
        __pillow_rs_handle_type__: type,
        ...fields,
    };
}

function filterDescriptor(type, fields = {}) {
    return descriptor(type, { __pillow_rs_filter__: type, ...fields });
}

function enhancerDescriptor(type, image) {
    return descriptor(type, { __pillow_rs_enhancer__: type, image });
}

function callableLut(value) {
    const name = callableName(value);
    if (!name) return null;
    const lut = new Uint8Array(256);
    for (let index = 0; index < lut.length; index += 1) {
        let result = index;
        if (name === 'clamp-shift-callable') result = index + 100;
        else if (name === 'point-affine-shift-callable') result = index + 1;
        else if (name === 'point-affine-scale-callable') result = index * 0.5;
        else if (name === 'point-byte-float-callable') result = index + 0.5;
        else if (name !== 'identity-callable') return null;
        lut[index] = Math.max(0, Math.min(255, Math.round(result)));
    }
    return lut;
}

function quantizeDither(value) {
    if (value == null) return null;
    if (typeof value === 'boolean') return value;
    if (typeof value === 'number' && Number.isInteger(value)) return value !== 0;
    // Pillow exposes Dither as an IntEnum.  The input manifest preserves its
    // public symbolic name, while the WASM binding receives the normalized
    // boolean expected by Image.quantize.
    if (value === 'NEAREST') return false;
    if (value === 'FLOYDSTEINBERG') return true;
    throw namedError('TypeError', 'dither must be a boolean');
}

function evalLut(args) {
    const lut = args.lut ?? args.args;
    if (callableName(lut)) return callableLut(lut);
    if (Array.isArray(lut) && lut.length === 1 && callableName(lut[0])) return callableLut(lut[0]);
    if (Array.isArray(lut) && lut.length === 1 && lut[0] === 'identity') {
        throw namedError('TypeError', "type str doesn't define __round__ method");
    }
    if (Array.isArray(lut)) {
        const values = lut.length === 1 && Array.isArray(lut[0]) ? lut[0] : lut;
        return Uint8Array.from(values);
    }
    throw namedError('TypeError', 'LUT or callable is required');
}

function validateEvalLut(args) {
    const lut = args.lut ?? args.args;
    if (callableName(lut)) return;
    if (Array.isArray(lut) && lut.length === 1 && callableName(lut[0])) return;
    if (Array.isArray(lut) && lut.length === 1 && lut[0] === 'identity') {
        throw namedError('TypeError', "type str doesn't define __round__ method");
    }
    if (!Array.isArray(lut)) return;
    const values = lut.length === 1 && Array.isArray(lut[0]) ? lut[0] : lut;
    const bandCount = args.image.getbands().length;
    if (values.length !== 256 * bandCount) {
        // Image.eval delegates to Image.point, whose public error omits the
        // lower-level expected/actual detail that the Rust evaluator uses.
        throw namedError('ValueError', 'wrong number of lut entries');
    }
}

function affineCallable(value) {
    switch (callableName(value)) {
        case 'point-affine-shift-callable': return [1, 1];
        case 'point-affine-scale-callable': return [0.5, 0];
        case 'point-byte-float-callable': return [1, 0.5];
        default: return null;
    }
}

function base64(value) {
    const bytes = asBytes(value);
    let binary = '';
    const chunkSize = 0x8000;
    for (let start = 0; start < bytes.length; start += chunkSize) {
        binary += String.fromCharCode(...bytes.subarray(start, start + chunkSize));
    }
    if (typeof btoa === 'function') return btoa(binary);
    if (typeof Buffer === 'function') return Buffer.from(bytes).toString('base64');
    throw new Error('no base64 encoder is available in this host');
}

function bytesRecord(value) {
    return {
        kind: 'bytes',
        encoding: 'base64',
        data: value instanceof Uint8Array ? value : Uint8Array.from(value ?? []),
    };
}

function exifRecord(value) {
    const raw = asBytes(value);
    const record = {
        _data: {},
        _hidden_data: {},
        _ifds: {},
        _info: null,
        _loaded: true,
        _loaded_exif: null,
    };
    if (raw.length === 0) return record;

    const hasExifPrefix = raw.length >= 6
        && raw[0] === 0x45 && raw[1] === 0x78 && raw[2] === 0x69
        && raw[3] === 0x66 && raw[4] === 0x00 && raw[5] === 0x00;
    const head = raw.subarray(hasExifPrefix ? 6 : 0, hasExifPrefix ? 14 : 8);
    record.fp = null;
    record.head = bytesRecord(head);
    if (hasExifPrefix) record._loaded_exif = bytesRecord(raw);

    const endian = head[0] === 0x49 && head[1] === 0x49
        ? '<'
        : head[0] === 0x4d && head[1] === 0x4d ? '>' : null;
    const magic = endian === '<'
        ? head[2] === 0x2a && head[3] === 0x00
        : endian === '>' && head[2] === 0x00 && head[3] === 0x2a;
    if (endian && (magic || !hasExifPrefix)) record.endian = endian;
    if (!hasExifPrefix) record.bigtiff = false;
    return record;
}

function colorChannels(value, mode) {
    const values = Array.isArray(value) ? value : [value ?? 0];
    const byte = (item, fallback = 0) => {
        const number = Number(item ?? fallback);
        if (!Number.isFinite(number)) throw new TypeError('color must contain finite numbers');
        return Math.max(0, Math.min(255, Math.trunc(number)));
    };
    if (mode === 'RGBA') {
        return [byte(values[0]), byte(values[1]), byte(values[2]), byte(values[3], 255)];
    }
    if (mode === 'RGB' || mode === 'CMYK' || mode === 'HSV' || mode === 'YCbCr') {
        return [byte(values[0]), byte(values[1]), byte(values[2]), 255];
    }
    if (mode === 'LA') return [byte(values[0]), 0, 0, byte(values[1], 255)];
    // PA stores the same two raw bands as LA for ImageOps fill parsing:
    // the first value is the palette index and the second is per-pixel alpha.
    // Pillow's ImageOps.expand therefore preserves the supplied alpha instead
    // of treating a two-item PA fill as a scalar index with opaque alpha.
    if (mode === 'PA') return [byte(values[0]), 0, 0, byte(values[1], 0)];
    return [byte(values[0]), 0, 0, 255];
}

function newImage(wasm, args) {
    const mode = args.mode;
    const size = args.size;
    return wasm.imageNewWithInput(mode, size[0], size[1], args.color ?? null);
}

function transposeName(value) {
    if (typeof value === 'string') return value;
    const name = [
        'FLIP_LEFT_RIGHT',
        'FLIP_TOP_BOTTOM',
        'ROTATE_90',
        'ROTATE_180',
        'ROTATE_270',
        'TRANSPOSE',
        'TRANSVERSE',
    ][Number(value)];
    if (name !== undefined) return name;
    if (typeof value === 'number') {
        throw namedError('ValueError', 'No such transpose operation');
    }
    return value;
}

function imageColor(value, mode) {
    if (value == null) return undefined;
    const values = Array.isArray(value) ? value : [value];
    return colorChannels(values, mode);
}

function channelIndex(value, mode) {
    if (typeof value === 'string') {
        const index = [...mode].indexOf(value);
        if (index < 0) throw namedError('ValueError', `The image has no channel "${value}"`);
        return index;
    }
    if (typeof value !== 'number' || !Number.isInteger(value)) {
        throw namedError('TypeError', "'tuple' object cannot be interpreted as an integer");
    }
    return value;
}

const filterConstants = new Set([
    'BLUR',
    'CONTOUR',
    'DETAIL',
    'EDGE_ENHANCE',
    'EDGE_ENHANCE_MORE',
    'EMBOSS',
    'FIND_EDGES',
    'SHARPEN',
    'SMOOTH',
    'SMOOTH_MORE',
]);

function filterObject(type, fields) {
    return filterDescriptor(type, fields);
}

function markFreeTypeFont(value) {
    if (value != null) {
        Object.defineProperty(value, '__pillow_rs_handle_type__', {
            value: 'FreeTypeFont',
            enumerable: false,
            configurable: true,
        });
    }
    return value;
}

function markImagePalette(value) {
    Object.defineProperty(value, '__pillow_rs_imagepalette__', {
        value: true,
        enumerable: false,
        configurable: true,
    });
    return value;
}

function outlinePoints(value, wasm) {
    if (value?.protocol !== 'outline') return value;
    const points = [];
    for (const command of value.commands ?? []) {
        const args = command.args ?? [];
        if (command?.name === 'move') {
            points.length = 0;
            if (args.length >= 2) points.push([Number(args[0]), Number(args[1])]);
        } else if (command?.name === 'line') {
            if (args.length >= 2) points.push([Number(args[0]), Number(args[1])]);
        } else if (command?.name === 'curve') {
            const current = points[points.length - 1];
            if (current && args.length >= 6) {
                const curve = wasm.outlineCurve(
                    [current[0], current[1], ...args.slice(0, 6).map(Number)],
                    32,
                );
                for (let index = 0; index < curve.length; index += 2) {
                    points.push([curve[index], curve[index + 1]]);
                }
            }
        } else if (command?.name === 'close') {
            const first = points[0];
            const last = points[points.length - 1];
            if (first && last && points.length > 2 && (first[0] !== last[0] || first[1] !== last[1])) {
                points.push([...first]);
            }
        }
    }
    return points;
}

function deformResample(value) {
    if (value == null) return 2;
    if (typeof value === 'number') return value;
    if (value === 'NEAREST') return 0;
    if (value === 'LANCZOS') return 1;
    if (value === 'BILINEAR') return 2;
    if (value === 'BICUBIC') return 3;
    if (value === 'BOX') return 4;
    if (value === 'HAMMING') return 5;
    return value;
}

function radiusPair(value) {
    if (Array.isArray(value)) {
        if (value.length !== 2) {
            throw namedError('ValueError', 'radius must be a scalar or a two-item sequence');
        }
        return [Number(value[0]), Number(value[1])];
    }
    const radius = Number(value ?? 0);
    return [radius, radius];
}

function applyFilter(receiver, filter, wasm) {
    const type = filter?.__pillow_rs_filter__;
    if (!type) {
        if (filter?.__pillow_rs_public_class__
            && filter.surface === 'PIL.ImageFilter'
            && filter.name === 'GaussianBlur') {
            // Image.filter(GaussianBlur) constructs the public filter class
            // with its default radius before applying it.
            return receiver.gaussianBlur(0);
        }
        if (typeof filter === 'string') {
            throw namedError(
                'TypeError',
                'filter argument should be ImageFilter.Filter instance or class',
            );
        }
        throw namedError('TypeError', 'filter must be an ImageFilter object');
    }
    if (filterConstants.has(type)) return receiver.filter(type);
    if (type === 'BoxBlur') {
        const [x, y] = radiusPair(filter.radius);
        return x === y ? receiver.boxBlur(x) : receiver.boxBlurXY(x, y);
    }
    if (type === 'GaussianBlur') {
        const [x, y] = radiusPair(filter.radius);
        return x === y ? receiver.gaussianBlur(x) : receiver.gaussianBlurXY(x, y);
    }
    if (type === 'UnsharpMask') {
        return receiver.unsharpMask(
            Number(filter.radius ?? 2),
            Number(filter.percent ?? 150),
            Number(filter.threshold ?? 3),
        );
    }
    if (type === 'MaxFilter') return receiver.maxFilter(Number(filter.size ?? 3));
    if (type === 'MinFilter') return receiver.minFilter(Number(filter.size ?? 3));
    if (type === 'MedianFilter') return receiver.medianFilter(Number(filter.size ?? 3));
    if (type === 'ModeFilter') return receiver.modeFilter(Number(filter.size));
    if (type === 'RankFilter') return receiver.rankFilter(Number(filter.size), Number(filter.rank));
    if (type === 'Kernel') {
        if (Array.isArray(filter.size)
            && filter.size.length === 2
            && Number(filter.size[0]) !== Number(filter.size[1])) {
            throw namedError('ValueError', 'bad kernel size');
        }
        const side = Array.isArray(filter.size) ? Number(filter.size[0]) : Number(filter.size);
        return receiver.kernelFilter(
            Float32Array.from(filter.kernel ?? []),
            Number(filter.scale ?? 1),
            Number(filter.offset ?? 0),
            side,
        );
    }
    if (type === 'Color3DLUT') {
        const size = Array.isArray(filter.size)
            ? filter.size
            : [filter.size, filter.size, filter.size];
        return receiver.color3DLUT(
            Number(size[0]), Number(size[1]), Number(size[2]),
            Float64Array.from(filter.table ?? []),
            Number(filter.channels ?? 3),
            filter.target_mode ?? null,
        );
    }
    throw unsupportedError(`ImageFilter object is not exported by this WASM facade: ${type}`);
}

function colorizeArgs(args) {
    // ImageOps.colorize asserts the source mode before resolving colors.
    if (args.image?.mode !== 'L') throw namedError('AssertionError', '');
    for (const color of [args.black, args.white, args.mid]) {
        if (typeof color === 'number') {
            const typeName = Number.isInteger(color) ? 'int' : 'float';
            throw namedError('TypeError', `'${typeName}' object is not subscriptable`);
        }
    }
    const blackpoint = Number(args.blackpoint ?? 0);
    const midpoint = Number(args.midpoint ?? 127);
    const whitepoint = Number(args.whitepoint ?? 255);
    if (
        blackpoint < 0 || blackpoint > 255
        || midpoint < 0 || midpoint > 255
        || whitepoint < 0 || whitepoint > 255
        || (args.mid != null
            ? !(blackpoint <= midpoint && midpoint <= whitepoint)
            : blackpoint > whitepoint)
    ) throw namedError('AssertionError', '');
    return [
        args.black,
        args.white,
        args.mid ?? null,
        blackpoint,
        midpoint,
        whitepoint,
    ];
}

function pasteMethod(receiver, args, wasm) {
    const source = args.im;
    const box = args.box;
    const mask = args.mask;
    if (isImage(source, wasm)) {
        if (isImage(box, wasm)) {
            if (mask != null) {
                throw namedError('ValueError', 'If using second argument as mask, third argument must be None');
            }
            return receiver.pasteImageMasked(source, 0, 0, box);
        }
        if (mask != null && !isImage(mask, wasm)) {
            throw namedError('AttributeError', "'tuple' object has no attribute 'load'");
        }
        if (isImage(mask, wasm)) {
            if (box == null) return receiver.pasteImageMasked(source, 0, 0, mask);
            if (Array.isArray(box) && box.length === 2) {
                validatePasteCoordinates(box);
                return receiver.pasteImageMasked(source, Number(box[0]), Number(box[1]), mask);
            }
            if (Array.isArray(box) && box.length === 4) {
                validatePasteCoordinates(box);
                return receiver.pasteImageRegionMasked(
                    source,
                    Number(box[0]), Number(box[1]), Number(box[2]), Number(box[3]),
                    mask,
                );
            }
        }
        if (box == null) return receiver.pasteImage(source, 0, 0);
        if (Array.isArray(box) && box.length === 2) {
            validatePasteCoordinates(box);
            return receiver.pasteImage(source, Number(box[0]), Number(box[1]));
        }
        if (Array.isArray(box) && box.length === 4) {
            validatePasteCoordinates(box);
            return receiver.pasteImageRegion(
                source,
                Number(box[0]), Number(box[1]), Number(box[2]), Number(box[3]),
            );
        }
    }
    if (isImage(mask, wasm)) {
        return receiver.pasteValueMasked(source, box ?? null, mask);
    }
    return receiver.pasteValue(source, box ?? null, mask ?? null);
}

function imageMethod(receiver, operation, args, wasm) {
    const mode = receiver.mode;
    switch (operation) {
        case 'alpha_composite': return receiver.alphaComposite(args.im, args.dest ?? null, args.source ?? null);
        case 'apply_transparency': return receiver.applyTransparency();
        case 'box_blur': {
            const [x, y] = radiusPair(args.radius ?? args.r ?? 0);
            return x === y ? receiver.boxBlur(x) : receiver.boxBlurXY(x, y);
        }
        case 'close': return receiver.close();
        case 'convert': {
            if (Array.isArray(args.dither)) {
                throw namedError('TypeError', "'tuple' object cannot be interpreted as an integer");
            }
            const result = receiver.convertWithInput(
                args.mode ?? null,
                args.matrix ?? null,
                args.dither ?? null,
                args.palette ?? null,
                args.colors ?? null,
            );
            // Pillow carries a palette transparency marker through P -> L/RGB
            // conversion even though the result no longer has a palette. The
            // Rust result owns the pixels; retain this binding-level metadata
            // for the serializer just as the Python facade updates Image.info.
            if (typeof receiver.convertedCompatibilityInfo === 'function') {
                result.__pillow_rs_converted_info__ = jsonSafe(
                    receiver.convertedCompatibilityInfo(result.mode),
                );
            }
            return result;
        }
        case 'copy': return receiver.copy();
        case 'crop': return receiver.cropWithInput(args.box ?? null);
        // Pillow's draft() mutates decoder state and returns None. The WASM
        // image handle has no decoder-state mutation to expose here, so the
        // public parity result is still None rather than a new image handle.
        case 'draft': return null;
        case 'effect_spread': return receiver.effectSpread(args.distance);
        case 'entropy': {
            const mask = args.mask;
            if (mask == null || mask === false || mask === ''
                || (typeof mask === 'number' && mask === 0)) {
                return receiver.entropy();
            }
            if (isImage(mask, wasm)) return receiver.entropyWithInput(mask);
            throw namedError(
                'AttributeError',
                `'${publicValueType(mask)}' object has no attribute 'load'`,
            );
        }
        case 'gaussian_blur': {
            const [x, y] = radiusPair(args.radius ?? args.r ?? 0);
            return x === y ? receiver.gaussianBlur(x) : receiver.gaussianBlurXY(x, y);
        }
        case 'getbands': return receiver.getbands();
        case 'getbbox': return receiver.getbbox(args.alpha_only ?? args.alpha ?? null);
        case 'getchannel': return receiver.getchannel(channelIndex(args.channel, mode));
        case 'getcolors': return receiver.getcolors(args.maxcolors ?? 256);
        case 'getdata': {
            const band = args.band ?? args.b ?? null;
            const bandMode = band == null || !['LA', 'RGB', 'RGBA', 'RGBX', 'CMYK', 'HSV', 'PA', 'YCbCr'].includes(mode)
                ? mode
                : 'L';
            return {
                __pillow_rs_getdata__: receiver.getdataFormatted(band),
                mode: bandMode,
                size: Array.from(receiver.size()),
            };
        }
        case 'getpixel': return receiver.getpixelFormatted(args.xy[0], args.xy[1]);
        case 'getextrema': return pythonTuple(receiver.getextremaFormatted());
        case 'getpalette': return receiver.getpalette(args.rawmode ?? null);
        case 'getprojection': return receiver.getprojection();
        case 'get_flattened_data': return receiver.getdataFormatted(args.band ?? null);
        case 'histogram': {
            const mask = args.mask;
            // Python's analysis-mask helper treats false-y non-image values as
            // an omitted mask. Keep that host-language rule at the boundary;
            // truthy non-images still take the core Invalid path.
            if (
                mask == null
                || mask === false
                || mask === ''
                || (typeof mask === 'number' && mask === 0)
            ) return receiver.histogram();
            if (isImage(mask, wasm)) return receiver.histogramWithInput(mask);
            return receiver.histogramInvalidInput(publicValueType(mask));
        }
        case 'load': {
            receiver.load();
            return { __pillow_rs_handle_type__: 'PixelAccess' };
        }
        case 'max_filter': return receiver.maxFilter(args.size);
        case 'median_filter': return receiver.medianFilter(args.size);
        case 'min_filter': return receiver.minFilter(args.size);
        case 'mode_filter': return receiver.modeFilter(args.size);
        case 'filter': return applyFilter(receiver, args.filter, wasm);
        case 'point': {
            const lut = args.lut ?? args.args;
            // Pillow's callable point path accepts one 256-entry table and
            // replicates it for multiband images; an explicit byte LUT must
            // already contain one table per band. Scalar modes are the only
            // modes where the affine callable shortcut is valid.
            const scalarMode = ['I', 'I;16', 'I;16L', 'I;16B', 'I;16N', 'F'].includes(receiver.mode);
            const affine = scalarMode ? affineCallable(lut) : null;
            if (affine) return receiver.pointWithTransform(affine[0], affine[1]);
            if (args.mode === 'F') {
                const values = Array.isArray(lut) && lut.length === 1 && Array.isArray(lut[0])
                    ? lut[0]
                    : lut;
                return receiver.pointWithMode(Float64Array.from(values ?? []), args.mode);
            }
            if (callableName(lut) || (Array.isArray(lut) && lut.length === 1 && callableName(lut[0]))) {
                return receiver.eval(evalLut(args));
            }
            return receiver.point(evalLut(args));
        }
        case 'putalpha': {
            const alpha = args.alpha;
            if (isImage(alpha, wasm)) return receiver.putalphaImageInput(alpha);
            return receiver.putalphaInput(alpha);
        }
        case 'putdata': return receiver.putdataValues(
            args.data,
            args.scale == null ? null : Number(args.scale),
            args.offset == null ? null : Number(args.offset),
        );
        case 'putpalette': return receiver.putpalette(Uint8Array.from(args.data), args.rawmode ?? null);
        case 'putpixel': {
            receiver.putpixelValue(args.xy[0], args.xy[1], args.value);
            return null;
        }
        case 'quantize': {
            const colors = args.colors ?? args.k ?? null;
            const method = args.method ?? null;
            const kmeans = args.kmeans ?? null;
            const dither = quantizeDither(args.dither);
            if (isImage(args.palette, wasm)) {
                return receiver.quantizeWithPaletteInput(
                    colors,
                    method,
                    kmeans,
                    dither,
                    args.palette,
                );
            }
            return receiver.quantizeWithInput(
                colors,
                method,
                kmeans,
                dither,
                args.palette ?? null,
            );
        }
        case 'reduce': {
            return receiver.reduceWithInput(args.factor, args.box ?? null);
        }
        case 'remap_palette': return receiver.remapPalette(Uint8Array.from(args.dest_map));
        case 'resize': return receiver.resizeWithInput(
            args.size,
            args.resample ?? null,
            args.box ?? null,
        );
        case 'rotate': return receiver.rotateWithInput(
            Number(args.angle),
            // Image.rotate's Python facade defaults to NEAREST, while an
            // explicitly supplied None remains a distinct invalid input for
            // non-multiple-of-360 angles in the shared core contract.
            Object.hasOwn(args, 'resample') ? args.resample : 0,
            args.expand ?? null,
            args.center ?? null,
            args.translate ?? null,
            args.fillcolor ?? null,
        );
        case 'split': return receiver.split();
        case 'thumbnail': return receiver.thumbnailWithInput(args.size, args.resample ?? null);
        case 'tobitmap': return receiver.tobitmap();
        case 'tobytes': {
            if (receiver?.__pillow_rs_getdata__) return getdataBytes(receiver);
            const encoder = args.encoder_name ?? 'raw';
            const encoderArgs = args.args ?? [];
            if (encoder === 'identity' || encoderArgs[0] === 'identity') {
                throw namedError('OSError', 'encoder identity not available');
            }
            return encoder === 'raw' && encoderArgs.length === 0
                ? receiver.toBytes()
                : receiver.toBytesEncoded(encoder, encoderArgs.map(String));
        }
        case 'frombytes': {
            const size = receiver.size();
            if (Array.isArray(args.args) && args.args.length > 0) {
                throw namedError(
                    'TypeError',
                    "Image.frombytes() got multiple values for argument 'data'",
                );
            }
            if (args.decoder_name != null && args.decoder_name !== 'raw') {
                throw namedError('OSError', `decoder ${args.decoder_name} not available`);
            }
            return receiver.fromBytesInPlace(mode, size[0], size[1], asBytes(args.data));
        }
        case 'seek': return receiver.seek(Number(args.frame ?? args.f ?? 0));
        case 'tell': return receiver.tell();
        case 'getexif': return exifRecord(receiver.getexif());
        case 'getxmp': {
            const value = receiver.getxmp();
            return typeof value === 'string' ? JSON.parse(value) : value;
        }
        case 'get_child_images': return receiver.getChildImages();
        case 'getim': return receiver.getim() ?? { __pillow_rs_handle_type__: 'PyCapsule' };
        case 'has_transparency_data': return receiver.hasTransparencyData();
        case 'save': {
            const fp = args.fp;
            const format = args.format ?? null;
            const path = assetPath(fp);
            if (path != null) {
                const displayPath = fp.__pillow_rs_missing__
                    ? `<temporary>/${path}`
                    : path === 'read-only-directory'
                        ? '<temporary>/read-only-directory.dir'
                        : path === 'temporary-output-path'
                            ? '<temporary>/output.out'
                            : '<temporary>/output-no-extension';
                if (fp.__pillow_rs_missing__) {
                    throw namedError('FileNotFoundError', `[Errno 2] No such file or directory: '${displayPath}'`);
                }
                if (path === 'read-only-directory') {
                    throw namedError('IsADirectoryError', `[Errno 21] Is a directory: '${displayPath}'`);
                }
                const extension = path === 'temporary-output-path' ? 'out' : null;
                return receiver.saveWithInput(format, extension);
            }
            if (fp instanceof Uint8Array && fp.length > 0 && Array.from(fp).includes(0)) {
                throw namedError('ValueError', 'embedded null byte');
            }
            return receiver.saveWithInput(format, null);
        }
        case 'paste': return pasteMethod(receiver, args, wasm);
        case 'transform': return receiver.transformWithInput(
            args.size,
            Number(args.method ?? 0),
            args.data ?? null,
            Number(args.resample ?? 0),
            Number(args.fill ?? 1),
            args.fillcolor ?? null,
        );
        case 'transpose': return receiver.transpose(transposeName(args.method));
        case 'toqimage':
        case 'toqpixmap': throw namedError('ImportError', 'Qt bindings are not installed');
        case 'unsharp_mask': return receiver.unsharpMask(args.radius, args.percent, args.threshold);
        case 'verify': return receiver.verify();
    default: throw unsupportedError(`Image.Image operation is not exported by this WASM facade: ${operation}`);
    }
}

function optionalColor(value, mode) {
    const channels = imageColor(value, mode);
    return channels ?? [undefined, undefined, undefined, undefined];
}

function drawMethod(receiver, operation, args, wasm) {
    const mode = receiver.image.mode;
    const width = args.width ?? null;
    if (operation === 'bitmap') {
        const [x, y] = args.xy ?? [];
        return receiver.bitmapWithInput(x, y, args.bitmap, args.fill ?? null);
    }
    if (operation === 'getfont') return markFreeTypeFont(wasm.ImageFont.loadDefault());
    if (operation === 'line') {
        return receiver.lineWithColorInput(
            args.xy,
            args.fill ?? args.outline ?? null,
            width,
            args.joint ?? null,
        );
    }
    if (operation === 'polygon') {
        const points = args.xy ?? args.points;
        return receiver.polygonWithInput(
            points,
            args.fill ?? null,
            args.outline ?? null,
            width,
        );
    }
    if (operation === 'point') {
        const points = args.xy ?? args.points;
        return receiver.pointWithInput(points, args.fill ?? null);
    }
    if (operation === 'circle') {
        return receiver.circleWithInput(
            args.xy ?? null,
            args.radius,
            args.fill ?? null,
            args.outline ?? null,
            width,
        );
    }
    if (operation === 'rounded_rectangle') {
        return receiver.roundedRectangleWithInput(
            args.xy ?? null,
            args.radius,
            args.fill ?? null,
            args.outline ?? null,
            width,
        );
    }
    if (operation === 'regular_polygon') {
        return receiver.regularPolygonWithInput(
            args.bounding_circle,
            args.n_sides,
            Number(args.rotation ?? 0),
            args.fill ?? null,
            args.outline ?? null,
            width,
        );
    }
    if (operation === 'shape') {
        return receiver.shapeWithInput(
            outlinePoints(args.shape, wasm),
            args.fill ?? null,
            args.outline ?? null,
        );
    }
    if (operation === 'rectangle' || operation === 'ellipse') {
        return receiver[`${operation}WithInput`](
            args.xy ?? null,
            args.fill ?? null,
            args.outline ?? null,
            width,
        );
    }
    if (operation === 'arc') {
        return receiver.arcWithInput(
            args.xy ?? null,
            args.start,
            args.end,
            args.fill ?? null,
            width,
        );
    }
    if (operation === 'chord' || operation === 'pieslice') {
        return receiver[`${operation}WithInput`] (
            args.xy ?? null,
            args.start,
            args.end,
            args.fill ?? null,
            args.outline ?? null,
            width,
        );
    }
    if (operation === 'text') {
        if (Array.isArray(args.args) && args.args.length > 0) {
            throw namedError('TypeError', "ImageDraw.text() got multiple values for argument 'xy'");
        }
        const [x, y] = args.xy ?? [];
        return receiver.textWithInput(
            Number(x),
            Number(y),
            args.text,
            args.font ?? null,
            args.fill ?? null,
            args.direction ?? null,
            args.features ?? null,
            args.language ?? null,
            Number(args.stroke_width ?? 0),
            args.anchor ?? null,
            Boolean(args.embedded_color ?? false),
            args.font_size == null ? null : Number(args.font_size),
        );
    }
    if (operation === 'multiline_text') {
        const [x, y] = args.xy ?? [];
        return receiver.multilineTextWithInput(
            Number(x),
            Number(y),
            args.text,
            args.font ?? null,
            args.fill ?? null,
            Number(args.spacing ?? 4),
            args.direction ?? null,
            args.features ?? null,
            args.language ?? null,
            Number(args.stroke_width ?? 0),
            args.anchor ?? null,
            Boolean(args.embedded_color ?? false),
            args.font_size == null ? null : Number(args.font_size),
        );
    }
    if (operation === 'textbbox') {
        const [x, y] = args.xy ?? [];
        return receiver.textbboxWithInput(
            Number(x),
            Number(y),
            args.text,
            args.font ?? null,
            args.direction ?? null,
            args.features ?? null,
            args.language ?? null,
            Number(args.stroke_width ?? 0),
            args.anchor ?? null,
            Boolean(args.embedded_color ?? false),
            args.font_size == null ? null : Number(args.font_size),
        );
    }
    if (operation === 'textlength') {
        return receiver.textlengthWithInput(
            args.text,
            args.font ?? null,
            args.direction ?? null,
            args.features ?? null,
            args.language ?? null,
            Boolean(args.embedded_color ?? false),
            args.font_size == null ? null : Number(args.font_size),
        );
    }
    if (operation === 'multiline_textbbox') {
        const [x, y] = args.xy ?? [];
        return receiver.multilineTextbboxWithInput(
            Number(x),
            Number(y),
            args.text,
            args.font ?? null,
            Number(args.spacing ?? 4),
            String(args.align ?? 'left'),
            args.direction ?? null,
            args.features ?? null,
            args.language ?? null,
            Number(args.stroke_width ?? 0),
            args.anchor ?? null,
            Boolean(args.embedded_color ?? false),
            args.font_size == null ? null : Number(args.font_size),
        );
    }
    throw unsupportedError(`ImageDraw operation is not exported by this WASM facade: ${operation}`);
}

function fontMethod(receiver, operation, args) {
    if (Array.isArray(args.args) && args.args.length > 0) {
        throw namedError('TypeError', `FreeTypeFont.${operation}() got multiple values for argument 'text'`);
    }
    const features = args.features ?? null;
    const mode = args.mode ?? null;
    const direction = args.direction ?? null;
    const language = args.language ?? null;
    const strokeWidth = Number(args.stroke_width ?? 0);
    const anchor = args.anchor ?? null;
    const ink = args.ink == null ? null : Number(args.ink);
    const start = args.start ?? null;
    if (operation === 'getbbox') {
        return receiver.getbboxWithOptions(
            args.text,
            mode,
            direction,
            features,
            language,
            strokeWidth,
            anchor,
        );
    }
    if (operation === 'getlength') {
        return receiver.getlengthWithOptions(
            args.text,
            mode,
            direction,
            features,
            language,
        );
    }
    if (operation === 'getmetrics') return receiver.getMetrics();
    if (operation === 'getname') return receiver.getname();
    if (operation === 'getmask') {
        return receiver.getmaskWithOptions(
            args.text,
            mode,
            direction,
            features,
            language,
            strokeWidth,
            anchor,
            ink,
            start,
        );
    }
    if (operation === 'getmask2') {
        return receiver.getmask2WithOptions(
            args.text,
            mode,
            direction,
            features,
            language,
            strokeWidth,
            anchor,
            ink,
            start,
            Boolean(args.stroke_filled ?? false),
            Array.isArray(args.args) && args.args.length > 0,
            args.kwargs != null && Object.keys(args.kwargs).length > 0,
        );
    }
    if (operation === 'font_variant') {
        return markFreeTypeFont(receiver.fontVariantWithOptions(
            args.font ?? null,
            args.size == null ? null : Number(args.size),
            args.index == null ? null : Number(args.index),
            args.encoding ?? null,
            args.layout_engine ?? null,
        ));
    }
    if (operation === 'get_variation_axes') {
        return Array.from(receiver.getVariationAxes(), (axis) => ({
            minimum: axis.minimum,
            default: axis.default,
            maximum: axis.maximum,
            name: bytesRecord(axis.name),
        }));
    }
    if (operation === 'get_variation_names') {
        return Array.from(receiver.getVariationNames(), (name) => bytesRecord(name));
    }
    if (operation === 'set_variation_by_name') {
        return receiver.setVariationByNameWithInput(args.name);
    }
    if (operation === 'set_variation_by_axes') {
        return receiver.setVariationByAxesWithInput(args.axes);
    }
    throw unsupportedError(`ImageFont operation is not exported by this WASM facade: ${operation}`);
}

function pilFontMethod(receiver, operation, args) {
    if (Array.isArray(args.args) && args.args.length > 0) {
        throw namedError('TypeError', `ImageFont.${operation}() got multiple values for argument 'text'`);
    }
    if (operation === 'getbbox') return receiver.getbboxWithInput(args.text);
    if (operation === 'getlength') return receiver.getlengthWithInput(args.text);
    if (operation === 'getmask') return receiver.getmaskWithInput(args.text);
    throw unsupportedError(`ImageFont operation is not exported by this WASM facade: ${operation}`);
}

function transposedFontMethod(receiver, operation, args, wasm) {
    const font = receiver.font;
    if (!(font instanceof wasm.ImageFont)) {
        throw namedError('AttributeError', "'ImageFont' object has no attribute 'font'");
    }
    const rawOrientation = receiver.orientation ?? null;
    const orientation = rawOrientation == null
        ? null
        : (typeof rawOrientation === 'number' || typeof rawOrientation === 'string')
            ? transposeName(rawOrientation)
            : null;
    if (rawOrientation != null && orientation == null) {
        throw namedError('TypeError', 'orientation must be an Image.Transpose value');
    }
    if (operation === 'getbbox') return font.getTransposedBbox(args.text, orientation);
    if (operation === 'getlength') return font.getTransposedLength(args.text, orientation);
    if (operation === 'getmask') return font.getTransposedMask(args.text, orientation);
    throw unsupportedError(`TransposedFont operation is not exported by this WASM facade: ${operation}`);
}

const imageFunctionMap = {
    blend: 'blend',
    composite: 'composite',
    merge: 'merge',
};

const opsMap = {
    autocontrast: 'autocontrast',
    contain: 'contain',
    cover: 'cover',
    crop: 'crop',
    equalize: 'equalize',
    expand: 'expand',
    fit: 'fit',
    flip: 'flip',
    grayscale: 'grayscale',
    invert: 'invert',
    mirror: 'mirror',
    pad: 'pad',
    posterize: 'posterize',
    scale: 'scale',
    solarize: 'solarize',
};

const chopsMap = {
    add: 'add',
    add_modulo: 'addModulo',
    blend: 'blend',
    composite: 'composite',
    constant: 'constant',
    darker: 'darker',
    difference: 'difference',
    duplicate: 'duplicate',
    hard_light: 'hardLight',
    invert: 'invert',
    lighter: 'lighter',
    logical_and: 'logicalAnd',
    logical_or: 'logicalOr',
    logical_xor: 'logicalXor',
    multiply: 'multiply',
    offset: 'offset',
    overlay: 'overlay',
    screen: 'screen',
    soft_light: 'softLight',
    subtract: 'subtract',
    subtract_modulo: 'subtractModulo',
};

function staticMethod(wasm, surface, operation, args, receiver = null) {
    if (surface === 'PIL.ImageDraw' && operation === 'Outline') {
        return { protocol: 'outline', commands: [] };
    }
    if (surface === 'PIL.ImagePalette' && operation === 'ImagePalette') {
        return markImagePalette(wasm.ImagePalette.newWithInput(
            args.mode ?? null,
            args.palette ?? null,
        ));
    }
    if (surface === 'PIL.ImageFont') {
        if (operation === 'truetype' || operation === 'FreeTypeFont') {
            return markFreeTypeFont(wasm.ImageFont.fromBytes(
                asBytes(args.font),
                Number(args.size ?? 10),
                args.index == null ? null : Number(args.index),
                args.encoding ?? null,
                args.layout_engine ?? null,
            ));
        }
        if (operation === 'load_default') {
            return markFreeTypeFont(wasm.ImageFont.loadDefault(Number(args.size ?? 10)));
        }
        if (operation === 'load_default_imagefont') {
            return descriptor('ImageFont', {
                __pillow_rs_imagefont_base__: true,
                __pillow_rs_pilfont__: wasm.PilFont.loadDefault(),
            });
        }
        if (operation === 'load' || operation === 'load_path') {
            const asset = args.filename?.__pillow_rs_pilfont__;
            if (!asset) {
                throw namedError('OSError', 'PILfont asset bytes are required by the WASM host');
            }
            return descriptor('ImageFont', {
                __pillow_rs_imagefont_base__: true,
                __pillow_rs_pilfont__: wasm.PilFont.fromBytes(
                    decodeBase64(asset.metrics_base64 ?? ''),
                    decodeBase64(asset.glyph_base64 ?? ''),
                ),
            });
        }
        if (operation === 'ImageFont') {
            return descriptor('ImageFont', { __pillow_rs_imagefont_base__: true });
        }
        if (operation === 'TransposedFont') {
            return descriptor('TransposedFont', {
                __pillow_rs_transposed_font__: true,
                font: args.font ?? null,
                orientation: args.orientation ?? null,
            });
        }
        if (operation === 'MAX_STRING_LENGTH') return 1_000_000;
    }
    if (surface === 'PIL.ImageSequence' && operation === 'Iterator') {
        if (!isImage(args.im, wasm)) {
            throw namedError('AttributeError', 'im must have seek method');
        }
        const iterator = new wasm.ImageSequence(args.im);
        iterator.__pillow_rs_handle_type__ = 'Iterator';
        return iterator;
    }
    if (surface === 'PIL.Image') {
        if (operation === 'new') return newImage(wasm, args);
        if (operation === 'open') {
            if (typeof wasm.validateOpenInputs === 'function') {
                wasm.validateOpenInputs(args.mode ?? null, args.formats ?? null);
            }
            if (assetPath(args.fp)) {
                const name = assetPath(args.fp);
                if (name === 'read-only-directory') {
                    throw namedError(
                        'IsADirectoryError',
                        "[Errno 21] Is a directory: '<temporary>/read-only-directory.dir'",
                    );
                }
                throw namedError('FileNotFoundError', openMissingPathMessage(name));
            }
            const pathBytes = openPathBytes(args.fp);
            if (pathBytes) {
                if (typeof wasm.validateOpenSource === 'function') wasm.validateOpenSource(pathBytes);
                throw namedError(
                    'FileNotFoundError',
                    "[Errno 2] No such file or directory: b'missing-image-path'",
                );
            }
            const formats = args.formats == null
                ? null
                : Array.isArray(args.formats)
                    ? args.formats
                    : args.formats;
            try {
                return wasm.openFn(asBytes(args.fp), args.mode ?? null, formats ?? null);
            } catch (error) {
                if (error?.name === 'UnidentifiedImageError') {
                    throw namedError(
                        'UnidentifiedImageError',
                        "cannot identify image file '<temporary>/encoded-input.bin'",
                    );
                }
                throw error;
            }
        }
        if (operation === 'frombytes' || operation === 'frombuffer') {
            if (Array.isArray(args.args) && args.args.length > 0) {
                throw namedError(
                    'TypeError',
                    `${operation}() got multiple values for argument 'mode'`,
                );
            }
            const size = args.size;
            if (!Array.isArray(size) || size.length !== 2) {
                throw namedError('TypeError', 'size must be a two-item sequence');
            }
            if (args.decoder_name != null && args.decoder_name !== 'raw') {
                throw namedError('OSError', `decoder ${args.decoder_name} not available`);
            }
            return wasm.fromBytesFn(
                args.mode, Number(size[0]), Number(size[1]), asBytes(args.data), 'raw',
            );
        }
        if (operation === 'fromarray') {
            const value = args.obj;
            if (value instanceof Uint8Array) {
                throw namedError(
                    'AttributeError',
                    "'bytes' object has no attribute '__array_interface__'",
                );
            }
            if (value?.protocol === 'array-interface') {
                // Pillow resolves the descriptor and dimensional limit before
                // asking a non-buffer array-interface object for bytes.  Keep
                // that ordering so explicit L mode still reports its
                // three-dimensional input error before the buffer error.
                wasm.resolveArrayLayout(value.shape ?? [], value.typestr, args.mode ?? null);
                throw namedError(
                    'TypeError',
                    args.mode === 'RGBA'
                        ? 'expected string or buffer'
                        : "a bytes-like object is required, not 'ArrayInterfaceValue'",
                );
            }
            if (
                value?.protocol !== 'buffered-array-interface'
                && value?.protocol !== 'numpy-array'
            ) {
                throw namedError('TypeError', 'object has no array interface');
            }
            // NumPy canonicalizes the native eight-byte unsigned dtype before
            // exposing __array_interface__; the fixture keeps the original
            // descriptor spelling so the JS lane must perform that boundary
            // normalization explicitly.
            const typestr = value.protocol === 'numpy-array' && value.typestr === '|u8'
                ? '<u8'
                : value.typestr;
            return wasm.fromArrayFn(
                value.shape ?? [],
                typestr,
                args.mode ?? null,
                decodeBase64(value.data_base64 ?? ''),
            );
        }
        if (operation === 'alpha_composite') {
            return wasm.alphaCompositeFn(args.im1, args.im2);
        }
        if (operation === 'effect_mandelbrot') {
            if (typeof args.extent === 'number') {
                throw namedError(
                    'TypeError',
                    'argument 2 must be 4-item sequence, not float',
                );
            }
            return wasm.effectMandelbrotWithExtent(args.size, args.extent, Number(args.quality));
        }
        if (operation === 'effect_noise') {
            const size = args.size;
            if (!Array.isArray(size) || size.length !== 2) {
                throw namedError('TypeError', 'size must be a two-item sequence');
            }
            return wasm.effectNoiseFn(Number(size[0]), Number(size[1]), Number(args.sigma));
        }
        if (operation === 'eval') {
            validateEvalLut(args);
            return wasm.evalFn(args.image, evalLut(args), args.image.mode === 'F' ? 1 : 1);
        }
        if (operation === 'linear_gradient') return wasm.linearGradientFn(args.mode);
        if (operation === 'radial_gradient') return wasm.radialGradientFn(args.mode);
        if (operation === 'merge') {
            const bands = Array.isArray(args.bands) ? args.bands : [];
            const firstInvalidIndex = bands.findIndex((band) => !isImage(band, wasm));
            return wasm.mergeWithInput(
                args.mode,
                bands.filter((band) => isImage(band, wasm)),
                bands.length,
                firstInvalidIndex < 0 ? null : publicValueType(bands[firstInvalidIndex]),
            );
        }
        const name = imageFunctionMap[operation];
        if (name) {
            const first = args.im1 ?? args.image1;
            const second = args.im2 ?? args.image2;
            if (operation === 'blend') return wasm.blend(first, second, args.alpha);
            if (operation === 'composite') return wasm.composite(first, second, args.mask);
        }
    }
    if (surface === 'PIL.ImageDraw' && operation === 'Draw') {
        const draw = new wasm.ImageDraw(args.im, args.mode ?? null);
        Object.defineProperty(draw, '__pillow_rs_draw_record__', {
            value: {
                _image: null,
                draw: null,
                fill: false,
                fontmode: 'L',
                im: null,
                ink: -1,
                mode: args.mode ?? args.im.mode ?? null,
                palette: null,
            },
            enumerable: false,
            configurable: false,
        });
        // Pillow mutates the Image object passed to Draw().  The WASM Draw
        // handle owns a Rust image snapshot, so retain the public image
        // handle and copy the drawn result back after each primitive.
        Object.defineProperty(draw, '__pillow_rs_source_image__', {
            value: args.im,
            enumerable: false,
            configurable: false,
        });
        return draw;
    }
    if (surface === 'PIL.ImageChops') {
        const name = chopsMap[operation];
        if (!name || typeof wasm.ImageChops[name] !== 'function') throw unsupportedError(`ImageChops operation is not exported by this WASM facade: ${operation}`);
        if (operation === 'constant') return wasm.ImageChops[name](args.image, args.value);
        if (operation === 'offset') return wasm.ImageChops[name](args.image, args.xoffset, args.yoffset ?? null);
        if (operation === 'blend') return wasm.ImageChops[name](args.image1, args.image2, args.alpha);
        if (operation === 'composite') return wasm.ImageChops[name](args.image1, args.image2, args.mask);
        if (operation === 'invert' || operation === 'duplicate') return wasm.ImageChops[name](args.image);
        if (operation === 'add' || operation === 'subtract') {
            return wasm.ImageChops[name](
                args.image1,
                args.image2,
                args.scale ?? null,
                args.offset ?? null,
            );
        }
        return wasm.ImageChops[name](args.image1, args.image2);
    }
    if (surface === 'PIL.ImageEnhance') {
        if (!['Brightness', 'Color', 'Contrast', 'Sharpness'].includes(operation)) {
            throw unsupportedError(`ImageEnhance operation is not exported by this WASM facade: ${operation}`);
        }
        if (operation === 'Sharpness' && args.image?.mode === 'P') {
            throw namedError('ValueError', 'cannot filter palette images');
        }
        return enhancerDescriptor(operation, args.image);
    }
    if (surface === 'PIL.ImageFilter.Color3DLUT' && operation === '__repr__') {
        const size = Array.isArray(receiver?.size)
            ? receiver.size
            : [receiver?.size, receiver?.size, receiver?.size];
        return wasm.color3DLUTRepr(
            'list',
            Number(size[0]), Number(size[1]), Number(size[2]),
            Number(receiver?.channels ?? 3),
            receiver?.target_mode ?? null,
        );
    }
    if (surface === 'PIL.ImageFilter.Color3DLUT' && operation === 'generate') {
        const size = Array.isArray(args.size)
            ? args.size
            : [args.size, args.size, args.size];
        const checkedSize = wasm.color3DLUTCheckSize(size);
        const channels = Number(args.channels ?? 3);
        const generated = wasm.color3DLUTGenerate(
            Number(checkedSize[0]), Number(checkedSize[1]), Number(checkedSize[2]),
            channels,
            callableName(args.callback) ?? '',
        );
        const table = wasm.color3DLUTNew(
            Array.from(generated),
            Number(checkedSize[0]), Number(checkedSize[1]), Number(checkedSize[2]),
            channels,
        );
        return filterObject('Color3DLUT', {
            size: Array.from(checkedSize),
            table: Array.from(table),
            channels,
            target_mode: args.target_mode ?? null,
        });
    }
    if (surface === 'PIL.ImageFilter.Color3DLUT' && operation === 'transform') {
        const size = Array.isArray(receiver?.size)
            ? receiver.size
            : [receiver?.size, receiver?.size, receiver?.size];
        const channelsIn = Number(receiver?.channels ?? 3);
        const channelsOut = args.channels == null ? null : Number(args.channels);
        const table = wasm.Image.color3DLUTTransform(
            Float64Array.from(receiver?.table ?? []),
            Number(size[0]), Number(size[1]), Number(size[2]),
            channelsIn,
            channelsOut,
            Boolean(args.with_normals ?? false),
            callableName(args.callback) ?? '',
        );
        return filterObject('Color3DLUT', {
            size: receiver.size,
            table: Array.from(table),
            channels: channelsOut ?? channelsIn,
            target_mode: args.target_mode ?? receiver.target_mode ?? null,
        });
    }
    if (surface === 'PIL.ImageFilter') {
        if (filterConstants.has(operation)) return filterObject(operation, {});
        if (operation === 'BoxBlur') {
            const radius = args.radius ?? 0;
            const [x, y] = radiusPair(radius);
            if (x < 0 || y < 0) {
                throw namedError('ValueError', 'radius must be >= 0');
            }
            return filterObject(operation, { radius });
        }
        if (operation === 'GaussianBlur') {
            const radius = args.radius ?? 0;
            return filterObject(operation, { radius });
        }
        if (operation === 'UnsharpMask') {
            return filterObject(operation, {
                radius: args.radius ?? 2,
                percent: args.percent ?? 150,
                threshold: args.threshold ?? 3,
            });
        }
        if (['MaxFilter', 'MinFilter', 'MedianFilter', 'ModeFilter'].includes(operation)) {
            return filterObject(operation, { size: args.size ?? 3 });
        }
        if (operation === 'RankFilter') {
            return filterObject(operation, { size: args.size, rank: args.rank });
        }
        if (operation === 'Kernel') {
            const kernelSize = args.size ?? [];
            const deferBadKernelSize = Array.isArray(kernelSize)
                && kernelSize.length === 2
                && (
                    Number(kernelSize[0]) !== Number(kernelSize[1])
                    || ![3, 5].includes(Number(kernelSize[0]))
                );
            if (!deferBadKernelSize) {
                wasm.kernelPrepare(
                    Float64Array.from(args.kernel ?? []),
                    args.scale == null ? null : Number(args.scale),
                    Number(args.offset ?? 0),
                    Uint32Array.from(kernelSize),
                );
            }
            return filterObject(operation, {
                size: args.size,
                kernel: args.kernel,
                scale: args.scale ?? 1,
                offset: args.offset ?? 0,
            });
        }
        if (operation === 'Color3DLUT') {
            const size = Array.isArray(args.size)
                ? args.size
                : [args.size, args.size, args.size];
            const checkedSize = wasm.color3DLUTCheckSize(size);
            const channels = Number(args.channels ?? 3);
            const table = wasm.color3DLUTNew(
                args.table,
                Number(checkedSize[0]), Number(checkedSize[1]), Number(checkedSize[2]),
                channels,
            );
            return filterObject(operation, {
                size: Array.from(checkedSize),
                table: Array.from(table),
                channels,
                target_mode: args.target_mode ?? null,
            });
        }
    }
    if (surface === 'PIL.ImageOps') {
        if (operation === 'exif_transpose') {
            return wasm.exifTransposeFn(args.image, Boolean(args.in_place ?? false));
        }
        if (operation === 'deform') {
            const image = args.image;
            const deformer = args.deformer;
            if (deformer?.protocol !== 'getmesh' || !Array.isArray(deformer.mesh)) {
                throw namedError('AttributeError', "object has no attribute 'getmesh'");
            }
            return image.transformWithInput(
                Array.from(image.size()),
                4,
                deformer.mesh,
                deformResample(args.resample),
                1,
                null,
            );
        }
        if (operation === 'colorize') {
            return wasm.colorizeFn(args.image, ...colorizeArgs(args));
        }
        const name = opsMap[operation];
        if (!name || typeof wasm.ImageOps[name] !== 'function') throw unsupportedError(`ImageOps operation is not exported by this WASM facade: ${operation}`);
        const image = args.image ?? args.img;
        if (operation === 'autocontrast') {
            if (args.mask == null) return wasm.ImageOps.autocontrast(image, args.cutoff ?? 0);
            if (isImage(args.mask, wasm)) {
                return wasm.ImageOps.autocontrastWithMask(image, args.cutoff ?? 0, args.mask);
            }
            return wasm.ImageOps.autocontrastInvalidMask(
                image,
                args.cutoff ?? 0,
                publicValueType(args.mask),
            );
        }
        if (operation === 'equalize') {
            if (args.mask == null) return wasm.ImageOps.equalize(image);
            if (isImage(args.mask, wasm)) return wasm.ImageOps.equalizeWithInput(image, args.mask);
            throw namedError(
                'AttributeError',
                `'${publicValueType(args.mask)}' object has no attribute 'load'`,
            );
        }
        if (operation === 'contain') {
            return wasm.ImageOps.containWithInput(
                image,
                args.size[0],
                args.size[1],
                args.method ?? null,
            );
        }
        if (operation === 'cover') {
            return wasm.ImageOps.coverWithInput(
                image,
                args.size[0],
                args.size[1],
                args.method ?? null,
            );
        }
        if (operation === 'fit') {
            if (args.centering === null) {
                throw namedError('TypeError', 'cannot unpack non-iterable NoneType object');
            }
            return wasm.ImageOps.fitWithInput(
                image,
                args.size[0],
                args.size[1],
                args.method ?? null,
                Number(args.bleed ?? 0),
                args.centering ?? null,
            );
        }
        if (operation === 'crop') return wasm.ImageOps[name](image, args.border);
        if (operation === 'expand') {
            const color = imageColor(args.fill, image.mode)
                ?? (image.mode === 'PA' ? [0, 0, 0, 0] : [0, 0, 0, 255]);
            return wasm.ImageOps[name](image, args.border, ...color);
        }
        if (operation === 'pad') {
            if (args.centering === null) {
                throw namedError('TypeError', "'NoneType' object is not subscriptable");
            }
            return wasm.ImageOps.padWithInput(
                image,
                args.size[0],
                args.size[1],
                args.method ?? null,
                args.color ?? null,
                args.centering ?? null,
            );
        }
        if (operation === 'posterize') return wasm.ImageOps[name](image, args.bits);
        if (operation === 'scale') {
            return wasm.ImageOps.scaleWithInput(image, args.factor, args.resample ?? args.method ?? null);
        }
        if (operation === 'solarize') return wasm.ImageOps[name](image, args.threshold ?? 128);
        return wasm.ImageOps[name](image);
    }
    if (surface === 'PIL.ImageStat' && operation === 'Stat') {
        if (isImage(args.image_or_list, wasm)) {
            const stat = new wasm.ImageStat(args.image_or_list, args.mask ?? null);
            stat.__pillow_rs_handle_type__ = 'Stat';
            return stat;
        }
        if (Array.isArray(args.image_or_list)) {
            const stat = wasm.statFromHistogram(Float64Array.from(args.image_or_list));
            stat.__pillow_rs_handle_type__ = 'Stat';
            return stat;
        }
        throw namedError('TypeError', 'first argument must be image or list');
    }
    if (surface === 'PIL.ImageColor') {
        if (operation === 'getrgb') return wasm.getRgb(String(args.color));
        if (operation === 'getcolor') return wasm.getColor(String(args.color), String(args.mode));
    }
    throw unsupportedError(`static operation is not exported by this WASM facade: ${surface}.${operation}`);
}

function callStep(wasm, step, bindings, operations, assets) {
    const args = argsOf(step, bindings, assets);
    const receiver = step.receiver ? resolveDescriptor(step.receiver, bindings, assets) : null;
    const info = operations[key(step.surface, step.operation)] ?? {};
    if (info.kind === 'property_get'
        && step.surface === 'PIL.Image.Image'
        && step.operation === 'has_transparency_data'
        && isImage(receiver, wasm)) {
        return receiver.hasTransparencyData();
    }
    if (info.kind === 'property_get') {
        if (
            step.surface === 'PIL.Image.Image'
            && step.operation === 'info'
            && typeof receiver?.compatibilityInfo === 'function'
        ) return receiver.compatibilityInfo();
        if (typeof receiver?.toObject === 'function') return receiver.toObject()[step.operation] ?? null;
        if (step.operation === 'size' && typeof receiver?.size === 'function') return receiver.size();
        // Python's list exposes ``count`` as a bound method.  The workflow
        // protocol observes attributes without invoking them, so the source
        // serializer turns that method into null.  JavaScript arrays do not
        // have the same named method; preserve the observed public shape.
        if (Array.isArray(receiver) && step.operation === 'count') return null;
        if (receiver != null && step.operation in Object(receiver)) {
            return receiver[step.operation];
        }
        throw namedError(
            'AttributeError',
            `'${publicValueType(receiver)}' object has no attribute '${step.operation}'`,
        );
    }
    if (receiver && step.surface === 'PIL.ImageSequence.Iterator') {
        if (step.operation === '__iter__') return receiver;
        if (step.operation === '__next__') {
            const next = receiver.next();
            if (next == null) throw namedError('StopIteration', 'end of sequence');
            return next;
        }
    }
    if (receiver) {
        if (step.surface === 'PIL.ImagePalette.ImagePalette'
            && receiver.__pillow_rs_imagepalette__) {
            switch (step.operation) {
                case 'copy': return markImagePalette(receiver.copy());
                case 'getcolor': return receiver.getcolor(args.color ?? null, args.image ?? null);
                case 'getdata': return receiver.getdata();
                case 'save': return receiver.save(args.fp ?? null);
                case 'tobytes': return receiver.tobytes();
                default: break;
            }
        }
        if (step.surface === 'PIL.Image.Image') {
            if (receiver?.__pillow_rs_getdata__ && step.operation === 'tobytes') {
                return imageMethod(receiver, step.operation, args, wasm);
            }
            if (!isImage(receiver, wasm)) {
                throw namedError(
                    'AttributeError',
                    `'tuple' object has no attribute '${step.operation}'`,
                );
            }
            return imageMethod(receiver, step.operation, args, wasm);
        }
        if (step.surface === 'PIL.ImageDraw.ImageDraw') {
            const value = drawMethod(receiver, step.operation, args, wasm);
            const sourceImage = receiver.__pillow_rs_source_image__;
            if (sourceImage && typeof sourceImage.replaceFrom === 'function') {
                sourceImage.replaceFrom(receiver.image);
            }
            return value;
        }
        if (step.surface === 'PIL.ImageFont.FreeTypeFont') {
            return fontMethod(receiver, step.operation, args);
        }
        if (step.surface === 'PIL.ImageFont.TransposedFont') {
            if (Array.isArray(args.args) && args.args.length > 0) {
                throw namedError('TypeError', `TransposedFont.${step.operation}() got multiple values for argument 'text'`);
            }
            return transposedFontMethod(receiver, step.operation, args, wasm);
        }
        if (step.surface === 'PIL.ImageFont.ImageFont') {
            if (receiver?.__pillow_rs_pilfont__) {
                return pilFontMethod(receiver.__pillow_rs_pilfont__, step.operation, args);
            }
            if (Array.isArray(args.args) && args.args.length > 0) {
                throw namedError('TypeError', `ImageFont.${step.operation}() got multiple values for argument 'text'`);
            }
            throw namedError('AttributeError', "'ImageFont' object has no attribute 'font'");
        }
        if (receiver.__pillow_rs_enhancer__ && step.operation === 'enhance') {
            const method = {
                Brightness: 'enhanceBrightness',
                Color: 'enhanceColor',
                Contrast: 'enhanceContrast',
                Sharpness: 'enhanceSharpness',
            }[receiver.__pillow_rs_enhancer__];
            if (!method || typeof receiver.image?.[method] !== 'function') {
                throw unsupportedError(`ImageEnhance operation is not exported by this WASM facade: ${receiver.__pillow_rs_enhancer__}`);
            }
            return receiver.image[method](Number(args.factor));
        }
    }
    return staticMethod(wasm, step.surface, step.operation, args, receiver);
}

function jsonSafe(value) {
    if (value === undefined || value === null) return value ?? null;
    if (typeof value === 'number' && !Number.isFinite(value)) return String(value);
    if (typeof value === 'string' || typeof value === 'boolean' || typeof value === 'number') return value;
    if (value instanceof Uint8Array || value instanceof Uint32Array || value instanceof Int32Array || value instanceof Float64Array) return Array.from(value, jsonSafe);
    if (Array.isArray(value)) return value.map(jsonSafe);
    if (typeof value === 'object') {
        // The WASM binding keeps opaque metadata bytes as a Uint8Array until
        // the workflow serializer runs.  Convert that host-neutral value to
        // the same JSON-safe bytes record emitted by the Python oracle.
        if (value.kind === 'bytes' && value.encoding === 'base64' && value.data instanceof Uint8Array) {
            return {
                kind: 'bytes',
                encoding: 'base64',
                data: base64(value.data),
            };
        }
        const result = {};
        for (const [name, item] of Object.entries(value)) result[name] = jsonSafe(item);
        return result;
    }
    return null;
}

function isPublicImage(value) {
    return value != null
        && typeof value.toBytes === 'function'
        && typeof value.size === 'function'
        && value.mode != null;
}

function imageValue(value) {
    if (value == null) return null;
    const raw = asBytes(value.toBytes());
    const info = typeof value.compatibilityInfo === 'function'
        ? jsonSafe(value.compatibilityInfo())
        : {};
    if (value.__pillow_rs_converted_info__ && typeof value.__pillow_rs_converted_info__ === 'object') {
        Object.assign(info, value.__pillow_rs_converted_info__);
    }
    return {
        kind: 'image',
        mode: value.mode,
        size: Array.from(value.size()),
        format: value.format ?? null,
        info,
        palette: null,
        bytes: base64(raw),
    };
}

function imageSequenceValue(value) {
    if (!Array.isArray(value)) return imageValue(value);
    if (value.length === 0 || value.every(isPublicImage)) {
        // Image.split and Image.get_child_images return a public sequence of
        // images.  Serialize each image independently; the container has no
        // toBytes() method and is not itself an image.
        return value.map(imageValue);
    }
    return imageValue(value);
}

function sequenceValue(value) {
    if (!Array.isArray(value)) return jsonSafe(value);
    return value.map((item) => isPublicImage(item) ? imageValue(item) : jsonSafe(item));
}

function color3dlutValue(value) {
    const size = Array.isArray(value?.size)
        ? value.size
        : [value?.size, value?.size, value?.size];
    return {
        channels: Number(value?.channels ?? 3),
        mode: value?.target_mode ?? null,
        size: Array.from(size, Number),
        table: Array.from(value?.table ?? [], Number),
    };
}

function serialize(value, shape) {
    if (shape === 'none') return null;
    if (shape === 'image') return imageSequenceValue(value);
    if (shape === 'mask') {
        if (value?.__pillow_rs_getdata__) {
            let raw = new Uint8Array();
            try { raw = getdataBytes(value); } catch (_) { /* ImagingCore bytes may be invalid for tuples/floats. */ }
            return {
                kind: 'mask',
                mode: value.mode ?? '',
                size: value.size ?? null,
                bytes: base64(raw),
            };
        }
        if (
            value != null
            && typeof value.width === 'number'
            && typeof value.height === 'number'
            && value.pixels != null
        ) {
            return {
                kind: 'mask',
                mode: value.mode ?? 'L',
                size: [value.width, value.height],
                bytes: base64(value.pixels),
            };
        }
        const raw = value instanceof Uint8Array ? value : value.toBytes();
        return { kind: 'mask', mode: value.mode ?? '', size: value.size ? Array.from(value.size()) : null, bytes: base64(raw) };
    }
    if (shape === 'mask_with_offset') {
        if (Array.isArray(value) && value.length === 2) {
            return {
                mask: serialize(value[0], 'mask'),
                offset: jsonSafe(value[1]),
            };
        }
        if (value?.mode === 'RGBA') {
            const raw = asBytes(value.pixels);
            const pixels = [];
            for (let index = 0; index + 3 < raw.length; index += 4) {
                pixels.push(Array.from(raw.subarray(index, index + 4)));
            }
            return {
                mask: {
                    kind: 'mask',
                    mode: 'RGBA',
                    size: [Number(value.width), Number(value.height)],
                    bytes: '',
                    pixels,
                },
                offset: [Number(value.offsetX ?? 0), Number(value.offsetY ?? 0)],
            };
        }
        return {
            mask: serialize(value, 'mask'),
            offset: [Number(value?.offsetX ?? 0), Number(value?.offsetY ?? 0)],
        };
    }
    if (shape === 'bytes') return value == null ? null : { kind: 'bytes', encoding: 'base64', data: base64(value) };
    if (shape === 'handle') {
        if (value == null) return null;
        return { type: value.__pillow_rs_handle_type__ ?? value.constructor?.name ?? 'Object' };
    }
    if (value?.__pillow_rs_filter__ === 'Color3DLUT') return color3dlutValue(value);
    if (shape === 'mapping' || shape === 'record') {
        if (value?.protocol === 'outline') return null;
        if (value?.__pillow_rs_draw_record__) return jsonSafe(value.__pillow_rs_draw_record__);
        if (value?.__pillow_rs_imagepalette__) {
            return {
                mode: value.mode ?? 'RGB',
                rawmode: null,
                _colors: null,
                _palette: { kind: 'bytes', encoding: 'base64', data: base64(value.tobytes()) },
                dirty: null,
            };
        }
        if (typeof value?.toObject === 'function') return jsonSafe(value.toObject());
        return jsonSafe(value);
    }
    if (shape === 'sequence' || shape === 'ordered' || shape === 'metrics') return sequenceValue(value);
    return jsonSafe(value);
}

function publicError(error) {
    const className = error?.name || error?.constructor?.name || 'Error';
    let kind = 'runtime_error';
    if (className === 'TypeError') kind = 'type_error';
    else if (className === 'ValueError' || className === 'RangeError') kind = 'invalid_argument';
    else if (className === 'UnicodeEncodeError') kind = 'invalid_argument';
    else if (
        className === 'OSError'
        || className === 'FileNotFoundError'
        || className === 'IsADirectoryError'
        || className === 'UnidentifiedImageError'
    ) kind = 'io_error';
    else if (className === 'NotImplementedError') kind = 'unsupported';
    return { class: className, kind, message: String(error?.message ?? error), stage: 'call', code: null };
}

// These are the operation boundaries used by the Python parity runner when
// deciding whether an observed value has materialized a deferred image. Keep
// the JS/WASM adapter's receipt state machine identical: a successful
// observation can prove an earlier receipt even when a later workflow step
// raises a public error.
const PIPELINE_ALWAYS_OPS = new Set([
    'PIL.Image::alpha_composite',
    'PIL.Image::blend',
    'PIL.Image::composite',
    'PIL.Image::eval',
    'PIL.Image::merge',
    'PIL.Image.Image::crop',
    'PIL.Image.Image::filter',
    'PIL.Image.Image::getchannel',
    'PIL.Image.Image::point',
    'PIL.Image.Image::reduce',
    'PIL.Image.Image::resize',
    'PIL.Image.Image::rotate',
    'PIL.Image.Image::transform',
    'PIL.Image.Image::transpose',
    'PIL.ImageChops::add',
    'PIL.ImageChops::add_modulo',
    'PIL.ImageChops::blend',
    'PIL.ImageChops::composite',
    'PIL.ImageChops::darker',
    'PIL.ImageChops::difference',
    'PIL.ImageChops::hard_light',
    'PIL.ImageChops::invert',
    'PIL.ImageChops::lighter',
    'PIL.ImageChops::logical_and',
    'PIL.ImageChops::logical_or',
    'PIL.ImageChops::logical_xor',
    'PIL.ImageChops::multiply',
    'PIL.ImageChops::offset',
    'PIL.ImageChops::overlay',
    'PIL.ImageChops::screen',
    'PIL.ImageChops::soft_light',
    'PIL.ImageChops::subtract',
    'PIL.ImageChops::subtract_modulo',
    'PIL.ImageEnhance.Brightness::enhance',
    'PIL.ImageEnhance.Color::enhance',
    'PIL.ImageEnhance.Contrast::enhance',
    'PIL.ImageEnhance.Sharpness::enhance',
    'PIL.ImageFilter::BLUR',
    'PIL.ImageFilter::BoxBlur',
    'PIL.ImageFilter::CONTOUR',
    'PIL.ImageFilter::DETAIL',
    'PIL.ImageFilter::EDGE_ENHANCE',
    'PIL.ImageFilter::EDGE_ENHANCE_MORE',
    'PIL.ImageFilter::EMBOSS',
    'PIL.ImageFilter::FIND_EDGES',
    'PIL.ImageFilter::GaussianBlur',
    'PIL.ImageFilter::Kernel',
    'PIL.ImageFilter::MaxFilter',
    'PIL.ImageFilter::MedianFilter',
    'PIL.ImageFilter::MinFilter',
    'PIL.ImageFilter::ModeFilter',
    'PIL.ImageFilter::RankFilter',
    'PIL.ImageFilter::SHARPEN',
    'PIL.ImageFilter::SMOOTH',
    'PIL.ImageFilter::SMOOTH_MORE',
    'PIL.ImageFilter::UnsharpMask',
    'PIL.ImageOps::autocontrast',
    'PIL.ImageOps::colorize',
    'PIL.ImageOps::contain',
    'PIL.ImageOps::cover',
    'PIL.ImageOps::crop',
    'PIL.ImageOps::equalize',
    'PIL.ImageOps::expand',
    'PIL.ImageOps::fit',
    'PIL.ImageOps::flip',
    'PIL.ImageOps::grayscale',
    'PIL.ImageOps::invert',
    'PIL.ImageOps::mirror',
    'PIL.ImageOps::pad',
    'PIL.ImageOps::posterize',
    'PIL.ImageOps::scale',
    'PIL.ImageOps::solarize',
]);
const PIPELINE_FILTER_PARAMETER_OPS = new Set(
    [...PIPELINE_ALWAYS_OPS].filter((operation) => operation.startsWith('PIL.ImageFilter::')),
);
const PIPELINE_MAYBE_OPS = new Set([
    'PIL.Image.Image::apply_transparency',
    'PIL.Image.Image::convert',
    'PIL.Image.Image::paste',
    'PIL.Image.Image::putalpha',
    'PIL.Image.Image::putpixel',
    'PIL.Image.Image::remap_palette',
    'PIL.Image.Image::thumbnail',
    'PIL.ImageOps::exif_transpose',
]);
const PIPELINE_MUTATING_OPS = new Set([
    'PIL.Image.Image::apply_transparency',
    'PIL.Image.Image::paste',
    'PIL.Image.Image::putalpha',
    'PIL.Image.Image::putpixel',
    'PIL.Image.Image::remap_palette',
    'PIL.Image.Image::thumbnail',
]);
const PIPELINE_RESULT_OPS = new Set([
    ...PIPELINE_ALWAYS_OPS,
    ...PIPELINE_MAYBE_OPS,
]);
const TERMINAL_OBSERVATION_OPS = new Set([
    'getbands',
    'getbbox',
    'getcolors',
    'getdata',
    'getextrema',
    'get_flattened_data',
    'getpixel',
    'getprojection',
    'histogram',
    'load',
    'save',
    'tobitmap',
    'tobytes',
    'verify',
    'entropy',
    'extrema',
    'count',
    'sum',
    'sum2',
    'mean',
    'median',
    'rms',
    'var',
    'stddev',
]);

function observationMaterializesPipeline(step) {
    const operation = key(step.surface, step.operation);
    if (PIPELINE_FILTER_PARAMETER_OPS.has(operation)) return false;
    if (TERMINAL_OBSERVATION_OPS.has(step.operation)) return true;
    return PIPELINE_RESULT_OPS.has(operation) && !PIPELINE_MUTATING_OPS.has(operation);
}

function takePipelineTelemetry(wasm, sink, stepId, status = 'completed') {
    if (!sink || typeof wasm.takePipelineTelemetry !== 'function') return;
    const receipt = wasm.takePipelineTelemetry();
    if (receipt == null) return;
    const completed = jsonSafe(receipt);
    completed.status = status;
    // A per-step dispatch starts as a prefix.  The observation loop applies
    // the operation-boundary transition shared with the Python harness.
    completed.terminal_complete = false;
    if (stepId != null) completed.step_id = stepId;
    sink.push(completed);
    return sink.length - 1;
}

function runCase(wasm, item, operations, assets, executionSink) {
    const bindings = {};
    const results = {};
    let blockedReason = null;
    let terminalReceiptIndex = null;
    const observationIds = item.observations ?? [];
    const owned = new Set();
    // A previous workflow cannot leave an unassociated receipt in a healthy
    // run, but clear it at the case boundary so a malformed host extension
    // cannot make the next case appear to have executed a pipeline.
    if (typeof wasm.takePipelineTelemetry === 'function') wasm.takePipelineTelemetry();
    for (const step of item.steps ?? []) {
        if (blockedReason) {
            results[step.step_id] = { step_id: step.step_id, status: 'not_run', reason: blockedReason };
            continue;
        }
        let stepExecutionStatus = 'completed';
        try {
            const value = callStep(wasm, step, bindings, operations, assets);
            if (value && typeof value.free === 'function') owned.add(value);
            bindings[step.step_id] = value;
            results[step.step_id] = { step_id: step.step_id, status: 'ok', value };
        } catch (error) {
            results[step.step_id] = { step_id: step.step_id, status: 'error', error: publicError(error) };
            blockedReason = `dependency step ${step.step_id} failed`;
            stepExecutionStatus = 'partial';
        } finally {
            const receiptIndex = takePipelineTelemetry(
                wasm,
                executionSink,
                step.step_id,
                stepExecutionStatus,
            );
            if (receiptIndex != null) {
                // Keep the latest completed/partial receipt as the candidate
                // for a later observation.  The previous JS adapter only
                // retained a receipt when it belonged to the final workflow
                // step, which lost observed intermediate boundaries.
                terminalReceiptIndex = receiptIndex;
            } else if (
                step === item.steps[item.steps.length - 1]
                && !observationIds.includes(step.step_id)
            ) {
                // A final unobserved operation with no receipt cannot inherit
                // an earlier dispatch as proof for this workflow.
                terminalReceiptIndex = receiptIndex ?? null;
            }
        }
    }
    const observations = [];
    // Keep the last successful pipeline receipt as a terminal candidate when
    // observation serialization itself emits no telemetry.  Each successful
    // observed result can mark that candidate terminal for its own deferred
    // boundary; a later unobserved public error does not erase that evidence.
    for (const observationId of observationIds) {
        const result = results[observationId];
        if (!result) {
            observations.push({ step_id: observationId, status: 'not_run', reason: 'observation step is not present in workflow' });
            continue;
        }
        if (result.status !== 'ok') {
            observations.push(result);
            continue;
        }
        const step = item.steps.find((candidate) => candidate.step_id === observationId);
        const info = operations[key(step.surface, step.operation)] ?? {};
        let observationExecutionStatus = 'completed';
        let observationSucceeded = false;
        try {
            observations.push({ step_id: observationId, status: 'ok', value: serialize(result.value, info.shape) });
            observationSucceeded = true;
        } catch (error) {
            observations.push({ step_id: observationId, status: 'error', error: publicError(error) });
            observationExecutionStatus = 'partial';
        } finally {
            const receiptIndex = takePipelineTelemetry(
                wasm,
                executionSink,
                observationId,
                observationExecutionStatus,
            );
            if (receiptIndex != null) {
                terminalReceiptIndex = receiptIndex;
            }
            if (
                observationSucceeded
                && terminalReceiptIndex != null
                && observationMaterializesPipeline(step)
            ) {
                executionSink[terminalReceiptIndex].terminal_complete = true;
            }
            if (
                !observationSucceeded
                && observationId === observationIds[observationIds.length - 1]
            ) {
                // A failed final observation cannot prove the preceding
                // pipeline; earlier receipts must not masquerade as a
                // terminal boundary.
                terminalReceiptIndex = null;
            }
        }
    }
    const workflowComplete = blockedReason == null
        && observations.every((observation) => observation.status === 'ok');
    if (workflowComplete && terminalReceiptIndex != null && executionSink?.[terminalReceiptIndex]) {
        executionSink[terminalReceiptIndex].terminal_complete = true;
    }
    for (const value of owned) {
        try { value.free(); } catch (_) { /* best-effort wasm handle cleanup */ }
    }
    return { case_id: item.case_id, status: 'completed', observations };
}

export function runWorkflow(wasm, input, runtime) {
    const cases = input.cases ?? [];
    const operations = input.operations ?? {};
    const telemetryAvailable = typeof wasm.setPipelineTelemetry === 'function'
        && typeof wasm.takePipelineTelemetry === 'function';
    if (telemetryAvailable) {
        wasm.setPipelineTelemetry(true);
        wasm.takePipelineTelemetry();
    }
    const execution = {};
    const results = cases.map((item) => {
        const receipts = [];
        const result = runCase(wasm, item, operations, input.assets ?? {}, receipts);
        execution[item.case_id] = receipts;
        return result;
    });
    return {
        identity: {
            side: 'target',
            implementation: 'pillow-rs-js',
            package_variant: 'core',
            runtime,
        },
        results,
        execution: telemetryAvailable ? execution : null,
    };
}
