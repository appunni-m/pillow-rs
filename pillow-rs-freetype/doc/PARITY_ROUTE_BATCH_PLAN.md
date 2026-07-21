# Parity Route Batch Plan

Current objective: exact same-input parity with pinned C FreeType for Rust FFI,
thin C ABI, and WASM ABI. Do not count coverage-only tests, generic fallback,
fixture substitutions, or green placeholders as parity.

Historical route-audit baseline before the FTC route batches:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 392, 'real-null-validation': 9, 'real-parity': 4568}
```

Current post-merge baseline on `main`:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 355, 'real-null-validation': 9, 'real-parity': 4605}
runtime_parity: passed=6876 failed=0 total=6876 covered_manifest_cases=3783
runtime_cases: runnable=6876 pending=359
```

This baseline means the maintained same-input routes are green; it does not mean
full public API parity is complete. The 355 route-pending rows remain outside
the real same-input Rust FFI / C ABI / WASM ABI comparison set.

After the FTC manager/cache creation route batch:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 382, 'real-null-validation': 9, 'real-parity': 4578}
```

After the FTC manager eviction/teardown route batch:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 371, 'real-null-validation': 9, 'real-parity': 4589}
```

After the direct FTC image-cache lookup route batch:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 367, 'real-null-validation': 9, 'real-parity': 4593}
```

After the FTC SBit cache creation route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 366, 'real-null-validation': 9, 'real-parity': 4594}
```

After the FTC cache type-contract route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 363, 'real-null-validation': 9, 'real-parity': 4597}
```

After the FTC FaceID pointer-identity route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 362, 'real-null-validation': 9, 'real-parity': 4598}
```

After the FTC Scaler descriptor-lifetime route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 361, 'real-null-validation': 9, 'real-parity': 4599}
```

After the FTC ImageType descriptor-lifetime route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 360, 'real-null-validation': 9, 'real-parity': 4600}
```

After the FTC ImageTypeRec image/sbit lookup route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 359, 'real-null-validation': 9, 'real-parity': 4601}
```

After the FTC CMap cache registration-limit route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 358, 'real-null-validation': 9, 'real-parity': 4602}
```

