# Rendering Backends — Reference Architecture

## The three implementations

| Implementation | FreeType version | Font hinting | How to invoke |
|---|---|---|---|
| **PIL ImageFont** | 2.14.3 (bundled) | Bytecode or autohint | `ImageFont.truetype(path, size).getmask(c)` |
| **System FreeType** | 2.13.2 (apt) | Bytecode or autohint | `freetype.Face(path).load_char(c, 0x4)` |
| **Our PureRust** | 2.14.1 (port) | Autohint only | `Font::truetype(data, size, PureRust).getmask(c)` |

## Why pixels differ across implementations

**Different FreeType versions produce different autohinter output.**  
Even with the same font file and same `FT_LOAD_RENDER` flags, FreeType 2.13.2,
2.14.1, and 2.14.3 rasterize slightly different coverage values for the same
glyph. The autohinter's edge-placement algorithm changed between versions.

**Bytecode vs autohint.**
Fonts with TrueType bytecode (`fonts/`) render via the bytecode interpreter.
Fonts with bytecode stripped (`fonts_autohint/`) fall back to the autohinter.
These produce different raster outputs even with the same FreeType version.

## What PIL actually does

PIL's `_imagingft.c` is a thin wrapper — it does **not** override FreeType's
rasterizer or autohinter:

1. Font load: `FT_New_Memory_Face` + `FT_Request_Size(NOMINAL, size*64, size*64, 0, 0)`
2. Glyph layout: `FT_Load_Glyph(index, FT_LOAD_DEFAULT)` — uses FreeType's native hinting
3. Bbox: `FT_Get_Glyph` → `FT_Glyph_Get_CBox(FT_GLYPH_BBOX_PIXELS)` — FreeType's own CBox
4. Render: `FT_Load_Glyph(index, FT_LOAD_DEFAULT | FT_LOAD_RENDER)` — FreeType's rasterizer
5. Mask assembly: copies FT bitmap pixels into a Python Image with padding

PIL's mask **pixels are FreeType's pixels**. The mask **dimensions** are PIL-specific
(ascender/descender-based sizing with padding), but the ink coverage values are
direct from FreeType's rasterizer.

## What the generator does

`scripts/generate_font_refs.py` uses `freetype-py` (system FreeType 2.13.2):
- Loads fonts from `tests/fixtures/input/fonts/` (bytecode intact)
- Calls `face.set_char_size(size << 6)` — differs from PIL's `FT_Request_Size`
- Calls `face.load_char(ch, 0x4)` — `FT_LOAD_RENDER` (no `FT_LOAD_DEFAULT` flag)
- Reads `glyph.bitmap.buffer` directly — the raw FreeType bitmap

## What the test does

`tests/coverage_matrix_tests.rs`:
- Loads fonts from `tests/fixtures/input/fonts_autohint/` (bytecode stripped)
- Calls `Font::truetype(&data, size)` — our PureRust port of FreeType 2.14.1 autohinter
- Compares output against `coverage_matrix.json` references

## The mismatch

| Component | Font source | FreeType | Hinting | Expected match |
|---|---|---|---|---|
| Generator | `fonts/` | system 2.13.2 | bytecode | — |
| Test input | `fonts_autohint/` | our port 2.14.1 | autohint | `coverage_matrix.json` |

**Three differences**: font file (bytecode vs nohint), FreeType version (2.13.2 vs 2.14.1),
and hinting mode (bytecode vs autohint). The 740 failures are entirely expected.

## Fix plan

### Step 1: Fix generator alignment
Change `generate_font_refs.py` INPUT_FONTS from `fonts/` to `fonts_autohint/`.
This makes generator and test use the same font files. Regenerate matrix.

### Step 2: Match FreeType version
The generator uses system FreeType 2.13.2 but our port targets 2.14.1.
Options:
- **A)** Install FreeType 2.14.1 on the system and regenerate
- **B)** Accept the version delta — focus on structural correctness, not pixel-exact SHAs
- **C)** Use PIL's bundled FreeType 2.14.3 via PIL itself to generate references

### Step 3: Add BitmapBackend dispatch
When `BitmapBackend::FreeType` is selected, route rendering to use the same algorithm
as the reference generator (raw FreeType bitmap output, not PIL-padded masks).
Each backend gets its own reference set and its own test pass.

### Step 4: Generate separate PIL reference set
For `BitmapBackend::PIL`, generate references using PIL's `getmask()`/`getbbox()` directly.
These will have PIL-specific mask sizing (ascender/descender padding) and PIL's
FreeType 2.14.3 rasterizer output.

## End state

```
BitmapBackend::PureRust → test against references from our autohinter port
BitmapBackend::FreeType → test against references from system FreeType 2.14.1
BitmapBackend::PIL      → test against references from PIL's bundled FreeType
```

## TODO: Upgrade vendored FreeType to 2.14.3

pillow-rs-freetype/freetype/ contains FreeType 2.14.1. PIL 12.2.0 bundles 2.14.3.
The autofit directory changed between these versions — every file in `src/autofit/`
differs. This is the primary reason our autohinter output doesn't match PIL's.

**Step 1:** Replace vendored FreeType C source with VER-2-14-3 tag
**Step 2:** Re-port the autohinter changes from updated `src/autofit/aflatin.c`, etc.
**Step 3:** Regenerate references via PIL's `getmask()`/`getbbox()` (uses PIL's
bundled 2.14.3) rather than system freetype-py (2.13.2)
**Step 4:** Run the coverage matrix — should approach 100% for the matching backend
