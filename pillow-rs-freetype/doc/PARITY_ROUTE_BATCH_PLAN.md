# Parity Route Batch Plan

Current objective: exact same-input parity with pinned C FreeType for Rust FFI,
thin C ABI, and WASM ABI. Do not count coverage-only tests, generic fallback,
fixture substitutions, or green placeholders as parity.

Current live baseline on `main` after the FT_Open_Args memory-source split:

```text
route audit concrete_cases=7270 category_counts={'compile-contract': 2266, 'pending-route': 227, 'real-null-validation': 9, 'real-parity': 4768}
pending_route_rows=227
duplicate_operation_input_buckets=41
```

This is the authoritative starting point for the next batch. The pending rows
are still visible and must remain visible until the same declared input runs
against pinned C, Rust FFI, thin C ABI, and WASM ABI with exact matching output.

High-leverage duplicate input buckets from
`python3 scripts/report_pending_route_buckets.py`:

| Pending rows | Bucket | Current decision |
|---:|---|---|
| 16 | `FT_TrueTypeGX_Validate` semantic constants / output slots | Keep pending. These rows have no maintained runtime-resolved input and the pinned oracle build has no active GX validator service. |
| 9 | `ftgxval.truetype_gx_validate` | Keep pending. Requires C-openable GX/AAT fixtures plus selected table output/free routing. |
| 9 | `ftotval.open_type_validate` success/selected tables | Keep pending. Required OpenType validator fixtures are absent and the pinned oracle currently reports validator service unavailable before selected table output can be observed. |
| 7 | `ftotval.open_type_validate` malformed/partial cleanup | Keep pending. Required malformed fixtures are absent; returning only the service-missing error would not prove malformed table parity. |
| 4 | `ftcid.get_cid_from_glyph_index` | Keep pending for the old broad rows. The SFNT-wrapped CID split is already real; the remaining bucket requires a non-SFNT CID-keyed Type1/CFF fixture or crash-isolated null-output proof. |
| 6 | `ftstroke.export_border` | Keep pending. Requires real stroker path/border geometry; an unparsed stroker no-op only proves the existing invalid-input row. |

Next surface audit decisions after the FT_Open_Args source-flag row:

- BDF/SFNT-BDF rows are fixture-generation work, not a classifier-only
  promotion. `FT_Get_BDF_Charset_ID` and `FT_Get_BDF_Property` already prove
  BDF success plus exact BDF error rows for the maintained BDF assets, but the
  SFNT bitmap rows still reference absent `sfnt-bdf-table.otb` assets. FreeType
  documents that the BDF APIs also apply to SFNT bitmap fonts containing a
  `BDF ` table, so the correct next step is a reproducible OTB/SFNT-BDF fixture
  generator plus same-input C/Rust/C-ABI/WASM comparison. Reusing the BDF-only
  row or classifying the missing-file error as SFNT-BDF parity would be a green
  placeholder.
- Type1 auxiliary attachment and track-kerning rows are a single fixture+route
  batch. The repo currently has `input/fonts/type1/attach-afm-base.pfb`, but no
  maintained matching AFM/PFM fixture under `input/aux/type1/`. FreeType's public
  reference says `FT_Get_Track_Kerning` is Type1-driver-only and uses AFM data
  attached with `FT_Attach_File` or `FT_Attach_Stream`; only a few AFM files have
  track-kerning data. The honest batch is: generate or import a matching AFM
  with `StartTrackKern`/`TrackKern`, add attach-file and attach-stream routes,
  implement pure-Rust AFM attachment state, then compare post-attach kerning or
  track-kerning output through all ABI lanes. Null/missing-file attach rows and
  no-track-data errors do not prove success.
- `ftdriver.hinting_engine_property` remains fixture-gated and route-gated. The
  four pending rows declare CFF, Type1, and CID hinting-sensitive fonts that are
  not present locally. Existing TrueType interpreter-version property rows prove
  the scalar property mechanism, but they do not prove the PostScript
  `hinting-engine` property or any public glyph-output effect. Promoting those
  rows through scalar macro values or no-op property acceptance would be a green
  placeholder.
- CID rows already have a valid SFNT-wrapped CID split using
  `input/fonts/cid/ot-cff-cid-keyed.otf`: focused verification passes
  `ftcid.get_cid_from_glyph_index` at `4/4` with four old non-SFNT semantic
  rows pending, and `ftcid.get_cid_is_internally_cid_keyed` at `2/2` with two
  old non-SFNT/null-output semantic rows pending. The old rows stay pending
  because they require a non-SFNT CID-keyed Type1/CFF fixture or a crash-isolated
  null-output probe. `ftcid.c` dereferences output pointers before service
  dispatch, so null-output behavior cannot be promoted through the normal
  same-process oracle.

Current-turn candidate decisions:

- `freetype.FT_Open_Args.memory_source_success_matches_c` and
  `freetype.FT_Open_Args.memory_source_error_variants_match_c` are part of the
  concrete split for the memory-source subset of `FT_Open_Args`.  They compare
  `FT_OPEN_MEMORY` success plus C-safe invalid rows for no source flag,
  multiple source flags, null args, null library, and null output face through
  pinned C, Rust FFI, thin C ABI, and WASM ABI.  The follow-up
  `memory_source_negative_face_index_probe_matches_c` and
  `memory_source_out_of_range_face_index_matches_c` rows prove the same
  memory-source route for the `face_index = -1` count probe and an
  out-of-range face index.  The `memory_source_short_sizes_match_c` and
  `memory_source_truncated_sfnt_size_matches_c` rows prove explicit
  `memory_size` truncation behavior: pinned C returns error 85 for
  0/4/64-byte buffers and error 2 for a 1024-byte truncated SFNT slice.  The
  `memory_source_optional_flags_noop_match_c` row proves that
  `FT_OPEN_MEMORY` still opens when combined with null `FT_OPEN_PARAMS`, null
  `FT_OPEN_DRIVER`, or both; pinned C returns success and a non-null face for
  those rows.  The `source_flag_error_matrix_matches_c` row proves additional
  C-matching source-flag errors for null stream-only, memory+pathname,
  stream+pathname, and memory+stream+pathname combinations.  The
  pathname-only/null-path row is intentionally not promoted in that matrix:
  pinned C returns `FT_Err_Cannot_Open_Resource` (1), while the current safe
  memory helper policy reports invalid source selection.  The
  attempted `memory_base = NULL` invalid row is intentionally not promoted:
  pinned FreeType 2.14.3 segfaults in the `--open-face-variants` oracle for
  `FT_OPEN_MEMORY` with a null memory base instead of returning a public
  `FT_Error`, so treating Rust's safe `Invalid_Argument` as exact C parity
  would be a green placeholder.
  Focused `test-op OP=freetype.open_face_args` reports `passed=8 failed=0
  total=8` with one pending broad row. Full `fontdone-test` reports
  `runtime_parity: passed=7038 failed=0 total=7038
  covered_manifest_cases=3931` and `runtime_cases: runnable=7038
  pending=232`.
- `ftglyph.FT_Done_Glyph.success_releases_owned_glyph` stays pending even
  though concrete outline and bitmap ownership sub-routes are now real.  The
  broad row still declares optional SVG glyphs, malformed glyph/class facades,
  allocation-failure facades, and multiple creation paths.  Re-adding the same
  bitmap route under the `FT_Done_Glyph` subject would create duplicate
  line-mapping evidence rather than new parity.
- `ftimage.FT_Pos.coordinate_outputs_use_ft_pos` stays pending.  The declared
  synthetic outline asset `outlines/synthetic/negative-and-large-coordinates.json`
  is absent, and no maintained `coordinate_endpoint_parity` runner currently
  compares the declared `FT_Load_Glyph` outline points, `FT_Outline_Get_CBox`,
  `FT_Vector_Transform`, and `FT_Outline_Decompose` outputs across pinned C,
  Rust FFI, C ABI, and WASM ABI.
- `freetype.FT_Open_Args.open_face_consumes_args_like_c` stays pending.  The
  current maintained routes cover specific memory/name-option/ignored-param and
  external-stream cases, but the broad row still declares pathname, driver,
  params, negative face-index, stream, multiple-source-flag, and no-source-flag
  behavior as one contract.  Promoting it without explicit variants would
  overclaim `FT_Open_Face` dispatch parity.
- `ftmm.FT_Get_Var_Design_Coordinates.excess_output_coordinates_zero_filled`
  stays pending.  Focused verification showed `runnable=0 pending=1`; the
  classifier records that pinned FreeType 2.14.3 clamps the active TrueType
  axis count but reads default values past the axis array for excess outputs,
  while Type1 MM zero-fills.  Safe Rust zero-fill is not exact same-input C
  behavior for the current TrueType variable fixture.

Rejected same-turn promotions:

- `ftstroke.FT_Stroker.lifecycle_contract` remains pending. The existing
  lifecycle runner proves non-null allocation and no-crash cleanup, but the row
  asks for allocator/crash lifecycle evidence across a manual path sequence.
  Counting the existing runner as this row would be a green placeholder.
- `ftstroke.FT_Stroker_Done.after_export_cleanup` remains pending. The row asks
  for exported outline preservation and allocation/free event behavior after a
  real path export; the current route only covers invalid/null/unparsed
  no-op export behavior.
- `ftcache.FTC_Node_Unref.null_or_invalid_inputs_noop` remains pending. The
  already-promoted split covers null node/manager inputs only. The original row
  also includes a non-null foreign/bad-cache-index node, where pinned C reads
  `node->cache_index`; null-only behavior cannot be generalized.

After the Type1 PS FontInfo/Private concrete split:

```text
route audit concrete_cases=7249 category_counts={'compile-contract': 2266, 'pending-route': 237, 'real-null-validation': 9, 'real-parity': 4737}
focused runtime_parity: passed=1 failed=0 total=1 covered_manifest_cases=1
full fontdone-test runtime_parity: passed=7007 failed=0 total=7007 covered_manifest_cases=3908
full fontdone-test runtime_cases: runnable=7007 pending=242
```

Added same-input rows:

- `t1tables.FT_Get_PS_Font_Info.type1_font_value_populated_success`
- `t1tables.FT_Get_PS_Font_Private.type1_font_value_populated_success`

Both rows use the maintained generated Type1 fixture
`input/fonts/type1/font-value-populated.pfb` and compare exact public
`PS_FontInfoRec` / `PS_PrivateRec` outputs through pinned C, Rust FFI, thin C
ABI, and WASM ABI. The broad
`t1tables.FT_Get_PS_Font_Info.signature_and_behavior_matrix`,
`t1tables.FT_Get_PS_Font_Private.signature_and_behavior_matrix`, and
`t1tables.FT_Has_PS_Glyph_Names.signature_and_behavior_matrix` rows remain
pending because they still include CFF2, CID, Type42, CFF glyph-name, and
other future assets or unimplemented public functions.

Focused verification:

```bash
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Get_PS_Font_Info.type1_font_value_populated_success
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Get_PS_Font_Private.type1_font_value_populated_success
```

After the `FT_Has_PS_Glyph_Names` concrete split:

```text
route audit concrete_cases=7252 category_counts={'compile-contract': 2266, 'pending-route': 237, 'real-null-validation': 9, 'real-parity': 4740}
focused operation runtime_parity: passed=3 failed=0 total=3 covered_manifest_cases=2
full fontdone-test runtime_parity: passed=7010 failed=0 total=7010 covered_manifest_cases=3910
full fontdone-test runtime_cases: runnable=7010 pending=242
```

Added same-input rows:

- `t1tables.FT_Has_PS_Glyph_Names.type1_font_value_populated_true`
- `t1tables.FT_Has_PS_Glyph_Names.cff_fontinfo_populated_true`
- `t1tables.FT_Has_PS_Glyph_Names.truetype_false`
- `t1tables.FT_Has_PS_Glyph_Names.null_face_false`

The rows prove the service-based `FT_Has_PS_Glyph_Names` behavior from
`freetype/src/base/fttype1.c`: Type 1 returns `1`, null returns `0`, and a
TrueType control returns `0` even when SFNT glyph-name flags can be present.
The CFF split proves `freetype/src/cff/cffdrivr.c:cff_ps_has_glyph_names`
follows the CFF face's `FT_FACE_FLAG_GLYPH_NAMES` state for the maintained
`input/fonts/cff/fontinfo-populated.otf` same input. Core owns the behavior;
the C ABI and WASM ABI exports are thin handle forwarders. The broad
`t1tables.FT_Has_PS_Glyph_Names.signature_and_behavior_matrix` row remains
pending because Type42, CFF-without-name, and CID fixture coverage is still
absent.

Focused verification:

```bash
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Has_PS_Glyph_Names.type1_font_value_populated_true
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Has_PS_Glyph_Names.cff_fontinfo_populated_true
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Has_PS_Glyph_Names.truetype_false
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Has_PS_Glyph_Names.null_face_false
make -C pillow-rs-freetype test-op OP=t1tables.has_ps_glyph_names
```

After the CFF glyph-name split:

```text
route audit concrete_cases=7258 category_counts={'compile-contract': 2266, 'pending-route': 237, 'real-null-validation': 9, 'real-parity': 4746}
runtime_parity: passed=4 failed=0 total=4 covered_manifest_cases=3
runtime_cases: runnable=4 pending=1
```

After the `FT_Get_PS_Font_Info` / `FT_Get_PS_Font_Private` null-error split:

```text
route audit concrete_cases=7256 category_counts={'compile-contract': 2266, 'pending-route': 237, 'real-null-validation': 9, 'real-parity': 4744}
focused FontInfo operation runtime_parity: passed=5 failed=0 total=5 covered_manifest_cases=4
focused FontPrivate operation runtime_parity: passed=16 failed=0 total=16 covered_manifest_cases=15
full fontdone-test runtime_parity: passed=7014 failed=0 total=7014 covered_manifest_cases=3912
full fontdone-test runtime_cases: runnable=7014 pending=242
```

