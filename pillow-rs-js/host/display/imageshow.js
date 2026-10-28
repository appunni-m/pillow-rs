export function createImageShow(Image) {
    const imageShowModeBase = Object.freeze({
        '1': 'L', L: 'L', I: 'L', F: 'L', P: 'P', RGB: 'RGB', RGBX: 'RGB',
        RGBA: 'RGB', CMYK: 'RGB', YCbCr: 'RGB', LAB: 'RGB', HSV: 'RGB',
        RGBa: 'RGB', LA: 'L', La: 'L', PA: 'RGB', 'I;16': 'L', 'I;16S': 'L',
        'I;16L': 'L', 'I;16LS': 'L', 'I;16B': 'L', 'I;16BS': 'L',
        'I;16N': 'L', 'I;16NS': 'L', 'I;32': 'L', 'I;32B': 'L',
        'I;32L': 'L', 'I;32S': 'L', 'I;32BS': 'L', 'I;32LS': 'L',
    });

    function imageShowError(name, message) {
        const error = new Error(message);
        error.name = name;
        return error;
    }

    function makeDefaultImageShowRuntime() {
        const files = new Map();
        let nextFile = 0;
        const unavailableProcess = () => {
            throw imageShowError('UnsupportedError', 'display process service is unavailable');
        };
        return {
            writeTemp(bytes, suffix = '') {
                const path = `pillow-rs-display-${nextFile++}${suffix}`;
                files.set(path, Uint8Array.from(bytes));
                return path;
            },
            exists(path) { return files.has(path); },
            readFile(path) {
                const bytes = files.get(path);
                if (bytes === undefined) throw imageShowError('FileNotFoundError', path);
                return bytes.slice();
            },
            remove(path) { files.delete(path); },
            Popen: unavailableProcess,
            call: unavailableProcess,
            system: unavailableProcess,
            pythonExecutable: null,
            scheduleRemove: null,
        };
    }

    let imageShowRuntime = makeDefaultImageShowRuntime();

    function concatenateBytes(...parts) {
        const length = parts.reduce((total, part) => total + part.length, 0);
        const output = new Uint8Array(length);
        let offset = 0;
        for (const part of parts) {
            output.set(part, offset);
            offset += part.length;
        }
        return output;
    }

    function encodeDisplayPpm(image) {
        let converted = null;
        let source = image;
        if (image.mode === '1') {
            converted = image.convertWithInput('L', null, null, null, null);
            source = converted;
        } else if (image.mode !== 'L' && image.mode !== 'RGB') {
            converted = image.convertWithInput('RGB', null, null, null, null);
            source = converted;
        }
        try {
            const [width, height] = Array.from(source.size());
            const mode = source.mode;
            const header = new TextEncoder().encode(
                `${mode === 'L' ? 'P5' : 'P6'}\n${width} ${height}\n255\n`,
            );
            return concatenateBytes(header, source.toBytes());
        } finally {
            if (converted) converted.free();
        }
    }

    function encodeViewerImage(image, format, options) {
        if (format == null || format === 'PPM') return encodeDisplayPpm(image);
        const pairs = [];
        for (const [name, value] of Object.entries(options ?? {})) {
            if (name === 'save_all') continue;
            pairs.push(name, String(value));
        }
        return image.saveWithOptions(format, pairs);
    }

    function quotedShellArgument(value) {
        const text = String(value);
        if (!text || /[^A-Za-z0-9_@%+=:,./-]/.test(text)) {
            return `'${text.replaceAll("'", "'\\''")}'`;
        }
        return text;
    }

    class Viewer {
        static format = null;
        static options = {};

        constructor() {
            this.format = this.constructor.format ?? null;
            this.options = { ...(this.constructor.options ?? {}) };
        }

        show(image, options = {}) {
            let shown = image;
            let converted = null;
            const preserve = image.mode === '1' || image.mode === 'RGBA'
                || (this.format === 'PNG' && ['I;16', 'LA'].includes(image.mode));
            if (!preserve) {
                const baseMode = imageShowModeBase[image.mode];
                if (baseMode === undefined) throw imageShowError('KeyError', image.mode);
                if (image.mode !== baseMode) {
                    converted = image.convertWithInput(baseMode, null, null, null, null);
                    shown = converted;
                }
            }
            try {
                return this.showImage(shown, options);
            } finally {
                if (converted) converted.free();
            }
        }

        getFormat(_image) { return this.format; }

        getCommand(_path, _options = {}) {
            throw imageShowError('NotImplementedError', 'unavailable in base viewer');
        }

        saveImage(image) {
            const format = this.getFormat(image);
            const bytes = encodeViewerImage(image, format, this.options);
            const suffix = format == null ? '' : `.${format}`;
            return imageShowRuntime.writeTemp(bytes, suffix);
        }

        showImage(image, options = {}) {
            return this.showFile(this.saveImage(image), options);
        }

        showFile(path, options = {}) {
            if (!imageShowRuntime.exists(path)) throw imageShowError('FileNotFoundError', path);
            const command = this.getCommand(path, options);
            imageShowRuntime.system.call(this, command);
            return 1;
        }
    }

    const pngViewerOptions = Object.freeze({ compress_level: 1, save_all: true });

    class WindowsViewer extends Viewer {
        static format = 'PNG';
        static options = pngViewerOptions;

        getCommand(path) {
            return `start "Pillow" /WAIT "${path}" && ping -n 4 127.0.0.1 >NUL && del /f "${path}"`;
        }

        showFile(path) {
            if (!imageShowRuntime.exists(path)) throw imageShowError('FileNotFoundError', path);
            imageShowRuntime.Popen.call(this, this.getCommand(path), { shell: true, creationflags: 0 });
            return 1;
        }
    }

    class MacViewer extends Viewer {
        static format = 'PNG';
        static options = pngViewerOptions;

        getCommand(path) {
            return `(open -a Preview.app ${quotedShellArgument(path)}; sleep 20; rm -f ${quotedShellArgument(path)})&`;
        }

        showFile(path) {
            if (!imageShowRuntime.exists(path)) throw imageShowError('FileNotFoundError', path);
            imageShowRuntime.call.call(this, ['open', '-a', 'Preview.app', path]);
            if (imageShowRuntime.pythonExecutable) {
                imageShowRuntime.Popen.call(this, [
                    imageShowRuntime.pythonExecutable,
                    '-c',
                    'import os, sys, time; time.sleep(20); os.remove(sys.argv[1])',
                    path,
                ]);
            } else if (typeof imageShowRuntime.scheduleRemove === 'function') {
                imageShowRuntime.scheduleRemove(path, 20_000);
            }
            return 1;
        }
    }

    class UnixViewer extends Viewer {
        getCommandEx(_path, _options = {}) {
            throw imageShowError('NotImplementedError', 'viewer executable is not configured');
        }

        getCommand(path, options = {}) {
            const [command] = this.getCommandEx(path, options);
            return `${command} ${quotedShellArgument(path)}`;
        }
    }

    class XDGViewer extends UnixViewer {
        static format = 'PNG';
        static options = pngViewerOptions;
        getCommandEx() { return ['xdg-open', 'xdg-open']; }
        showFile(path) {
            if (!imageShowRuntime.exists(path)) throw imageShowError('FileNotFoundError', path);
            imageShowRuntime.Popen.call(this, ['xdg-open', path]);
            return 1;
        }
    }

    class DisplayViewer extends UnixViewer {
        static format = 'PNG';
        static options = pngViewerOptions;
        getCommandEx(_path, options = {}) {
            const command = options.title
                ? `display -title ${quotedShellArgument(options.title)}` : 'display';
            return [command, 'display'];
        }
        showFile(path, options = {}) {
            if (!imageShowRuntime.exists(path)) throw imageShowError('FileNotFoundError', path);
            const args = ['display'];
            if (options.title) args.push('-title', options.title);
            args.push(path);
            imageShowRuntime.Popen.call(this, args);
            return 1;
        }
    }

    class GmDisplayViewer extends UnixViewer {
        static format = 'PNG';
        static options = pngViewerOptions;
        getCommandEx() { return ['gm display', 'gm']; }
        showFile(path) {
            if (!imageShowRuntime.exists(path)) throw imageShowError('FileNotFoundError', path);
            imageShowRuntime.Popen.call(this, ['gm', 'display', path]);
            return 1;
        }
    }

    class EogViewer extends UnixViewer {
        static format = 'PNG';
        static options = pngViewerOptions;
        getCommandEx() { return ['eog -n', 'eog']; }
        showFile(path) {
            if (!imageShowRuntime.exists(path)) throw imageShowError('FileNotFoundError', path);
            imageShowRuntime.Popen.call(this, ['eog', '-n', path]);
            return 1;
        }
    }

    class XVViewer extends UnixViewer {
        static format = 'PNG';
        static options = pngViewerOptions;
        getCommandEx(_path, options = {}) {
            const command = options.title
                ? `xv -name ${quotedShellArgument(options.title)}` : 'xv';
            return [command, 'xv'];
        }
        showFile(path, options = {}) {
            if (!imageShowRuntime.exists(path)) throw imageShowError('FileNotFoundError', path);
            const args = ['xv'];
            if (options.title) args.push('-name', options.title);
            args.push(path);
            imageShowRuntime.Popen.call(this, args);
            return 1;
        }
    }

    const imageShowViewers = [];

    const ImageShow = {
        Viewer,
        WindowsViewer,
        MacViewer,
        UnixViewer,
        XDGViewer,
        DisplayViewer,
        GmDisplayViewer,
        EogViewer,
        XVViewer,
        _viewers: imageShowViewers,

        register(viewer, order = 1) {
            if (typeof viewer === 'function' && viewer.prototype instanceof Viewer) {
                viewer = new viewer();
            }
            if (Number(order) > 0) imageShowViewers.push(viewer);
            else imageShowViewers.unshift(viewer);
        },

        show(image, title = null, options = {}) {
            if (!(image instanceof Image)) {
                throw imageShowError('TypeError', 'image must be a Pillow image');
            }
            const viewerOptions = { ...options, title };
            for (const viewer of imageShowViewers) {
                if (viewer.show(image, viewerOptions)) return true;
            }
            return false;
        },

        _setRuntime(runtime) {
            const previous = imageShowRuntime;
            imageShowRuntime = runtime;
            return () => { imageShowRuntime = previous; };
        },
    };

    return {
        ImageShow,
        Viewer,
        WindowsViewer,
        MacViewer,
        UnixViewer,
        XDGViewer,
        DisplayViewer,
        GmDisplayViewer,
        EogViewer,
        XVViewer,
    };
}
