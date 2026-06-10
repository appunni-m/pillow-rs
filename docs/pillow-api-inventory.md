# Pillow 12.2.0 API Surface — Complete Inventory

Auto-generated from `inspect.signature()` on installed Pillow 12.2.0.
This is the master checklist. Every method listed here must be in manifest.yaml.

---

## MODULE: Image (class) — 52 methods + 8 properties

### Classmethods
| # | Method | Signature |
|---|--------|-----------|
| 1 | open | `(fp: StrOrBytesPath | IO[bytes], mode: Literal['r'] = 'r', formats: list[str] | tuple[str, ...] | None = None) -> ImageFile` |
| 2 | new | `(mode: str, size: tuple[int, int] | list[int], color: float | tuple[float, ...] | str | None = 0) -> Image` |

### Instance Methods
| # | Method | Signature |
|---|--------|-----------|
| 3 | alpha_composite | `(self, im: Image, dest: Sequence[int] = (0,0), source: Sequence[int] = (0,0)) -> None` |
| 4 | apply_transparency | `(self) -> None` |
| 5 | close | `(self) -> None` |
| 6 | convert | `(self, mode: str | None = None, matrix: tuple[float, ...] | None = None, dither: Dither | None = None, palette: Palette = WEB, colors: int = 256) -> Image` |
| 7 | copy | `(self) -> Image` |
| 8 | crop | `(self, box: tuple[float,float,float,float] | None = None) -> Image` |
| 9 | draft | `(self, mode: str | None, size: tuple[int,int] | None) -> tuple[str, tuple[int,int,float,float]] | None` |
| 10 | effect_spread | `(self, distance: int) -> Image` |
| 11 | entropy | `(self, mask: Image | None = None, extrema: tuple[float,float] | None = None) -> float` |
| 12 | filter | `(self, filter: ImageFilter.Filter | type[ImageFilter.Filter]) -> Image` |
| 13 | frombytes | `(self, data: bytes | bytearray | SupportsArrayInterface, decoder_name: str = 'raw', *args: Any) -> None` |
| 14 | get_child_images | `(self) -> list[ImageFile.ImageFile]` |
| 15 | get_flattened_data | `(self, band: int | None = None) -> tuple[tuple[int,...],...] | tuple[float,...]` |
| 16 | getbands | `(self) -> tuple[str, ...]` |
| 17 | getbbox | `(self, *, alpha_only: bool = True) -> tuple[int,int,int,int] | None` |
| 18 | getchannel | `(self, channel: int | str) -> Image` |
| 19 | getcolors | `(self, maxcolors: int = 256) -> list[tuple[int,tuple[int,...]]] | list[tuple[int,float]] | None` |
| 20 | getdata | `(self, band: int | None = None) -> core.ImagingCore` |
| 21 | getexif | `(self) -> Exif` |
| 22 | getextrema | `(self) -> tuple[float,float] | tuple[tuple[int,int],...]` |
| 23 | getim | `(self) -> CapsuleType` |
| 24 | getpalette | `(self, rawmode: str | None = 'RGB') -> list[int] | None` |
| 25 | getpixel | `(self, xy: tuple[int,int] | list[int]) -> float | tuple[int,...] | None` |
| 26 | getprojection | `(self) -> tuple[list[int], list[int]]` |
| 27 | getxmp | `(self) -> dict[str, Any]` |
| 28 | histogram | `(self, mask: Image | None = None, extrema: tuple[float,float] | None = None) -> list[int]` |
| 29 | load | `(self) -> core.PixelAccess | None` |
| 30 | paste | `(self, im: Image | str | float | tuple[float,...], box: Image | tuple[int,int,int,int] | tuple[int,int] | None = None, mask: Image | None = None) -> None` |
| 31 | point | `(self, lut: Sequence[float] | NumpyArray | Callable[[int],float] | Callable[[ImagePointTransform], ImagePointTransform | float] | ImagePointHandler, mode: str | None = None) -> Image` |
| 32 | putalpha | `(self, alpha: Image | int) -> None` |
| 33 | putdata | `(self, data: Sequence[float] | Sequence[Sequence[int]] | core.ImagingCore | NumpyArray, scale: float = 1.0, offset: float = 0.0) -> None` |
| 34 | putpalette | `(self, data: ImagePalette.ImagePalette | bytes | Sequence[int], rawmode: str = 'RGB') -> None` |
| 35 | putpixel | `(self, xy: tuple[int,int], value: float | tuple[int,...] | list[int]) -> None` |
| 36 | quantize | `(self, colors: int = 256, method: int | None = None, kmeans: int = 0, palette: Image | None = None, dither: Dither = FLOYDSTEINBERG) -> Image` |
| 37 | reduce | `(self, factor: int | tuple[int,int], box: tuple[int,int,int,int] | None = None) -> Image` |
| 38 | remap_palette | `(self, dest_map: list[int], source_palette: bytes | bytearray | None = None) -> Image` |
| 39 | resize | `(self, size: tuple[int,int] | list[int] | NumpyArray, resample: int | None = None, box: tuple[float,float,float,float] | None = None, reducing_gap: float | None = None) -> Image` |
| 40 | rotate | `(self, angle: float, resample: Resampling = NEAREST, expand: int | bool = False, center: tuple[float,float] | None = None, translate: tuple[int,int] | None = None, fillcolor: float | tuple[float,...] | str | None = None) -> Image` |
| 41 | save | `(self, fp: StrOrBytesPath | IO[bytes], format: str | None = None, **params: Any) -> None` |
| 42 | seek | `(self, frame: int) -> None` |
| 43 | show | `(self, title: str | None = None) -> None` |
| 44 | split | `(self) -> tuple[Image, ...]` |
| 45 | tell | `(self) -> int` |
| 46 | thumbnail | `(self, size: tuple[float,float], resample: Resampling = BICUBIC, reducing_gap: float | None = 2.0) -> None` |
| 47 | tobitmap | `(self, name: str = 'image') -> bytes` |
| 48 | tobytes | `(self, encoder_name: str = 'raw', *args: Any) -> bytes` |
| 49 | toqimage | `(self) -> ImageQt.ImageQt` |
| 50 | toqpixmap | `(self) -> ImageQt.QPixmap` |
| 51 | transform | `(self, size: tuple[int,int], method: Transform | ImageTransformHandler | SupportsGetData, data: Sequence[Any] | None = None, resample: int = NEAREST, fill: int = 1, fillcolor: float | tuple[float,...] | str | None = None) -> Image` |
| 52 | transpose | `(self, method: Transpose) -> Image` |
| 53 | verify | `(self) -> None` |