After the FTC node lifecycle route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 355, 'real-null-validation': 9, 'real-parity': 4605}
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
| `ftcache.FTC_SBitCache_LookupScaler` | `make -C pillow-rs-freetype test-op OP=ftcache.sbit_cache_lookup_scaler` | `passed=7 pending=0` | Completed. Route compares scaler size selection and FT_ULong→FT_Int32 load-flag truncation against pinned C through Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_CMapCache_Lookup` | `make -C pillow-rs-freetype test-op OP=ftcache.cmap_cache_lookup` | `passed=18 pending=0` | Completed. Route compares pinned C FTC lookup output for glyph index, repeat lookup, requester count, negative cmap-index behavior, RemoveFaceID, and Manager_Reset against Rust FFI, C ABI, and WASM ABI observable behavior. |
| `ftcache.FTC_ImageCache_LookupScaler` | `make -C pillow-rs-freetype test-op OP=ftcache.image_cache_lookup_scaler` | `passed=24 pending=0` | Completed. Route compares actual pinned C `FTC_ImageCache_LookupScaler` output for scaler size selection, glyph hit/miss, FT_ULong→FT_Int32 load-flag truncation, public glyph records, and node-unref classification against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_Manager_LookupSize` + `FTC_ScalerRec` | `make -C pillow-rs-freetype test-op OP=ftcache.manager_lookup_size` | `passed=8 pending=0` | Completed. Route compares actual pinned C `FTC_Manager_LookupSize` output for scaler metrics, requester-count behavior, and immediate repeat identity classification against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_Manager_LookupFace` | `make -C pillow-rs-freetype test-op OP=ftcache.manager_lookup_face` | `passed=7 pending=0` | Completed. Route compares actual pinned C `FTC_Manager_LookupFace` output for requester count, cached versus reloaded face identity class, RemoveFaceID behavior, public face fields, and C's no-current-size result against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_Manager_New` | `make -C pillow-rs-freetype test-op OP=ftcache.manager_new` | `passed=6 pending=0` | Completed. Route compares actual pinned C `FTC_Manager_New` zero/custom limit creation, requester `req_data`, lookup requester counts, reset, and manager-done lifecycle against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_CMapCache_New` | `make -C pillow-rs-freetype test-op OP=ftcache.cmap_cache_new` | `passed=5 pending=0` | Completed. Route compares actual pinned C CMap cache creation, manager-owned destruction, lookup usability, reset-preserved cache handle, and repeated registration-limit behavior against Rust FFI, C ABI, and WASM ABI. The registration-limit case proves `FTC_Manager_RegisterCache` accepts 16 cache registrations, rejects the 17th with `FT_Err_Too_Many_Caches`, leaves the failed output null, and preserves prior cache lookup usability. |
| `ftcache.FTC_ImageCache_New` | `make -C pillow-rs-freetype test-op OP=ftcache.image_cache_new` | `passed=5 pending=0` | Completed. Route compares actual pinned C Image cache creation, glyph lookup, node-unref classification, manager-owned destruction, and reset-preserved cache handle against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_Manager_RemoveFaceID` | `make -C pillow-rs-freetype test-op OP=ftcache.manager_remove_face_id` | `passed=6 pending=0` | Completed. Route compares actual pinned C face-id eviction, distinct face-id isolation, referenced-node unref sequencing, unknown/null face-id no-ops, and null-manager no-op behavior against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_Manager_Done` | `make -C pillow-rs-freetype test-op OP=ftcache.manager_done` | `passed=5 pending=0` | Completed. Route compares actual pinned C null-manager no-op, empty-manager teardown, populated cache/face/size/node release-before-done lifecycle, and void-return ownership behavior against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_ImageCache_Lookup` | `make -C pillow-rs-freetype test-op OP=ftcache.image_cache_lookup` | `passed=30 pending=0` | Completed. Route compares actual pinned C direct `FTC_ImageCache_Lookup` image-type sizing, glyph output, repeat lookup, requester count, null/non-null anode ownership classification, and node-unref behavior against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_SBitCache_New` | `make -C pillow-rs-freetype test-op OP=ftcache.sbit_cache_new` | `passed=3 pending=0` | Completed. Route compares pinned C successful `FTC_SBitCache_New` non-null cache handle creation and manager-owned lifecycle against Rust FFI, C ABI, and WASM ABI; existing null/invalid argument rows remain exact error parity. |
| `ftcache.FTC_CMapCache` / `FTC_ImageCache` / `FTC_SBitCache` type contracts | `make -C pillow-rs-freetype test-op OP=ftcache.type_contract` | `passed=3 pending=0` | Completed. Route compares pinned C live-manager cache constructor output for public opaque handle nullness, manager ownership identity class, and manager-done lifecycle class against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_FaceID` pointer identity | `make -C pillow-rs-freetype test-op OP=ftcache.face_id_identity` | `passed=1 pending=0` | Completed. Route compares pinned C raw `FTC_FaceID` pointer identity behavior: same font bytes under a distinct request object are a distinct cache key, while reusing the exact same face-id address is a cache hit; Rust FFI, C ABI, and WASM ABI validate the same public identity-class output. |
| `ftcache.FTC_Scaler` pointer lifetime | `make -C pillow-rs-freetype test-op OP=ftcache.scaler_descriptor_lifetime` | `passed=1 pending=0` | Completed. Route calls actual pinned C `FTC_SBitCache_LookupScaler` with a caller-owned `FTC_ScalerRec`, snapshots public `FTC_SBit` fields, mutates the caller scaler after lookup, and compares unchanged result fields against Rust FFI, C ABI, and WASM ABI. The route preserves the FTC record distinction that `FTC_SBit.max_grays` is the maximum gray value (`255`) while rendered `FT_Bitmap.num_grays` is the gray-level count (`256`). |
| `ftcache.FTC_ImageType` pointer lifetime | `make -C pillow-rs-freetype test-op OP=ftcache.image_type_descriptor_lifetime` | `passed=1 pending=0` | Completed. Route calls actual pinned C `FTC_ImageCache_Lookup` with a caller-owned `FTC_ImageTypeRec`, snapshots returned `FT_Glyph` public fields, mutates the caller descriptor after lookup, and compares unchanged existing node/glyph observations against Rust FFI, C ABI, and WASM ABI. |
| `ftcache.FTC_ImageTypeRec` image/sbit lookup fields | `make -C pillow-rs-freetype test-op OP=ftcache.image_type_lookup_probe` | `passed=1 pending=0` | Completed. Route compares actual pinned C `FTC_ImageCache_Lookup` and `FTC_SBitCache_Lookup` driven by the same `FTC_ImageTypeRec` face-id, width, height, flags, and glyph index against Rust FFI, C ABI, and WASM ABI. The sbit side models cache materialization by loading without the descriptor's `FT_LOAD_RENDER` bit and rendering once to the `FTC_SBit` public field shape. |
| `ftcache.FTC_Node` / `FTC_Node_Unref` lifecycle | `make -C pillow-rs-freetype test-op OP=ftcache.node_lifecycle`, `make -C pillow-rs-freetype test-op OP=ftcache.node_unref` | `node_lifecycle passed=1 pending=0`; `node_unref passed=2 pending=1` | Completed for lookup-acquired nodes. Route compares actual pinned C `FTC_SBitCache_Lookup` with non-null `anode`, public `FTC_SBitRec` fields, node cache index, refcount before/after one `FTC_Node_Unref`, pressure lookup statuses, and post-unref survival class against Rust FFI, C ABI, and WASM ABI. `FTC_Node_Unref.null_or_invalid_inputs_noop` remains pending because the fixture includes a foreign/bad-cache-index node that requires a maintained safe layout facade instead of a generic no-op. |