Added same-input rows:

- `t1tables.FT_Get_PS_Font_Info.null_face_invalid_face_handle`
- `t1tables.FT_Get_PS_Font_Info.null_output_invalid_argument`
- `t1tables.FT_Get_PS_Font_Private.null_face_invalid_face_handle`
- `t1tables.FT_Get_PS_Font_Private.null_output_invalid_argument`

These rows prove the `src/base/fttype1.c` public error paths for null face and
null output pointers through pinned C, Rust FFI, thin C ABI, and WASM ABI.
The null-output rows reuse the maintained generated Type1 fixture
`input/fonts/type1/font-value-populated.pfb`; null-face rows require no asset.
The broad FontInfo/Private matrices remain pending for CFF2, CID, Type42, and
other format/service cases.

Focused verification:

```bash
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Get_PS_Font_Info.null_face_invalid_face_handle
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Get_PS_Font_Info.null_output_invalid_argument
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Get_PS_Font_Private.null_face_invalid_face_handle
make -C pillow-rs-freetype test-case CASE=t1tables.FT_Get_PS_Font_Private.null_output_invalid_argument
make -C pillow-rs-freetype test-op OP=t1tables.get_ps_font_info
make -C pillow-rs-freetype test-op OP=t1tables.get_ps_font_private
```

Historical route-audit baseline before the FTC route batches:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 392, 'real-null-validation': 9, 'real-parity': 4568}
```

Current live baseline on `main` after the stroker lifecycle/no-op route batch:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 336, 'real-null-validation': 9, 'real-parity': 4627}
runtime_parity: passed=6897 failed=0 total=6897 covered_manifest_cases=3804
runtime_cases: runnable=6897 pending=341
```

This baseline means the maintained same-input routes are green; it does not mean
full public API parity is complete. The 336 route-pending rows remain outside
the real same-input Rust FFI / C ABI / WASM ABI comparison set.

The remaining rows are not independent one-off issues. They should be attacked
as shared implementation surfaces. The current largest related buckets are:

| Surface | Current pending shape | Correct batch strategy |
|---|---:|---|
| COLR/CPAL paint graph, layers, gradients, transforms, clipboxes, foreground color | 90+ rows across `ftcolor.*` | Implement maintained COLR v0/v1 fixtures plus core paint graph traversal, opaque paint handles, colorline iterators, transform normalization, and palette/foreground semantics. Do not count absent color fonts or scalar constants as paint parity. |
| Stroker path, border, count, export, glyph-stroke geometry | 50+ rows across `ftstroke.*` | Port the pure-Rust stroker path/border model from `freetype/src/base/ftstroke.c`, then promote line/conic/cubic/begin/end/count/export rows together. Do not promote `Set`, `Rewind`, or optional-output rows from an unparsed zero-count stroker; their expectations depend on path state and geometry. |
| GX/OpenType/classic-kern validation | 40+ rows across `ftgxval.*` and `ftotval.*` | Resolve the pinned-build validator service contract, add C-openable table fixtures, implement validation buffer output/free semantics, and promote selected/all/free rows together. Existing green rows are only invalid/service-missing behavior. |
| CID/PFR/BDF/Bzip/SVG/incremental/glyph object lifecycle | smaller service/fixture clusters | Keep as separate subsystem batches because each requires distinct assets or ownership facades. Combine rows within each subsystem, not across unrelated APIs. |

Do not split future work by individual row when one implementation surface owns
the behavior. Also do not merge dissimilar missing-fixture rows into a fake
"10+" batch; a batch counts only when the same code path and same maintained
fixture/oracle route prove the promoted rows.

After the `FT_Gzip_Uncompress` gzip/zlib buffer success batch:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 335, 'real-null-validation': 9, 'real-parity': 4628}
runtime_parity: passed=6898 failed=0 total=6898 covered_manifest_cases=3805
runtime_cases: runnable=6898 pending=340
```

This batch adds the maintained `font-fixture-gzip` generator, a deterministic
small-text/empty gzip+zlib fixture manifest, pure-Rust `FT_Gzip_Uncompress`
buffer decompression, thin C/WASM ABI exports, and a pinned C oracle route for
exact output bytes. `FT_Stream_OpenGzip` remains pending because stream wrapper
state, source-position behavior, and close lifecycle are separate semantics.

After the `FT_Stream_OpenGzip` small/large stream success batch:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 334, 'real-null-validation': 9, 'real-parity': 4629}
runtime_parity: passed=6899 failed=0 total=6899 covered_manifest_cases=3806
runtime_cases: runnable=6899 pending=339
```

This batch extends `font-fixture-gzip` with `small-and-large-streams.json`, adds
pure-Rust `FT_Stream_OpenGzip` stream setup over the existing rust-backend gzip
decoder, exposes only thin FreeType-shaped C/WASM wrappers, and compares pinned
C stream classes plus beginning/middle/end decompressed range bytes. The stream
row covers small in-memory stream behavior and large callback-style stream
behavior for both zero and nonzero initial source positions. Bzip2, LZW, and
gzip close-lifecycle rows remain separate pending surfaces.

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

After the FT_StreamRec memory-stream probe route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 354, 'real-null-validation': 9, 'real-parity': 4606}
runtime_parity: passed=6877 failed=0 total=6877 covered_manifest_cases=3784
runtime_cases: runnable=6877 pending=358
```

The route compares `FT_New_Memory_Face` public memory stream state for
`input/fonts/DejaVuSans.ttf`: base nullness, size, final stream position,
cursor/limit nullness, and declared frame byte reads. Pinned FreeType 2.14.3
leaves the stream position at the `cvt ` table offset (`2908`) after opening
this TrueType face, so Rust now derives the public memory-stream position from
the retained parsed table directory rather than defaulting it to zero.

Verification command:

```bash
make -C pillow-rs-freetype route-audit
```

After the `FT_Outline_Get_Bitmap` mono dropout flag route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 352, 'real-null-validation': 9, 'real-parity': 4608}
runtime_parity: passed=6879 failed=0 total=6879 covered_manifest_cases=3786
runtime_cases: runnable=6879 pending=356
```

The route compares exact MONO bitmap bytes for the maintained
`dropout-thin-stems` outline across `FT_OUTLINE_NONE`,
`FT_OUTLINE_IGNORE_DROPOUTS`, `FT_OUTLINE_SMART_DROPOUTS`, and the combined
smart+ignore flag scenario through pinned C, Rust FFI, C ABI, and WASM ABI.
`FT_OUTLINE_INCLUDE_STUBS.mono_stub_dropout_behavior` remains pending because
the maintained `outlines/synthetic/dropout-stubs-scantype.json` fixture is
absent.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_get_bitmap
make -C pillow-rs-freetype route-audit
```

After the `FT_OUTLINE_INCLUDE_STUBS` mono dropout route:

```text
route audit concrete_cases=7242 category_counts={'compile-contract': 2266, 'pending-route': 235, 'real-null-validation': 9, 'real-parity': 4732}
runtime_parity: passed=1 failed=0 total=1 covered_manifest_cases=1
```

The existing `FT_Outline_Get_Bitmap` dropout route now includes
`ftimage.FT_OUTLINE_INCLUDE_STUBS.mono_stub_dropout_behavior`. The maintained
synthetic outline fixture `outlines/synthetic/dropout-stubs-scantype.json`
exists, and the route compares exact MONO bitmap bytes for
`FT_OUTLINE_NONE`, `FT_OUTLINE_INCLUDE_STUBS`,
`FT_OUTLINE_INCLUDE_STUBS|FT_OUTLINE_SMART_DROPOUTS`, and
`FT_OUTLINE_INCLUDE_STUBS|FT_OUTLINE_IGNORE_DROPOUTS` through pinned C, Rust
FFI, thin C ABI, and WASM ABI. This is not a constant-only route; the public
flag is proven through rendered bitmap output.

Focused verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_OUTLINE_INCLUDE_STUBS.mono_stub_dropout_behavior
```

Rejected in the same issue set:

- `ftmm.FT_Set_MM_Blend_Coordinates.output_changes_for_active_blend` remains
  pending. Pinned FreeType 2.14.3 returns error `-2` for
  `FT_Set_MM_Blend_Coordinates` on the current `gvar-hvar-wght.ttf` row with
  `coords=[65536]`; the sibling Var-blend row succeeds only for its declared
  glyph. Reusing the Var route or changing the expected output would be a
  green placeholder. The correct future fix is a maintained MM fixture whose
  pinned C route is an observable glyph-output success row.

After the absent incremental parameter embedded-data route:

```text
route audit concrete_cases=7242 category_counts={'compile-contract': 2266, 'pending-route': 234, 'real-null-validation': 9, 'real-parity': 4733}
runtime_parity: passed=1 failed=0 total=1 covered_manifest_cases=1
```

The row
`ftincrem.FT_Incremental_InterfaceRec.absent_parameter_uses_embedded_data` now
has a maintained same-input route. The route opens the existing
`input/fonts/DejaVuSans.ttf` fixture with `FT_New_Memory_Face` and no
`FT_PARAM_TAG_INCREMENTAL` parameter, loads glyph 36, and compares
`open_error`, `load_error`, `callback_count=0`, and `embedded_data_used`
through pinned C, Rust FFI, thin C ABI, and WASM ABI.

Pinned C behavior checked before routing: FreeType 2.14.3 returns `open=0` and
`load=0` for `FT_New_Memory_Face` followed by `FT_Load_Glyph(face, 36,
FT_LOAD_DEFAULT)` on this fixture without an explicit size call. Therefore the
row proves embedded font data is used and no incremental callbacks are involved.

Focused verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftincrem.FT_Incremental_InterfaceRec.absent_parameter_uses_embedded_data
```

Rejected in the same issue set:

- `ftincrem.FT_Incremental_Interface.null_or_absent_interface_behavior`
  remains pending. It includes a `FT_PARAM_TAG_INCREMENTAL` row with
  `data=NULL`; the thin C ABI can express that parameter, but the current Rust
  FFI and WASM ABI do not expose arbitrary `FT_Open_Face` parameter records.
  Counting it by treating null-parameter open as identical to absent-parameter
  open would not prove the declared same input across all ABI lanes.

After the `FT_Get_PS_Font_Value` selector-matrix route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 351, 'real-null-validation': 9, 'real-parity': 4609}
runtime_parity: passed=6880 failed=0 total=6880 covered_manifest_cases=3787
runtime_cases: runnable=6880 pending=355
```

The route uses maintained generator-backed Type1 and CFF assets at the declared
fixture paths and compares `FT_Get_PS_Font_Value` for scalar FontInfo,
string FontInfo, Private-dictionary array, custom Encoding, sizing query,
short-buffer preservation, negative `value_len`, invalid index,
unsupported CFF service, non-PostScript face, and null face through pinned C,
Rust FFI, thin C ABI, and WASM ABI. The core implementation now routes the
public `PS_Dict_Keys` selectors used by that matrix instead of only
`PS_DICT_ENCODING_TYPE`/`PS_DICT_ENCODING_ENTRY`.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=t1tables.get_ps_font_value
make -C pillow-rs-freetype route-audit
```

After the `TT_MAC_ID_JAPANESE` mac-encoding metadata route:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 290, 'real-null-validation': 9, 'real-parity': 4673}
runtime_parity_progress: compared=3 total=3 passed=3 failed=0
runtime_cases: runnable=3 pending=0
```

The route extends the maintained name/cmap fixture generator with
`input/fonts/name-cmap/mac-japanese.ttf`, a compact SFNT fixture containing one
Macintosh Japanese full-name record and deterministic cmap entries for
U+3042, U+30A2, and U+4E00. The manifest operation
`sfnt.charmap_and_name_metadata` now routes through the existing exact
mac-encoding runner instead of remaining hardcoded as a selection-skipped row.
The three concrete variants compare pinned C, Rust FFI, thin C ABI, and WASM ABI
for charmap metadata, glyph indices, and matched SFNT name record bytes.

After the COLRv1 static gradient and ColorLine stop route batch:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 266, 'real-null-validation': 9, 'real-parity': 4697}
runtime_parity_progress:
  ftcolor.get_gradient_paint_and_stops compared=6 total=6 passed=6 failed=0
  ftcolor.get_colorline_stops compared=5 total=5 passed=5 failed=0
  ftcolor.get_paint compared=31 total=31 passed=31 failed=0
  ftcolor.traverse_gradient_paints compared=1 total=1 passed=1 failed=0