### Properties
| # | Name | Type |
|---|------|------|
| 54 | mode | `str` |
| 55 | size | `tuple[int,int]` |
| 56 | width | `int` |
| 57 | height | `int` |
| 58 | format | `str | None` |
| 59 | info | `dict` |
| 60 | palette | `ImagePalette | None` |
| 61 | readonly | `int` |
| 62 | filename | `str` |
| 63 | im | `core.ImagingCore` |
| 64 | has_transparency_data | `bool` |

**Total Image: 64 API items**

---

## MODULE: Image (module-level) — 30 functions

| # | Function | Signature |
|---|----------|-----------|
| 1 | alpha_composite | `(im1: Image, im2: Image) -> Image` |
| 2 | blend | `(im1: Image, im2: Image, alpha: float) -> Image` |
| 3 | composite | `(image1: Image, image2: Image, mask: Image) -> Image` |
| 4 | effect_mandelbrot | `(size: tuple[int,int], extent: tuple[float,float,float,float], quality: int) -> Image` |
| 5 | effect_noise | `(size: tuple[int,int], sigma: float) -> Image` |
| 6 | eval | `(image: Image, *args: Callable[[int],float]) -> Image` |
| 7 | fromarray | `(obj: SupportsArrayInterface, mode: str | None = None) -> Image` |
| 8 | fromarrow | `(obj: SupportsArrowArrayInterface, mode: str, size: tuple[int,int]) -> Image` |
| 9 | frombuffer | `(mode: str, size: tuple[int,int], data: bytes | SupportsArrayInterface, decoder_name: str = 'raw', *args: Any) -> Image` |
| 10 | frombytes | `(mode: str, size: tuple[int,int], data: bytes | bytearray | SupportsArrayInterface, decoder_name: str = 'raw', *args: Any) -> Image` |
| 11 | fromqimage | `(im: ImageQt.QImage) -> ImageFile` |
| 12 | fromqpixmap | `(im: ImageQt.QPixmap) -> ImageFile` |
| 13 | getmodebandnames | `(mode: str) -> tuple[str, ...]` |
| 14 | getmodebands | `(mode: str) -> int` |
| 15 | getmodebase | `(mode: str) -> str` |
| 16 | getmodetype | `(mode: str) -> str` |
| 17 | init | `() -> bool` |
| 18 | linear_gradient | `(mode: str) -> Image` |
| 19 | merge | `(mode: str, bands: Sequence[Image]) -> Image` |
| 20 | open | `(fp: StrOrBytesPath | IO[bytes], mode: Literal['r'] = 'r', formats: list[str] | tuple[str,...] | None = None) -> ImageFile` |
| 21 | radial_gradient | `(mode: str) -> Image` |
| 22 | register_decoder | `(name: str, decoder: type[ImageFile.PyDecoder]) -> None` |
| 23 | register_encoder | `(name: str, encoder: type[ImageFile.PyEncoder]) -> None` |
| 24 | register_extension | `(id: str, extension: str) -> None` |
| 25 | register_extensions | `(id: str, extensions: list[str]) -> None` |
| 26 | register_mime | `(id: str, mimetype: str) -> None` |
| 27 | register_open | `(id: str, factory: ..., accept: ... = None) -> None` |
| 28 | register_save | `(id: str, driver: ...) -> None` |
| 29 | register_save_all | `(id: str, driver: ...) -> None` |
| 30 | registered_extensions | `() -> dict[str, str]` |

