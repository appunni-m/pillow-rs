# Parity Route Batch Plan

Current objective: exact same-input parity with pinned C FreeType for Rust FFI,
thin C ABI, and WASM ABI. Do not count coverage-only tests, generic fallback,
fixture substitutions, or green placeholders as parity.

Current verified route-audit baseline:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 442, 'real-null-validation': 9, 'real-parity': 4518}
```

Verification command:

```bash
make -C pillow-rs-freetype route-audit
```

## Turn triage: 2026-07-21

Rows checked before selecting the next implementation batch:

| Surface | Focus command | Result | Decision |
|---|---|---:|---|
| `freetype.FT_Open_Args.open_face_consumes_args_like_c` | `make -C pillow-rs-freetype test-case CASE=freetype.FT_Open_Args.open_face_consumes_args_like_c` | `runnable=0 pending=1` | Do not promote. The row still declares abstract `arg_variants`; the maintained route must consume explicit same-input memory/pathname/stream/driver/params rows before it can count. |
| `ftcid.get_cid_from_glyph_index` | `make -C pillow-rs-freetype test-op OP=ftcid.get_cid_from_glyph_index` | `passed=1`, `pending=7` | Do not promote the semantic CID rows. They lack maintained runtime-resolved CID inputs. |
| `ftotval.open_type_validate` | `make -C pillow-rs-freetype test-op OP=ftotval.open_type_validate` | `passed=3`, `pending=17` | Do not generate placeholder OTFs. Core still returns `FT_Err_Unimplemented_Feature`; table-success/malformed rows need real validator behavior or corrected pinned-build contract. |
| malformed `new_memory_face` error rows | focused `test-case` commands for post/name-table malformed rows | `runnable=0 pending=1` for each checked row | Do not promote. Pinned C opens or returns a different public error for the generated fixtures. |
| `freetype.new_face` | `make -C pillow-rs-freetype test-op OP=freetype.new_face` | `passed=6 pending=0` | Already real for current maintained route; not the next gap. |
| `ftbzip2.*` | `make -C pillow-rs-freetype test-op OP=ftbzip2.stream_open_bzip2`, `stream_read`, `stream_close` | open errors `passed=2`, success/read/close pending | Error rows are already real. Success/read/close need a pure-Rust bzip2 stream implementation; do not fake with fixture-byte comparisons. |

## Next 10+ row batches

These are the viable high-count batches. Each must be attacked as an actual
implementation surface, not as route-audit classification only.

### Batch A: FTC cache manager and cache lookup routes

Current pending rows: at least 70 across `ftcache.*`.

Primary operations:

- `ftcache.image_cache_lookup_scaler`
- `ftcache.cmap_cache_lookup`
- `ftcache.manager_lookup_size`
- `ftcache.manager_remove_face_id`
- `ftcache.sbit_cache_lookup_scaler`
- `ftcache.manager_done`
- `ftcache.manager_lookup_face`
- `ftcache.cmap_cache_new`
- `ftcache.image_cache_new`
- `ftcache.node_unref`

Required implementation:

1. Add pure-Rust cache manager state keyed by public `FTC_FaceID` pointer
   identity, not pointed-to bytes or fallback strings.
2. Implement requester invocation accounting, face/size ownership, cache node
   ownership, node reference counts, reset, remove-face-id, and manager-done
   effects.
3. Add pinned C oracle routes that run identical request sequences and compare
   public outputs: glyph index, glyph/bitmap presence, cache handle nullness,
   requester count, node nullness, repeat-hit behavior, reset/remove effects,
   and load-flag truncation.
4. Keep C ABI and WASM ABI thin: only handle validation, record copying, and
   public handle lifetime bookkeeping.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=ftcache.cmap_cache_lookup
make -C pillow-rs-freetype test-op OP=ftcache.image_cache_lookup_scaler
make -C pillow-rs-freetype test-op OP=ftcache.manager_lookup_face
make -C pillow-rs-freetype route-audit
```

### Batch B: COLR/CPAL paint graph traversal

Current pending rows: at least 60 across `ftcolor.*`.

Primary operations:

- `ftcolor.get_paint_graph`
- `ftcolor.traverse_paint_graph`
- `ftcolor.get_paint`
- `ftcolor.get_color_glyph_paint_and_get_paint`
- `ftcolor.get_color_glyph_paint_then_get_paint`
- `ftcolor.get_gradient_paint_and_stops`
- `ftcolor.get_colorline_stops`
- `ftcolor.get_normalized_transform_paint`

