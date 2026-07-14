# fontdone

Pure-Rust font rendering engine — a FreeType-compatible TrueType loader,
Latin auto-hinter, bytecode hinter, and smooth anti-aliased rasterizer.

**Runtime is 100% Rust: zero FreeType C FFI, zero unsafe.**

The crate publishes two API surfaces:

- A **FreeType FFI facade** (`fontdone::ffi`) that mirrors `FT_Init_FreeType`,
  `FT_New_Memory_Face`, `FT_Load_Glyph`, `FT_Get_Kerning`, etc. with exact
  parity against pinned FreeType C 2.14.3.
- A **compact Font helper** (`fontdone::Font`) for Pillow-style integration:
  `truetype()`, `getmask()`, `getbbox()`, `getname()`, `getlength()`.

Workspace crates:

| Crate | Purpose |
|-------|---------|
| `fontdone` | Pure-Rust core — font parsing, auto-hinter, rasterizer, FreeType FFI facade |
| `fontdone-ffi-c` | Thin C ABI wrapper: exposes `FT_*` symbols for native FFI parity testing |
| `fontdone-ffi-wasm` | Thin WASM ABI wrapper: exposes handle-based API for browser parity testing |

Project goal: exact FreeType C parity across every public endpoint — pixel
bytes, metrics, outline geometry, error codes, struct layouts, type sizes,
and constant values. C is an oracle for fixtures only. See `PROJECT_GOALS.md`.

## Install

```bash
cargo add fontdone
```

From source:

```bash
git clone https://github.com/appunni-m/fontdone
cd fontdone
make setup
make test
```

The crate uses the Rust 2024 edition. Minimum supported Rust version: 1.87.
The checked-in toolchain file pins Rust 1.96.1 for local development.

## Quick Start

### FreeType FFI facade

```rust
use fontdone::ffi::*;

fn main() -> Result<(), fontdone::FontError> {
    let data = std::fs::read("DejaVuSans.ttf")?;
    let library = FT_Init_FreeType();
    let mut face = FT_New_Memory_Face(&library, &data, 0, 20.0)?;

    // Load and render a glyph
    let slot = FT_Load_Glyph(&face, 36, FT_LOAD_RENDER | FT_LOAD_TARGET_NORMAL)?;
    let bitmap = slot.bitmap.as_ref().expect("rendered bitmap");
    println!("bitmap {}×{} pitch={}", bitmap.width, bitmap.rows, bitmap.pitch);

    // Kerning
    let (x, y) = FT_Get_Kerning(&face, 65, 66, FT_KERNING_DEFAULT);
    println!("kerning A→B: ({}, {})", x, y);

    Ok(())
}
```

### Compact Font API

```rust
use fontdone::{Font, FontError};

fn main() -> Result<(), FontError> {
    let font_data = std::fs::read("DejaVuSans.ttf")?;
    let font = Font::truetype(&font_data, 12.0)?;

    let (family, style) = font.getname();
    println!("Font: {} {}", family, style);

    let mask = font.getmask("A")?;
    println!("Mask {}×{} advance={}", mask.width, mask.height, mask.advance_width);

    let bbox = font.getbbox("A");
    println!("BBox: ({}, {}) → ({}, {})", bbox.0, bbox.1, bbox.2, bbox.3);

    Ok(())
}
```

## Features