**Total Image module: 30 functions**

---

## MODULE: ImageDraw — 17 drawing methods

| # | Method | Signature |
|---|--------|-----------|
| 1 | arc | `(self, xy: Coords, start: float, end: float, fill: _Ink | None = None, width: int = 1) -> None` |
| 2 | bitmap | `(self, xy: Sequence[int], bitmap: Image, fill: _Ink | None = None) -> None` |
| 3 | chord | `(self, xy: Coords, start: float, end: float, fill: _Ink | None = None, outline: _Ink | None = None, width: int = 1) -> None` |
| 4 | circle | `(self, xy: Sequence[float], radius: float, fill: _Ink | None = None, outline: _Ink | None = None, width: int = 1) -> None` |
| 5 | ellipse | `(self, xy: Coords, fill: _Ink | None = None, outline: _Ink | None = None, width: int = 1) -> None` |
| 6 | getfont | `(self) -> ImageFont...` |
| 7 | line | `(self, xy: Coords, fill: _Ink | None = None, width: int = 0, joint: str | None = None) -> None` |
| 8 | multiline_text | `(self, xy: tuple[float,float], text: AnyStr, fill: _Ink | None = None, font: ... = None, anchor: str | None = None, spacing: float = 4, align: str = 'left', direction: str | None = None, features: list[str] | None = None, language: str | None = None, stroke_width: float = 0, stroke_fill: _Ink | None = None, embedded_color: bool = False, *, font_size: float | None = None) -> None` |
| 9 | multiline_textbbox | `(self, xy, text, font=None, anchor=None, spacing=4, align='left', direction=None, features=None, language=None, stroke_width=0, embedded_color=False, *, font_size=None) -> tuple[float,float,float,float]` |
| 10 | pieslice | `(self, xy: Coords, start: float, end: float, fill: _Ink | None = None, outline: _Ink | None = None, width: int = 1) -> None` |
| 11 | point | `(self, xy: Coords, fill: _Ink | None = None) -> None` |
| 12 | polygon | `(self, xy: Coords, fill: _Ink | None = None, outline: _Ink | None = None, width: int = 1) -> None` |
| 13 | rectangle | `(self, xy: Coords, fill: _Ink | None = None, outline: _Ink | None = None, width: int = 1) -> None` |
| 14 | regular_polygon | `(self, bounding_circle: ..., n_sides: int, rotation: float = 0, fill: ... = None, outline: ... = None, width: int = 1) -> None` |
| 15 | rounded_rectangle | `(self, xy: Coords, radius: float = 0, fill: ... = None, outline: ... = None, width: int = 1, *, corners: ... = None) -> None` |
| 16 | text | `(self, xy: tuple[float,float], text: AnyStr | ImageText.Text[AnyStr], fill: _Ink | None = None, font: ... = None, anchor: str | None = None, spacing: float = 4, align: str = 'left', direction: str | None = None, features: list[str] | None = None, language: str | None = None, stroke_width: float = 0, stroke_fill: _Ink | None = None, embedded_color: bool = False, *args, **kwargs) -> None` |
| 17 | textbbox | `(self, xy: tuple[float,float], text: AnyStr, font: ... = None, anchor: str | None = None, spacing: float = 4, align: str = 'left', direction: str | None = None, features: list[str] | None = None, language: str | None = None, stroke_width: float = 0, embedded_color: bool = False, *, font_size: float | None = None) -> tuple[float,float,float,float]` |
| 18 | textlength | `(self, text: AnyStr, font: ... = None, direction: str | None = None, features: list[str] | None = None, language: str | None = None, embedded_color: bool = False, *, font_size: float | None = None) -> float` |