Required implementation:

1. Implement COLR v1 paint table graph resolution in core, including paint
   offsets, layer iterators, transforms, composites, gradients, color lines, and
   foreground palette index behavior.
2. Preserve pinned C failure policy for null output, missing COLR, invalid root
   paint, malformed offsets, and unsupported paint formats.
3. Add same-input oracle routes that compare exact `FT_Bool`/`FT_Error`, paint
   format, iterator state, color stops, transform matrices, palette indices,
   and traversal order.
4. Keep fixture requirements honest: if the current color font asset is absent
   or not C-openable, generate a maintained compact fixture before promotion.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint_graph
make -C pillow-rs-freetype test-op OP=ftcolor.traverse_paint_graph
make -C pillow-rs-freetype test-op OP=ftcolor.get_gradient_paint_and_stops
make -C pillow-rs-freetype route-audit
```

### Batch C: Stroker path construction, border export, and geometry

Current pending rows: at least 30 across `ftstroke.*`.

Primary operations:

- `ftstroke.open_path_geometry`
- `ftstroke.join_geometry`
- `ftstroke.parse_outline`
- `ftstroke.export_border`
- `ftstroke.get_counts`
- `ftstroke.get_border_counts`
- `ftstroke.line_to`
- `ftstroke.conic_to`
- `ftstroke.cubic_to`

Required implementation:

1. Implement the pure-Rust stroker state machine from FreeType's stroker code:
   subpath open/close state, line caps, joins, conic/cubic flattening or exact
   curve handling, inside/outside borders, and export count semantics.
2. Compare exact public geometry in 26.6 units: point count, contour count,
   tags, contours, border selection, exported outline orientation, and error
   codes.
3. Add oracle routes for path row sequences rather than one-off expected JSON.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=ftstroke.open_path_geometry
make -C pillow-rs-freetype test-op OP=ftstroke.export_border
make -C pillow-rs-freetype test-op OP=ftstroke.get_counts
make -C pillow-rs-freetype route-audit
```

### Batch D: GX/OpenType validators

Current pending rows: at least 40 across `ftgxval.*` and `ftotval.*`.

Do not start by generating placeholder fonts. The current `ftotval` focused
test proves only three exact rows and shows 17 pending rows. The missing OTF
fixtures are not sufficient because core `FT_OpenType_Validate` still returns
`FT_Err_Unimplemented_Feature`.

Required implementation:

1. Decide and document whether the pinned build contract includes active
   `otvalid`/`gxvalid` services.
2. If active, implement table validation and validate/free buffer ownership in
   core, then expose it through thin C/WASM handles.
3. If inactive in the pinned build, update the manifest/input expectation only
   through the maintained review flow; do not keep success rows that no pinned
   C build can execute.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=ftotval.open_type_validate
make -C pillow-rs-freetype test-op OP=ftgxval.truetype_gx_validate
make -C pillow-rs-freetype test-op OP=ftgxval.classic_kern_validate
make -C pillow-rs-freetype route-audit
```

### Batch E: Bzip2 stream route

Current pending rows: 4 in `ftbzip2.FT_Stream_OpenBzip2`.

This is below the 10-row preference, but it is a self-contained public stream
surface and should be handled independently if cache/color/stroker work is too
large for a turn.

Required implementation:

1. Add a pure-Rust bzip2 stream decoder route; do not call native bzip2 or shell
   commands at runtime.
2. Compare target stream fields, source stream position, decompressed byte
   ranges, read status, close lifecycle, and build-policy behavior against
   pinned C `freetype/src/bzip2/ftbzip2.c`.
3. Keep current exact-error rows real.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=ftbzip2.stream_open_bzip2
make -C pillow-rs-freetype test-op OP=ftbzip2.stream_read
make -C pillow-rs-freetype test-op OP=ftbzip2.stream_close
make -C pillow-rs-freetype route-audit
```

## Non-batch blockers to keep visible

- `freetype.FT_Open_Args.open_face_consumes_args_like_c`: requires explicit
  maintained `variants[]` rows and matching Rust/C/WASM route support for
  memory, pathname, stream, driver, params, no-source, and multiple-source
  behavior.
- `freetype.FT_FaceRec.populated_public_fields_match_c`: must be split into
  concrete C-openable operation stages before it can prove public face-record
  parity.
- CID semantic rows: need real CID-keyed same-input assets, not fallback
  non-CID error rows.
