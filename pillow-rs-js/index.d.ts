/**
 * Public declarations for the single pillow-rs npm package. Browser
 * bundlers resolve the public wrapper around the generated web module; Node.js
 * resolves node.js, whose no-argument initializer reads the bundled WASM file
 * from disk.
 */
export * from './pkg/core/pillow_rs_js';

import type { ImageDraw as CoreImageDraw } from './pkg/core/pillow_rs_js';

/** Drawing context that applies mutations to the image passed to its constructor. */
export declare class ImageDraw extends CoreImageDraw {}

/** Host metadata associated with an image. Derived images follow Pillow's
 * shallow-copy behavior for operations that return a new image. */
export interface Image {
    info: Record<string, unknown>;
    /** Applies a byte lookup table or a callback to each image value. */
    point(
        lut: Uint8Array | number[] | ((value: number) => number),
        mode?: string | null,
    ): Image;
    /** Materializes pixels and returns a mutable pixel view. */
    load(): PixelAccess;
    /** Returns this WASM-backed image as its live core handle. */
    getim(): Image;
    /** Decoder-specific draft optimization is unavailable for WASM images. */
    draft(mode: string, size: [number, number]): null;
    /** Defaults to RGB; pass null to select the retained palette layout. */
    getpalette(rawmode?: string | null): number[] | null;
    /** Remaps palette indices and optionally supplies the source palette bytes. */
    remapPalette(destMap: Uint8Array, sourcePalette?: Uint8Array | null): Image;
    /** @internal Validates a putdata sequence before reading its values. */
    __validatePutdataLength(length: number): void;
    /** @internal Checks whether a numeric array is safe for bulk conversion. */
    __putdataArrayIsSimple(data: unknown): boolean;
    /** @internal Writes one coerced value before the next sequence item is read. */
    __putdataValueAt(index: number, value: unknown, scale?: number | null, offset?: number | null): void;
}

/** Mutable pixel view returned by Image.load(). */
export declare class PixelAccess {
    getPixel(x: number, y: number): number | number[];
    putPixel(x: number, y: number, value: number | number[]): void;
}

export interface ViewerOptions {
    title?: string | null;
    [name: string]: unknown;
}

/** Base class for an image display viewer. A viewer may override `show`,
 * `showImage`, or `showFile` to connect to its host display service. */
export declare class Viewer {
    static format: string | null;
    static options: Record<string, unknown>;
    format: string | null;
    options: Record<string, unknown>;
    show(image: Image, options?: ViewerOptions): number;
    getFormat(image: Image): string | null;
    getCommand(path: string, options?: ViewerOptions): string;
    saveImage(image: Image): string;
    showImage(image: Image, options?: ViewerOptions): number;
    showFile(path: string, options?: ViewerOptions): number;
}

export declare class WindowsViewer extends Viewer {}
export declare class MacViewer extends Viewer {}
export declare class UnixViewer extends Viewer {}
export declare class XDGViewer extends UnixViewer {}
export declare class DisplayViewer extends UnixViewer {}
export declare class GmDisplayViewer extends UnixViewer {}
export declare class EogViewer extends UnixViewer {}
export declare class XVViewer extends UnixViewer {}

/** Viewer registry and dispatcher for the host environment. */
export declare const ImageShow: {
    Viewer: typeof Viewer;
    WindowsViewer: typeof WindowsViewer;
    MacViewer: typeof MacViewer;
    UnixViewer: typeof UnixViewer;
    XDGViewer: typeof XDGViewer;
    DisplayViewer: typeof DisplayViewer;
    GmDisplayViewer: typeof GmDisplayViewer;
    EogViewer: typeof EogViewer;
    XVViewer: typeof XVViewer;
    _viewers: Viewer[];
    register(viewer: Viewer | (new () => Viewer), order?: number): void;
    show(image: Image, title?: string | null, options?: ViewerOptions): boolean;
};

/** Mutable path builder consumed by ImageDraw.shape(). */
export declare class Outline {
    move(x: number, y: number): void;
    line(x: number, y: number): void;
    curve(x1: number, y1: number, x2: number, y2: number, x3: number, y3: number): void;
    close(): void;
    toPoints(): number[][];
}

import type {
    InitInput,
    InitOutput,
    SyncInitInput,
} from './pkg/core/pillow_rs_js';

/** Initialize from bundled bytes or an optional synchronous WASM input. */
export declare function initSync(
    module?: { module: SyncInitInput } | SyncInitInput,
): InitOutput;

/** Initialize the browser or Node.js package entry for the current runtime. */
export default function init(
    moduleOrPath?:
        | { module_or_path: InitInput | Promise<InitInput> }
        | InitInput
        | Promise<InitInput>,
): Promise<InitOutput>;
