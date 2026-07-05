# C API Compatibility Plan

`fontdone` is not trying to be a Rust-flavored font library that happens to
look like FreeType. The target is version-pinned C FreeType compatibility,
implemented in Rust.

Servo `rust-freetype` is useful only as a binding reference. It shows how a
Rust crate can expose C FreeType through FFI, but it is not the design target.
The design target is the FreeType C public headers and the behavior of the
pinned C oracle.

## Compatibility Definition

Full replacement means all of these are true for every in-scope FreeType
endpoint:

1. **Function interface parity**: every exported `FT_*`, `FTC_*`, `TT_*`, or
   related public C function is classified as implemented, planned, or
   intentionally excluded with a durable reason.
2. **Import/link parity**: a future C ABI crate exports symbols with the same C
   names and call shapes expected by existing FreeType C users.
3. **Usage interface parity**: client code follows the same lifecycle:
   `FT_Init_FreeType`, face creation, size selection, glyph load/render,
   metadata access, and teardown.
4. **Record parity**: ABI-facing records use `#[repr(C)]` and preserve C field
   order, field names in generated headers, numeric type widths, pointer
   ownership rules, and units.
5. **Constant parity**: public constants and enum values preserve the C numeric
   values exactly.
6. **Output parity**: rendered bytes, bitmap metadata, metrics, bbox/cbox,
   outline geometry, table bytes, and public errors match C FreeType exactly.

Idiomatic Rust wrappers are allowed as public APIs, but they are not the
FreeType-shaped compatibility surface. They count toward parity only when a
matching `fontdone::ffi` endpoint maps to them and the harness compares that
endpoint against the C oracle.

## Layers

The project should have four deliberate layers:

| Layer | Purpose | Public Shape |
|---|---|---|
| Pure Rust core | Implementation of parsing, hinting, metrics, rasterization | Internal Rust modules, mostly `pub(crate)` |
| Idiomatic public Rust API | Ergonomic Rust access to FreeType semantics | `Library`, `Face`, `GlyphSlot`, `LoadFlags`, `RenderedBitmap` |
| `fontdone::ffi` compatibility API | Public, non-idiomatic, 1:1 FreeType-shaped Rust API that wraps the idiomatic/core layer | `FT_*`-shaped functions, C numeric constants, C-shaped records exposed as Rust items |
| Future exported C ABI | Drop-in surface for C FreeType users | exported `FT_*` symbols, `#[repr(C)]` records, exact constants |

The flow is:

```text
pure Rust core -> idiomatic public Rust API -> fontdone::ffi compatibility API -> future exported C ABI
```

`fontdone::ffi` is not a native FreeType binding. It is the public Rust
compatibility surface that intentionally keeps FreeType's non-idiomatic names,
types, constants, ownership shape, and lifecycle so each C endpoint has a
matching Rust endpoint before C symbols are exported. The future C ABI layer may
contain `extern "C"` exports because that is its purpose. It must still not
call C FreeType or link to FreeType. The current runtime core must remain pure
Rust.

## What We Are Doing Extra Today

These are useful for development but should not be treated as stable FreeType
replacement API:

- Public internal modules: `autohint`, `grays`, `scaler`, `tables`, and `tt`.
  FreeType does not expose our internal pipeline this way. These should become
  `pub(crate)` or be hidden behind deliberate C/Rust API functions.
- Pillow-style helpers: `getmask`, `getbbox`, `getlength`, `getmetrics`, and
  `GlyphMask`. They are useful adapters, but they are not FreeType C API
  equivalents. Keep them behind an adapter feature or mark them as convenience
  APIs, not compatibility proof.
- Extra Rust-only fields on C-shaped records, such as DPI/request metadata in
  `SizeMetrics` or placement fields inside `RenderedBitmap`. They are useful,
  but the future ABI records must have exact C-compatible layouts.
- Broad `pub` table/parser structs that expose implementation details before
  the C-compatible surface is complete.
- Treating a semantic match as ABI match. For example, `GlyphSlotMetrics`
  semantically maps `FT_Glyph_Metrics`, but the field names/order are not C ABI
  exact until represented by a `#[repr(C)]` ABI record.

## What Must Be Added

1. Generate and maintain an exact C header compatibility inventory from pinned
   FreeType headers.
2. For every C function, record:
   - C return type
   - C params and ownership rules
   - Rust core implementation path
   - safe Rust wrapper path, if any
   - `fontdone::ffi` compatibility endpoint path
   - future C ABI symbol path
   - fixture or scalar test proving output parity
3. Add exact numeric constant checks for load flags, render modes, pixel modes,
   face flags, style flags, encodings, kerning modes, bbox modes, and error
   codes.
4. Add `#[repr(C)]` ABI records for the future C layer:
   - `FT_Vector`
   - `FT_BBox`
   - `FT_Bitmap`
   - `FT_Glyph_Metrics`
   - `FT_Size_Metrics`
   - `FT_GlyphSlotRec`
   - `FT_FaceRec`
   - `FT_CharMapRec`
   - `FT_Outline`
5. Add C ABI compile/link tests that build small C programs against the
   generated Rust-backed headers/library and compare output with C FreeType.
6. Add migration examples that compile unchanged or minimally changed FreeType
   C usage against the Rust implementation.

## Current Audit Command

Run:

```bash
make api-abi-audit
```

Outputs:

- `target/api-abi-audit/api_abi_audit.md`
- `target/api-abi-audit/api_abi_audit.json`

This audit is not a parity gate yet. It is the map of what remains before the
C ABI replacement can be claimed.

## Promotion Rule

An endpoint is not complete for C replacement until it has:

1. C interface mapping.
2. Safe Rust or internal implementation.
3. `fontdone::ffi` compatibility endpoint.
4. Future C ABI mapping when applicable.
5. Exact output fixture/scalar/error parity through `fontdone::ffi`.
6. Constant/record exactness where the endpoint exposes public C data.

If any of those are missing, the endpoint is partial or planned, not complete.