```

This batch adds `fonts/color/colr-v1-static-gradients.ttf`, a maintained static
COLRv1 fixture with one PaintLinearGradient/PAD/3-stop root, one
PaintRadialGradient/REPEAT/2-stop root, and one
PaintSweepGradient/REFLECT/1-stop root.  The routed rows compare public
gradient formats, static `FT_ColorLine` extend and iterator fields, exact
`FT_ColorStop` stop offsets/palette indices/alpha values, iterator advancement,
and terminal false behavior through pinned C, Rust FFI, thin C ABI, and WASM
ABI.

After the COLRv1 clipbox route batch and the 2026-07-22 pending-surface audit:

```text
route audit concrete_cases=7242 category_counts={'compile-contract': 2266, 'pending-route': 238, 'real-null-validation': 9, 'real-parity': 4729}
duplicate_operation_input_buckets=41
runtime_parity: passed=6999 failed=0 total=6999
runtime_cases: pending=243
```

Rows audited and intentionally kept pending:

| Surface | Pending rows checked | Current blocker | Correct next batch |
|---|---:|---|---|
| `ftdriver.hinting_engine_property` | 4 | Declared fixtures `fonts/cff/cff-hinting-sensitive.otf`, `fonts/type1/type1-hinting-sensitive.pfb`, and `fonts/cid/cid-keyed-type1-hinting-sensitive.otf` are absent. Substitute CFF/Type1 fonts would not prove glyph output after the `hinting-engine` property toggle. | Add maintained hinting-sensitive CFF/Type1/CID fixtures, then implement `cff` driver `hinting-engine` property state through Rust FFI, C ABI, WASM, and oracle route. |
| `ftglyph.type_runtime` / non-null `ftglyph.done_glyph` | 7 | Existing maintained route proves `FT_Done_Glyph(NULL)` and `FT_OutlineGlyphRec.owns_outline_arrays` only. The pending rows require real bitmap glyph creation through `FT_Glyph_To_Bitmap`, optional SVG classification, allocation/free event logging, class identity behavior, and lifetime ordering. Treating outline-only glyphs or opaque non-null handles as proof would be a green placeholder. | Implement owned bitmap glyph records and `FT_Glyph_To_Bitmap` success first, then route glyph class/type/lifecycle rows together. |
| BDF success variants | 2 | `fonts/pcf/properties-signed-only.pcf` and `fonts/bitmap/sfnt-bdf-table.otb` are absent. The BDF `.bdf` row and exact error rows are already maintained; PCF/SFNT-BDF success needs distinct parser/fixture coverage. | Add C-openable PCF and SFNT-BDF fixtures, then extend the existing BDF property route for signed PCF properties and selected-strike SFNT-BDF properties. |
| bzip2 stream validation | 2 | The active pinned oracle build is bzip2-disabled and returns `FT_Err_Unimplemented_Feature` before null/source/header validation. The pending rows describe enabled-build behavior, so counting disabled-build `Unimplemented_Feature` would be false parity. | Split enabled-vs-disabled bzip2 policy, or add a bzip2-enabled pinned oracle profile plus pure-Rust bzip2 stream wrapper. |
| CID success/null-output rows | 10 | Required CID-keyed and SFNT-wrapped CID fixtures are absent (`type1-cid-ros-and-glyph-map.pfb`, `ot-cff-cid-keyed.otf`). Existing non-CID/null-face error controls are already real. | Add maintained CID-keyed fixtures and implement CID service metadata/glyph-index mapping before routing CID success and null-output behavior. |
| `ftparams.face_properties_then_render` | 2 | Required CFF fixtures `randomized-cff.otf` and `stem-darkening-sensitive.otf` are absent. Existing DejaVu null-data/internal-state rows are already routed; render-output rows need an observable CFF/Type1/CID driver effect. | Add C-openable property-sensitive CFF/Type1 fixtures, then compare internal property state plus glyph metrics/outline/bitmap output. |
| GX/OpenType/classic-kern validator success rows | 30+ | Declared AAT/GX/OpenType validator fixtures are absent, including all-table, selected-table, malformed-table, and length-matrix fonts. Existing green rows cover invalid arguments and missing services only. | Build deterministic validator fixtures, then implement validation output-buffer allocation/free semantics and route selected/all/free rows as one validator batch. |
| `ftmodapi.add_module` success/renderer/styler rows | 3 | These require synthetic `FT_Module_Class`/`FT_Renderer_Class` callback facades with callback logs and rollback semantics. Current maintained rows cover null/future-version/duplicate error behavior only. | Add a maintained ABI test facade for synthetic module classes and route add/remove/interface callback behavior across C, Rust, C ABI, and WASM. |

This audit narrows the next safe high-value work. It does not reduce the full
parity target or reclassify any row as real. Each listed surface remains visible
as `pending-route` until the same declared inputs compare exact pinned C output
through Rust FFI, thin C ABI, and WASM ABI.

Pinned C behavior checked:

- `freetype/src/sfnt/ttcolr.c:500-520`: `read_color_line` reads extend/count,
  validates extend range, and initializes the public color-stop iterator.
- `freetype/src/sfnt/ttcolr.c:724-870`: gradient paints expose static
  coordinates/radii/angles as FreeType public fixed-point values and normalize
  public paint formats.
- `freetype/src/sfnt/ttcolr.c:1585-1650`: `FT_Get_Colorline_Stops` emits one
  stop, advances the iterator pointer/current index, and returns false without
  modifying output once iteration is exhausted.

Rows deliberately left pending: variable ColorLine/VarColorStop deltas,
`FT_ColorStop.iterator_output_values` because it still includes variable-stop
coverage, broad all-paint linear-gradient matrix rows, and root-transform
gradient cases.  Those require separate maintained variable/all-paints fixtures
and exact variable-delta/root-transform routes, not static-gradient
classification.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=ftcolor.get_gradient_paint_and_stops
make -C pillow-rs-freetype test-op OP=ftcolor.get_colorline_stops
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint
make -C pillow-rs-freetype test-op OP=ftcolor.traverse_gradient_paints
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
make fontdone-lint
```

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=sfnt.charmap_and_name_metadata
make -C pillow-rs-freetype route-buckets
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

## Missing fixture sourcing sweep: 2026-07-21

Internet search was used only to classify fixture acquisition strategy. Do not
import third-party binaries until license, provenance, and pinned-C output are
checked into a maintained generator or fixture note.

| Fixture family | Search result | Plan |
|---|---|---|
| OT-SVG / SBIX color fonts | Public candidates exist in `simoncozens/test-fonts` and `googlefonts/color-fonts`. | Use only after license review and pinned-C output capture. If output shape must be minimal, generate repo-local fixtures instead. |
| PCF and SFNT-BDF/OTB | No exact declared files found. `monobit` can generate SFNT bitmap outputs; BDF-to-PCF workflows exist via `bdftopcf`. | Prefer deterministic repo-local generators. Do not add arbitrary public PCF/OTB files. |
| OpenType validation BASE/GDEF/GPOS/GSUB/JSTF/MATH | Public test fonts exist for some layout tables, but exact malformed rows are not available under the declared names. | Generate minimal valid/malformed FontTools fixtures per table, then implement `FT_OpenType_Validate` behavior before promoting. |
| CID/PFR/GX/AAT | Public corpus fonts exist, but no compact exact declared fixtures were found. | Generate/subset controlled fixtures where possible; otherwise keep rows pending until a license-compatible compact corpus asset is selected and pinned-C behavior is recorded. |
| TrueType phantom-point backward-compatibility and synthetic outline JSON | No public exact fixtures found. | Generate locally; these are behavior-specific fixtures, not internet corpus assets. |
| malformed `maxp` | Exact declared files were placeholder symlinks; no internet asset should be used. | Completed. `scripts/build_sfnt_fixtures.py` generates malformed SFNTs and `face.load_then_get_sfnt_table.maxp` now compares pinned-C face-load status, pointer nullness, and adjusted `TT_MaxProfile` fields through Rust FFI, C ABI, and WASM ABI. |

### Basic DejaVu fixture path standardization

The missing-fixture sweep reported many references to
`fonts/basic/dejavu-sans.ttf`, `fonts/basic/DejaVuSans.ttf`, and
`fixtures/assets/fonts/DejaVuSans.ttf`. These are not independent missing
fixtures. They are legacy path spellings for the maintained standard input
`input/fonts/DejaVuSans.ttf`. The unified parity harness resolves all three
spellings to that single checked-in font so same-input C/Rust/C-ABI/WASM
comparisons use identical bytes.

Do not apply the same shortcut to `fonts/basic/dejavu-serif.ttf`. The current
tree has `input/fonts/LiberationSerif-Regular.ttf`, but that is not the same
font as DejaVu Serif. Rows that truly require a second serif face must either
vendor a license-reviewed DejaVu Serif asset with recorded provenance and
pinned-C output, or be changed through a reviewed manifest update to name
`LiberationSerif-Regular.ttf` as the actual same input.

### Type1/CFF PostScript table fixture split

The full `FT_Get_PS_Font_Info` and `FT_Get_PS_Font_Private` signature matrices
remain pending because the declared Type1/CFF2/CID/Type42 success and error
asset set is not fully maintained. Do not mark the aggregate matrix complete.

Checked and rejected in this sweep:

- Generating `input/fonts/type1/fontinfo-populated.pfb` and
  `input/fonts/type1/private-dict-populated.pfb` from the existing compact Type1
  builder was not a valid success fixture for these APIs. Pinned C returned
  error `7` for `FT_Get_PS_Font_Info`/`FT_Get_PS_Font_Private` on those same
  bytes while Rust returned success, so keeping those rows would expose a real
  Rust/C contract gap, not a parity fixture.
- The maintained `input/fonts/cff/fontinfo-populated.otf` fixture is a valid
  current pinned-C `FT_Get_PS_Font_Info` success input. Pinned FreeType 2.14.3
  returns null `version`/`notice`, full name `Hybrid OTTO Coverage Regular`,
  family `Hybrid OTTO Coverage`, weight `Regular`, italic/fixed-pitch zero,
  and underline defaults `-100/50`.

Promoted in this sweep:

- `t1tables.FT_Get_PS_Font_Private.cff_invalid_argument` uses the maintained
  `input/fonts/cff/fontinfo-populated.otf` asset and compares the exact
  `FT_Err_Invalid_Argument` unsupported-service behavior through pinned C,
  Rust FFI, C ABI, and WASM ABI.
- `t1tables.FT_Get_PS_Font_Info.truetype_invalid_argument` and
  `t1tables.FT_Get_PS_Font_Private.truetype_invalid_argument` use the
  maintained `input/fonts/DejaVuSans.ttf` control face and compare the exact
  `FT_Err_Invalid_Argument` unsupported-service behavior through pinned C,
  Rust FFI, C ABI, and WASM ABI. These split only the non-PostScript control
  scenarios out of the broad matrices; they do not complete the Type1/CID/Type42
  success obligations.
- `t1tables.FT_Get_PS_Font_Info.cff_fontinfo_populated_success` uses the
  maintained `input/fonts/cff/fontinfo-populated.otf` CFF face and compares the
  exact top-dict `PS_FontInfoRec` strings and scalars through pinned C,
  Rust FFI, C ABI, and WASM ABI. This splits only the CFF success scenario out
  of the broad matrix; CID, Type42, and CFF2 obligations remain pending until
  maintained same-input fixtures exist.

After the CFF FontInfo route:

```text
route audit concrete_cases=7257 category_counts={'compile-contract': 2266, 'pending-route': 237, 'real-null-validation': 9, 'real-parity': 4745}
runtime_parity: passed=6 failed=0 total=6 covered_manifest_cases=5
runtime_cases: runnable=6 pending=1
```

After the malformed `TT_MaxProfile` route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 349, 'real-null-validation': 9, 'real-parity': 4611}
runtime_parity: passed=1 failed=0 total=1 covered_manifest_cases=1
runtime_cases: runnable=1 pending=0
```

After the FTC manager reset/done lifecycle route:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 348, 'real-null-validation': 9, 'real-parity': 4615}
runtime_parity: passed=1 failed=0 total=1 covered_manifest_cases=1
runtime_cases: runnable=1 pending=0
```

The route compares `ftcache.FTC_Manager.reset_and_done_lifecycle` on the
maintained `input/fonts/DejaVuSans.ttf` same input. It proves the public
void-return lifecycle shape, reset usability/requester counts, and populated
manager-done cache/face/size/node observations through pinned C, Rust FFI,
thin C ABI, and WASM ABI. This promotes the concrete reset/done lifecycle row;
`ftcache.FTC_Manager.owns_faces_sizes_and_cache_nodes` remains pending because
its face/size/node finalizer-count requirement is broader than this route.

Focused verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftcache.FTC_Manager.reset_and_done_lifecycle
make -C pillow-rs-freetype test-op OP=ftcache.manager_lifecycle
make -C pillow-rs-freetype route-audit
```

After the SFNT name/cmap metadata fixture batch:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 342, 'real-null-validation': 9, 'real-parity': 4621}
runtime_parity: passed=42 failed=0 total=42 covered_manifest_cases=26
runtime_cases: runnable=42 pending=0
```

This batch promotes six related `ttnameid` runtime rows by generating compact
same-input SFNT fixtures under `input/fonts/name-cmap/` and routing them through
the maintained `sfnt.mac_encoding_record` comparison:

- `TT_PLATFORM_APPLE_UNICODE.unicode_charmap_platform_runtime`
- `TT_APPLE_ID_ISO_10646.deprecated_apple_unicode_encoding_runtime`
- `TT_PLATFORM_ISO.deprecated_iso_platform_runtime`
- `TT_ISO_ID_10646.deprecated_iso_10646_runtime`
- `TT_PLATFORM_MICROSOFT.microsoft_unicode_platform_runtime`
- `TT_PLATFORM_CUSTOM.custom_charmap_platform_runtime`

Each generated fixture contains one focused `name` record and one deterministic
`cmap` mapping for U+0041, then compares public charmap platform/encoding,
matched name-record fields/bytes, and glyph index through pinned C, Rust FFI,
thin C ABI, and WASM ABI. The Mac Japanese and Adobe rows remain pending
because their manifest requirements need real Mac Japanese multi-codepoint or
Adobe CFF/Type1 charmap behavior, not this compact single-mapping SFNT fixture.

Focused verification:

```bash
make -C pillow-rs-freetype test-op OP=sfnt.mac_encoding_record
make -C pillow-rs-freetype route-audit
```

Pinned FreeType 2.14.3 keeps both generated malformed faces open and exposes a
non-null `FT_Get_Sfnt_Table(FT_SFNT_MAXP)` record. For the truncated table,
`tt_face_load_maxp` reads from the `maxp` stream offset beyond the declared
four-byte table length and applies the FreeType compatibility adjustment that
forces `maxFunctionDefs` to at least 64. Rust now mirrors that for the
face-owned `TT_MaxProfile` state while keeping `FT_Load_Sfnt_Table`
length-bounded.