**Total ImageDraw: 18 methods**

---

## MODULE: ImageFilter — 17 filter classes

| # | Class | Constructor |
|---|-------|-------------|
| 1 | BLUR | Built-in kernel |
| 2 | BoxBlur | `(radius: float | Sequence[float])` |
| 3 | CONTOUR | Built-in kernel |
| 4 | Color3DLUT | `(size: int | tuple[int,int,int], table: Sequence[float] | ..., channels: int = 3, target_mode: str | None = None, **kwargs: bool)` |
| 5 | DETAIL | Built-in kernel |
| 6 | EDGE_ENHANCE | Built-in kernel |
| 7 | EDGE_ENHANCE_MORE | Built-in kernel |
| 8 | EMBOSS | Built-in kernel |
| 9 | FIND_EDGES | Built-in kernel |
| 10 | GaussianBlur | `(radius: float | Sequence[float] = 2)` |
| 11 | Kernel | `(size: tuple[int,int], kernel: Sequence[float], scale: float | None = None, offset: float = 0)` |
| 12 | MaxFilter | `(size: int = 3)` |
| 13 | MedianFilter | `(size: int = 3)` |
| 14 | MinFilter | `(size: int = 3)` |
| 15 | ModeFilter | `(size: int = 3)` |
| 16 | RankFilter | `(size: int, rank: int)` |
| 17 | SHARPEN | Built-in kernel |
| 18 | SMOOTH | Built-in kernel |
| 19 | SMOOTH_MORE | Built-in kernel |
| 20 | UnsharpMask | `(radius: float = 2, percent: int = 150, threshold: int = 3)` |

**Total ImageFilter: 20 classes**

---

## MODULE: ImageEnhance — 4 enhancement classes

| # | Class | Constructor | Method |
|---|-------|-------------|--------|
| 1 | Brightness | `(image: Image)` | `enhance(self, factor: float) -> Image` |
| 2 | Color | `(image: Image)` | `enhance(self, factor: float) -> Image` |
| 3 | Contrast | `(image: Image)` | `enhance(self, factor: float) -> Image` |
| 4 | Sharpness | `(image: Image)` | `enhance(self, factor: float) -> Image` |

**Total ImageEnhance: 4 classes (+ 4 enhance methods = 8)**

---

## MODULE: ImageOps — 19 functions