## Post-merge triage: 2026-07-21

Rows checked after `main` moved to `293f1c151`:

| Surface | Evidence | Decision |
|---|---|---|
| `ftcolor.FT_Get_Color_Glyph_Layer.{layer_iteration_success,foreground_color_index,terminal_false_preserves_last_outputs}` | Declared assets `fonts/color/colr-v0-layers-cpal.ttf` and `fonts/color/colr-v0-foreground-layer-ffff.ttf` are absent from `tests/fixtures/input/fonts/color/`; only the CPAL palette fixtures are present. | Do not promote. Real parity requires maintained COLR v0 fonts plus a layer-iterator route that compares successive glyph indexes, color indexes, terminal false behavior, and iterator fields through pinned C, Rust FFI, C ABI, and WASM ABI. |
| `ftcid.FT_Get_CID_From_Glyph_Index.*` and `ftcid.FT_Get_CID_Is_Internally_CID_Keyed.*` success/null-output semantic rows | Declared assets `fonts/cid/type1-cid-ros-and-glyph-map.pfb` and `fonts/cid/ot-cff-cid-keyed.otf` are absent. Existing non-CID/null-face error rows are already real parity. | Do not promote. Need real CID-keyed and SFNT-wrapped CID fixtures accepted by pinned C before the service-output rows can count. |
| `ftstroke.*` non-null geometry/lifecycle rows | Core currently routes exact null-stroker no-ops only; `FT_Stroker` is still an opaque pointer alias for the null routes, and no non-null stroker state machine exists. | Do not classify geometry/count/lifecycle rows as real. Batch C must implement the FreeType stroker state machine and compare exported 26.6 outline geometry. |
| `ftcolor.FT_Palette_*.*color_layers_disabled` | The active pinned build has color-layer support enabled; current CPAL success/error routes already compare real current-build behavior. | Keep pending. A disabled-build row needs a maintained alternate pinned build or explicit build-configuration oracle route. Counting current enabled-build results as disabled-build parity would be false. |
| `ftgzip.FT_Gzip_Uncompress.uncompresses_valid_gzip_buffer` and `ftgzip.FT_Stream_OpenGzip.opens_valid_gzip_stream` | The declared manifests `compressed/gzip/small-text-and-empty-payloads.json` and `compressed/gzip/small-and-large-streams.json` are absent, and `fontdone` has no explicit gzip dependency or public gzip stream implementation. Existing gzip exact-error rows are real. | Do not fake with static bytes. A valid success route needs maintained compressed byte fixtures plus pure-Rust gzip/zlib behavior exposed through Rust FFI, C ABI, and WASM ABI. |
| `ftbdf.*` SFNT-BDF and PCF success rows | Missing `fonts/bdf/sfnt-bdf-table.otb`, `fonts/bitmap/sfnt-bdf-table.otb`, and `fonts/pcf/properties-signed-only.pcf`. BDF file success/error rows with present `.bdf` fixtures are already real. | Do not promote. Need C-openable bitmap/SFNT-BDF and PCF fixtures before routing these rows. |
| malformed `new_memory_face` rows for post/name-table-specific errors | Route audit reports the generated unsupported-post fixture opens successfully in pinned C, bad-storage name returns public error 3, and no-name-table opens successfully. | Treat as fixture-contract blockers. Do not classify a different Rust error as parity; produce a C-observable malformed fixture or update the manifest through review. |
| `freetype.FT_Face_Properties.error_null_face` | Route audit records pinned FreeType 2.14.3 dereferences a null face for `num_properties > 0` and segfaults. | Keep pending unless a maintained process-survival route is designed. Returning Rust `Invalid_Face_Handle` is safer, but it is not exact C behavior for this declared input. |

## Next 10+ row batches

These are the viable high-count batches. Each must be attacked as an actual
implementation surface, not as route-audit classification only.

### Batch A: FTC cache manager and cache lookup routes

Current pending rows: at least 70 across `ftcache.*`.

Primary operations:

- `ftcache.manager_remove_face_id`
- `ftcache.manager_done`
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