Focused verification:

```bash
make -C pillow-rs-freetype test-case CASE=tttables.TT_MaxProfile.malformed_table_error_source
```

### Full unresolved fixture search: 2026-07-21

Current route-audit extraction found 35 fixture-like pending rows. After
removing wording false positives such as "unavailable module" rows, the real
asset backlog falls into the buckets below. Internet search did not find
license-reviewed, exact drop-in files for the declared repo paths; every asset
below still needs either deterministic generation under `scripts/` or a
separate provenance/licensing note plus pinned-C output capture before it can be
promoted.

| Declared fixture / bucket | Blocking rows | Internet/source result | Required action |
|---|---:|---|---|
| `fonts/svg/color-svg-glyph.ttf` | 1 | `simoncozens/test-fonts` has compact CFF+SVG fonts; `googlefonts/color-fonts` has generated OT-SVG variants. | Prefer a compact generated OT-SVG fixture if possible; otherwise import one candidate only after license/provenance review and C output pinning. |
| `fonts/color/sbix-outline.ttf` | 1 | `simoncozens/test-fonts` has `CFF-and-SBIX.otf`; `googlefonts/color-fonts` has generated `sbix` color bitmap fonts. | Use as candidate corpus only; declared row still needs exact `FT_PARAM_TAG_IGNORE_SBIX` open-face route and output shape. |
| `fonts/opentype/valid-all-layout.otf`, `valid-base.otf`, `valid-gdef.otf`, `valid-gpos.otf`, `valid-gsub.otf`, `valid-jstf.otf`, `valid-math.otf` | 8 | `simoncozens/test-fonts` has BASE examples and feature/layout playground fonts; FontTools can build/round-trip TTX fixtures. | Generate minimal table-specific OpenType fixtures with FontTools/TTX and pin C validator output; do not import broad corpus fonts blindly. |
| `fonts/opentype/malformed-selected-layout.otf`, `malformed-gdef.otf`, `malformed-gpos.otf`, `malformed-gsub.otf`, `malformed-jstf.otf`, `malformed-math.otf`, `partial-malformed-layout.otf` | 7 | No exact malformed public corpus matching declared names found. BrokenType provides font mutation tooling, but deterministic fixture generation is safer for parity. | Generate minimal malformed table fixtures locally and document the byte-level corruption that triggers pinned-C behavior. |
| `input/fonts/bdf/sfnt-bdf-table.otb`, `fonts/bitmap/sfnt-bdf-table.otb` | 3 | No exact OTB/SFNT-BDF fixture found. FreeType documents BDF/PCF public behavior, but that is not a fixture source. | Build deterministic BDF and SFNT-wrapped bitmap fixtures locally, then route `FT_Get_BDF_Charset_ID` / `FT_Get_BDF_Property`. |
| `fonts/pcf/properties-signed-only.pcf` | 1 | No exact PCF found. FreeType docs state PCF integer properties are signed, which defines the semantic target. | Generate a tiny PCF with signed-only properties from maintained BDF/X11 tooling; pin exact C property output. |
| `fonts/truetype/backward-compat-phantom-points.ttf` | 3 | No exact public behavior-specific fixture found. | Generate locally from a controlled TrueType program that isolates interpreter-version phantom-point behavior. |
| `outlines/synthetic/negative-and-large-coordinates.json` and other synthetic outline JSON assets | 1+ | Not an internet corpus problem. | Generate maintained JSON outline fixtures in-repo and route coordinate endpoints. |
| CID-keyed Type1/CFF fixtures such as `input/fonts/cid/type1-cid-ros-and-glyph-map.pfb`, `input/fonts/cid/ot-cff-cid-keyed.otf` | multiple route rows outside the 35-row path extraction | Adobe special-purpose CID-keyed OpenType/CFF fonts exist, including Adobe-Identity-0 based projects; no compact exact declared Type1 CID map fixture found. | Prefer generated/subset CID fixtures with explicit ROS/glyph map; imported Adobe candidates need license/provenance review and C-output pinning. |
| COLR v0/v1 layer, foreground, and clipbox fixtures | multiple route rows outside this extraction | `googlefonts/color-fonts` provides COLRv1 test fonts including static/variable test glyphs and no-cliplist variants; `simoncozens/test-fonts` has simpler COLR/CPAL examples. | Use candidates to design minimal generated COLR fixtures; exact route still needs layer iterator, foreground sentinel, and clipbox implementation across Rust FFI, C ABI, and WASM. |
| Compressed stream fixtures for gzip/bzip2/LZW | multiple pending success rows | Internet search is not useful; these are byte-stream facades, not fonts. | Generate maintained compressed byte fixtures and implement pure-Rust stream behavior; no static-byte shortcut. |

Sources checked: `https://github.com/simoncozens/test-fonts`,
`https://github.com/googlefonts/color-fonts`,
`https://github.com/freetype/freetype2-testing`,
`https://github.com/fonttools/fonttools`,
`https://github.com/googleprojectzero/BrokenType`,
FreeType 2.14.3 API reference for BDF/PCF behavior, and Adobe
Adobe-Identity-0/CID-keyed font project pages.

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
| `fterrdef.FT_Err_Missing_Property.known_property_success` | Completed 2026-07-21. The active row now uses documented `autofitter:fallback-script` property get output instead of the invalid `svg:svg-hooks` spelling. Focused `FT_Property_Get` passes exact pinned C, Rust FFI, C ABI, and WASM ABI parity; route audit moved `pending-route 351 -> 350` and `real-parity 4609 -> 4610`. | Keep the old `svg:svg-hooks` finding as fixture-contract history; do not reintroduce it as a success row unless a typed `ot-svg:svg-hooks` route is implemented and the input is reviewed. |
| `ftdriver.*.hinting_engine_property_runtime` | Focused run: `runnable=0 pending=4`. The rows require CFF, Type 1, and CID hinting-sensitive fixtures plus CFF/type1/t1cid `FT_Property_Set/Get` routing and public glyph-output comparison. | Do not count scalar constant values or no-op property sets. Real parity requires engine readback and changed public glyph metrics/outline/bitmap behavior where pinned C supports it. |

### COLRv1 all-paints route classifier correction: 2026-07-21

Rows promoted:

- `ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values`
- `ftcolor.FT_PaintColrLayers.get_paint_initializes_layer_iterator`

Evidence:

- The maintained all-paints route consumes
  `fonts/color/colr-v1-all-paints.ttf` and compares pinned C, Rust FFI, thin
  C ABI, and WASM ABI output for `FT_Get_Color_Glyph_Paint`,
  `FT_Get_Paint`, `FT_PaintColrGlyph`, and the initialized
  `FT_PaintColrLayers.layer_iterator`.
- Focused run: `make -C pillow-rs-freetype test-op OP=ftcolor.get_paint`
  passed `38/38`; pending rows dropped from six to four.
- Route audit moved `pending-route 251 -> 249`, `real-parity 4712 -> 4714`,
  and duplicate pending buckets `46 -> 45`.

Reasoning:

- These rows had already-run exact routes, but stale classifier logic treated a
  declared future `malformed_font` asset as blocking. That asset is not consumed
  by the maintained public payload route for these two rows.
- No fixture output, expected output, threshold, or comparison logic was
  weakened. The remaining COLRv1 layer terminal/iterator and linear-gradient
  rows stay pending until they have exact same-input routes.

### COLRv1 paint-layer iteration route classifier correction: 2026-07-21

Rows promoted:

- `ftcolor.FT_Get_Paint_Layers.success_iterates_colr_v1_layers`
- `ftcolor.FT_Get_Paint_Layers.end_of_iteration`

Evidence:

- The maintained route already compares `FT_Get_Paint_Layers` layer paint
  handles, iterator fields, and exhausted-call output preservation for
  `fonts/color/colr-v1-all-paints.ttf` through pinned C, Rust FFI, thin C ABI,
  and WASM ABI.
- Focused run: `make -C pillow-rs-freetype test-op OP=ftcolor.get_paint_layers`
  passed `4/4`; pending rows for the operation dropped from three to one.
- Route audit moved `pending-route 249 -> 247`, `real-parity 4714 -> 4716`,
  and duplicate pending buckets `45 -> 44`.

Reasoning:

- These rows are COLR v1-only and match the maintained layer-list route.
- `ftcolor.FT_LayerIterator.initialized_and_advanced_by_layer_apis` remains
  pending because its manifest covers both COLR v0 and COLR v1 layer APIs; the
  current route proves only the COLR v1 side.

### Split COLRv1 FT_LayerIterator paint-layer row: 2026-07-22

Status: implemented as an additive split row; the original broad row remains
pending.

Scope:

- Added `ftcolor.FT_LayerIterator.initialized_and_advanced_by_paint_layers_v1`
  for the maintained `tests/fixtures/fonts/color/colr-v1-all-paints.ttf`
  fixture.
- Reused the existing pinned-C/Rust/C ABI/WASM `FT_Get_Paint_Layers` route for
  COLR v1 PaintColrLayers glyphs, comparing return sequence, iterator
  `num_layers`, `layer`, pointer/null identity class, and returned
  `FT_OpaquePaint` fields.

Why this was split instead of promoting the older broad row:

- `ftcolor.FT_LayerIterator.initialized_and_advanced_by_layer_apis` declares
  both `FT_Get_Color_Glyph_Layer` COLR v0 and `FT_Get_Paint_Layers` COLR v1
  behavior.  This row proved only the COLR v1 side.  Counting it as the broad
  row would have hidden the missing same-input COLR v0 proof.

Observed impact:

- Route audit: `concrete_cases` 7239 → 7240, `real-parity` 4722 → 4723,
  `pending-route` remains 242.
- Focused runtime:
  `make -C pillow-rs-freetype test-case CASE=ftcolor.FT_LayerIterator.initialized_and_advanced_by_paint_layers_v1`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

### Split COLR v0 FT_LayerIterator public-record row: 2026-07-22

Status: implemented as an additive split row; the original broad row remains
pending.

Scope:

- Added
  `ftcolor.FT_LayerIterator.initialized_and_advanced_by_color_glyph_layers_v0`
  for the maintained
  `tests/fixtures/fonts/color/colr-v0-layers-cpal.ttf` fixture.
- Reused the existing pinned-C/Rust/C ABI/WASM `FT_Get_Color_Glyph_Layer` route
  for the three-layer COLR v0 base glyph, comparing the return sequence, layer
  glyph indexes, layer color indexes, and public `FT_LayerIterator` fields
  `num_layers`, `layer`, and pointer/null identity class.

Why this is split instead of promoting the older broad row:

- `ftcolor.FT_LayerIterator.initialized_and_advanced_by_layer_apis` declares
  both `FT_Get_Color_Glyph_Layer` COLR v0 and `FT_Get_Paint_Layers` COLR v1
  behavior in one row.  The v0 and v1 sides are now separately proved, but the
  broad mixed row still lacks a maintained same-input route proving both API
  families together under its exact declared input shape.
- The split row names exactly the COLR v0 layer iterator input it proves.

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

Promoted disabled/missing-service split:

- `ftotval.FT_OpenType_Validate.type1_font_value_service_missing_error` uses
  maintained `input/fonts/type1/font-value-populated.pfb` and compares exact
  `FT_Err_Unimplemented_Feature` output for a face without the
  `OPENTYPE_VALIDATE` service through pinned C, Rust FFI, C ABI, and WASM ABI.
  This does not prove selected-table, malformed-table, or validation-buffer
  ownership behavior.
- `ftotval.FT_OpenType_Validate.cff_fontinfo_service_missing_error` uses
  maintained `input/fonts/cff/fontinfo-populated.otf` and compares the same
  active-build `FT_Err_Unimplemented_Feature` missing-service result for a CFF
  face through pinned C, Rust FFI, C ABI, and WASM ABI. This broadens exact
  face-format coverage without claiming OpenType validator table success.

After the Type1/CFF OpenType missing-service split:

```text
route audit concrete_cases=7260 category_counts={'compile-contract': 2266, 'pending-route': 237, 'real-null-validation': 9, 'real-parity': 4748}
runtime_parity: passed=1 failed=0 total=1 covered_manifest_cases=1
runtime_cases: runnable=1 pending=0
```

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

## Missing fixture internet audit: 2026-07-21 exact inventory

Current local extraction from `tests/manifest.yaml` and public API input JSONs
finds 84 unique unresolved fixture-like references. Exact basename checks
against likely upstream corpora found zero direct matches in:

- `googlefonts/color-fonts`
- `fonttools/fonttools`
- `freetype/freetype`
- `dejavu-fonts/dejavu-fonts`
- `adobe-fonts/source-han-sans`
- `adobe-type-tools/afdko`
- `simoncozens/test-fonts`
- `harfbuzz/harfbuzz`

Internet search found family-level candidates, not exact declared fixtures:

- COLRv1: `googlefonts/color-fonts` has generated wide-coverage COLRv1 test
  fonts, including paint formats not expressible from SVG, but not our declared
  `colr_v1_*` file names. Use these only after license review and pinned-C
  output capture, or write a local generator.
- CID/CJK: Source Han / Noto CJK family fonts exist and are CID/CJK-scale
  candidates, but they are large corpus assets rather than compact declared
  fixtures. Prefer a subset/generator with recorded provenance.
- PFR: public conversion tooling exists, but no exact `basic-metrics.pfr` or
  `basic-kerning.pfr` was found. Prefer deterministic generation from a
  checked-in small TTF if PFR support is pursued.
- SVG/SBIX: public color emoji/SVGinOT/SBIX fonts exist, but exact same-input
  rows should not use arbitrary emoji fonts unless the row is rewritten with
  provenance and pinned-C behavior.