| # | Function | Signature |
|---|----------|-----------|
| 1 | autocontrast | `(image: Image, cutoff: float | tuple[float,float] = 0, ignore: int | Sequence[int] | None = None, mask: Image | None = None, preserve_tone: bool = False) -> Image` |
| 2 | colorize | `(image: Image, black: str | tuple[int,...], white: str | tuple[int,...], mid: str | int | tuple[int,...] | None = None, blackpoint: int = 0, whitepoint: int = 255, midpoint: int = 127) -> Image` |
| 3 | contain | `(image: Image, size: tuple[int,int], method: int = BICUBIC) -> Image` |
| 4 | cover | `(image: Image, size: tuple[int,int], method: int = BICUBIC) -> Image` |
| 5 | crop | `(image: Image, border: int = 0) -> Image` |
| 6 | deform | `(image: Image, deformer: SupportsGetMesh, resample: int = BILINEAR) -> Image` |
| 7 | equalize | `(image: Image, mask: Image | None = None) -> Image` |
| 8 | exif_transpose | `(image: Image, *, in_place: bool = False) -> Image | None` |
| 9 | expand | `(image: Image, border: int | tuple[int,...] = 0, fill: str | int | tuple[int,...] = 0) -> Image` |
| 10 | fit | `(image: Image, size: tuple[int,int], method: int = BICUBIC, bleed: float = 0.0, centering: tuple[float,float] = (0.5,0.5)) -> Image` |
| 11 | flip | `(image: Image) -> Image` |
| 12 | grayscale | `(image: Image) -> Image` |
| 13 | invert | `(image: Image) -> Image` |
| 14 | mirror | `(image: Image) -> Image` |
| 15 | pad | `(image: Image, size: tuple[int,int], method: int = BICUBIC, color: str | int | tuple[int,...] | None = None, centering: tuple[float,float] = (0.5,0.5)) -> Image` |
| 16 | posterize | `(image: Image, bits: int) -> Image` |
| 17 | scale | `(image: Image, factor: float, resample: int = BICUBIC) -> Image` |
| 18 | solarize | `(image: Image, threshold: int = 128) -> Image` |

**Total ImageOps: 18 functions**

---

## MODULE: ImageChops — 20 functions

| # | Function | Signature |
|---|----------|-----------|
| 1 | add | `(image1: Image, image2: Image, scale: float = 1.0, offset: float = 0) -> Image` |
| 2 | add_modulo | `(image1: Image, image2: Image) -> Image` |
| 3 | blend | `(image1: Image, image2: Image, alpha: float) -> Image` |
| 4 | composite | `(image1: Image, image2: Image, mask: Image) -> Image` |
| 5 | constant | `(image: Image, value: int) -> Image` |
| 6 | darker | `(image1: Image, image2: Image) -> Image` |
| 7 | difference | `(image1: Image, image2: Image) -> Image` |
| 8 | duplicate | `(image: Image) -> Image` |
| 9 | hard_light | `(image1: Image, image2: Image) -> Image` |
| 10 | invert | `(image: Image) -> Image` |
| 11 | lighter | `(image1: Image, image2: Image) -> Image` |
| 12 | logical_and | `(image1: Image, image2: Image) -> Image` |
| 13 | logical_or | `(image1: Image, image2: Image) -> Image` |
| 14 | logical_xor | `(image1: Image, image2: Image) -> Image` |
| 15 | multiply | `(image1: Image, image2: Image) -> Image` |
| 16 | offset | `(image: Image, xoffset: int, yoffset: int | None = None) -> Image` |
| 17 | overlay | `(image1: Image, image2: Image) -> Image` |
| 18 | screen | `(image1: Image, image2: Image) -> Image` |
| 19 | soft_light | `(image1: Image, image2: Image) -> Image` |
| 20 | subtract | `(image1: Image, image2: Image, scale: float = 1.0, offset: float = 0) -> Image` |
| 21 | subtract_modulo | `(image1: Image, image2: Image) -> Image` |

**Total ImageChops: 21 functions**

---