- **TrueType outline loading**: Parses `glyf`/`loca` tables, composite glyphs
- **Auto-hinting**: Latin script auto-hinter (FreeType's `af_latin_*` engine)
  — grid-fits edges to pixel boundaries for crisp small-size text
- **Bytecode hinter**: TrueType instruction interpreter (`tt/hinter/exec.rs`)
  — executes CVT programs, IUP interpolation, and glyph programs
- **Smooth rasterizer**: FT_INT64 DDA path from `ftgrays.c` — 8-bit alpha output
- **Table parsing**: `cmap`, `head`, `hhea`, `hmtx`, `maxp`, `name`, `OS/2`, `post`,
  `vhea`, `vmtx`, `kern`, `hdmx`, `loca`
- **SFNT utilities**: `FT_Load_Sfnt_Table`, `FT_Sfnt_Table_Info`, `FT_Get_Sfnt_Table`,
  `FT_Get_Sfnt_Name`, `FT_Get_Sfnt_Name_Count`
- **Fixed-point math**: `FT_MulFix`, `FT_DivFix`, `FT_CeilFix`, `FT_FloorFix`,
  `FT_RoundFix`, `FT_MulDiv`
- **Trigonometric functions**: `FT_Sin`, `FT_Cos`, `FT_Tan`, `FT_Atan2`,
  `FT_Vector_Unit`, `FT_Vector_Length`, `FT_Vector_Rotate`
- **No runtime FreeType C dependency**: pinned C source and oracle scripts are
  offline fixture references only

## Parity Harness

The manifest-driven unified parity harness is the single source of truth for
correctness. Every FreeType public API endpoint is tested against a pinned
C FreeType 2.14.3 oracle.

### Architecture

```
manifest.yaml        → 543 operation definitions
       ↓
build_unified_fixture_inputs.py
       ↓
tests/fixtures/inputs/public-api/  → 1,543 JSON fixture input files
       ↓
gen_unified_oracle.c              → C FreeType oracle (djb2 hash, exact metric match)
       ↓
unified_fixture_parity.rs         → Rust FFI  ╮
                                     C ABI     ├ compare all 4 backends
                                     WASM ABI  ╯
```

### Running the parity gate

```bash
make test
```

This runs the full chain:

1. **`unified-oracle`** — Build pinned FreeType C 2.14.3 and the unified oracle binary
2. **`unified-inputs-check`** — Verify all fixture inputs are current
3. **`api-abi-check`** — Verify manifest coverage, C ABI / WASM ABI export surfaces
4. **`test-unified-fixtures`** — Run 4,097 fixture cases across all 4 backends
5. **`test-ffi`** — Enforce zero runtime FreeType C dependencies in core rendering code

### Current status

| Metric | Count |
|--------|-------|
| Fixture cases | 4,097 |
| Manifest subjects covered | 4,080 |
| Backends compared | 4 (Rust FFI, C ABI, WASM ABI, C oracle) |
| Pending (missing fixture fonts) | 16 |
| Pass | **4,097 (100%)** |

### Narrower targets

```bash
make test-parity         # Parity + test-ffi
make test-ffi            # No-runtime-FFI guard only
make test-ffi-compat     # API/ABI coverage gate only
make test-unified-fixtures-release # Parity in release mode
```

## FreeType FFI Facade

The `fontdone::ffi` module mirrors the FreeType 2.14.3 public C API surface.

```rust
use fontdone::ffi::*;

// Library management
let library = FT_Init_FreeType();
FT_Done_FreeType(Some(library));

// Face loading
let mut face = FT_New_Memory_Face(&library, font_bytes, 0, 20.0)?;
let flags = face.face_flags;

// Glyph loading and rendering
let slot = FT_Load_Glyph(&face, glyph_index, FT_LOAD_DEFAULT)?;
let slot = FT_Load_Char(&face, 'A' as u64, FT_LOAD_RENDER)?;
let slot = FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL)?;

// Metrics and geometry
let metrics = face.size_metrics;
let advance = FT_Get_Advance(&face, glyph_index, FT_LOAD_DEFAULT)?;
let cbox = FT_Outline_Get_CBox(&face, glyph_index, FT_LOAD_DEFAULT)?;

// Kerning, charmaps, SFNT tables
let (x, y) = FT_Get_Kerning(&face, left, right, FT_KERNING_DEFAULT);
let char_index = FT_Get_Char_Index(&face, codepoint);
let charmap = face.charmaps.first()
    .map(|record| (record as *const FT_CharMapRecPublic).cast_mut().cast())
    .unwrap_or(std::ptr::null_mut());
FT_Set_Charmap(Some(&mut face), charmap);
let table = FT_Load_Sfnt_Table(&face, 0x68656164, 0, Some(&mut len))?;
```

## Architecture

```
┌──────────┐    ┌───────────┐    ┌────────────┐    ┌───────────┐
│ tt/      │    │ scaler.rs │    │ autohint/  │    │ grays.rs  │
│ glyf.rs  │───→│           │───→│ latin.rs   │───→│           │
│ cmap.rs  │    │ pp1x shift│    │ loader.rs  │    │ DDA raster│
│ hmtx.rs  │    │ FU→26.6   │    │ types.rs   │    │ sweep     │
│ ...      │    │ bbox      │    │ cjk.rs     │    │           │
└──────────┘    └───────────┘    └────────────┘    └───────────┘
     ↑                                                  │
     │ font.rs            ┌──────────────────────┐      │
     │ Font::truetype()   │ ffi/                  │      │
     │ getmask/getbbox    │ handles.rs            │      │
     │ getname/getlength  │ convert.rs            │      │
     │                    │ constants.rs          │      │
     │                    │ types.rs              │      │
     │                    │ generated_constants.rs│      │
     │                    └──────────────────────┘      │
     │                         ↑                       ▼
     │                    ffi-c/  ffi-wasm/      8-bit alpha bitmap
     │                    C ABI   WASM ABI
     │                    wrapper  wrapper
```

Key modules:

| Module | Purpose |
|--------|---------|
| `tt/` | TrueType table parsers (glyf, cmap, hmtx, head, hhea, maxp, name, OS/2, post, vhea, vmtx, kern, hdmx, loca) |
| `tt/hinter/` | TrueType bytecode interpreter — CVT programs, glyph programs, IUP interpolation |
| `scaler` | Glyph scaling (FU → 26.6), pp1.x origin shift, cbox computation |
| `autohint` | Latin + CJK auto-hinter: reload, segments, edges, 4-phase snapping, IUP |
| `grays` | Smooth anti-aliased rasterizer (FT_INT64 DDA sweep) |
| `render` | Render-mode dispatch and bitmap assembly |
| `font` | Compact Font API: `truetype()`, `getmask()`, `getbbox()` |
| `ffi/` | FreeType FFI facade — `FT_*` functions, types, constants, error codes |
| `ffi-c/` | Thin C ABI wrapper crate (`fontdone-ffi-c`) |
| `ffi-wasm/` | Thin WASM ABI wrapper crate (`fontdone-ffi-wasm`) |

## Testing

```bash
make test        # Parity harness (4,097 cases) + api-abi check + no-FFI guard
make fmt         # Check rustfmt
make clippy      # Run clippy with warnings denied
make lint        # fmt + clippy
make ci          # Full CI sequence: setup + fmt + clippy + doc + test + bench-self-test
```

## Benchmarking

```bash
make bench           # Full Rust vs C FreeType comparison report
make bench-quick     # 2-sample smoke comparison
make bench-self-test # Summarizer self-test
```

Reports are written to `target/fontdone-bench/latest.json` and
`target/fontdone-bench/latest.md`. See `doc/PERFORMANCE_BENCHMARKING.md` for
trust labels, timing boundaries, machine metadata, and review rules.

## Fixtures

Fixture inputs are generated from the manifest by
`scripts/build_unified_fixture_inputs.py`. The C oracle is compiled from
`scripts/gen_unified_oracle.c` against pinned FreeType 2.14.3 C source
(fetched to `freetype/` by `make oracle-fetch`).

Oracle cache and outputs live under `tests/fixtures/outputs/unified_oracle_cache/`
and are version-controlled with a `.gitignore`; they are regenerated on test
runs when stale or missing.

To regenerate everything:

```bash
make clean
make setup
make test
```

## Making Changes

- If you add a new public API endpoint: add it to `manifest.yaml`, regenerate
  fixture inputs, add a handler in `oracle_args()` and `run_rust_ffi()` in
  `tests/unified_fixture_parity.rs`, and verify with `make test`.
- If you change core rendering code (`src/font.rs`, `src/scaler.rs`,
  `src/render.rs`, `src/autohint/`, `src/grays.rs`): run the full parity
  harness with `make test`. Do not weaken fixtures, thresholds, or expected
  outputs.
- If you change the FreeType FFI facade (`src/ffi/handles.rs`,
  `src/ffi/constants.rs`): verify both `make test` and `make test-ffi-compat`.
- Generated constants in `src/ffi/generated_constants.rs` are produced by
  `make public-constants`. Do not edit them by hand.
- See `PROJECT_GOALS.md`, `CONTRIBUTING.md`, and `doc/GENERATOR_SYSTEM.md`.

## Contributing

Start with `CONTRIBUTING.md` and `PROJECT_GOALS.md`. The short version:

- Keep runtime pure Rust — zero FreeType C FFI
- Keep C FreeType as offline oracle tooling only
- Run the full parity harness before claiming correctness
- Do not weaken fixtures, thresholds, or tests
- Document fixture and benchmark changes as project infrastructure

## License

FreeType License (`FTL`) — see `LICENSE` and `FTL.TXT`.

The pinned FreeType C source is fetched into ignored `freetype/` by
`make oracle-fetch` as an offline oracle for fixture generation and diagnosis.
Runtime code is pure Rust and does not link to FreeType C.