Current missing-reference buckets:

| Bucket | Count | Acquisition decision |
|---|---:|---|
| name/cmap language fixtures | 20 | Generate locally with FontTools/name-table scripts; external fonts would add licensing and unrelated glyph data. |
| OS/2 unicode-range fixtures | 17 | Generate locally by mutating OS/2 `ulUnicodeRange` bits. |
| misc/control fixtures | 9 | Mostly deliberate missing-path/control aliases; resolve by manifest cleanup or standard path mapping, not internet downloads. |
| ABI probe source snippets | 7 | Repo-owned C probe sources; write tracked inputs if the route is implemented. |
| Type1/AFM/PS fixtures | 7 | Generate with the existing Type1 builder or keep pending until pinned C opens the exact bytes. |
| synthetic outline/path fixtures | 6 | Repo-owned JSON/outline fixtures; generate locally. |
| CFF/charmap fixtures | 4 | Generate compact CFF/charmap variants locally unless a license-reviewed corpus asset is intentionally selected. |
| COLRv1 color fixtures | 4 | Candidate upstream exists, but exact fixtures should be generated/subset before promotion. |
| CID-keyed fixtures | 3 | Candidate upstream exists, but needs compact CID-keyed same-input asset and pinned-C capture. |
| SBIX/SVG color fixtures | 3 | Candidate upstream exists, but route-specific compact fixtures are preferred. |
| BDF/OTB fixtures | 2 | Generate locally; the BDF malformed generator already covers neighboring BDF rows. |
| PFR fixtures | 2 | Generate locally or keep pending; no exact public fixtures found. |

Do not import a downloaded font just because it exercises the same broad table
family. A missing fixture becomes real parity only when the manifest row names
the exact input bytes, the license/provenance is acceptable, pinned C output is
captured, and Rust FFI, C ABI, and WASM ABI compare the same public result.

## Batch: Apple full-Unicode format-13 charmap route

Status: implemented 2026-07-21.

Scope:

- `ttnameid.TT_APPLE_ID_FULL_UNICODE.representative_charmap_encoding_match`
- Operation `face.enumerate_charmaps`

Fix:

- Generated the declared same-input fixture at
  `tests/fixtures/input/fonts/charmaps/apple-full-unicode-type13.ttf` with an
  Apple Unicode platform (`platform_id=0`), full Unicode encoding ID
  (`encoding_id=6`), and cmap format 13.
- Added safe Rust cmap format-13 parsing and lookup.  FreeType's format-13
  class has the format-12 group layout but returns each group's constant glyph
  ID (`src/sfnt/ttcmap.c:tt_cmap13_char_index`), so Rust now stores a distinct
  `Format13Subtable`.
- Routed `face.enumerate_charmaps` through the existing exact charmap inventory
  path used by Rust FFI, thin C ABI, WASM ABI, and the pinned C oracle.

Verification:

```bash
make -C pillow-rs-freetype test-op OP=face.enumerate_charmaps
```

Observed impact:

- Route audit: `pending-route` 290 → 289, `real-parity` 4673 → 4674.
- Focused runtime: compared 1 / total 1, passed 1, failed 0.

Remaining blocker in this operation:

- `ttnameid.TT_ADOBE_ID_CUSTOM.representative_charmap_encoding_match` remains
  pending.  Existing Type 1 custom-encoding fixtures prove
  `T1_EncodingType`, but Rust Type1 loading currently leaves `FontData.cmap`
  empty.  Promoting this row requires core support for FreeType's synthesized
  Adobe-platform Type1/CFF charmaps (`src/type1/t1objs.c:539-560`,
  `src/cff/cffobjs.c:1063-1081`) and then the declared
  `input/fonts/charmaps/adobe-custom-cmap.pfb` fixture.

## Batch triage: pending-route surfaces after Apple format-13

Status: triaged 2026-07-21; no rows promoted.

Baseline:

- Commit: `56396cc2c`
- Route audit: `real-parity=4674`, `pending-route=289`,
  `real-null-validation=9`, `compile-contract=2266`.

Focused probes:

```bash
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint
make -C pillow-rs-freetype test-op OP=ftbdf.get_bdf_charset_id
make -C pillow-rs-freetype test-op OP=ftlzw.stream_open_lzw
make -C pillow-rs-freetype test-op OP=ftmm.get_var_design_coordinates
make -C pillow-rs-freetype test-op OP=freetype.inspect_charmaps
make -C pillow-rs-freetype test-op OP=ftcid.get_cid_is_internally_cid_keyed
make -C pillow-rs-freetype test-op OP=ftcid.get_cid_registry_ordering_supplement
make -C pillow-rs-freetype test-op OP=ftpfr.get_pfr_metrics
make -C pillow-rs-freetype test-op OP=ftdriver.hinting_engine_property
make -C pillow-rs-freetype test-op OP=FT_Property_Get_then_FT_Load_Glyph
make -C pillow-rs-freetype test-op OP=FT_Property_Set_then_FT_Load_Glyph
```

Findings:

- COLRv1: existing Rust, C ABI, and WASM routes prove solid, glyph, and
  composite paint nodes only.  The remaining color pending rows are not
  duplicate tests.  They need real COLRv1 parser/runtime support for root
  transform synthesis, gradient colorlines and color-stop iteration, v1 layer
  iteration, clip boxes, and transform paint formats before any route can be
  counted as real parity.
- BDF/PCF: `ftbdf.get_bdf_charset_id` passed `3/3` runnable rows and kept two
  SFNT-BDF rows pending because `input/fonts/bdf/sfnt-bdf-table.otb` is absent.
  Local PCF files are 8-byte control stubs, not property fixtures.  FreeType's
  BDF API explicitly covers BDF and PCF, and PCF integer properties are signed,
  so PCF/SFNT-BDF rows must not be collapsed into the existing BDF success
  row.  Implement by adding a C-openable PCF or SFNT-BDF fixture with
  provenance, then adding pure-Rust PCF/SFNT-BDF property and charset support.
- LZW: `ftlzw.stream_open_lzw` passed `4/4` runnable error/build-policy rows
  and kept the success row pending because `streams/lzw/small-valid-pcf.Z` and
  the memory-stream facade fixture are absent.  Real promotion requires a
  pure-Rust LZW stream route matching FreeType `src/lzw/ftlzw.c:221-308` and
  `337-383` for open/read/seek/close.
- MM coordinates: `ftmm.get_var_design_coordinates` passed `4/4`; the remaining
  excess-output row is intentionally unsound for the current TrueType variable
  fixture because pinned FreeType reads past the active axis defaults while
  Type1 MM zero-fills.  Do not model adjacent memory or safe zero-fill as
  parity for that row.  Replace the input with a sound Type1 MM same-input row
  if this surface is pursued.
- Encoding-none charmap: `freetype.inspect_charmaps` passed `2/2`; the remaining
  row uses a tracked encoding-none font that pinned C does not open.  Promotion
  requires a C-openable encoding-none fixture, not a fallback error route.
- CID/PFR: focused CID and PFR probes pass only non-CID/non-PFR or error rows.
  Success rows need real compact CID-keyed and PFR fixtures plus pure-Rust
  service implementations; fallback non-service errors are not parity for
  registry/ordering/supplement, CID-from-glyph, internal-CID status, metrics,
  advances, or kerning.
- Driver properties: hinting-engine and autohint property rows are zero-runnable
  until typed `FT_Property_Set`/`FT_Property_Get` routing exists and a glyph-load
  observation proves the property changes public metrics, outline, or bitmap
  output like pinned C.  A no-op accepting property values would be a green
  placeholder.

Next implementation order for this batch:

1. COLRv1 parser extension: add one format family at a time, starting with
   `PaintColrLayers`/`FT_Get_Paint_Layers` or `PaintTransform`, because these
   can reuse the existing opaque-paint graph and ABI test support.
2. BDF/PCF fixture path: add a maintained generator for compact PCF and OTB
   fixtures, then implement PCF/SFNT-BDF property routing in safe Rust.
3. LZW stream path: add a deterministic `.Z` fixture and implement the stream
   adapter behavior before promoting any LZW success row.

Do not promote any of the rows above by changing the audit allowlist alone.
Each row needs the same input bytes executed through pinned C, Rust FFI, thin
C ABI, and WASM ABI with exact public output comparison.

## Batch: COLRv1 PaintColrLayers payload route

Status: implemented 2026-07-21.

Scope:

- `ftcolor.FT_COLR_PAINTFORMAT_COLR_LAYERS.paint_colr_layers_payload`
- Operation `ftcolor.get_color_glyph_paint_graph`

Fix:

- Added a deterministic FontTools-generated fixture at
  `tests/fixtures/fonts/color/colr-v1-paint-colr-layers-cpal.ttf` through the
  maintained `font-fixture-color` target.
- Extended the safe Rust COLRv1 parser with PaintColrLayers and LayerV1List
  offset traversal.  FreeType 2.14.3 `src/sfnt/ttcolr.c:641-662` initializes
  `FT_PaintColrLayers.layer_iterator` from `NumLayers` and `FirstLayerIndex`;
  `src/sfnt/ttcolr.c:1518-1570` consumes that iterator by reading each layer
  paint offset relative to LayerV1List.  Rust now mirrors that public iterator
  state without exposing raw pointer values in comparisons.