## MODULE: ImageColor — 2 functions
| # | Function | Signature |
|---|----------|-----------|
| 1 | getcolor | `(color: str, mode: str) -> int | tuple[int,...]` |
| 2 | getrgb | `(color: str) -> tuple[int,int,int] | tuple[int,int,int,int]` |

---

## MODULE: ImageFont — 4 classes + 5 functions

### Classes
| # | Class | Key Methods |
|---|-------|-------------|
| 1 | FreeTypeFont | `font_variant()`, `getbbox()`, `getlength()`, `getmask()`, `getmask2()`, `getmetrics()`, `getname()`, `set_variation_by_axes()`, `set_variation_by_name()` |
| 2 | ImageFont | `getbbox()`, `getlength()`, `getmask()` |
| 3 | TransposedFont | `getbbox()`, `getlength()`, `getmask()` |
| 4 | Layout | Enum: BASIC, RAQM |

### Functions
| # | Function | Signature |
|---|----------|-----------|
| 1 | load | `(filename: str) -> ImageFont` |
| 2 | load_default | `(size: float | None = None) -> FreeTypeFont | ImageFont` |
| 3 | load_default_imagefont | `() -> ImageFont` |
| 4 | load_path | `(filename: str | bytes) -> ImageFont` |
| 5 | truetype | `(font: StrOrBytesPath | BinaryIO, size: float = 10, index: int = 0, encoding: str = '', layout_engine: Layout | None = None) -> FreeTypeFont` |

**Total ImageFont: 9 items**

---

## MODULE: ImagePalette — 1 class (6 methods)
| # | Method | Signature |
|---|--------|-----------|
| 1 | copy | `(self) -> ImagePalette` |
| 2 | getcolor | `(self, color: tuple[int,...], image: Image | None = None) -> int` |
| 3 | getdata | `(self) -> tuple[str, Sequence[int] | bytes | bytearray]` |
| 4 | save | `(self, fp: str | IO[str]) -> None` |
| 5 | tobytes | `(self) -> bytes` |
| 6 | tostring | `(self) -> bytes` |

---

## MODULE: ImageStat — 1 class (.Stat)
Properties: extrema, count, sum, sum2, mean, median, rms, var, stddev

---

## MODULE: ImageCms — 17 functions
Full color management with ICC profiles via LittleCMS2.

---

## MODULE: ImageMorph — 2 classes (MorphOp, LutBuilder)

---

## MODULE: ImageSequence — 1 class (Iterator)

---

## MODULE: ImageMath — eval() expression-based pixel math

---

## MODULE: ImageGrab — Screen capture (platform-specific, skip for WASM)

---

## MODULE: ImageTk — Tkinter integration (skip for WASM)

---

## MODULE: ImageWin — Windows GDI (skip)

---

## MODULE: ImageQt — Qt integration (skip)

---

## GRAND TOTAL: ~200 API items across 15 modules

Priority ranking for Phase 2:
1. Image classmethods: open, new (2) — DONE stubbed
2. Image save (1)
3. Image core transforms: resize, crop, rotate, transpose, convert (5)
4. Image compositing: paste, alpha_composite, blend, composite (4)
5. Image analysis: split, getbands, getbbox, getchannel, getpixel, getcolors, histogram, entropy (8)
6. Image I/O: tobytes, frombytes, load, seek, tell, close, verify (7)
7. Image filters: ImageFilter module (20 filter classes)
8. Image enhancement: ImageEnhance module (4 classes)
9. Image pixel ops: point, putalpha, putdata, putpixel, quantize (5)
10. Image transforms: transform (AFFINE, PERSPECTIVE, etc.), reduce (2)
11. Image drawing: ImageDraw module (18 methods) — low priority for WASM
12. ImageOps: (18 functions)
13. ImageChops: (21 functions)
14. ImageFont: (9 items) — needs FreeType, complex for WASM
15. ImageColor, ImagePalette, ImageStat: support modules
16. ImageCms: color management — complex, v2
17. ImageMath, ImageMorph, ImageSequence: niche