- Added thin C ABI and WASM ABI forwarding for public `FT_Get_Paint_Layers`.
- Added a pinned-C oracle route comparing root lookup, `FT_Get_Paint`
  PaintColrLayers format, iterator initialization, layer iteration, returned
  opaque-paint classes, and terminal false behavior for the maintained fixture.

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_paint_graph
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
make fontdone-lint
git diff --check
```

Observed impact:

- Route audit: `pending-route` 289 → 288, `real-parity` 4674 → 4675.
- Focused runtime: compared 1 / total 1, passed 1, failed 0.

Remaining blockers in nearby operations:

- `ftcolor.get_paint_layers` still has three pending rows because they are tied
  to the broader absent `fonts/color/colr-v1-all-paints.ttf` fixture.  Do not
  count those rows until that declared all-paint fixture exists and proves
  zero/one/many layer traversal with the same C/Rust/C-ABI/WASM output.
- `ftcolor.get_paint` still has `FT_PaintColrLayers.get_paint_initializes_layer_iterator`
  pending for the same all-paint fixture surface.

## Batch: COLRv1 PaintColrGlyph recursive route

Status: implemented 2026-07-21.

Scope:

- `ftcolor.FT_COLR_PAINTFORMAT_COLR_GLYPH.paint_colr_glyph_runtime`
- Operation `ftcolor.get_paint_graph`

Fix:

- Added deterministic FontTools-generated fixtures through the maintained
  `font-fixture-color` target:
  - `tests/fixtures/fonts/color/colr-v1-colr-glyph-recursive.ttf`
  - `tests/fixtures/fonts/color/colr-v0-layer-control.ttf`
- Extended the safe Rust COLRv1 parser with PaintColrGlyph format 11.
  FreeType 2.14.3 `src/sfnt/ttcolr.c:706-711` reads this paint as the scalar
  `FT_PaintColrGlyph.glyphID`; `ftcolor.h` documents that clients resolve the
  referenced base glyph through `FT_Get_Color_Glyph_Paint`.  Rust now exposes
  the same scalar payload and graph snapshot behavior without moving recursive
  lookup logic into the thin C or WASM wrappers.
- Added a pinned-C oracle route comparing the root PaintColrGlyph payload, the
  referenced base glyph root paint, and the graph snapshot for the maintained
  recursive COLRv1 fixture across Rust FFI, C ABI, and WASM ABI.

Verification:

```bash
make -C pillow-rs-freetype font-fixture-color
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint_graph
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
make fontdone-lint
git diff --check
```

Observed impact:

- Route audit: `pending-route` 288 → 287, `real-parity` 4675 → 4676.
- Focused runtime: compared 22 / total 22, passed 22, failed 0.

Remaining nearby blocker:

- `ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values` remains pending
  because it is part of the broader absent
  `fonts/color/colr-v1-all-paints.ttf` fixture surface.  Do not count it until
  the all-paints fixture exists and proves its direct `FT_Get_Paint` output
  with the same C/Rust/C-ABI/WASM comparison.

## Batch: COLRv1 explicit transform paint routes

Status: implemented 2026-07-21.

Scope:

- Operation `ftcolor.get_normalized_transform_paint`
  - `ftcolor.FT_COLR_PAINTFORMAT_ROTATE.paint_rotate_normalized_payload`
  - `ftcolor.FT_COLR_PAINTFORMAT_SCALE.paint_scale_normalized_payload`
  - `ftcolor.FT_COLR_PAINTFORMAT_SKEW.paint_skew_normalized_payload`
  - `ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.explicit_transform_payload`
  - `ftcolor.FT_COLR_PAINTFORMAT_TRANSLATE.paint_translate_payload`
- Operation `ftcolor.get_color_glyph_paint_and_get_paint`
  - `ftcolor.FT_PaintRotate.get_paint_rotate_values`
  - `ftcolor.FT_PaintScale.get_paint_scale_values`
  - `ftcolor.FT_PaintSkew.get_paint_skew_values`
  - `ftcolor.FT_PaintTransform.get_paint_transform_values`
  - `ftcolor.FT_PaintTranslate.get_paint_translate_values`

Fix:

- Added one deterministic FontTools-generated fixture at
  `tests/fixtures/fonts/color/colr-v1-transform-paints.ttf` through the
  maintained `font-fixture-color` target.  The fixture covers explicit
  PaintTransform, PaintTranslate, non-uniform scale, centered scale, uniform
  scale, centered uniform scale, rotate, centered rotate, skew, and centered
  skew roots.
- Updated the normalized enum rows that previously referenced separate future
  per-format fixture names to use the shared maintained transform fixture
  instead of creating duplicate font files.
- Extended the safe Rust COLRv1 parser with non-variable transform formats
  12, 14, 16, 18, 20, 22, 24, 26, 28, and 30.  FreeType 2.14.3
  `src/sfnt/ttcolr.c:903-926` reads explicit Affine2x3 matrices as 16.16
  fixed-point fields; `:973-1079`, `:1081-1154`, and `:1158-1227` normalize
  the scale, rotate, and skew table families to public `FT_PaintScale`,
  `FT_PaintRotate`, and `FT_PaintSkew` records, zero-filling absent centers and
  duplicating uniform scale into `scale_y`.
- Extended the feature-gated COLRv1 graph snapshot with safe fixed-point
  payload values, avoiding unsafe union reads in the test harness while still
  comparing the same public C/Rust/C-ABI/WASM outputs.
- Added a pinned-C oracle route comparing root lookup, first `FT_Get_Paint`
  format, and recursive graph payload snapshots for the maintained transform
  fixture.

Verification:

```bash
make -C pillow-rs-freetype font-fixture-color
make -C pillow-rs-freetype test-op OP=ftcolor.get_normalized_transform_paint
make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_paint_and_get_paint
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
make fontdone-lint
git diff --check
```

Observed impact:

- Route audit: `pending-route` 287 → 277, `real-parity` 4676 → 4686.
- Focused normalized runtime: compared 5 / total 5, passed 5, failed 0.
- Focused direct-record runtime: compared 6 / total 6, passed 6, failed 0;
  the remaining two rows in that operation are unrelated radial/sweep gradient
  payload routes.

Remaining nearby blockers:

- `ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.included_root_transform_payload` and
  `FT_Get_Paint.success_inserts_root_transform` remain pending because they
  require FreeType's synthetic root-transform path from active size and
  `FT_Set_Transform` state, not explicit COLR table transform parsing.
- Gradient/colorline rows remain pending until maintained linear/radial/sweep
  gradient fixtures and `FT_Get_Colorline_Stops` traversal compare exact C
  colorline iterator state and stops.

## Batch: COLRv1 included root-transform synthesis routes

Status: implemented 2026-07-21.

Scope:

- Operation `ftcolor.get_color_glyph_paint_then_get_paint`
  - `ftcolor.FT_COLOR_INCLUDE_ROOT_TRANSFORM.include_transform_runtime`
  - `ftcolor.FT_COLOR_NO_ROOT_TRANSFORM.omit_transform_runtime`
  - `ftcolor.FT_Color_Root_Transform.root_transform_controls_initial_paint`
- Operation `ftcolor.get_root_transform_paint`
  - `ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.included_root_transform_payload`

Fix:

- Added the maintained fixture
  `tests/fixtures/fonts/color/colr-v1-root-transform.ttf` through
  `scripts/build_cpal_palette_fixtures.py` and the `font-fixture-color`
  target.  The fixture uses a real COLRv1 PaintGlyph root wrapping PaintSolid,
  so the synthetic top-level transform is not confused with an explicit root
  PaintTransform node.
- Implemented FreeType 2.14.3 `src/sfnt/ttcolr.c:1660-1715` behavior in
  pure Rust `FT_Get_Paint`: when `FT_OpaquePaint.insert_root_transform` is set,
  Rust now returns `FT_COLR_PAINTFORMAT_TRANSFORM`, scales active
  `FT_Size_Metrics` from 26.6 to 16.16 with C's rounding, multiplies the active
  `FT_Set_Transform` matrix, shifts the active transform delta by 10 bits, and
  clears `insert_root_transform` on the nested child paint.
- Added thin C ABI and WASM ABI `FT_Set_Transform` wrappers so the route can set
  the same active transform state through every public ABI leg.  The wrappers
  only validate/convert records and delegate to core; they contain no color
  parsing or parity behavior.
- Added safe ABI-test-support helpers for copying `FT_PaintTransform` payloads
  so Rust, C ABI, and WASM tests compare exact affine fields without unsafe
  union reads in the test harness.
- Updated the pinned C oracle and unified parity harness to apply the same
  pixel sizes and transforms, then compare root lookup, first `FT_Get_Paint`
  output, affine fields, nested opaque paint identity class, and no-root
  behavior exactly.

Verification:

```bash
make -C pillow-rs-freetype font-fixture-color
make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_paint_then_get_paint
make -C pillow-rs-freetype test-op OP=ftcolor.get_root_transform_paint
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
make fontdone-lint
git diff --check
```

Observed impact:

- Route audit: `pending-route` 266 → 258, `real-parity` 4697 → 4705.
- Focused root-transform runtime: compared 6 / total 6, passed 6, failed 0.
- Focused transform payload runtime: compared 2 / total 2, passed 2, failed 0.

Remaining nearby blockers:

- `ftcolor.FT_Get_Paint.success_inserts_root_transform` and
  `ftcolor.FT_Affine23.root_transform_values` still sit in the broader
  `fonts/color/colr-v1-all-paints.ttf` bucket.  They should only move when that
  shared all-paints fixture or an explicit manifest update gives those rows the
  same maintained root-transform route.

## Batch: COLRv1 all-paints direct FT_Get_Paint routes

Status: implemented 2026-07-21.

Scope:

- Operation `ftcolor.get_paint`
  - `ftcolor.FT_Get_Paint.success_resolves_each_supported_paint_format`
  - `ftcolor.FT_Get_Paint.success_inserts_root_transform`
  - `ftcolor.FT_Affine23.root_transform_values`
  - `ftcolor.FT_ColorStopIterator.initialized_by_get_paint`
  - `ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values`
  - `ftcolor.FT_PaintColrLayers.get_paint_initializes_layer_iterator`

Fix:

- Add one maintained generated fixture at
  `tests/fixtures/fonts/color/colr-v1-all-paints.ttf` rather than separate
  per-case duplicate font files.  It covers every currently supported COLRv1
  paint family needed by the direct `FT_Get_Paint` public-record rows:
  PaintColrLayers, PaintSolid, PaintGlyph, PaintColrGlyph, static
  linear/radial/sweep gradients, explicit transform/translate/scale/rotate/skew
  paints, composite paint, foreground solid, and a real root-transform target.
- Extend the pinned C oracle and unified Rust/C ABI/WASM parity route to compare
  direct root lookup, first `FT_Get_Paint` return and format, recursive paint
  nodes, initialized ColorLine iterators, initialized layer iterators, graph
  snapshots, and inserted root-transform affine payloads.

Verification:

```bash
make -C pillow-rs-freetype font-fixture-color
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
make fontdone-lint
git diff --check
```

Observed impact:

- Route audit: `pending-route` 258 → 254, `real-parity` 4705 → 4709.
- Focused runtime: compared 35 / total 35, passed 35, failed 0.

Remaining nearby blockers:

- `ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values` and
  `ftcolor.FT_PaintColrLayers.get_paint_initializes_layer_iterator` still have
  a declared `fonts/color/malformed-colr-v1-paints.ttf` malformed-asset input.
  They remain pending until that separate malformed fixture is generated and
  routed; the success all-paints fixture is now covered.

## Batch: COLRv1 direct gradient records and ColorIndex values

Status: implemented 2026-07-21.

Scope:

- Operation `ftcolor.get_color_glyph_paint_and_get_paint`
  - `ftcolor.FT_PaintRadialGradient.get_paint_radial_gradient_values`
  - `ftcolor.FT_PaintSweepGradient.get_paint_sweep_gradient_values`
- Operation `ftcolor.get_paint_and_colorline_stops`
  - `ftcolor.FT_ColorIndex.solid_and_color_stop_values`

Fix:

- Pointed the radial and sweep public-record inputs at the maintained
  `tests/fixtures/fonts/color/colr-v1-static-gradients.ttf` fixture instead of
  absent per-format duplicate fixture names.
- Reused the pinned C/Rust/C ABI/WASM static-gradient route to compare radial
  and sweep `FT_Get_Paint` public union fields plus attached ColorLine iterator
  state and stop traversal.
- Added an all-paints ColorIndex route comparing normal solid paint,
  foreground `0xFFFF` solid paint, and linear-gradient `FT_ColorStop.color`
  palette/alpha values through the same pinned C oracle and every ABI leg.
- Fixed the color-paint success dispatcher to include
  `ftcolor.get_paint_and_colorline_stops`; before this, routed ColorIndex rows
  fell through to `FT_Err_Unimplemented_Feature` in Rust/C ABI/WASM while the
  pinned C oracle was producing real output.

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_paint_and_get_paint
make -C pillow-rs-freetype test-case CASE=ftcolor.FT_ColorIndex.solid_and_color_stop_values
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint_and_colorline_stops
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
make fontdone-lint
git diff --check
```

Observed impact:

- Route audit: `pending-route` 254 → 251, `real-parity` 4709 → 4712.
- Focused direct-gradient runtime: compared 8 / total 8, passed 8, failed 0.
- Focused ColorIndex runtime: compared 1 / total 1, passed 1, failed 0.

Remaining nearby blocker:

- `ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_values` still
  declares unresolved variable-gradient and malformed-COLR assets.  Keep it
  pending until those separate inputs exist or the manifest is deliberately
  split into a static-success row and future variable/malformed rows.

### Route FT_Palette_Set_Foreground_Color default policy: 2026-07-22

Status: implemented as a real route promotion for the existing manifest row.

Scope:

- Promoted
  `ftcolor.FT_Palette_Set_Foreground_Color.default_foreground_color_policy`
  from pending-route to real-parity.
- Added pinned-C oracle, Rust FFI, C ABI, and WASM ABI runtime output for all
  CPAL palettes in `tests/fixtures/fonts/color/colr-v1-all-paints.ttf`.
- The route selects each palette without calling
  `FT_Palette_Set_Foreground_Color`, compares the selected palette flags, the
  C-compatible default BGRA foreground color, and the public COLR v1
  foreground `0xFFFF` `PaintSolid` reference.

C behavior pinned:

- FreeType 2.14.3 `src/sfnt/ttcolr.c:1834-1851` resolves COLR palette index
  `0xFFFF` to opaque white when the selected CPAL palette has
  `FT_PALETTE_FOR_DARK_BACKGROUND`; otherwise it resolves to opaque black.

Observed impact:

- Route audit: `pending-route` 238 → 237, `real-parity` 4729 → 4730.
- Focused runtime:
  `make -C pillow-rs-freetype test-op OP=ftcolor.palette_set_foreground_color`
  passed 4/4 across Rust FFI, C ABI, and WASM ABI.

Remaining nearby blocker:

- `ftcolor.FT_Palette_Set_Foreground_Color.error_color_layers_disabled`
  remains pending because this build does not provide a maintained same-input
  runtime condition with `TT_CONFIG_OPTION_COLOR_LAYERS` disabled. Promoting it
  in the enabled-color build would be a green placeholder.

### Route combined FT_LayerIterator v0/v1 API parity: 2026-07-22

Status: implemented as a real route promotion for the existing broad
`FT_LayerIterator` manifest row.

Scope:

- Promoted
  `ftcolor.FT_LayerIterator.initialized_and_advanced_by_layer_apis` from
  pending-route to real-parity.
- Expanded the row output so it no longer proves only the COLR v1
  `FT_Get_Paint_Layers` iterator path.  It now compares:
  - COLR v0 `FT_Get_Color_Glyph_Layer` calls for base glyph 36.
  - COLR v1 `FT_Get_Paint_Layers` calls for base glyphs 36 and 37.
- The route compares public `FT_LayerIterator` fields (`num_layers`, `layer`,
  pointer class) plus call return values and public payload fields across the
  pinned C oracle, Rust FFI, C ABI, and WASM ABI.

Why this was not counted earlier:

- Previous route support for this broad row emitted only the COLR v1 paint-layer
  sequences.  The manifest row explicitly declares both `colr_v0_layers` and
  `colr_v1_layers_many`; counting v1-only output would have been a green
  placeholder.

Observed impact:

- Route audit: `pending-route` 237 → 236, `real-parity` 4730 → 4731.
- Focused runtime:
  `make -C pillow-rs-freetype test-case CASE=ftcolor.FT_LayerIterator.initialized_and_advanced_by_layer_apis`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

### Split static FT_PaintLinearGradient public-record row: 2026-07-22

Status: implemented as an additive split row; the original broad row remains
pending.

Scope:

- Added `ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_static_values`
  for the maintained `tests/fixtures/fonts/color/colr-v1-static-gradients.ttf`
  fixture.
- Reused the existing pinned-C/Rust/C ABI/WASM static-gradient route for glyph
  36 (`linear_pad`), comparing `FT_Get_Color_Glyph_Paint`, `FT_Get_Paint`,
  the active `FT_COLR_Paint.u.linear_gradient` public union payload, attached
  `FT_ColorLine`, graph snapshot, and `FT_Get_Colorline_Stops` iteration.

Why this is split instead of promoting the older broad row:

- `ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_values` declares
  static, variable, and malformed COLR inputs.  The malformed fixture
  `fonts/color/malformed-colr-v1-paints.ttf` is not present, and the broad row
  also requires non-default variable-gradient interpolation proof.  Counting a
  single static glyph as that full row would be a green placeholder.
- The split row is narrower by design and names exactly the input it proves.

Observed impact:

- Route audit: `concrete_cases` 7238 → 7239, `real-parity` 4721 → 4722,
  `pending-route` remains 242.
- Focused runtime:
  `make -C pillow-rs-freetype test-case CASE=ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_static_values`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

### Split variable FT_PaintLinearGradient public-record row: 2026-07-22

Status: implemented as an additive split row; the original broad row remains
pending.

Scope:

- Added
  `ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_variable_values`
  for the maintained
  `tests/fixtures/fonts/color/colr-v1-variable-gradients.ttf` fixture.
- Reused the existing pinned-C/Rust/C ABI/WASM variable-gradient route,
  comparing default and `wght=900, GRAD=1` design-coordinate runs for
  `FT_Get_Color_Glyph_Paint`, `FT_Get_Paint`, the active
  `FT_COLR_Paint.u.linear_gradient` public union payload, attached
  `FT_ColorLine`, and `FT_Get_Colorline_Stops` iteration.

Why this is split instead of promoting the older broad row:

- `ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_values` still
  declares the unresolved malformed COLR input
  `fonts/color/malformed-colr-v1-paints.ttf`.  Promoting it based on the
  maintained static and variable fixtures would hide that missing same-input
  malformed route.
- The split row names exactly the variable-gradient input it proves.

### FTGlyph bitmap/SVG false-green route correction: 2026-07-22

Status: route classification corrected; no parity row promoted.

Scope:

- Moved seven `ftglyph` rows out of `real-parity` because the maintained
  runners did not prove the manifest-declared bitmap/SVG glyph payloads:
  - `ftglyph.FT_Get_Glyph.success_bitmap_slot_deep_copy`
  - `ftglyph.FT_Get_Glyph.success_svg_slot_deep_copy`
  - `ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap` (two
    concrete variants)
  - `ftglyph.FT_Glyph_Copy.success_bitmap_copy_is_independent`
  - `ftglyph.FT_Glyph_Copy.success_svg_copy_is_independent`
  - `ftglyph.FT_SvgGlyphRec.fields_match_svg_get_copy_transform`
- The focused bitmap `FT_Get_Glyph` row previously passed while the oracle
  output showed format `1869968492` (`FT_GLYPH_FORMAT_OUTLINE`), not
  `FT_GLYPH_FORMAT_BITMAP`.  That proves the route was observing a glyph-slot
  root snapshot, not an actual `FT_BitmapGlyphRec` deep-copy payload.

Why this must stay pending:

- The manifest rows require exact public `FT_BitmapGlyphRec` or
  `FT_SvgGlyphRec` fields: root, left/top, bitmap descriptor, buffer bytes,
  document bytes, metrics, glyph range, transform, delta, and ownership or copy
  independence where applicable.
- Counting slot `format`/advance equality or a non-null handle would be a green
  placeholder.  The correct next batch is to implement real bitmap glyph object
  creation/copy/record inspection through `FT_Get_Glyph` and
  `FT_Glyph_To_Bitmap`, then add the SVG route or explicit pinned-C unsupported
  classification.

Observed impact:

- Route audit on `main` at `ad4cc6453` before this correction:
  `concrete_cases=7242`, `pending-route=234`, `real-parity=4733`.
- Route audit after this correction:
  `concrete_cases=7242`, `pending-route=241`, `real-parity=4726`.
- Focused confirmation:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Get_Glyph.success_bitmap_slot`
  previously passed 1/1 but the oracle cache output was an outline glyph root
  (`format=1869968492`), so it is no longer accepted as real bitmap-glyph
  parity.

### FT_Get_Glyph real bitmap glyph object route: 2026-07-22

Status: implemented for the narrow bitmap `FT_Get_Glyph` row.

Scope:

- Retargeted the shared logical bitmap-strike fixture
  `fonts/bitmap-strikes/public-bitmap-strike.ttf` to the maintained
  `fixtures/assets/fonts/sbit_gray_format1.ttf`, which has a 20 ppem EBLC/EBDT
  strike and glyph 1 bitmap coverage.
- Resolved the semantic test selector `glyph_index: "bitmap_glyph"` to glyph 1
  instead of the previous placeholder glyph 0.
- Added a safe Rust `FT_BitmapGlyphOwned` model plus thin C ABI and WASM ABI
  owned bitmap glyph records.  The wrappers now allocate/free bitmap glyph
  records around the core-owned bitmap payload and expose test-only ABI
  snapshots; they do not parse fonts or implement glyph logic.
- Extended the pinned-C oracle's `FT_Get_Glyph` record output so bitmap glyphs
  include the public `FT_BitmapGlyphRec` payload.

Proven row:

- `ftglyph.FT_Get_Glyph.success_bitmap_slot_deep_copy@gbitmap_glyph`

Pinned C behavior checked:

- The focused oracle now returns `FT_GLYPH_FORMAT_BITMAP` (`1651078259`) with
  exact public bitmap payload:
  `width=2`, `rows=2`, `pitch=2`, `pixel_mode=2`, `num_grays=256`,
  `left=1`, `top=2`, `buffer_hex=1180c0ff`.
- Rust FFI, thin C ABI, and WASM ABI now match that same output.

Observed impact:

- Route audit: `pending-route` 241 → 240, `real-parity` 4726 → 4727.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Get_Glyph.success_bitmap_slot`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

Rows deliberately left pending in the same surface:

- `ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap` still
  declares both `FT_Get_Glyph bitmap` and `FT_Glyph_To_Bitmap outline`
  creation paths; the new route proves only the former.
- `ftglyph.FT_Glyph_Copy.success_bitmap_copy_is_independent` still needs a
  real bitmap glyph copy route proving target independence after source
  destruction.
- SVG rows still need an SVG-enabled fixture route or exact unsupported
  classification against pinned C.

### FT_Glyph_Copy owned outline dispatch route: 2026-07-22

Status: implemented for the current concrete
`ftglyph.FT_Glyph_Copy.success_bitmap_copy_is_independent` input.

Scope:

- Added a safe Rust `FT_Outline_Glyph_Copy` helper that clones the owned root
  record and outline arrays, matching FreeType `src/base/ftglyph.c:542-574`
  class-copy behavior without sharing the source object.
- Routed thin C ABI `FT_Glyph_Copy` and WASM `fontdone_wasm_glyph_copy`
  through owned outline glyph copies before the generic unimplemented
  fallback.  Bitmap owned glyph copy support remains present, but is not what
  this concrete input currently exercises.
- Updated the runtime parity harness so `ftglyph.glyph_copy` rows call the
  actual Rust/C/WASM copy endpoints and snapshot the copied glyph root instead
  of reporting the loaded slot.

Pinned C behavior checked:

- Despite the manifest row name, the maintained concrete input passes
  `glyph_index=0` for `fonts/bitmap-strikes/public-bitmap-strike.ttf`; pinned
  FreeType returns `FT_GLYPH_FORMAT_OUTLINE` (`1869968492`) with root advance
  `917504`, not an `FT_BitmapGlyphRec` bitmap payload.
- The promoted route therefore proves owned outline `FT_Glyph_Copy` dispatch
  for this concrete input.  It does not prove the manifest's intended bitmap
  copy semantics.

Observed impact:

- Route audit: `pending-route` 240 → 239, `real-parity` 4727 → 4728.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_Copy.success_bitmap_copy`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

Rows deliberately left pending in the same surface:

- A true bitmap `FT_Glyph_Copy` row still needs a concrete bitmap glyph input
  such as `glyph_index: "bitmap_glyph"` or another maintained bitmap-copy
  fixture route.  The current row name is misleading relative to its oracle
  payload.
- `ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap` remains
  pending because it also declares `FT_Glyph_To_Bitmap outline`.
- SVG copy/record rows remain pending until SVG fixture support or exact
  unsupported classification is implemented against pinned C.

### FT_OutlineGlyph public alias route: 2026-07-22

Status: implemented for `ftglyph.FT_OutlineGlyph.pointer_alias_matches_record`.

Scope:

- Added a maintained `ftglyph.type_runtime` route for the outline alias row.
- The route uses the existing pinned-C `--glyph-transform` no-op path because
  it creates a real `FT_Glyph` through `FT_Get_Glyph`, casts it to
  `FT_OutlineGlyph`, and prints the public `FT_OutlineGlyphRec` payload:
  outline arrays, root advance, CBox, status, and mutation class.
- Rust FFI, C ABI, and WASM ABI now execute the same operation shape: open the
  declared outline font, load outline glyph 36 when the row does not declare a
  more specific glyph index, create the detached outline glyph, snapshot the
  cast record, and destroy the owned glyph handle.

Pinned C behavior checked:

- For the declared `input/fonts/DejaVuSans.ttf` outline font at 20 ppem,
  glyph 36 produces a real `FT_GLYPH_FORMAT_OUTLINE` owned glyph whose
  `FT_OutlineGlyph` cast exposes the same public outline record fields that
  the transform route already compares.
- This route proves the outline alias only.  It does not prove bitmap or SVG
  alias rows, which have their own pending manifest cases.

Observed impact:

- Route audit: `pending-route` 239 → 238, `real-parity` 4728 → 4729.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_OutlineGlyph.pointer_alias_matches_record`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

Rows deliberately left pending in the same surface:

- `ftglyph.FT_SvgGlyph.pointer_alias_matches_record_when_enabled` still needs
  SVG-enabled fixture support or exact unsupported classification.
- `ftglyph.FT_GlyphRec.clazz_is_private_identity_only` and
  `ftglyph.FT_Glyph_Class.opaque_class_identity_only` require broader
  outline/bitmap/SVG public-behavior class classification and were not
  promoted by this outline-only route.

### FT_BitmapGlyph public alias route: 2026-07-22

Status: implemented for `ftglyph.FT_BitmapGlyph.pointer_alias_matches_record`.

Scope:

- Extended the maintained `ftglyph.type_runtime` route to branch on
  `format_filter=FT_GLYPH_FORMAT_BITMAP`.
- The bitmap branch explicitly selects the declared `bitmap_strike_font`
  instead of the row's `outline_font`, preventing accidental fallback to an
  outline-only font.
- The route uses the pinned-C `--glyph-record` path with glyph 1 from
  `fonts/bitmap-strikes/public-bitmap-strike.ttf`, then compares the cast
  `FT_BitmapGlyphRec` public payload: root format and advance, left/top,
  bitmap descriptor, and bitmap buffer bytes.
- Rust FFI, C ABI, and WASM ABI reuse the real owned bitmap glyph route added
  for `FT_Get_Glyph`, with named-font face setup matching the oracle's size
  setup.

Pinned C behavior checked:

- For the maintained bitmap strike font at 20 ppem, glyph 1 produces
  `FT_GLYPH_FORMAT_BITMAP` (`1651078259`) and exposes:
  `width=2`, `rows=2`, `pitch=2`, `pixel_mode=2`, `num_grays=256`,
  `left=1`, `top=2`, `buffer_hex=1180c0ff`.
- This route proves the bitmap alias row only.  It does not prove the broader
  `FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap` row because that
  row also declares `FT_Glyph_To_Bitmap outline`.

Observed impact:

- Route audit: `pending-route` 238 → 237, `real-parity` 4729 → 4730.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_BitmapGlyph.pointer_alias_matches_record`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

Rows deliberately left pending in the same surface:

- `ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap` remains
  pending until the `FT_Glyph_To_Bitmap outline` creation path is implemented
  and compared for the same public record fields.
- `ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer` still needs an ownership and
  release-event route, not only field equality before destruction.
- SVG alias/record rows still need SVG fixture support or exact unsupported
  classification against pinned C.

### Split FT_BitmapGlyphRec FT_Get_Glyph bitmap record row: 2026-07-22

Status: implemented as an additive split row; the original broad row remains
pending.

Scope:

- Added `ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_bitmap`.
- The split row names exactly the maintained input it proves:
  `FT_Get_Glyph bitmap` on
  `fonts/bitmap-strikes/public-bitmap-strike.ttf` with
  `glyph_index: "bitmap_glyph"`.
- Reused the existing real bitmap glyph record route to compare the public
  `FT_BitmapGlyphRec` payload across pinned C, Rust FFI, thin C ABI, and WASM
  ABI: root format and advance, left/top, bitmap descriptor, and bitmap buffer
  bytes.

Why this is split instead of promoting the older broad row:

- `ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap` declares
  both `FT_Get_Glyph bitmap` and `FT_Glyph_To_Bitmap outline` creation paths.
- The new split row proves the `FT_Get_Glyph bitmap` half only.  Counting it as
  the full broad row would hide the unresolved `FT_Glyph_To_Bitmap outline`
  path.

Observed impact:

- Route audit: `concrete_cases` 7242 → 7243, `real-parity` 4730 → 4731,
  `pending-route` remains 237 because the original broad row is still pending.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_bitmap`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

Rows deliberately left pending in the same surface:

- `ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap` remains
  pending for the `FT_Glyph_To_Bitmap outline` path.
- `ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer` still needs a destruction and
  ownership-event route.

### Split FT_BitmapGlyphRec FT_Get_Glyph bitmap ownership row: 2026-07-22

Status: implemented as an additive split row; the original broad ownership row
remains pending.

Scope:

- Added `ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer_get_glyph_bitmap`.
- Added a pinned-C `--done-glyph-bitmap` oracle command that creates a real
  `FT_BitmapGlyph` through `FT_Get_Glyph`, records the bitmap glyph payload
  before destruction, then calls `FT_Done_Glyph` exactly once.
- Added Rust FFI, C ABI, and WASM ABI runners that perform the same operation
  on the maintained bitmap strike font/glyph and compare:
  created glyph nullness, creation error, format before done, bitmap buffer
  owner class, bitmap descriptor before done, and public release event string.

Why this is split instead of promoting the older broad row:

- `ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer` declares both
  `FT_Get_Glyph bitmap` and `FT_Glyph_To_Bitmap outline` creation paths.
- The split row proves only the `FT_Get_Glyph bitmap` ownership path.  Counting
  it as the broad row would hide the unresolved `FT_Glyph_To_Bitmap outline`
  ownership path.

Observed impact:

- Route audit: `concrete_cases` 7243 → 7244, `real-parity` 4731 → 4732,
  `pending-route` remains 237 because the original broad row is still pending.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer_get_glyph_bitmap`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

Rows deliberately left pending in the same surface:

- `ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer` remains pending for the
  `FT_Glyph_To_Bitmap outline` ownership path.
- `ftglyph.FT_Done_Glyph.success_releases_owned_glyph` and
  `ftglyph.FT_Glyph.caller_owned_lifetime` still require broader
  outline/bitmap/SVG and creation-path lifecycle matrices.

### Route FT_Glyph_To_Bitmap through owned C/WASM glyph handles: 2026-07-22

Status: implemented as a real ABI-path strengthening; no broad pending row was
promoted.

Scope:

- Added a core `FT_Outline_Glyph_To_Bitmap` helper that renders an owned
  `FT_OutlineGlyphOwned` into an owned `FT_BitmapGlyphOwned`.
- Updated the C ABI `FT_Glyph_To_Bitmap` to recognize crate-owned outline and
  bitmap glyph handles:
  - bitmap glyph input returns success without replacement, matching FreeType
    `src/base/ftglyph.c:794-795`;
  - outline glyph input allocates a bitmap glyph, copies root advance and
    rendered bitmap payload, replaces the caller handle, and frees the original
    only when `destroy != 0`, matching FreeType `src/base/ftglyph.c:809-869`.
- Added the equivalent WASM handle entry point
  `fontdone_wasm_glyph_to_bitmap_handle`.
- Updated the focused `ftglyph.glyph_to_bitmap` Rust/C/WASM runners so success
  cases exercise the real owned-glyph conversion path rather than a slot-render
  surrogate.

Why the broad lifecycle rows remain pending:

- `ftglyph.FT_Done_Glyph.success_releases_owned_glyph` still declares outline,
  bitmap, optional SVG, `FT_Get_Glyph`, and `FT_Glyph_To_Bitmap` lifecycle
  coverage plus free-event sequencing.  This change proves the maintained
  outline-to-bitmap conversion route, but does not yet provide SVG
  classification or allocation/failure facade coverage.
- `ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer` still needs a combined
  ownership/free-event route for both `FT_Get_Glyph bitmap` and
  `FT_Glyph_To_Bitmap outline`.  Existing record-field checks now exercise the
  real conversion path, but the broad ownership row should remain visible until
  the lifecycle/free-event route is complete.

Observed impact:

- Focused verification:
  - `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap.success_outline_to_bitmap_destroy_false`
    passed 4/4 across Rust FFI, C ABI, and WASM ABI.
  - `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap.success_outline_to_bitmap_destroy_true`
    passed 2/2 across Rust FFI, C ABI, and WASM ABI.
  - `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap.error_invalid_arguments_or_unrenderable_format`
    passed 1/1 across Rust FFI, C ABI, and WASM ABI.
  - `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap`
    passed 2/2 across Rust FFI, C ABI, and WASM ABI.

### Promote FT_BitmapGlyphRec broad bitmap-buffer ownership row: 2026-07-22

Status: implemented as the declared broad ownership row; broader
`FT_Done_Glyph` lifecycle rows remain pending.

Scope:

- Promoted `ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer`.
- Added a pinned-C `--done-bitmap-glyph-paths` oracle command that creates two
  real bitmap glyphs:
  - `FT_Get_Glyph bitmap` from the maintained bitmap strike fixture;
  - `FT_Glyph_To_Bitmap outline` from the maintained outline fixture.
- For both creation paths, the oracle records the public bitmap glyph state
  before destruction and then calls `FT_Done_Glyph` once.
- Added matching Rust FFI, thin C ABI, and WASM ABI runners.  The C/WASM
  outline path uses the real owned `FT_Glyph_To_Bitmap` handle conversion, not
  a slot-render surrogate.
- The broad row's fixture omits explicit indices and size, so the route uses
  existing maintained harness defaults already used by the record-path route:
  face index `0`, pixel size `0x20`, bitmap glyph index `1`, outline glyph
  index fallback `0`, and normal render mode.

Why adjacent rows remain pending:

- `ftglyph.FT_Done_Glyph.success_releases_owned_glyph` still declares outline,
  bitmap, optional SVG, multiple creation paths, allocation/free-event
  sequencing, and malformed/allocation facades.  This promoted row proves
  bitmap-glyph buffer ownership and release for the two maintained bitmap
  creation paths only.
- `ftglyph.FT_Done_Glyph.lifetime_before_library_done` still needs explicit
  glyph/library ordering and stale-handle/invalid-use classification.

Observed impact:

- Route audit: `real-parity` 4759 → 4760, `pending-route` 228 → 227.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer`
  passed 2/2 across Rust FFI, C ABI, and WASM ABI.

### Split FT_Stream_OpenBzip2 disabled-build precedence rows: 2026-07-22

Status: implemented as additive active-build split rows; the enabled-build
stream validation rows remain pending.

Scope:

- Added
  `ftbzip2.FT_Stream_OpenBzip2.disabled_build_precedes_null_validation`.
- Added
  `ftbzip2.FT_Stream_OpenBzip2.disabled_build_precedes_header_validation`.
- Reused the maintained pinned-C `--bzip2-stream-disabled-policy` oracle and
  the Rust FFI, thin C ABI, and WASM ABI `FT_Stream_OpenBzip2` routes to
  compare the active build's public behavior: `build_features.bzip2=false`,
  `FT_Err_Unimplemented_Feature`, and untouched target stream pointer classes.

Why this is split instead of promoting the older enabled-build rows:

- The pinned FreeType 2.14.3 oracle build currently excludes bzip2 support.
  In this build, `FT_Stream_OpenBzip2` returns `Unimplemented_Feature` before
  null target/source validation or invalid/truncated header reads.
- The existing `error_null_stream_or_source` and
  `error_invalid_or_truncated_bzip2_header` rows describe enabled-build
  validation behavior. Promoting those rows with disabled-build output would be
  false parity.

Observed impact:

- Route audit: `concrete_cases` 7244 → 7246, `real-parity` 4732 → 4734,
  `pending-route` remains 237 because the enabled-build rows remain pending.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftbzip2.FT_Stream_OpenBzip2.disabled_build_precedes`
  passed 2/2 across Rust FFI, C ABI, and WASM ABI.

Rows deliberately left pending in the same surface:

- `ftbzip2.FT_Stream_OpenBzip2.error_null_stream_or_source` remains pending for
  bzip2-enabled null validation.
- `ftbzip2.FT_Stream_OpenBzip2.error_invalid_or_truncated_bzip2_header`
  remains pending for bzip2-enabled header validation.
- `success_open_valid_bzip2_stream`, `success_read_decompressed_bytes`, and
  `lifecycle_close_does_not_close_source` still require a pure-Rust bzip2
  stream wrapper or a maintained bzip2-enabled pinned oracle profile.

### Split FTC_Node_Unref null-input row: 2026-07-22

Status: implemented as an additive null-only split row; the original mixed
null/foreign row remains pending.

Scope:

- Added `ftcache.FTC_Node_Unref.null_inputs_noop`.
- Added a direct core `FTC_Node_Unref` facade for the public null-node no-op
  path and exposed it through the thin C ABI and WASM ABI.
- Added a pinned-C `--cache-node-unref-null-only` oracle command that calls
  `FTC_Node_Unref(NULL, NULL)` and compares the void/no-side-effect output
  through Rust FFI, C ABI, and WASM ABI.

Why this is split instead of promoting the older broad row:

- `ftcache.FTC_Node_Unref.null_or_invalid_inputs_noop` also declares a
  non-null `foreign_or_bad_cache_index` node with a live manager.
- Pinned FreeType `src/cache/ftcmanag.c:667-677` returns immediately only when
  `node == NULL`; for non-null nodes it reads `node->cache_index` and requires
  a maintained cache-node/manager layout facade.
- The current split proves only the direct public `(node=NULL, manager=NULL)`
  no-op, which is the same input across pinned C, Rust FFI, thin C ABI, and
  WASM ABI.

Observed impact:

- Route audit: `concrete_cases` 7246 → 7247, `real-parity` 4734 → 4735,
  `pending-route` remains 237 because the original mixed row remains pending.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=ftcache.FTC_Node_Unref.null_inputs_noop`
  passed 1/1 across Rust FFI, C ABI, and WASM ABI.

Rows deliberately left pending in the same surface:

- `ftcache.FTC_Node_Unref.null_or_invalid_inputs_noop` remains pending for the
  live-manager and foreign/bad-cache-index node variants.

### Pending surface audit after CFF OpenType validate split: 2026-07-22

Baseline:

```text
git rev-parse --short HEAD
cd84dcf11

make -C pillow-rs-freetype route-audit
route audit concrete_cases=7260 category_counts={'compile-contract': 2266, 'pending-route': 237, 'real-null-validation': 9, 'real-parity': 4748}

make fontdone-test
runtime_parity: passed=7018 failed=0 total=7018
runtime_cases: runnable=7018 pending=242
```

Current pending-route blocker shape from
`python3 scripts/report_pending_route_buckets.py` and direct
`route_audit.json` inspection:

| Blocker class | Rows | Decision |
| --- | ---: | --- |
| Missing parser/service/route surface | 62+ | Do not promote by adding scalar/no-op rows. Implement as subsystem batches: PFR metrics/advance/kerning service, CID metadata/glyph mapping, PCF/SFNT-BDF property services, AFM attach/track kerning, glyph object lifetime, and stroker geometry. |
| No maintained runtime-resolved input | 34 | Do not count semantic rows. Add exact same-input fixture and route first, then compare pinned C, Rust FFI, thin C ABI, and WASM ABI output. |
| Missing fixture path | 24 | Search or generate license-compatible fixtures, but do not add files unless the current Rust implementation can actually expose the declared public output. |
| Active pinned build disables the feature | 7 | Keep enabled-build rows pending or split an explicit active-build row. Do not count `Unimplemented_Feature` as enabled-build validation parity. |
| Pinned C crash or fixture mismatch | remaining mixed rows | Keep pending unless a maintained subprocess/crash-classification route is designed. Do not strengthen Rust behavior and call it C parity. |

Checked candidate fixture sources:

- `twardoch/test-fonts` publishes `totopfr` PFR test fonts under Apache-2.0,
  based on Noto Sans.  This is a viable provenance source for future PFR
  fixture work, but the current Rust core has no PFR face parser or
  `PFR_METRICS` service.  Dropping in `basic-metrics.pfr` would leave Rust
  unable to produce `FT_Get_PFR_Metrics` / `FT_Get_PFR_Advance` success output,
  so the PFR success rows remain pending.
- `adobe-fonts/fdarray-test` publishes OFL-licensed CID-keyed OpenType/CFF
  fonts using the Adobe-Identity-0 ROS.  This is a viable candidate source for
  SFNT-wrapped CID fixture research, but the current Rust core has no CID
  public API implementation and the CFF parser is explicitly scoped to
  non-CID CFF.  The CID success/null-output rows remain pending until CID
  metadata, ROS, and glyph-index mapping are implemented.
- The existing repo has generated BDF fixtures and a BDF property route, but
  no PCF property parser and no SFNT-BDF/OTB fixture.  The PCF/SFNT-BDF rows
  must remain pending until a deterministic generator or license-reviewed asset
  exists and the Rust service reads PCF/SFNT-BDF properties exactly like pinned
  C.
- The repo has `input/fonts/type1/attach-afm-base.pfb`, but no matching
  `input/aux/type1/attach-afm-base.afm` and no maintained attach/track-kerning
  service.  `FT_Attach_File`, `FT_Attach_Stream`, and
  `FT_Get_Track_Kerning.type1_afm_track_kerning_success` remain pending until
  AFM parsing and post-attach observable kerning/track-kerning output are
  implemented.

Next high-value implementation batches:

1. **PCF/SFNT-BDF property batch**
   - Add deterministic tiny PCF and SFNT-BDF/OTB fixtures with provenance.
   - Extend safe Rust font parsing to expose PCF signed properties and
     selected-strike SFNT-BDF properties/charset strings.
   - Route `FT_Get_BDF_Property` and `FT_Get_BDF_Charset_ID` success variants
     through Rust FFI, C ABI, WASM, and pinned C.

2. **CID CFF batch**
   - Select or generate compact CID-keyed fixtures; imported candidates need
     license/provenance review and should be subset or generated when possible.
   - Implement CID-aware CFF metadata: ROS, supplement, internal-CID flag, and
     glyph-index-to-CID mapping.
   - Add public `ftcid` Rust FFI, thin C ABI, WASM ABI, and oracle routes.

3. **PFR service batch**
   - Generate or import a compact PFR with known metrics, advances, and kerning.
   - Implement safe Rust PFR parsing sufficient for `PFR_METRICS`.
   - Route `FT_Get_PFR_Metrics`, `FT_Get_PFR_Advance`, and PFR kerning success
     rows together; keep the existing non-PFR fallback/error rows unchanged.

4. **Type1 AFM attach batch**
   - Add a reproducible AFM generator for the existing Type1 attach font, or
     regenerate both PFB and AFM together.
   - Implement AFM attachment state and `FT_Get_Track_Kerning` behavior in
     core, then expose through thin C/WASM wrappers.
   - Compare attach return code plus post-attach kerning/track-kerning deltas.

Rejected quick fixes in this audit:

- Reclassifying PFR/CID/PCF/AFM rows based only on a found internet fixture.
  Same-input parity requires the Rust implementation to produce the same public
  output, not just for pinned C to open the file.
- Adding broad matrix rows that duplicate already-routed Type1/CFF/null cases.
  The remaining broad rows name CFF2, CID, Type42, CFF-without-glyph-names, or
  other still-missing obligations.
- Counting active-build `Unimplemented_Feature` as enabled-build bzip2,
  OpenType validator, or SVG behavior.  Those rows intentionally remain
  pending unless split by build configuration.
