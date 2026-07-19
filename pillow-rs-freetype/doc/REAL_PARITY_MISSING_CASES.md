# Real-Parity Missing Cases

## 2026-07-18 Parity-Only Fix Plan

Current branch: `ftmm-route-audit-placeholder-parity`.

Current non-coverage parity command:

```bash
make -C pillow-rs-freetype test
```

Current verified result after face/table and Smooth module public API exact classification:

- Runnable public parity rows: `7144 / 7144` pass.
- Pending runtime rows: `90`.
- Route audit concrete rows: `7234`.
- Route audit categories:
  - `real-parity`: `4147`
  - `real-null-validation`: `8`
  - `raw-slot-null-validation`: `4`
  - `wrapper-null-validation`: `1`
  - `compile-contract`: `2229`
  - `generic-fallback`: `696`
  - `generic-error-fallback`: `48`
  - `pending-route`: `82`
  - `pending-core`: `7`
  - `null-error-fallback`: `6`
  - `explicit-unsupported`: `6`
  - `void-fallback`: removed

Parity-only rule for this phase:

- Do not use coverage targets or Coverage MCP to choose work.
- Do not use `make -C pillow-rs-freetype real-parity-verify` for this phase
  unless it remains parity-only; this target now expands to
  `test-unified-fixtures`, route audit, FFI gates, fmt, and clippy, not
  `cargo llvm-cov`.
- Do not delete tests because they are pending or fallback.
- Do not add green placeholder rows.
- A row is fixed only when the same public input has exact output agreement
  against pinned C FreeType through Rust FFI, thin C ABI, and WASM ABI, or when
  the row is explicitly documented as a real unsupported/pending public surface.

### Issue Set Current: batched public API exact error routes

Previous blocker:

- Ten concrete public error rows for face opening, stream/frame errors,
  module properties, and variation axis flags were classified as
  `generic-error-fallback`.
- Each row already executed through the pinned C oracle, Rust FFI, thin C ABI,
  and WASM ABI, but fallback classification accepted any error instead of
  requiring exact public status/output parity for the same input.

Promoted rows:

- `fterrdef.FT_Err_Cannot_Open_Resource.missing_path_returns_error`
- `fterrdef.FT_Err_Cannot_Open_Stream.zero_length_file_returns_error`
- `fterrdef.FT_Err_Invalid_Stream_Operation.stream_operation_failure`
- `fterrdef.FT_Err_Invalid_Stream_Seek.stream_seek_failure`
- `ftdriver.FT_Prop_GlyphToScriptMap.invalid_face_error_matches_c`
- `fterrdef.FT_Err_Missing_Property.driver_property_unknown_name`
- `fterrdef.FT_Err_Invalid_Stream_Handle.null_stream_rejected`
- `fterrdef.FT_Err_Invalid_Frame_Operation.stream_frame_access_rejects_invalid_sequence`
- `ftmm.FT_Get_Var_Axis_Flags.out_of_range_axis_error`
- `ftmm.FT_Get_Var_Axis_Flags.null_master_or_flags_error`

Verified progress:

- Exact comparison passed for all ten rows after promotion.
- No runtime behavior change was needed; the existing pure-Rust implementation,
  C ABI, and WASM ABI outputs already matched pinned C FreeType once the
  fallback guard was removed.
- Route audit classifies all ten rows as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Cannot_Open_Resource.missing_path_returns_error
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `4079`, `generic-error-fallback` `116`.

### Issue Set Current: second batched public API exact error routes

Previous blocker:

- Singleton public error rows across module properties, MM descriptors, glyph
  object lifecycle, LCD filter APIs, SFNT name/table APIs, request sizing,
  stream frame lifecycle, and glyph dictionary lookup were still classified as
  `generic-error-fallback`.
- The rows had runnable pinned C, Rust FFI, thin C ABI, and WASM ABI coverage,
  but fallback classification did not require exact public status/output
  equality.

Promoted rows:

- `ftdriver.FT_Prop_IncreaseXHeight.invalid_face_error_matches_c`
- `ftmm.FT_Get_Multi_Master.null_output_error`
- `ftcolor.FT_Palette_Select.error_color_layers_disabled`
- `fterrdef.FT_Err_Invalid_Handle.generic_object_handle_validation`
- `ftlcdfil.FT_Library_SetLcdFilter.unimplemented_without_subpixel_filtering`
- `ftlcdfil.FT_Library_SetLcdFilterWeights.unimplemented_without_subpixel_filtering`
- `tttables.FT_Sfnt_Table_Info.invalid_face_error`
- `ftsnames.FT_Get_Sfnt_LangTag.format0_invalid_table_error`
- `fterrdef.FT_Err_Invalid_Pixel_Size.bitmap_strike_mismatch`
- `fterrdef.FT_Err_Invalid_Driver_Handle.module_driver_handle_validation`
- `fterrdef.FT_Err_Nested_Frame_Access.stream_nested_frame_guard`
- `fterrdef.FT_Err_Invalid_Character_Code.char_index_name_lookup_invalid_code`

Rejected exact-error candidates:

- `fterrdef.FT_Err_Invalid_Argument.null_output_or_bad_flag_arguments`:
  exact-error promotion failed because pinned C returned `Ok`. The row remains
  a value-contract/oracle-policy issue, not an exact-error row.
- `ftlcdfil.FT_Library_SetLcdGeometry.unimplemented_with_subpixel_filtering`:
  exact-error promotion failed because pinned C returned `Ok`. The row remains
  a value-contract/oracle-policy issue, not an exact-error row.

Verified progress:

- Exact comparison passed for all twelve promoted rows after the two rejected
  candidates were removed from exact-error enforcement.
- No runtime behavior change was needed; the existing pure-Rust implementation,
  C ABI, and WASM ABI outputs already matched pinned C FreeType for the
  promoted rows once the fallback guard was removed.
- Route audit classifies all twelve promoted rows as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftdriver.FT_Prop_IncreaseXHeight.invalid_face_error_matches_c
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `4091`, `generic-error-fallback` `104`.

### Issue Set Current: outline/allocator public API exact error routes

Previous blocker:

- Singleton public error rows across size/load setup, optional bzip2 streams,
  allocator failure harnesses, FFI error mapping, PostScript table loading,
  face properties, and outline lifecycle/utilities were still classified as
  `generic-error-fallback`.
- These rows had runnable pinned C, Rust FFI, thin C ABI, and WASM ABI coverage,
  but fallback classification accepted any error instead of requiring exact
  public status/output equality.

Promoted rows:

- `fterrdef.FT_Err_Invalid_PPem.tt_size_reset_zero_ppem`
- `fterrdef.FT_Err_Unimplemented_Feature.optional_module_feature_disabled`
- `fterrdef.FT_Err_Array_Too_Large.allocator_growth_overflow_returns_error`
- `fterrdef.FT_Err_Out_Of_Memory.allocator_failure_injection`
- `fterrdef.FT_Err_Invalid_Outline.rust_invalid_outline_mapping`
- `tttables.TT_Postscript.invalid_post_format_error_runtime`
- `ftparams.FT_PARAM_TAG_LCD_FILTER_WEIGHTS.face_property_ignored`
- `ftoutln.FT_Outline_Copy.invalid_pointer_or_size_mismatch`
- `ftoutln.FT_Outline_Done.invalid_library_or_outline_errors`
- `ftoutln.FT_Outline_New.invalid_arguments_and_limits`
- `ftoutln.FT_Outline_Embolden.invalid_or_indeterminate_orientation_errors`
- `ftoutln.FT_Outline_EmboldenXY.invalid_orientation_or_null_errors`

Rejected or deferred candidates:

- `freetype.FT_New_Face.error_null_library_or_aface`: exact-error promotion
  failed because pinned C returned `Ok`. The row remains a
  value-contract/oracle-policy issue, not an exact-error row.
- `ftimage.FT_Outline_MoveTo_Func.decompose_propagates_callback_error`:
  focused probe produced no runnable row because the callback trace route is
  still pending-core. It was not promoted.

Verified progress:

- Exact comparison passed for all twelve promoted rows after the rejected
  `FT_New_Face` row was removed from exact-error enforcement.
- No runtime behavior change was needed; the existing pure-Rust implementation,
  C ABI, and WASM ABI outputs already matched pinned C FreeType for the
  promoted rows once the fallback guard was removed.
- Route audit classifies all twelve promoted rows as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_PPem.tt_size_reset_zero_ppem
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `4103`, `generic-error-fallback` `92`.

### Issue Set Current: SFNT/render-callback public API exact routes

Previous blocker:

- Singleton public error rows across SFNT table/charmap/metrics parsing, PCF
  stream skipping, cache lookup OOM, module-prefixed errors, SVG preset slot,
  custom renderer lifecycle, direct span callbacks, outline orientation, and
  incremental glyph callback propagation were still classified as
  `generic-error-fallback`.
- These rows had runnable pinned C, Rust FFI, thin C ABI, and WASM ABI coverage,
  but fallback classification accepted any error instead of requiring exact
  public status/output equality.

Promoted rows:

- `fterrdef.FT_Err_Table_Missing.sfnt_required_table_missing`
- `fterrdef.FT_Err_Invalid_CharMap_Format.sfnt_cmap_format_rejected`
- `fterrdef.FT_Err_Invalid_Offset.table_offset_bounds_rejected`
- `fterrdef.FT_Err_Invalid_Horiz_Metrics.sfnt_hmtx_metrics_rejected`
- `fterrdef.FT_Err_Invalid_Stream_Skip.pcf_table_skip_failure`
- `fterrdef.FT_Err_Out_Of_Memory.cache_flush_then_oom`
- `ftmoderr.FT_Mod_Err_Raster.prefixed_error_base`
- `ftmoderr.FT_Mod_Err_Sdf.prefixed_error_base`
- `fterrdef.FT_Err_Bad_Argument.svg_preset_slot_bad_argument`
- `ftimage.FT_Raster_Span_Func.missing_span_callback_errors`
- `ftoutln.FT_Orientation.geometry_fixture_matrix`
- `ftimage.FT_Raster_New_Func.renderer_new_error_propagates`
- `ftincrem.FT_Incremental_FuncsRec.callback_error_propagates`

Rejected candidates:

- `fterrdef.FT_Err_Invalid_Glyph_Format.render_or_load_rejects_unsupported_glyph_format`:
  exact-error promotion failed because pinned C returned `Ok`. The row remains
  a value-contract/oracle-policy issue, not an exact-error row.
- `fterrdef.FT_Err_Missing_SVG_Hooks.svg_render_without_hooks`: exact-error
  promotion failed because pinned C returned `Ok`. The row remains a
  value-contract/oracle-policy issue, not an exact-error row.
- `ftimage.FT_RASTER_FLAG_DIRECT.mono_rejects_direct`: exact promotion failed
  on status mismatch. The row remains an implementation/status parity gap, not
  an exact-error classification row.

Verified progress:

- Exact comparison passed for all thirteen promoted rows after the rejected
  render rows were removed from exact-error enforcement.
- No runtime behavior change was needed; the existing pure-Rust implementation,
  C ABI, and WASM ABI outputs already matched pinned C FreeType for the
  promoted rows once the fallback guard was removed.
- Route audit classifies all thirteen promoted rows as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Table_Missing.sfnt_required_table_missing
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `4116`, `generic-error-fallback` `79`.

### Issue Set Current: module/load public API exact error routes

Previous blocker:

- Singleton public error rows across SFNT, TrueType, Type1, Type42, WinFNT,
  render-mode validation, slot validation, malformed outlines, malformed SFNT
  tables, and missing cmap operations were still classified as
  `generic-error-fallback`.
- These rows had runnable pinned C, Rust FFI, thin C ABI, and WASM ABI coverage,
  but fallback classification accepted any error instead of requiring exact
  public status/output equality.

Promoted rows:

- `ftmoderr.FT_Mod_Err_SFNT.prefixed_error_base`
- `ftmoderr.FT_Mod_Err_TrueType.prefixed_error_base`
- `ftmoderr.FT_Mod_Err_Type1.prefixed_error_base`
- `ftmoderr.FT_Mod_Err_Type42.prefixed_error_base`
- `ftmoderr.FT_Mod_Err_Winfonts.prefixed_error_base`
- `fterrdef.FT_Err_Cannot_Render_Glyph.unsupported_render_mode_returns_error`
- `fterrdef.FT_Err_Invalid_Slot_Handle.null_or_invalid_slot_rejected`
- `fterrdef.FT_Err_Invalid_Outline.malformed_outline_rejected`
- `fterrdef.FT_Err_Invalid_Table.malformed_sfnt_table_rejected`
- `fterrdef.FT_Err_CMap_Table_Missing.sfnt_without_cmap_returns_error_where_required`

Verified progress:

- Exact comparison passed for all ten promoted rows.
- No runtime behavior change was needed; the existing pure-Rust implementation,
  C ABI, and WASM ABI outputs already matched pinned C FreeType for the
  promoted rows once the fallback guard was removed.
- Route audit classifies all ten promoted rows as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftmoderr.FT_Mod_Err_SFNT.prefixed_error_base
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `4126`, `generic-error-fallback` `69`.

### Issue Set Current: render/raster public API exact error routes

Previous blocker:

- Ten public render/raster error rows across `FT_Outline_Render`,
  `renderer.raster_render`, and `FT_Outline_Get_Bitmap` target-buffer
  validation were still classified as `generic-error-fallback`.
- These rows had runnable pinned C, Rust FFI, thin C ABI, and WASM ABI coverage,
  but fallback classification accepted any error instead of requiring exact
  public status/output equality.

Promoted rows:

- `fterrdef.FT_Err_Cannot_Render_Glyph.outline_raster_unsupported_mode_returns_error`
- `fterrdef.FT_Err_Raster_Corrupted.sdf_raster_missing_flag`
- `fterrdef.FT_Err_Raster_Corrupted.bsdf_empty_contours_corrupted`
- `fterrdef.FT_Err_Raster_Negative_Height.monochrome_raster_negative_height`
- `fterrdef.FT_Err_Raster_Uninitialized.raster_render_without_pool`
- `ftimage.FT_RASTER_FLAG_SDF.non_sdf_raster_rejects_sdf_shape`
- `ftimage.FT_Raster.null_raster_errors`
- `ftimage.FT_Raster_Funcs.render_callback_error_contract`
- `ftimage.FT_Raster_Params.invalid_param_errors`
- `ftimage.FT_Bitmap.invalid_target_buffer_errors`

Rejected exact-error candidates:

- The first attempted `load_glyph` bytecode/interpreter exact-error batch was
  not promoted. Exact probes showed pinned C returned `Ok` for the sampled
  rows, including invalid jump, jump overflow, PUSH truncation, glyph-program
  FDEF, DEBUG opcode, divide-by-zero, stray ENDF, execution-limit, and invalid
  opcode cases. Those rows remain oracle-policy/test-input issues until the
  fixture inputs prove the named public errors on pinned C FreeType.

Verified progress:

- Exact comparison passed for all ten promoted render/raster rows.
- No runtime behavior change was needed; the existing pure-Rust implementation,
  C ABI, and WASM ABI outputs already matched pinned C FreeType for the
  promoted rows once the fallback guard was removed.
- Route audit classifies all ten promoted rows as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Cannot_Render_Glyph.outline_raster_unsupported_mode_returns_error
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `4136`, `generic-error-fallback` `59`.

### Issue Set Current: face/table and Smooth module public API exact error routes

Previous blocker:

- Face opening, table-load, and Smooth renderer module-prefixed public error
  rows were still classified as `generic-error-fallback`.
- These rows had runnable pinned C, Rust FFI, thin C ABI, and WASM ABI coverage,
  but fallback classification accepted any error instead of requiring exact
  public status/output equality.

Promoted rows:

- `fterrdef.FT_Err_Cannot_Open_Resource.resource_fork_open_failure_returns_error`
- `fterrdef.FT_Err_Cannot_Open_Stream.resource_fork_stream_failure_returns_error`
- `fterrdef.FT_Err_Hmtx_Table_Missing.incremental_metrics_exception_matches_c`
- `fterrdef.FT_Err_Missing_Module.no_driver_matches_face`
- `fterrdef.FT_Err_Unknown_File_Format.malformed_container_probe_unknown`
- `fterrdef.FT_Err_Unknown_File_Format.face_open_unknown_format`
- `fterrdef.FT_Err_Horiz_Header_Missing.sfnt_missing_hhea_table`
- `fterrdef.FT_Err_Invalid_Frame_Read.stream_frame_bounds_rejected`
- `ftmoderr.FT_Mod_Err_Smooth.prefixed_error_base`

Rejected exact-error candidates:

- `fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error`:
  exact-error promotion failed because pinned C returned `Ok`. The row remains
  a value-contract/oracle-policy issue, not an exact-error row.
- `fterrdef.FT_Err_Invalid_Library_Handle.library_api_rejects_null_library`:
  exact-error promotion failed because pinned C returned `Ok`. The row remains
  a value-contract/oracle-policy issue, not an exact-error row.
- `ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs`:
  exact probe failed before comparison because fixture
  `input/fonts/bdf/charset-registry.bdf` is missing. The row remains an
  incomplete fixture/input issue until the required asset exists.
- `ftbdf.FT_Get_BDF_Charset_ID.error_sfnt_bdf_without_selected_strike`:
  exact probe failed before comparison because fixture
  `input/fonts/bdf/sfnt-bdf-table.otb` is missing. The row remains an
  incomplete fixture/input issue until the required asset exists.
- `ftimage.FT_PIXEL_MODE_NONE.invalid_render_target_errors`: exact promotion
  exposed a real status mismatch: pinned C returned error code `6`, while Rust
  FFI returned error code `35`. This row remains an implementation parity gap.
- `ftoutln.FT_Outline_Get_Bitmap.null_bitmap_and_delegate_errors`: exact
  promotion exposed a real status mismatch: pinned C returned error code `6`,
  while Rust FFI returned error code `35`. This row remains an implementation
  parity gap.

Verified progress:

- Exact comparison passed for all promoted rows. The Smooth module case
  represents three concrete rows, so this batch promotes eleven concrete
  route rows in total.
- No runtime behavior change was needed; the existing pure-Rust implementation,
  C ABI, and WASM ABI outputs already matched pinned C FreeType for the
  promoted rows once the fallback guard was removed.
- Route audit classifies the promoted rows as `real-parity`.

Focused non-coverage results:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Cannot_Open_Resource.resource_fork_open_failure_returns_error
make -C pillow-rs-freetype test-case CASE=ftmoderr.FT_Mod_Err_Smooth.prefixed_error_base
```

Results: `1 / 1` and `3 / 3` runtime parity rows passed, `0` failed, `0`
pending. Route audit: `real-parity` `4147`, `generic-error-fallback` `48`.

### Issue Set Current: `FT_Outline_Get_Bitmap` delegated error parity

Previous blocker:

- `ftimage.FT_PIXEL_MODE_NONE.invalid_render_target_errors` and
  `ftoutln.FT_Outline_Get_Bitmap.null_bitmap_and_delegate_errors` were still
  classified as `generic-error-fallback`.
- Earlier exact probes exposed real implementation gaps: Rust FFI used the
  generic no-font expected-error shortcut and returned
  `FT_Err_Invalid_Face_Handle` (`35`) instead of pinned C's
  `FT_Err_Invalid_Argument` (`6`) for the null-bitmap scenario.
- After bypassing the generic shortcut, the oversized-outline scenario still
  diverged: pinned C returned `FT_Err_Invalid_Outline` (`20`), while Rust
  returned success.

Fix:

- Exclude `ftoutln.outline_get_bitmap` from the generic no-font expected-error
  shortcut so its dedicated public runner owns the null/delegated scenarios.
- Match FreeType 2.14.3 `src/base/ftoutln.c:669-689` delegation to
  `FT_Outline_Render` by rejecting cboxes outside +/-0x1000000 with
  `FT_Err_Invalid_Outline` before rasterizing.
- Preserve the adjacent existing exact route
  `ftimage.FT_Bitmap.invalid_target_buffer_errors` by routing it through the
  dedicated outline-bitmap runner and matching FreeType 2.14.3
  `src/smooth/ftgrays.c:2012-2019`: non-empty bitmap targets with NULL storage
  return `FT_Err_Invalid_Argument` with no output payload.
- Promote both rows to exact-error comparison across pinned C oracle, Rust FFI,
  thin C ABI, and WASM ABI.

Promoted rows:

- `ftimage.FT_PIXEL_MODE_NONE.invalid_render_target_errors`
- `ftoutln.FT_Outline_Get_Bitmap.null_bitmap_and_delegate_errors`

Rejected exact-error candidates:

- `ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output`: exact promotion
  failed because pinned C returned `Ok`. This row remains a fixture/oracle
  policy issue, not an exact-error row.

Verified progress:

- Focused exact comparison passed for both promoted rows.
- Regression guard `ftimage.FT_Bitmap.invalid_target_buffer_errors` also passed
  focused exact comparison after the shared route fix.
- Route audit classifies both promoted rows as `real-parity`.

Focused non-coverage results:

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Outline_Get_Bitmap.null_bitmap_and_delegate_errors
make -C pillow-rs-freetype test-case CASE=ftimage.FT_PIXEL_MODE_NONE.invalid_render_target_errors
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Bitmap.invalid_target_buffer_errors
```

Results: each focused probe passed `1 / 1` runtime parity row, `0` failed,
`0` pending. Route audit after promotion: `real-parity` `4149`,
`generic-error-fallback` `46`.

### Issue Set Current: mixed render and delegated public error routes

Previous blocker:

- Seven non-load-glyph public error rows were still classified as
  `generic-error-fallback` even though they have deterministic pinned C oracle,
  Rust FFI, thin C ABI, and WASM ABI runners.
- `ftimage.FT_RASTER_FLAG_DIRECT.mono_rejects_direct` exposed a real status
  mismatch during exact probing: pinned C returned `FT_Err_Invalid_Argument`
  (`6`) for DIRECT rendering without `FT_RASTER_FLAG_AA`, while Rust returned
  `FT_Err_Cannot_Render_Glyph` (`19`).

Fix:

- Match FreeType 2.14.3 DIRECT rendering validation order: DIRECT without AA is
  rejected as `FT_Err_Invalid_Argument` before entering the mono raster path.
- Promote only rows that passed exact comparison across pinned C oracle,
  Rust FFI, thin C ABI, and WASM ABI.

Promoted rows:

- `ftimage.FT_RASTER_FLAG_DIRECT.mono_rejects_direct`
- `fterrdef.FT_Err_Invalid_CodeRange.truetype_invalid_coderange`
- `fterrdef.FT_Err_Locations_Missing.glyf_present_loca_missing`
- `fterrdef.FT_Err_Too_Many_Caches.cache_manager_cache_limit`
- `fterrdef.FT_Err_Ignore.parser_ignore_sentinel_not_public_success`
- `fterrdef.FT_Err_Invalid_Stream_Read.png_embedded_bitmap_read_failure`
- `fterrdef.FT_Err_Invalid_SVG_Document.svg_document_failure_policy`

Rejected exact-error candidates:

- `ftimage.FT_Outline_MoveTo_Func.decompose_propagates_callback_error`: exact
  probe was not runnable because the `ftoutln.outline_decompose` callback trace
  route remains `pending-core`. It is not promoted to real parity.

Verified progress:

- Focused exact comparison passed for all seven promoted rows.
- Route audit classifies the promoted rows as `real-parity`.

Focused non-coverage results:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_RASTER_FLAG_DIRECT.mono_rejects_direct
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_CodeRange.truetype_invalid_coderange
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Locations_Missing.glyf_present_loca_missing
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Too_Many_Caches.cache_manager_cache_limit
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Ignore.parser_ignore_sentinel_not_public_success
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Stream_Read.png_embedded_bitmap_read_failure
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_SVG_Document.svg_document_failure_policy
```

Results: each focused probe passed `1 / 1` runtime parity row, `0` failed,
`0` pending. Route audit after promotion: `real-parity` `4156`,
`generic-error-fallback` `39`.

### Issue Set Current: existing primary-font pending assets now real parity

Previous blocker:

- Several rows were classified as `pending-route` because their primary
  `font` asset still carried `required_future_asset` metadata.
- The referenced fixture files now exist under `tests/fixtures`, and these
  routes use the primary `font` asset directly.
- A broad "existing file means ready" rule was rejected because it also
  promoted rows whose full exact oracle returned errors. The final change uses
  an explicit allow-list of only rows proven by full refreshed parity.

Promoted rows:

- `freetype.FT_Face_CheckTrueTypePatents.non_truetype_face_result`
- `freetype.FT_Get_FSType_Flags.sfnt_installable_embedding`
- `freetype.FT_Get_FSType_Flags.sfnt_restricted_embedding_bits`
- `freetype.FT_Get_First_Char.charcode_zero_disambiguated_by_glyph_index`
- `freetype.FT_HAS_FIXED_SIZES.bitmap_strike_font_true`
- `freetype.FT_HAS_GLYPH_NAMES.glyph_names_font_true`
- `freetype.FT_HAS_GLYPH_NAMES.no_glyph_names_control_false`
- `freetype.FT_Open_Face.success_open_variation_named_instance`
- `freetype.FT_Request_Size.success_bitmap_request_match`
- `freetype.FT_Select_Charmap.success_select_present_encoding`

Rejected rows from the same primary-font probe:

- `freetype.FT_ENCODING_NONE.representative_runtime_observation`: full exact
  classification failed because pinned C returned error `23`.
- `freetype.FT_IS_SCALABLE.bitmap_only_face_returns_false`: full exact
  classification failed because pinned C returned error `85`.
- `freetype.FT_HAS_HORIZONTAL.no_horizontal_metrics_control`: full exact
  classification failed because pinned C returned error `85`.
- `ftcache.FTC_SBitCache_Lookup.missing_bitmap_has_null_buffer`: full exact
  classification failed because pinned C returned error `6`.

Verified progress:

- Full refreshed parity passed after promoting only the ten proven rows.
- Route audit classifies the ten rows as `real-parity`.

Non-coverage verification:

```bash
make -C pillow-rs-freetype route-audit
FONTDONE_UNIFIED_ORACLE_REFRESH=1 make -C pillow-rs-freetype test
```

Results: `7154 / 7154` runtime parity rows passed, `0` failed, `80` pending.
Route audit after promotion: `real-parity` `4166`, `pending-route` `72`,
`generic-fallback` `696`, `generic-error-fallback` `39`.

### Issue Set Deferred: `FT_Get_MM_Var` descriptor success route classification

Probe result:

- Eight `FT_Get_MM_Var` descriptor success rows were still classified as
  `generic-fallback` even though each row already had runnable pinned C oracle,
  Rust FFI, thin C ABI, and WASM ABI comparison.
- Focused probes were not sufficient evidence because generic fallback allowed
  oracle errors.
- Full refreshed parity with exact route classification showed pinned C returns
  `FT_Err_Invalid_Argument` (`7`) for the eight rows, so they are not real
  success parity today.

Rejected success-route candidates:

- `ftmm.FT_Get_MM_Var.variable_font_descriptor_success`
- `ftmm.FT_Get_MM_Var.adobe_mm_descriptor_success`
- `ftmm.FT_MM_Var.populated_for_variable_true_type`
- `ftmm.FT_MM_Var.populated_for_adobe_mm`
- `ftmm.FT_Var_Axis.variable_font_axis_values`
- `ftmm.FT_Var_Axis.adobe_mm_axis_values`
- `ftmm.FT_Var_Named_Style.coordinates_array_matches_axis_count`
- `ftmm.FT_Var_Named_Style.psid_missing_sentinel_matches_c`

Other rejected or deferred candidates checked in the same pass:

- `freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes`:
  the referenced Apple Roman fixture now exists, but the current generic
  `get_char_index` route still uses the primary `font` asset and does not
  exercise the `non_unicode_charmap_font` selection metadata. This remains a
  route/fixture-model issue, not a safe classification-only promotion.
- `ftbbox.FT_Outline_Get_BBox.error_malformed_outline`: the standalone public
  `FT_Outline_Get_BBox` symbol is not implemented in Rust FFI/C ABI/WASM yet;
  existing bbox parity is derived from loaded glyph snapshots. This remains a
  core public endpoint/route implementation task.
- `ftmm.FT_Get_Default_Named_Instance.service_without_default_instance_success`:
  still requires a real Adobe Multiple Master Type1 fixture exposing the
  service-with-null-callback behavior. It must not be replaced with a variable
  font placeholder.
- `ftcid.FT_Get_CID_From_Glyph_Index.cid_face_returns_cid`,
  `ftcid.FT_Get_CID_From_Glyph_Index.opentype_cid_face_supported`, and
  `ftcid.FT_Get_CID_From_Glyph_Index.null_cid_output_matches_c`: focused
  probes passed under generic fallback, but full exact classification failed
  all seven concrete rows because pinned C returned `FT_Err_Invalid_Argument`
  (`7`). These rows remain fixture/oracle-policy issues until the inputs prove
  the named success behavior on pinned C FreeType.

Verification:

- `FONTDONE_UNIFIED_ORACLE_REFRESH=1 make -C pillow-rs-freetype test` failed
  the eight rows under exact route classification with `oracle returned
  unexpected error 7`.
- The promotion was reverted. Route audit remains at `real-parity` `4156`,
  `generic-fallback` `696`, `generic-error-fallback` `39`.

### Issue Set Current: `FT_Open_Face` invalid source-flag exact-error route

Previous blocker:

- `freetype.FT_Open_Face.error_invalid_source_flags` was classified as
  `generic-error-fallback`.
- The fixture contains three public `FT_Open_Args.flags` variants: no source
  flag, multiple source flags, and an unsupported stream-source variant. The
  shared memory-face row encoding did not serialize the flags, so the runners
  could not prove the same input as pinned C FreeType.

Fix plan:

1. Carry `FT_Open_Args.flags` through the maintained variant row encoding.
2. Route `FT_Open_Face` rows through the pinned C `--open-face-variants`
   command instead of `FT_New_Memory_Face`.
3. Make Rust FFI, thin C ABI, and WASM lanes reject invalid source-flag
   combinations with the same `FT_Err_Invalid_Argument` result as C for the
   currently modeled observable fields.
4. Keep stream lifecycle callback evidence visible as future work; do not invent
   a green `stream_close_count` placeholder.

Verified progress:

- Pinned C oracle, Rust FFI, C ABI, and WASM ABI now receive the same
  `FT_Open_Args.flags` values for this row.
- The thin C ABI runner dispatches flag-only `FT_Open_Face` rows to
  `FT_Open_Face`, not `FT_New_Memory_Face`.
- Focused exact comparison passes for
  `freetype.FT_Open_Face.error_invalid_source_flags`.
- Route audit classifies the row as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Open_Face.error_invalid_source_flags
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `3851`, `generic-error-fallback` `344`.

### Issue Set Current: `FT_RASTER_FLAG_AA` mono target exact-error route

Previous blocker:

- `ftimage.FT_RASTER_FLAG_AA.mono_rejects_aa` was classified as
  `generic-error-fallback`.
- Enabling exact comparison exposed a real Rust FFI divergence: pinned C
  returned `FT_Err_Invalid_Argument` for AA rendering into an
  `FT_PIXEL_MODE_MONO` target, while the Rust FFI path returned
  `FT_Err_Cannot_Render_Glyph` and reported a synthetic preserved bitmap
  payload.

Fix plan:

1. Promote only the concrete mono-AA row to exact-error comparison.
2. Match pinned C FreeType's `ftgrays.c:2014-2016` validation result:
   non-gray AA targets return `FT_Err_Invalid_Argument` before writing caller
   storage.
3. Do not fabricate an error-output bitmap when the C oracle exposes no output
   payload for this validation failure.
4. Verify the same input through Rust FFI, thin C ABI `FT_Outline_Render`, and
   WASM ABI.

Verified progress:

- Rust FFI now returns `FT_Err_Invalid_Argument` for AA outline rendering into
  a mono target, with a C-reference comment at the implementation site.
- The unified Rust/C/WASM runners now preserve the absence of an error-output
  payload for this C validation failure instead of turning the sentinel buffer
  into an apparent public output.
- Focused exact comparison passes for
  `ftimage.FT_RASTER_FLAG_AA.mono_rejects_aa`.
- Route audit classifies the row as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_RASTER_FLAG_AA.mono_rejects_aa
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `3852`, `generic-error-fallback` `343`.

### Issue Set Current: `FT_New_Memory_Face` bad-size/unknown-format exact-error route

Previous blocker:

- `freetype.FT_New_Memory_Face.error_bad_size_or_unknown_format` was
  classified as `generic-error-fallback` across 17 concrete public inputs.
- The row family already ran through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI, but fallback classification only proved that an error happened,
  not that the exact public status/output matched C for the same input.

Fix plan:

1. Promote only the concrete bad-size/unknown-format family to exact-error
   comparison.
2. Keep the existing generated fixture inputs unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_New_Memory_Face`, and WASM ABI before counting the route as
   `real-parity`.

Verified progress:

- Exact comparison passed for all 17 concrete bad-size/unknown-format rows.
- No runtime Rust behavior change was needed; the existing Rust FFI, C ABI, and
  WASM ABI outputs already matched pinned C FreeType once the fallback guard was
  removed.
- Route audit classifies the row family as `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_New_Memory_Face.error_bad_size_or_unknown_format
```

Result: `17 / 17` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3868`, `generic-error-fallback` `327`.

### Issue Set Current: `FT_Load_Glyph` matrix-load exact-error route

Previous blocker:

- `freetype.FT_Load_Glyph.matrix_load` had `76` concrete error rows
  classified as `generic-error-fallback`.
- The full case family already ran through pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI, but fallback classification accepted any error on the
  error rows instead of requiring exact public status/output parity.

Fix plan:

1. Promote only the concrete matrix-load case family to exact-error comparison.
2. Keep all existing matrix fixture inputs unchanged.
3. Verify all matrix-load variants through Rust FFI, thin C ABI
   `FT_Load_Glyph`, and WASM ABI before counting the error rows as
   `real-parity`.

Verified progress:

- Exact comparison passed for all `305` concrete matrix-load rows.
- The `76` previously fallback-classified error rows now validate exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed; the existing implementation
  already matched once the fallback guard was removed.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Load_Glyph.matrix_load
```

Result: `305 / 305` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3944`, `generic-error-fallback` `251`.

### Issue Set Current: `FT_Load_Glyph` invalid-input exact-error route

Previous blocker:

- `freetype.FT_Load_Glyph.error_out_of_range_null_face_or_invalid_flags` had
  `4` concrete error rows classified as `generic-error-fallback`.
- The row family already ran through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete out-of-range/null-face/invalid-load-flag family to
   exact-error comparison.
2. Keep the generated invalid input rows unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI `FT_Load_Glyph`, and
   WASM ABI before counting the rows as `real-parity`.

Verified progress:

- Exact comparison passed for all `7` concrete rows in the family.
- The `4` previously fallback-classified error rows now validate exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Load_Glyph.error_out_of_range_null_face_or_invalid_flags
```

Result: `7 / 7` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3948`, `generic-error-fallback` `247`.

### Issue Set Current: `FT_Sfnt_Table_Info` invalid-argument exact-error route

Previous blocker:

- `tttables.FT_Sfnt_Table_Info.invalid_index_or_arguments` had `3` concrete
  error rows classified as `generic-error-fallback`.
- The row family already ran through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete invalid-index/argument family to exact-error
   comparison.
2. Keep the generated SFNT table-info inputs unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Sfnt_Table_Info`, and WASM ABI before counting the rows as
   `real-parity`.

Verified progress:

- Exact comparison passed for all `3` concrete rows.
- The previously fallback-classified error rows now validate exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=tttables.FT_Sfnt_Table_Info.invalid_index_or_arguments
```

Result: `3 / 3` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3951`, `generic-error-fallback` `244`.

### Issue Set Current: `FT_Load_Sfnt_Table` missing-table exact-error route

Previous blocker:

- `tttables.FT_Load_Sfnt_Table.missing_table_or_invalid_face_error` had `2`
  concrete error rows classified as `generic-error-fallback`.
- The row family already ran through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete missing-table/invalid-face family to exact-error
   comparison.
2. Keep the generated SFNT load-table inputs unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Load_Sfnt_Table`, and WASM ABI before counting the rows as
   `real-parity`.

Verified progress:

- Exact comparison passed for all `2` concrete rows.
- The previously fallback-classified error rows now validate exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=tttables.FT_Load_Sfnt_Table.missing_table_or_invalid_face_error
```

Result: `2 / 2` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3953`, `generic-error-fallback` `242`.

### Issue Set Current: `FT_Err_Raster_Overflow` render exact-error route

Previous blocker:

- `fterrdef.FT_Err_Raster_Overflow.raster_buffer_or_cell_overflow` had `2`
  concrete error rows classified as `generic-error-fallback`.
- The row family already ran through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete raster-overflow render-glyph family to exact-error
   comparison.
2. Keep the generated overflow input rows unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI `FT_Render_Glyph`,
   and WASM ABI before counting the rows as `real-parity`.

Verified progress:

- Exact comparison passed for all `2` concrete rows.
- The previously fallback-classified error rows now validate exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Raster_Overflow.raster_buffer_or_cell_overflow
```

Result: `2 / 2` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3955`, `generic-error-fallback` `240`.

### Issue Set Current: `FT_LOAD_FORCE_AUTOHINT` load-glyph exact-error route

Previous blocker:

- `freetype.FT_LOAD_FORCE_AUTOHINT.load_glyph_force_autohint_behavior` had a
  concrete error row classified as `generic-error-fallback`.
- The row family already ran through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete force-autohint load-glyph family to exact-error
   comparison.
2. Keep the generated force-autohint input rows unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI `FT_Load_Glyph`, and
   WASM ABI before counting the row as `real-parity`.

Verified progress:

- Exact comparison passed for all `6` concrete rows.
- The previously fallback-classified error row now validates exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_LOAD_FORCE_AUTOHINT.load_glyph_force_autohint_behavior
```

Result: `6 / 6` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3956`, `generic-error-fallback` `239`.

### Issue Set Current: `FT_LOAD_PEDANTIC` load-glyph exact-error route

Previous blocker:

- `freetype.FT_LOAD_PEDANTIC.pedantic_error_behavior` had a concrete error row
  classified as `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete pedantic load-glyph row to exact-error comparison.
2. Keep the generated pedantic input row unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI `FT_Load_Glyph`, and
   WASM ABI before counting the row as `real-parity`.

Verified progress:

- Rust now matches FreeType `Compute_Point_Displacement` validation for
  pedantic SHP/SHC/SHZ movement. When `rp1`/`rp2` references an empty or
  out-of-range zone, non-pedantic execution ignores the movement and
  `FT_LOAD_PEDANTIC` returns `FT_Err_Invalid_Reference`.
- Exact comparison passed for the concrete row.
- The previously fallback-classified error row now validates exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_LOAD_PEDANTIC.pedantic_error_behavior
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3957`, `generic-error-fallback` `238`.

### Issue Set Pending: `FT_Err_Divide_By_Zero` load-glyph fixture mismatch

Current blocker:

- `fterrdef.FT_Err_Divide_By_Zero.bytecode_div_zero_returns_error` had a
  concrete TrueType bytecode error row classified as `generic-error-fallback`.
- The row uses a generated TrueType font that executes `DIV` with a zero
  divisor, but fallback classification only proved that an error happened.
- Promoting it to exact-error comparison exposed that the pinned C oracle returns
  `ok` for the generated row, not `FT_Err_Divide_By_Zero`.

Fix plan:

1. Do not classify this row as `real-parity` while the pinned C oracle returns
   `ok`.
2. Audit the generated `generated/truetype/divide-by-zero.ttf` bytecode and the
   fixture's `fixture_defined_error_glyph` selection.
3. If the fixture is wrong, update the maintained generator so the selected
   public glyph actually reaches FreeType `Ins_DIV` with `args[1] == 0`.
4. Only after the pinned C oracle returns `FT_Err_Divide_By_Zero`, verify exact
   status/output through Rust FFI, thin C ABI `FT_Load_Glyph`, and WASM ABI.

Non-coverage probe:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Divide_By_Zero.bytecode_div_zero_returns_error
```

Result under attempted exact-error classification: failed because the oracle
returned `ok`. The row remains `generic-error-fallback` until the generator and
selected glyph are corrected.

### Issue Set Pending: `FT_Err_Invalid_Reference` load-glyph fixture mismatch

Current blocker:

- `fterrdef.FT_Err_Invalid_Reference.tt_bytecode_invalid_point_reference` had a
  concrete TrueType bytecode error row classified as `generic-error-fallback`.
- Promoting it to exact-error comparison exposed that the pinned C oracle returns
  `ok` for the generated row, not `FT_Err_Invalid_Reference`.

Fix plan:

1. Do not classify this row as `real-parity` while the pinned C oracle returns
   `ok`.
2. Audit the generated TrueType invalid-reference fixture and the selected
   public glyph index.
3. If the fixture is wrong, update the maintained generator so the selected
   public glyph actually reaches the documented invalid-reference bytecode path.
4. Only after the pinned C oracle returns `FT_Err_Invalid_Reference`, verify
   exact status/output through Rust FFI, thin C ABI `FT_Load_Glyph`, and WASM
   ABI.

Non-coverage probe:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Reference.tt_bytecode_invalid_point_reference
```

Result under attempted exact-error classification: failed because the oracle
returned `ok`. The row remains `generic-error-fallback` until the generator and
selected glyph are corrected.

### Issue Set Current: `FT_Get_BDF_Property` null-argument exact-error route

Previous blocker:

- `ftbdf.FT_Get_BDF_Property.error_null_face_or_output` had a concrete BDF
  public error row classified as `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete BDF null-face/null-output row to exact-error
   comparison.
2. Keep the BDF fixture input unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_BDF_Property`, and WASM ABI before counting the row as
   `real-parity`.

Verified progress:

- Exact comparison passed for the concrete BDF null-argument row.
- The previously fallback-classified error row now validates exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftbdf.FT_Get_BDF_Property.error_null_face_or_output
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3958`, `generic-error-fallback` `237`.

### Issue Set Pending: `FT_Get_BDF_Charset_ID` missing charset fixture

Current blocker:

- `ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs` references
  `input/fonts/bdf/charset-registry.bdf`.
- The focused parity command fails before C/Rust comparison because that asset
  is missing from the current fixture tree.

Fix plan:

1. Do not classify this row as `real-parity` until the same BDF charset asset is
   present and deterministic.
2. Add or regenerate the maintained `charset-registry.bdf` fixture through the
   project fixture workflow.
3. Re-run the focused row and promote only if exact status/output matches
   pinned C FreeType through Rust FFI, thin C ABI, and WASM ABI.

Non-coverage probe:

```bash
make -C pillow-rs-freetype test-case CASE=ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs
```

Result: failed before parity comparison with missing asset
`input/fonts/bdf/charset-registry.bdf`; row remains `generic-error-fallback`.

### Issue Set Pending: `FT_Get_BDF_Charset_ID` missing SFNT-BDF fixture

Current blocker:

- `ftbdf.FT_Get_BDF_Charset_ID.error_sfnt_bdf_without_selected_strike`
  references `input/fonts/bdf/sfnt-bdf-table.otb`.
- The focused parity command fails before C/Rust comparison because that asset
  is missing from the current fixture tree.

Fix plan:

1. Do not classify this row as `real-parity` until the same SFNT-BDF/OTB asset
   is present and deterministic.
2. Add or regenerate the maintained `sfnt-bdf-table.otb` fixture through the
   project fixture workflow.
3. Re-run the focused row and promote only if exact status/output matches
   pinned C FreeType through Rust FFI, thin C ABI, and WASM ABI.

Non-coverage probe:

```bash
make -C pillow-rs-freetype test-case CASE=ftbdf.FT_Get_BDF_Charset_ID.error_sfnt_bdf_without_selected_strike
```

Result: failed before parity comparison with missing asset
`input/fonts/bdf/sfnt-bdf-table.otb`; row remains
`generic-error-fallback`.

### Issue Set Current: `FT_Get_BDF_Charset_ID` non-BDF-face exact-error route

Previous blocker:

- `ftbdf.FT_Get_BDF_Charset_ID.error_non_bdf_face` had a concrete BDF public
  error row classified as `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete `FT_Get_BDF_Charset_ID` non-BDF-face row to
   exact-error comparison.
2. Keep the existing non-BDF font input unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_BDF_Charset_ID`, and WASM ABI before counting the row as
   `real-parity`.

Verified progress:

- Exact comparison passed for the concrete BDF charset non-BDF-face row.
- The previously fallback-classified error row now validates exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.
- The broader `ftbdf.get_bdf_charset_id` operation lane still fails before
  parity comparison because
  `ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs` references missing
  fixture `input/fonts/bdf/charset-registry.bdf`. That blocker remains tracked
  separately above and is not promoted by this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftbdf.FT_Get_BDF_Charset_ID.error_non_bdf_face
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3964`, `generic-error-fallback` `231`.

### Issue Set Current: `FT_Get_PFR_Advance` non-PFR-face exact-error route

Previous blocker:

- `ftpfr.FT_Get_PFR_Advance.non_pfr_returns_invalid_argument` had a concrete
  PFR public error row classified as `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete `FT_Get_PFR_Advance` non-PFR-face row to
   exact-error comparison.
2. Keep the existing non-PFR font input unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_PFR_Advance`, and WASM ABI before counting the row as
   `real-parity`.

Verified progress:

- Exact comparison passed for the concrete PFR advance non-PFR-face row.
- The previously fallback-classified error row now validates exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftpfr.FT_Get_PFR_Advance.non_pfr_returns_invalid_argument
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3965`, `generic-error-fallback` `230`.

### Issue Set Current: `FT_Get_PFR_Advance` null-face/output exact-error route

Previous blocker:

- `ftpfr.FT_Get_PFR_Advance.null_face_or_output_errors` had a concrete PFR
  public error row classified as `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete `FT_Get_PFR_Advance` null-face/output row to
   exact-error comparison.
2. Keep the existing null-argument input variants unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_PFR_Advance`, and WASM ABI before counting the row as
   `real-parity`.

Verified progress:

- Exact comparison passed for the concrete PFR advance null-face/output row.
- The previously fallback-classified error row now validates exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftpfr.FT_Get_PFR_Advance.null_face_or_output_errors
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3966`, `generic-error-fallback` `229`.

### Issue Set Current: `FT_Get_PFR_Kerning` null-face/vector exact-error route

Previous blocker:

- `ftpfr.FT_Get_PFR_Kerning.null_face_or_vector_errors` had a concrete PFR
  public error row classified as `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete `FT_Get_PFR_Kerning` null-face/vector row to
   exact-error comparison.
2. Keep the existing null-argument input variants unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_PFR_Kerning`, and WASM ABI before counting the row as
   `real-parity`.

Verified progress:

- Exact comparison passed for the concrete PFR kerning null-face/vector row.
- The previously fallback-classified error row now validates exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftpfr.FT_Get_PFR_Kerning.null_face_or_vector_errors
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3967`, `generic-error-fallback` `228`.

### Issue Set Current: `FT_Get_PFR_Metrics` non-PFR-face exact-error/output route

Previous blocker:

- `ftpfr.FT_Get_PFR_Metrics.non_pfr_outputs_valid_values_and_unknown_format`
  had two concrete PFR public rows classified as `generic-error-fallback`.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete `FT_Get_PFR_Metrics` non-PFR-face rows to
   exact-error comparison.
2. Keep the existing non-PFR font input and optional-output variants unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_PFR_Metrics`, and WASM ABI before counting the rows as
   `real-parity`.

Verified progress:

- Exact comparison passed for both concrete PFR metrics non-PFR-face rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftpfr.FT_Get_PFR_Metrics.non_pfr_outputs_valid_values_and_unknown_format
```

Result: `2 / 2` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3969`, `generic-error-fallback` `226`.

### Issue Set Current: batched PFR metrics and ftcolor exact-error routes

Previous blocker:

- Ten concrete public rows across PFR metrics and ftcolor error-policy surfaces
  were classified as `generic-error-fallback`.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftpfr.FT_Get_PFR_Metrics.optional_outputs_and_null_face`
   - `ftcolor.FT_COLOR_ROOT_TRANSFORM_MAX.invalid_runtime_behavior`
   - `ftcolor.FT_COLR_PAINTFORMAT_UNSUPPORTED.invalid_format_returns_false`
   - `ftcolor.FT_COLR_PAINT_FORMAT_MAX.read_paint_rejects_max_and_above`
   - `ftcolor.FT_Get_Color_Glyph_ClipBox.null_and_non_sfnt_rejected`
   - `ftcolor.FT_Get_Color_Glyph_ClipBox.malformed_clipbox_false_behavior`
   - `ftcolor.FT_Get_Color_Glyph_Layer.invalid_inputs_rejected`
   - `ftcolor.FT_Get_Color_Glyph_Layer.malformed_layer_record_false_behavior`
   - `ftcolor.FT_Get_Color_Glyph_Paint.missing_or_invalid_root_returns_false`
   - `ftcolor.FT_Get_Color_Glyph_Paint.non_null_opaque_paint_rejected`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all ten rows before promotion.
- Exact comparison after promotion passed for all ten rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: all ten focused exact rows passed. Route audit:
`real-parity` `3979`, `generic-error-fallback` `216`.

### Issue Set Current: batched ftcolor iterator, paint, and palette exact-error routes

Previous blocker:

- Ten concrete public ftcolor rows were classified as
  `generic-error-fallback`.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftcolor.FT_Get_Color_Glyph_Paint.null_and_non_sfnt_rejected`
   - `ftcolor.FT_Get_Colorline_Stops.error_null_or_invalid_iterator`
   - `ftcolor.FT_Get_Colorline_Stops.error_null_color_stop_policy`
   - `ftcolor.FT_Get_Paint.error_null_or_missing_colr`
   - `ftcolor.FT_Get_Paint.error_null_output_policy`
   - `ftcolor.FT_Get_Paint_Layers.error_invalid_iterator_or_paint_offset`
   - `ftcolor.FT_Get_Paint_Layers.error_null_arguments_policy`
   - `ftcolor.FT_Palette_Data_Get.error_null_face_or_output`
   - `ftcolor.FT_Palette_Data_Get.error_color_layers_disabled`
   - `ftcolor.FT_Palette_Select.error_null_face_or_invalid_palette_index`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all ten rows before promotion.
- Exact comparison after promotion passed for all ten rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: all ten focused exact rows passed. Route audit:
`real-parity` `3989`, `generic-error-fallback` `206`.

### Issue Set Current: batched ftstroke exact-error routes

Previous blocker:

- Ten concrete public ftstroke error-policy rows were classified as
  `generic-error-fallback`.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftstroke.FT_Stroker_BeginSubPath.invalid_arguments`
   - `ftstroke.FT_Stroker_ConicTo.invalid_arguments`
   - `ftstroke.FT_Stroker_CubicTo.invalid_arguments`
   - `ftstroke.FT_Stroker_EndSubPath.invalid_stroker`
   - `ftstroke.FT_Stroker_GetBorderCounts.invalid_stroker_or_border`
   - `ftstroke.FT_Stroker_GetCounts.invalid_stroker`
   - `ftstroke.FT_Glyph_Stroke.invalid_glyph_arguments`
   - `ftstroke.FT_Glyph_Stroke.failure_sets_output_null_when_preserving_original`
   - `ftstroke.FT_Glyph_StrokeBorder.invalid_glyph_arguments`
   - `ftstroke.FT_Stroker_LineTo.invalid_arguments`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all ten rows before promotion.
- Exact comparison after promotion passed for all ten rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: all ten focused exact rows passed. Route audit:
`real-parity` `3999`, `generic-error-fallback` `196`.

### Issue Set Current: batched OpenType/GX validation exact-error routes

Previous blocker:

- Ten concrete public OpenType/GX validation rows were classified as
  `generic-error-fallback`.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftotval.FT_OpenType_Validate.service_missing_error`
   - `ftotval.FT_OpenType_Validate.malformed_table_error`
   - `ftotval.FT_VALIDATE_GDEF.malformed_table_error`
   - `ftotval.FT_VALIDATE_GPOS.malformed_table_error`
   - `ftotval.FT_VALIDATE_GSUB.malformed_table_error`
   - `ftotval.FT_VALIDATE_JSTF.absent_or_malformed_table`
   - `ftotval.FT_VALIDATE_MATH.absent_or_malformed_table`
   - `ftotval.FT_VALIDATE_OT.partial_failure_cleanup_contract`
   - `ftgxval.FT_TrueTypeGX_Validate.rejects_invalid_arguments`
   - `ftgxval.FT_TrueTypeGX_Validate.reports_unimplemented_or_invalid_table`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all ten rows before promotion.
- Exact comparison after promotion passed for all ten rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: all ten focused exact rows passed. Route audit:
`real-parity` `4009`, `generic-error-fallback` `186`.

### Issue Set Current: batched gzip/LZW exact-error routes

Previous blocker:

- Ten concrete public compression rows were classified as
  `generic-error-fallback`.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftgzip.FT_Gzip_Uncompress.rejects_invalid_arguments`
   - `ftgzip.FT_Gzip_Uncompress.reports_buffer_too_small`
   - `ftgzip.FT_Gzip_Uncompress.reports_invalid_compressed_data`
   - `ftgzip.FT_Gzip_Uncompress.reports_unimplemented_without_zlib`
   - `ftgzip.FT_Stream_OpenGzip.rejects_invalid_stream_handles`
   - `ftgzip.FT_Stream_OpenGzip.rejects_invalid_gzip_header`
   - `ftgzip.FT_Stream_OpenGzip.reports_unimplemented_without_zlib`
   - `ftlzw.FT_Stream_OpenLZW.invalid_header_error`
   - `ftlzw.FT_Stream_OpenLZW.null_stream_or_source_error`
   - `ftlzw.FT_Stream_OpenLZW.unsupported_build_error`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all ten rows before promotion.
- Exact comparison after promotion passed for all ten rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: all ten focused exact rows passed. Route audit:
`real-parity` `4019`, `generic-error-fallback` `176`.

### Issue Set Current: batched ftcache exact-error routes

Previous blocker:

- Ten concrete public ftcache rows were classified as
  `generic-error-fallback`.
- Two selected case IDs expand to multiple maintained concrete variants:
  `FTC_CMapCache_Lookup.error_null_cache_returns_zero` covers three rows, and
  `FTC_ImageCache_LookupScaler.error_null_scaler_or_aglyph` covers four rows.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftcache.FTC_CMapCache_Lookup.error_null_cache_returns_zero` — 3 rows
   - `ftcache.FTC_ImageCache_LookupScaler.error_null_scaler_or_aglyph` — 4 rows
   - `ftcache.FTC_CMapCache_New.error_null_manager_or_output`
   - `ftcache.FTC_ImageCache_Lookup.error_null_aglyph`
   - `ftcache.FTC_ImageCache_Lookup.error_invalid_cache_type_face_or_glyph`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all selected case IDs before
  promotion, covering ten concrete rows total.
- Exact comparison after promotion passed for all ten concrete rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: all selected focused exact case IDs passed, covering ten concrete rows.
Route audit: `real-parity` `4029`, `generic-error-fallback` `166`.

### Issue Set Current: second batched ftcache exact-error routes

Previous blocker:

- Ten additional concrete public ftcache rows were probed from
  `generic-error-fallback`.
- Nine rows passed exact comparison after promotion. One row,
  `ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output`, failed exact
  promotion because the pinned C oracle returned `Ok`, while exact-error
  classification would require an error; it remains visible as fallback until
  the SBit cache output-value contract is fixed.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that a broad error path or
  value path ran.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftcache.FTC_ImageCache_New.error_null_manager_or_output`
   - `ftcache.FTC_ImageCache_New.error_too_many_caches`
   - `ftcache.FTC_Manager_LookupFace.error_null_output_or_manager`
   - `ftcache.FTC_Manager_LookupFace.error_requester_failure`
   - `fterrdef.FT_Err_Invalid_Cache_Handle.cache_lookup_rejects_null_manager`
   - `ftcache.FTC_Manager_LookupSize.error_null_scaler_output_or_manager`
   - `ftcache.FTC_Manager_LookupSize.error_requester_or_size_selection_failure`
   - `ftcache.FTC_Manager_New.error_null_library`
   - `ftcache.FTC_Manager_New.error_null_requester_or_output`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all ten candidate rows before
  promotion.
- Exact comparison after promotion passed for nine rows.
- `ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output` was not promoted:
  exact rerun reported `requires an exact C error, but the oracle returned ok`.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: nine focused exact rows passed. Route audit:
`real-parity` `4038`, `generic-error-fallback` `157`.

### Issue Set Current: final batched ftcache SBit exact routes

Previous blocker:

- Six SBit cache rows remained under `generic-error-fallback`.
- Four rows passed exact comparison and can be promoted.
- `ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output` and
  `ftcache.FTC_SBitCache_Lookup.clears_outputs_before_lookup` remain
  unpromoted: exact-error classification would require an error, but the pinned
  C oracle returns `Ok`. Those rows need a value-contract fix/classification,
  not a forced exact-error promotion.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftcache.FTC_SBitCache_LookupScaler.rejects_null_sbit_or_scaler`
   - `ftcache.FTC_SBitCache_LookupScaler.clears_outputs_before_lookup`
   - `ftcache.FTC_SBitCache_New.error_outputs_null_cache`
   - `ftcache.FTC_SBitCache_New.invalid_arguments_match_c`
2. Keep the two `ftcache.FTC_SBitCache_Lookup` rows visible as remaining
   fallback until their `Ok` status and output-value contract are handled
   correctly.
3. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
4. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all five candidate rows before
  promotion.
- Exact comparison after promotion passed for four rows.
- `ftcache.FTC_SBitCache_Lookup.clears_outputs_before_lookup` was not promoted:
  exact rerun reported `requires an exact C error, but the oracle returned ok`.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: four focused exact rows passed. Route audit:
`real-parity` `4042`, `generic-error-fallback` `153`.

### Issue Set Current: GX validation, bzip2, and palette foreground exact routes

Previous blocker:

- Eight concrete public rows across classic kern validation, bzip2 streams, and
  palette foreground color were classified as `generic-error-fallback`.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that a broad fallback path
  ran.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftgxval.FT_ClassicKern_Validate.rejects_invalid_arguments`
   - `ftgxval.FT_ClassicKern_Validate.reports_unimplemented_or_invalid_table`
   - `ftgxval.FT_VALIDATE_APPLE.absent_or_invalid_kern_table`
   - `ftgxval.FT_VALIDATE_CKERN.malformed_table_error_matches_c`
   - `ftbzip2.FT_Stream_OpenBzip2.error_null_stream_or_source`
   - `ftbzip2.FT_Stream_OpenBzip2.error_invalid_or_truncated_bzip2_header`
   - `ftcolor.FT_Palette_Set_Foreground_Color.error_null_face`
   - `ftcolor.FT_Palette_Set_Foreground_Color.error_color_layers_disabled`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all eight rows before promotion.
- Exact comparison after promotion passed for all eight rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: all eight focused exact rows passed. Route audit:
`real-parity` `4050`, `generic-error-fallback` `145`.

### Issue Set Current: glyph, list, renderer, and SFNT lang-tag exact routes

Previous blocker:

- Ten concrete public rows across glyph helpers, list iteration, renderer
  selection, and SFNT language tags were classified as `generic-error-fallback`.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that a broad fallback path
  ran.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftglyph.FT_New_Glyph.error_null_library_or_output`
   - `ftglyph.FT_New_Glyph.error_unsupported_format`
   - `ftglyph.FT_New_Glyph.error_allocation_failure`
   - `ftglyph.FT_Glyph_Transform.error_null_or_bad_glyph`
   - `ftglyph.FT_Glyph_Transform.error_non_scalable_bitmap`
   - `ftlist.FT_List_Iterate.stops_on_callback_error`
   - `ftlist.FT_List_Iterate.null_list_or_iterator_error`
   - `ftrender.FT_Set_Renderer.invalid_library_renderer_or_params`
   - `ftrender.FT_Set_Renderer.set_mode_parameter_error_propagates`
   - `ftsnames.FT_Get_Sfnt_LangTag.invalid_argument_errors`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all ten rows before promotion.
- Exact comparison after promotion passed for all ten rows.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: all ten focused exact rows passed. Route audit:
`real-parity` `4060`, `generic-error-fallback` `135`.

### Issue Set Current: stroker, WinFNT, and outline utility exact routes

Previous blocker:

- Ten concrete public rows across stroker construction/parse-outline, WinFNT
  header lookup, and outline utility validation were classified as
  `generic-error-fallback`.
- Nine rows passed exact comparison after promotion. One row,
  `ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output`, failed exact
  promotion because the pinned C oracle returned `Ok`, while exact-error
  classification would require an error; it remains visible as fallback until
  the bbox output-value contract is handled correctly.
- The rows already ran through pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI, but fallback classification only proved that a broad fallback path
  ran.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftstroke.FT_Stroker_New.invalid_library`
   - `ftstroke.FT_Stroker_New.invalid_output_pointer`
   - `ftstroke.FT_Stroker_New.allocation_failure`
   - `ftstroke.FT_Stroker_ParseOutline.invalid_outline`
   - `ftstroke.FT_Stroker_ParseOutline.invalid_stroker`
   - `ftwinfnt.FT_Get_WinFNT_Header.null_face_returns_invalid_face_handle`
   - `ftwinfnt.FT_Get_WinFNT_Header.null_output_returns_invalid_argument`
   - `ftwinfnt.FT_Get_WinFNT_Header.non_winfnt_face_returns_invalid_argument`
   - `ftoutln.FT_Outline_Check.invalid_null_or_count_mismatch`
2. Keep all fixture inputs, oracle outputs, and comparison rules unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI, and WASM ABI before
   counting these rows as `real-parity`.

Verified progress:

- Focused generic-mode probes passed for all ten rows before promotion.
- Exact comparison after promotion passed for nine rows.
- `ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output` was not promoted:
  exact rerun reported `requires an exact C error, but the oracle returned ok`.
- The previously fallback-classified rows now validate exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for these rows.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=<each listed case id>
```

Result: nine focused exact rows passed. Route audit:
`real-parity` `4069`, `generic-error-fallback` `126`.

### Issue Set Current: `FT_Get_BDF_Property` missing-property exact-error route

Previous blocker:

- `ftbdf.FT_Get_BDF_Property.error_missing_property_sets_none` had a concrete
  BDF public error row classified as `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete BDF missing-property row to exact-error comparison.
2. Keep the BDF fixture input unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_BDF_Property`, and WASM ABI before counting the row as
   `real-parity`.

Verified progress:

- Exact comparison passed for the concrete BDF missing-property row.
- The previously fallback-classified error row now validates exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftbdf.FT_Get_BDF_Property.error_missing_property_sets_none
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3959`, `generic-error-fallback` `236`.

### Issue Set Current: `FT_Get_BDF_Property` unsupported-face exact-error route

Previous blocker:

- `ftbdf.FT_Get_BDF_Property.error_unsupported_face_or_unselected_strike` had a
  concrete BDF public error row classified as `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete BDF unsupported-face/unselected-strike row to
   exact-error comparison.
2. Keep the BDF/SFNT fixture inputs unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_BDF_Property`, and WASM ABI before counting the row as
   `real-parity`.

Verified progress:

- Exact comparison passed for the concrete BDF unsupported-face row.
- The previously fallback-classified error row now validates exact status/output
  against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftbdf.FT_Get_BDF_Property.error_unsupported_face_or_unselected_strike
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3960`, `generic-error-fallback` `235`.

### Issue Set Current: `FT_Get_CID_From_Glyph_Index` non-CID/null-face exact-error route

Previous blocker:

- `ftcid.FT_Get_CID_From_Glyph_Index.non_cid_or_null_face_errors_and_clears_output`
  had a concrete FTCID public error row classified as
  `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete `FT_Get_CID_From_Glyph_Index` non-CID/null-face
   row to exact-error comparison.
2. Keep the existing non-CID font input unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_CID_From_Glyph_Index`, and WASM ABI before counting the row as
   `real-parity`.

Verified progress:

- Exact comparison passed for the concrete FTCID non-CID/null-face row.
- The previously fallback-classified error row now validates exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftcid.FT_Get_CID_From_Glyph_Index.non_cid_or_null_face_errors_and_clears_output
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3961`, `generic-error-fallback` `234`.

### Issue Set Current: `FT_Get_CID_Is_Internally_CID_Keyed` non-CID/null-face exact-error route

Previous blocker:

- `ftcid.FT_Get_CID_Is_Internally_CID_Keyed.non_cid_or_null_face_errors_and_clears_output`
  had a concrete FTCID public error row classified as
  `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete `FT_Get_CID_Is_Internally_CID_Keyed`
   non-CID/null-face row to exact-error comparison.
2. Keep the existing non-CID font input unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_CID_Is_Internally_CID_Keyed`, and WASM ABI before counting the row
   as `real-parity`.

Verified progress:

- Exact comparison passed for the concrete FTCID non-CID/null-face row.
- The previously fallback-classified error row now validates exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftcid.FT_Get_CID_Is_Internally_CID_Keyed.non_cid_or_null_face_errors_and_clears_output
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3962`, `generic-error-fallback` `233`.

### Issue Set Current: `FT_Get_CID_Registry_Ordering_Supplement` non-CID/null-output exact-error route

Previous blocker:

- `ftcid.FT_Get_CID_Registry_Ordering_Supplement.error_non_cid_or_null_outputs`
  had a concrete FTCID public error row classified as
  `generic-error-fallback`.
- The row already ran through pinned C FreeType, Rust FFI, thin C ABI, and WASM
  ABI, but fallback classification only proved that an error happened.

Fix plan:

1. Promote only the concrete `FT_Get_CID_Registry_Ordering_Supplement`
   non-CID/null-output row to exact-error comparison.
2. Keep the existing non-CID font input unchanged.
3. Verify exact status/output through Rust FFI, thin C ABI
   `FT_Get_CID_Registry_Ordering_Supplement`, and WASM ABI before counting the
   row as `real-parity`.

Verified progress:

- Exact comparison passed for the concrete FTCID non-CID/null-output row.
- The previously fallback-classified error row now validates exact
  status/output against pinned C FreeType through Rust FFI, C ABI, and WASM ABI.
- No runtime Rust behavior change was needed for this row.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftcid.FT_Get_CID_Registry_Ordering_Supplement.error_non_cid_or_null_outputs
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3963`, `generic-error-fallback` `232`.

### Issue Set A: `ftoutln.outline_render` pending outline fixtures

Current pending count by operation: `15` rows.

Largest blockers from the current parity run:

- `11` rows: missing `outlines/synthetic/simple-rectangle.json`
- `6` rows: missing `outlines/synthetic/thin-diagonal-stems.json`
  (fixed by `FT_OUTLINE_HIGH_PRECISION` verified progress below)
- `6` rows: missing `outlines/synthetic/large-render-limits.json`
  (fixed for the synthetic `FT_Outline_Render` rows by
  `FT_OUTLINE_SINGLE_PASS` verified progress below)
- `3` rows: missing `outlines/synthetic/dropout-thin-stems-scantype.json`
- `3` rows: missing `outlines/synthetic/simple-overlap-thin-matrix.json`
  (fixed for the `FT_OUTLINE_NONE` baseline rows by verified progress below)
- additional outline-render rows reference other missing synthetic outline
  assets.

Fix plan:

1. Add real fixture JSON files for the referenced outline assets.
2. Make the unified harness load `outline_model` fixture JSON for
   `ftoutln.outline_render` instead of relying on unknown-ID square fallback.
3. Keep existing hardcoded generated render topologies only where no fixture file
   exists and the ID is explicitly generated by the harness.
4. Run focused parity with:

   ```bash
   make -C pillow-rs-freetype test-case CASE=ftimage.FT_RASTER_FLAG_AA
   ```

   then broaden to:

   ```bash
   make -C pillow-rs-freetype test-op OP=ftoutln.outline_render
   ```

5. If all ABIs match pinned C for the newly runnable rows, run:

   ```bash
   make -C pillow-rs-freetype test-ffi
   make -C pillow-rs-freetype test-ffi-compat
   make -C pillow-rs-freetype fmt
   make -C pillow-rs-freetype clippy
   ```

Status: in progress.

Verified progress:

- Added real `outline_model` fixture
  `outlines/synthetic/simple-rectangle.json`.
- Unified harness now loads `outline_model` assets for `ftoutln.outline_render`
  rows instead of silently falling back to a hardcoded square when a referenced
  fixture file exists.
- Pinned C oracle now emits the actual
  `ftimage.FT_RASTER_FLAG_AA.smooth_requires_aa` flag matrix:
  `FT_RASTER_FLAG_DEFAULT` and `FT_RASTER_FLAG_AA`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for
  `FT_RASTER_FLAG_AA` outline-render behavior:
  no-AA gray targets render packed mono bytes and return success; AA mono
  targets return `FT_Err_Cannot_Render_Glyph`.
- Added real `outline_model` fixture
  `outlines/synthetic/thin-diagonal-stems.json`.
- Unified harness now recognizes `outline_flags_matrix` separately from raster
  `flags_matrix`, so rows that vary `FT_Outline.flags` compare the intended
  public field instead of staying pending for missing raster flags.
- Pinned C oracle now emits the
  `ftimage.FT_OUTLINE_HIGH_PRECISION.raster_hint_behavior` outline flag matrix:
  `FT_OUTLINE_NONE` and `FT_OUTLINE_HIGH_PRECISION`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for all six
  `FT_OUTLINE_HIGH_PRECISION` synthetic diagonal-stem variants.
- Added real `outline_model` fixture
  `outlines/synthetic/large-render-limits.json`.
- Pinned C oracle now emits the
  `ftimage.FT_OUTLINE_SINGLE_PASS.large_outline_raster_hint_behavior` outline
  flag matrix: `FT_OUTLINE_NONE` and `FT_OUTLINE_SINGLE_PASS`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for all six
  synthetic `FT_OUTLINE_SINGLE_PASS` `FT_Outline_Render` variants. This does
  not claim glyph-slot mono `FT_Render_Glyph` parity for `OUTLINE_SINGLE_PASS`;
  that separate surface is still tracked below.
- Added real `outline_model` fixture
  `outlines/synthetic/simple-overlap-thin-matrix.json`.
- Unified harness now loads `synthetic_outlines` outline-model assets and
  supports a single `outline_flags` list for `FT_Outline.flags`.
- Pinned C oracle now renders the same overlap/thin-stem topology for
  `ftimage.FT_OUTLINE_NONE.default_outline_render_baseline`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for all three
  synthetic `FT_OUTLINE_NONE` baseline render variants.
- Added real `outline_model` fixtures
  `outlines/render/simple-filled-square.json` and
  `outlines/render/direct-spans-clipped.json`.
- Pinned C oracle now records the
  `ftoutln.FT_Outline_Render.direct_render_clip_and_spans` direct-span route
  instead of returning bitmap fallback output.
- Rust FFI, C ABI, and WASM ABI direct-span test payloads now include the
  observed public `clip_box`, matching pinned FreeType's no-`CLIP` cbox preset
  behavior for this direct render row.
- Added real `outline_model` fixture
  `outlines/synthetic/crossing-clip-boundaries.json`.
- Pinned C oracle now records the two `FT_RASTER_FLAG_CLIP` direct-render rows:
  caller-supplied `CLIP` bounds and no-`CLIP` CBox preset behavior.
- Rust direct-span rendering now passes the caller clip box to the gray
  rasterizer when `FT_RASTER_FLAG_CLIP` is set, presets the integer-pixel CBox
  from `FT_Outline_Get_CBox` when `CLIP` is absent, preserves signed
  `FT_Span.x` bit patterns for negative direct spans, and skips target-buffer
  writes in direct callback mode.
- Added real `outline_model` fixture
  `outlines/synthetic/simple-non-overlap.json`.
- Pinned C oracle now emits the
  `ftimage.FT_OUTLINE_OVERLAP.non_overlap_no_spurious_change` outline flag
  matrix: `FT_OUTLINE_NONE` and `FT_OUTLINE_OVERLAP`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for the synthetic
  non-overlapping outline; `FT_OUTLINE_OVERLAP` causes no spurious bitmap
  change for this non-overlap input.
- Added real `outline_model` fixtures
  `outlines/render/empty-outline.json` and
  `outlines/synthetic/empty-outline.json`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for the explicit
  `FT_Outline_Render` empty-outline route and the public
  `FT_Outline.empty_outline_success` row. The same synthetic fixture also
  keeps the `FT_PIXEL_MODE_NONE.empty_bitmap_state` route exact.
- Added real `outline_model` fixture
  `outlines/synthetic/clip-sensitive-rectangle.json`.
- Pinned C oracle now emits the exact
  `ftimage.FT_Raster_Params.clip_box_matches_c` direct-render matrix:
  no-`CLIP` with a sentinel initial `clip_box`, and `CLIP` with caller bounds
  `{1,2,8,10}`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType's
  `FT_Outline_Render` side effect: direct no-`CLIP` calls mutate
  `params.clip_box` to the outline CBox in integer pixels before rasterizing,
  while direct `CLIP` calls preserve the caller-provided bounds.
- Added real `outline_model` fixture `outlines/cbox/mixed-extrema.json`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for
  `ftoutln.FT_Outline_Get_CBox.null_inputs_noop`: a null outline pointer
  leaves the caller's sentinel `FT_BBox` unchanged, and a null output pointer
  performs no write.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_RASTER_FLAG_AA
```

Result: `5 / 5` runtime parity rows passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_OUTLINE_HIGH_PRECISION
```

Result: `7 / 7` runtime parity rows passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_OUTLINE_SINGLE_PASS
```

Result: `7 / 7` runtime parity rows passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_OUTLINE_NONE
```

Result: `4 / 4` runtime parity rows passed, `0` failed, `0` pending.

Full non-coverage result after the `FT_Outline_Get_CBox` null-input fixture:

```bash
make -C pillow-rs-freetype test
```

Result: `7132 / 7132` runnable rows passed, `0` failed, `102` pending. Route
audit: `real-parity` `3715`, `pending-route` `94`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Raster_Params.clip_box_matches_c
```

Result: `1 / 1` runtime parity rows passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Raster_Params
```

Result: `5 / 5` runtime parity rows passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Outline_Get_CBox.null_inputs_noop
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Outline_Get_CBox
```

Result: `3 / 3` runtime parity rows passed, `0` failed, `0` pending.

Broadened non-coverage result:

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_render
```

Result after the clip-box direct-render parity fix: `79 / 79` runnable rows
passed, `0` failed, `10` pending.

Current remaining `ftoutln.outline_render` blockers are missing explicit
fixtures or non-fixture public harness surfaces, led by:
`dropout-thin-stems-scantype.json`, `cw-ccw-orientation-pairs.json`, and
`params-logging-renderer.json`. Keep fixing these as separate exact C oracle
routes; do not reintroduce generic square fallback for missing assets.

### Issue Set B: `FT_RASTER_FLAG_DIRECT` direct-span callback parity

Current focused command:

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_render
```

Original result before this issue set: `54 / 58` runnable rows passed, `4`
failed, `31` remained pending.

Failing direct-span rows:

- `ftimage.FT_Raster_Params.direct_span_render_matches_c`
- `ftimage.FT_Span.direct_span_values_match_c`
- `ftimage.FT_RASTER_FLAG_DIRECT.direct_gray_span_callback`
- `ftimage.FT_RASTER_FLAG_DIRECT.direct_missing_callback_noop`

Fix plan:

1. Add pinned C oracle support for direct-span rows using a real
   `FT_SpanFunc` callback that records `y`, `x`, `len`, `coverage`, and whether
   the user pointer was observed. Do not infer the expected spans from Rust.
2. Add a pure-Rust direct-span model in `fontdone` core that derives spans from
   the existing gray rasterizer without calling native FreeType.
3. Keep C ABI and WASM ABI thin: they may validate/copy callback metadata and
   invoke/copy core-produced spans, but must not contain raster logic.
4. Verify the four direct-span rows with the focused operation command above.
5. Only after direct rows are real green, broaden to the whole
   `ftoutln.outline_render` operation and then the normal FFI/API guards.

Verified progress:

- C oracle now records direct spans through a native `FT_SpanFunc`; the
  simple-rectangle direct callback emits two spans per covered row, matching
  FreeType's gray sweep first-cell/tail-span behavior.
- Rust gray rasterizer now exposes sweep-level direct spans rather than
  reconstructing spans from bitmap bytes.
- Rust FFI, C ABI, and WASM ABI direct render paths now match the oracle for
  `FT_RASTER_FLAG_DIRECT`.

Focused non-coverage results:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_RASTER_FLAG_DIRECT
```

Result: `4 / 4` runtime parity rows passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_render
```

Result: `58 / 58` runnable rows passed, `0` failed, `31` pending fixture rows.

### Issue Set C: `FT_Outline_Decompose` callback trace route

Newly visible after adding `outlines/synthetic/simple-rectangle.json`:

- `ftimage.FT_Outline_Funcs.shift_delta_transform_matches_c`

Current status: verified for the simple-rectangle callback trace, the
line/conic/cubic event-order trace, and callback error propagation. Other
`ftoutln.outline_decompose` rows remain explicitly pending unless they have a
maintained C oracle route and matching Rust/C/WASM callback trace. The rows
must not be treated as runnable via the generic fallback oracle because that
returns `FT_Err_Unimplemented_Feature` instead of a real callback event trace.

Fix plan:

1. Add native C oracle support for `FT_Outline_Decompose` callback recording:
   event kind, transformed points, callback order, `shift`, `delta`, and
   user-pointer observation.
2. Add a pure-Rust core callback trace path that applies FreeType's
   `(coord << shift) - delta` transform exactly.
3. Add thin C ABI and WASM ABI test-support routes that expose the same
   callback event trace without implementing outline walking in wrappers.
4. Move the runtime classifier from pending to runnable only after all three
   backends match pinned C output.

Verified progress:

- Native C oracle now records `FT_Outline_Decompose` callback events for
  `ftimage.FT_Outline_Funcs.shift_delta_transform_matches_c`, including event
  kind, transformed callback points, `shift`, `delta`, and user-pointer
  observation.
- `fontdone` core now exposes a pure-Rust callback-trace route for the same
  outline walking behavior and FreeType's `(coord << shift) - delta` callback
  transform.
- C ABI and WASM ABI expose only feature-gated test-support routes that delegate
  the trace to core; wrappers do not own outline walking logic.
- Added real `outline_model` fixture
  `outlines/decompose/line-conic-cubic.json`.
- Native C oracle now records the
  `ftoutln.FT_Outline_Decompose.line_conic_cubic_event_order` callback trace
  with line, consecutive-conic, and cubic contours.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for that trace row.
  The harness accepts both maintained transform parameter shapes:
  `shift_delta_cases[]` and the public `funcs { shift, delta }` form.
- Native C oracle now records
  `ftoutln.FT_Outline_Decompose.callback_error_propagates`: when a callback
  returns `FT_Err_Invalid_Argument` (`0x06`), FreeType stops immediately and
  returns that same error with only the events emitted before the failing
  callback.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for that callback
  error row. The generic no-font expected-error fallback explicitly does not
  intercept `ftoutln.outline_decompose`, because these rows are outline-only
  and still require real callback routing.
- Added real `outline_model` fixture
  `outlines/decompose/scaled-delta-square.json`.
- Native C oracle now records
  `ftoutln.FT_Outline_Decompose.shift_delta_applied_to_callbacks` with
  `FT_Outline_Funcs.shift = 2` and `delta = 7`. The first divergence during
  this conversion was in the new oracle metadata: callbacks used the correct
  function table values, but the JSON initially printed the default
  `shifts[]/deltas[]` entry. The oracle now prints `funcs.shift` and
  `funcs.delta`, matching the values passed to pinned FreeType.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for the shift/delta
  callback transform row.
- Added real `outline_model` fixture
  `outlines/synthetic/conic-single-and-consecutive.json`.
- Native C oracle now records
  `ftimage.FT_CURVE_TAG_CONIC.conic_decomposition_matches_c` with a single
  conic segment and consecutive conic controls. The first divergence during
  this conversion was oracle matrix width: the manifest row defines two
  `shift_delta_cases`, while the oracle defaulted to the three transform cases
  used by `FT_Outline_Funcs.shift_delta_transform_matches_c`. The oracle now
  emits the same two transform runs as the manifest row.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for the conic
  callback row, including FreeType's implied midpoint for consecutive conic
  controls.
- Added real `outline_model` fixture
  `outlines/synthetic/mixed-line-conic-cubic.json`.
- Native C oracle now records
  `ftimage.FT_Outline_Funcs.callback_order_matches_c` against that mixed
  outline, covering line callbacks, consecutive-conic callbacks, cubic
  callbacks, contour closure callbacks, and user-pointer observation.
- Native C oracle now records
  `ftimage.FT_Outline_Funcs.callback_error_propagates` for the same mixed
  outline. It injects callback return `123` at the first `move_to`, `line_to`,
  `conic_to`, and `cubic_to` callbacks. FreeType stops before recording the
  failing callback and returns that exact callback value. The manifest row now
  sets `compare_error_output: true` so this is exact error-output parity, not a
  generic expected-error fallback.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for both
  `FT_Outline_Funcs` callback-order and callback-error rows.
- Added real `outline_model` fixture
  `outlines/synthetic/on-curve-lines-multicontour.json`.
- Native C oracle now records
  `ftimage.FT_CURVE_TAG_ON.on_curve_decomposition_matches_c` with two
  line-only on-curve contours and the row's two transform cases. This pins
  FreeType's behavior that each contour starts with `move_to`, every on-curve
  point emits `line_to`, and each contour closes back to its start point.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for the on-curve
  multi-contour row.
- Added real cubic fixtures:
  `outlines/synthetic/cubic-paired-controls.json` and
  `outlines/synthetic/cubic-malformed.json`.
- Native C oracle now records
  `ftimage.FT_CURVE_TAG_CUBIC.cubic_decomposition_matches_c` with paired cubic
  controls and the row's two transform cases. The same row also records
  `malformed_status` by calling pinned FreeType on `contour_starts_with_cubic`
  and `unpaired_cubic_control`; both malformed cases return
  `FT_Err_Invalid_Outline`.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for the cubic trace
  and malformed cubic status row.
- Added real `outline_model` fixture
  `outlines/synthetic/tags-with-touch-and-scan-bits.json`.
- Native C oracle now records
  `ftimage.FT_CURVE_TAG.classifies_outline_tags` with high curve-tag bits mixed
  into on-curve, conic, and cubic tags. This pins FreeType's public
  `FT_CURVE_TAG(flag)` behavior from `ftimage.h`: only the low two bits select
  the curve type, while `TOUCH_X`, `TOUCH_Y`, and `HAS_SCANMODE` remain stored
  in the tag byte.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for the high-bit tag
  classification trace and the emitted `masked_tags` vector.
- Added real `outline_model` fixture
  `outlines/multi-contour-negative-coordinates.json`.
- Native C oracle now records both concrete variants of
  `ftimage.FT_Outline_MoveTo_Func.decompose_starts_each_contour` against a
  three-contour outline containing negative coordinates, line callbacks, conic
  callbacks, and cubic callbacks. The first divergence during this conversion
  was asset selection: the Rust/C-ABI/WASM runtime loaded the default square
  while pinned C used the new negative-coordinate outline. The harness now
  treats the existing `synthetic_outline` asset key as an `outline_model`
  fixture key. The second divergence was oracle transform width: the C oracle
  initially emitted one run while the public row defines two
  `shift_delta_matrix` entries. The oracle now emits both runs.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for both MoveTo
  contour-start variants and both callback coordinate transforms.

### Issue Set D: `FT_Get_TrueType_Engine_Type` missing module lifecycle

Previous blocker:

- `ftmodapi.FT_Get_TrueType_Engine_Type.missing_truetype_module_returns_none`
  was classified as `pending-route` because the runtime and oracle only modeled
  a null library or a default `FT_Init_FreeType` library.

Verified progress:

- Native C oracle now constructs a library with `FT_New_Library` and no
  `FT_Add_Default_Modules`, then records `FT_Get_Module(library, "truetype")`
  and `FT_Get_TrueType_Engine_Type`.
- Pinned FreeType returns `FT_TRUETYPE_ENGINE_TYPE_NONE` when the library lacks
  the `truetype` module.
- `fontdone` now tracks whether an `FT_Library` has the TrueType module instead
  of treating every non-null library as bytecode-interpreter capable.
- C ABI and WASM ABI remain thin test routes for this fixture: they construct
  or observe the core library state and do not implement module or engine
  behavior in the wrappers.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Get_TrueType_Engine_Type
```

Result: `3 / 3` runtime parity rows passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-op OP=ftmodapi.get_truetype_engine_type
```

Result: `6 / 6` runtime parity rows passed, `0` failed, `0` pending.

### Issue Set E: `FT_Set_Debug_Hook` slot mutation and no-op behavior

Previous blocker:

- `ftmodapi.FT_Set_Debug_Hook` rows were placeholder/fallback routes even
  though the C behavior is a bounded public slot mutation:
  `library && debug_hook && hook_index < 4`.

Verified progress:

- Native C oracle now records FreeType's `library->debug_hooks[4]` slot state
  for valid hook storage, null-library no-op, invalid-index no-op, and null-hook
  no-op.
- `fontdone` now models the four public debug-hook slots in `FT_Library` and
  implements `FT_Set_Debug_Hook` with the same three C preconditions from
  `freetype/src/base/ftobjs.c:FT_Set_Debug_Hook`.
- C ABI exposes the public `FT_Set_Debug_Hook` symbol as a thin wrapper that
  delegates to core state.
- WASM and C ABI test-support observation routes expose only hook identity
  classes for parity comparison; they do not implement interpreter/debugger
  behavior in wrappers.
- The separate
  `ftmodapi.FT_DEBUG_HOOK_TRUETYPE.debug_hook_index_import_contract` row still
  needs the missing `fonts/truetype/bytecode-debug-hook.ttf` fixture before
  hook invocation during glyph loading can be claimed as real parity. It remains
  fallback evidence and is not counted as a real-parity route by this fix.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Set_Debug_Hook
```

Result: `3 / 3` exact `FT_Set_Debug_Hook` runtime parity rows passed, `0`
failed, `0` pending.

```bash
make -C pillow-rs-freetype test-op OP=ftmodapi.set_debug_hook
```

Result: `4 / 4` operation-filtered rows passed, `0` failed, `0` pending. This
includes the unchanged generic fallback row for
`FT_DEBUG_HOOK_TRUETYPE.debug_hook_index_import_contract`; only the three
`FT_Set_Debug_Hook` rows are classified as real parity.

### Issue Set F: `FT_Add_Default_Modules` null-library no-return behavior

Previous blocker:

- `ftmodapi.FT_Add_Default_Modules.null_library_no_return_error` was classified
  as `void-fallback`, so the harness accepted a generic void placeholder
  instead of comparing the exact no-return/no-crash C behavior.

Verified progress:

- Native C oracle now calls pinned FreeType `FT_Add_Default_Modules(NULL)` and
  records the public observable output: `return="void"`, `crashed=false`, and
  no observable writes.
- `fontdone` now exposes `FT_Add_Default_Modules`; null libraries are a no-op,
  matching C's swallowed `FT_Add_Module(NULL, ...)` errors inside the void API.
- C ABI exposes public `FT_Add_Default_Modules` as a thin wrapper.
- WASM test-support calls the same core function for the null-library route.
- The broader `installs_default_module_table` row still requires exact default
  module table/order modeling and remains fallback evidence. It is not claimed
  as real parity by this fix.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Add_Default_Modules
```

Result: `2 / 2` operation rows passed, `0` failed, `0` pending. Only
`null_library_no_return_error` is classified as real parity; the module-table
install row remains fallback.

```bash
make -C pillow-rs-freetype test-op OP=ftmodapi.add_default_modules
```

Result: `2 / 2` operation rows passed, `0` failed, `0` pending, with the same
classification split.

### Issue Set G: `FT_Done_FreeType` null-library exact error route

Previous blocker:

- `freetype.FT_Done_FreeType.error_null_library` was classified as
  `generic-error-fallback`, even though the row already had exact native C
  oracle, Rust FFI, C ABI, and WASM ABI comparisons for
  `FT_Done_FreeType(NULL)`.

Verified progress:

- The route audit now recognizes this row as real parity only when the public
  input is `freetype.done_freetype`, the case id is
  `freetype.FT_Done_FreeType.error_null_library`, and the library handle is
  explicitly null.
- Runtime behavior is unchanged: pinned C FreeType and the Rust/C/WASM routes
  all return `FT_Err_Invalid_Library_Handle` (`35`) for the null-library input.
- This is an audit-classification fix for an already exact route, not a
  placeholder or broader error fallback.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Done_FreeType.error_null_library
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `3732`, `generic-error-fallback` `461`.

```bash
make -C pillow-rs-freetype test-op OP=freetype.done_freetype
```

Result: `3 / 3` operation rows passed, `0` failed, `0` pending, with the
null-library error row classified as real parity.

### Issue Set H: `FT_Done_Face` null-face exact error route

Previous blocker:

- `freetype.FT_Done_Face.error_null_face` was classified as
  `generic-error-fallback`, so the audit did not distinguish the exact
  `FT_Done_Face(NULL)` comparison from permissive error fallback rows.

Verified progress:

- The route audit now recognizes only the explicit null-face row as real parity:
  operation `freetype.done_face`, case id
  `freetype.FT_Done_Face.error_null_face`, and `face` handle `null`.
- The fixture loader now requires exact error status/output comparison for
  null-handle `FT_Done_Face` and `FT_Done_FreeType` lifecycle rows.
- The broader foreign/poisoned handle row remains visible as incomplete safety
  policy work; it is not claimed as exact C dereference parity.
- Runtime behavior is unchanged: pinned C FreeType and the Rust/C/WASM routes
  all return `FT_Err_Invalid_Face_Handle` (`35`) for `FT_Done_Face(NULL)`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Done_Face.error_null_face
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `3733`, `generic-error-fallback` `460`.

```bash
make -C pillow-rs-freetype test-op OP=freetype.done_face
```

Result: `4 / 4` operation rows passed, `0` failed, `0` pending. The
foreign/poisoned handle row remains outside real-parity classification.

### Issue Set I: `FT_Get_Kerning` null-face/null-output exact error routes

Previous blocker:

- `freetype.FT_Get_Kerning.error_null_face_or_output@null-face` and
  `freetype.FT_Get_Kerning.error_null_face_or_output@null-output` were
  classified as `generic-error-fallback`, even though they already had
  dedicated pinned C oracle commands and Rust FFI, C ABI, and WASM ABI runners.

Verified progress:

- The fixture loader now requires exact error status/output comparison for
  expected-error `freetype.get_kerning` rows.
- The route audit now treats the two kerning null-error rows as real parity via
  the existing explicit C oracle and backend routes.
- Runtime behavior is unchanged:
  - `FT_Get_Kerning(NULL, ..., &akerning)` returns
    `FT_Err_Invalid_Face_Handle`.
  - `FT_Get_Kerning(face, ..., NULL)` returns `FT_Err_Invalid_Argument`.
  - Rust FFI, C ABI, and WASM ABI preserve the same status and output snapshots.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Get_Kerning.error_null_face_or_output
```

Result: `2 / 2` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `3735`, `generic-error-fallback` `458`.

```bash
make -C pillow-rs-freetype test-op OP=freetype.get_kerning
```

Result: `12 / 12` operation rows passed, `0` failed, `0` pending.

### Issue Set J: `FT_Get_SubGlyph_Info` null-slot exact error route

Previous blocker:

- `freetype.FT_Get_SubGlyph_Info.error_null_slot` was classified as
  `generic-error-fallback`, even though it already had a pinned C oracle
  command and Rust FFI, C ABI, and WASM ABI null-slot runners.

Verified progress:

- The fixture loader now requires exact error status/output comparison for
  expected-error `freetype.get_subglyph_info` rows.
- The route audit now treats the null-slot row as real parity through the
  existing explicit C oracle and backend routes.
- The null-output row remains a wrapper-null-validation route because pinned C
  dereferences those output pointers after slot/subglyph validation; it is not
  claimed as native C null-output parity.
- Runtime behavior is unchanged: pinned C FreeType and the Rust/C/WASM routes
  all return `FT_Err_Invalid_Slot_Handle` for
  `FT_Get_SubGlyph_Info(NULL, ...)`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Get_SubGlyph_Info.error_null_slot
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `3736`, `generic-error-fallback` `457`.

```bash
make -C pillow-rs-freetype test-op OP=freetype.get_subglyph_info
```

Result: `11 / 11` operation rows passed, `0` failed, `0` pending.

### Issue Set K: `FT_Load_Char` null-face exact error route

Previous blocker:

- `freetype.FT_Load_Char.error_null_face_or_invalid_flags.null_face` was
  classified as `generic-error-fallback`, even though it already had a pinned C
  oracle command and Rust FFI, C ABI, and WASM ABI null-face runners.

Verified progress:

- The fixture loader now requires exact error status/output comparison only for
  `load_char` rows whose `face` parameter is explicitly `null`.
- The route audit now treats that single null-face row as real parity through
  the existing explicit C oracle and backend routes.
- The reserved-load-flag asset variant under the same logical family remains
  `generic-error-fallback` until independently proven; this change does not
  claim broad `load_char` expected-error parity.
- Runtime behavior is unchanged: pinned C FreeType and the Rust/C/WASM routes
  all return `FT_Err_Invalid_Face_Handle` and null output for
  `FT_Load_Char(NULL, 65, FT_LOAD_DEFAULT)`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Load_Char.error_null_face_or_invalid_flags.null_face
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `3737`, `generic-error-fallback` `456`.

```bash
make -C pillow-rs-freetype test-op OP=load_char
```

Result: `1854 / 1854` operation rows passed, `0` failed, `0` pending.

### Issue Set L: `FT_Load_Glyph` null-face exact error route

Previous blocker:

- `freetype.FT_Load_Glyph.error_null_face_or_invalid_flags.null_face` was
  classified as `generic-error-fallback`, even though it already had a pinned C
  oracle command and Rust FFI, C ABI, and WASM ABI null-face runners.

Verified progress:

- The fixture loader now requires exact error status/output comparison only for
  `load_glyph` rows whose `face` parameter is explicitly `null`.
- The route audit now treats that single null-face row as real parity through
  the existing explicit C oracle and backend routes.
- Other `load_glyph` expected-error rows, including asset-backed invalid flag,
  bytecode, and malformed-font rows, remain in their existing categories until
  independently proven; this change does not claim broad `load_glyph`
  expected-error parity.
- Runtime behavior is unchanged: pinned C FreeType and the Rust/C/WASM routes
  all return `FT_Err_Invalid_Face_Handle` and null slot output for
  `FT_Load_Glyph(NULL, 36, FT_LOAD_DEFAULT)`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Load_Glyph.error_null_face_or_invalid_flags.null_face
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `3738`, `generic-error-fallback` `455`.

```bash
make -C pillow-rs-freetype test-op OP=load_glyph
```

Result: `587 / 587` operation rows passed, `0` failed, `0` pending.

### Issue Set N: `FT_Load_Char` reserved-load-flag exact error route

Previous blocker:

- `freetype.FT_Load_Char.error_null_face_or_invalid_flags` was classified as
  `generic-error-fallback`, even though the row used a real font asset and
  already had maintained pinned C oracle, Rust FFI, C ABI, and WASM ABI
  `load_char` routes.

Verified progress:

- The fixture loader now requires exact error status/output comparison for that
  concrete reserved-load-flag row.
- The route audit now treats the row as real parity through the existing
  explicit C oracle and backend routes.
- This change does not broaden all `load_char` expected-error rows; it claims
  only the concrete `FT_Load_Char` reserved-load-flag row plus the already
  verified null-face row.
- Runtime behavior matches pinned C FreeType for
  `FT_Load_Char(face, 65, 8388608)`: pinned C, Rust FFI, C ABI, and WASM ABI
  all return the same error status and slot snapshot.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Load_Char.error_null_face_or_invalid_flags
```

Result: `2 / 2` runtime parity rows passed, `0` failed, `0` pending. The
filter includes both the reserved-load-flag row and the already verified
`.null_face` row. Route audit: `real-parity` `3740`,
`generic-error-fallback` `453`.

```bash
make -C pillow-rs-freetype test-op OP=load_char
```

Result: `1854 / 1854` operation rows passed, `0` failed, `0` pending.

### Issue Set M: `FT_Err_Invalid_Face_Handle` uppercase NULL load-glyph route

Previous blocker:

- `fterrdef.FT_Err_Invalid_Face_Handle.face_api_rejects_null_face` was
  classified as `generic-error-fallback`.
- The row used `face: "NULL"` and also carried a font asset. The harness
  recognized lowercase `null` in the earlier `FT_Load_Glyph` row, but this
  uppercase spelling let the C oracle, C ABI, and WASM ABI routes open a real
  face and load glyph zero instead of exercising `FT_Load_Glyph(NULL, ...)`.

Verified progress:

- Public `load_glyph` routing now treats `null` and `NULL` as the same null
  handle for the pinned C oracle, Rust FFI, C ABI, and WASM ABI paths.
- The fixture loader now requires exact error status/output comparison for the
  `fterrdef.FT_Err_Invalid_Face_Handle.face_api_rejects_null_face` row.
- The route audit now treats that row as real parity through the corrected
  explicit C oracle and backend routes.
- Runtime behavior now matches pinned C FreeType: `FT_Load_Glyph(NULL, 0,
  FT_LOAD_DEFAULT)` returns `FT_Err_Invalid_Face_Handle` with null slot output.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Face_Handle.face_api_rejects_null_face
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `3739`, `generic-error-fallback` `454`.

```bash
make -C pillow-rs-freetype test-op OP=load_glyph
```

Result: `587 / 587` operation rows passed, `0` failed, `0` pending.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Outline_Funcs.shift_delta_transform_matches_c
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Outline_Funcs.callback_order_matches_c
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Outline_Funcs.callback_error_propagates
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Outline_Decompose.line_conic_cubic_event_order
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Outline_Decompose.shift_delta_applied_to_callbacks
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_CURVE_TAG_CONIC.conic_decomposition_matches_c
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_CURVE_TAG_ON.on_curve_decomposition_matches_c
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_CURVE_TAG_CUBIC.cubic_decomposition_matches_c
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_CURVE_TAG.classifies_outline_tags
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Outline_MoveTo_Func.decompose_starts_each_contour
```

Result: `2 / 2` runtime parity rows passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Outline_Decompose.callback_error_propagates
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_decompose
```

Result after the line/conic/cubic fixture, shift/delta fixture, conic fixture,
mixed callback fixture, on-curve fixture, cubic fixtures, callback-error
routes, high-bit curve-tag fixture, and MoveTo multi-contour fixture:
`12 / 12` runtime parity rows passed, `0` failed, `3` pending.

Full non-coverage result:

```bash
make -C pillow-rs-freetype test
```

Result after the line/conic/cubic fixture, shift/delta fixture, conic fixture,
mixed callback fixture, on-curve fixture, cubic fixtures, callback-error
routes, high-bit curve-tag fixture, and MoveTo multi-contour fixture:
`7143 / 7143` runnable rows passed, `0` failed, `91` pending. Route audit:
`real-parity` `3726`, `pending-route` `83`.

Full non-coverage result after the `FT_Get_TrueType_Engine_Type` missing-module
lifecycle route:

```bash
make -C pillow-rs-freetype test
```

Result: `7144 / 7144` runnable rows passed, `0` failed, `90` pending. Route
audit: `real-parity` `3727`, `pending-route` `82`, `pending-core` `7`.

Full non-coverage result after the `FT_Set_Debug_Hook` slot mutation/no-op
route:

```bash
make -C pillow-rs-freetype test
```

Result: `7144 / 7144` runnable rows passed, `0` failed, `90` pending. Route
audit: `real-parity` `3730`, `pending-route` `82`, `pending-core` `7`.

Full non-coverage result after the `FT_Add_Default_Modules` null-library route:

```bash
make -C pillow-rs-freetype test
```

Result: `7144 / 7144` runnable rows passed, `0` failed, `90` pending. Route
audit: `real-parity` `3731`, `pending-route` `82`, `pending-core` `7`, and
`void-fallback` removed from the route-audit categories.

Full non-coverage result after the `FT_Done_FreeType` null-library exact-error
route classification:

```bash
make -C pillow-rs-freetype test
```

Result: `7144 / 7144` runnable rows passed, `0` failed, `90` pending. Route
audit: `real-parity` `3732`, `generic-error-fallback` `461`, `pending-route`
`82`, and `pending-core` `7`.

Full non-coverage result after the `FT_Done_Face` null-face exact-error route
classification:

```bash
make -C pillow-rs-freetype test
```

Result: `7144 / 7144` runnable rows passed, `0` failed, `90` pending. Route
audit: `real-parity` `3733`, `generic-error-fallback` `460`, `pending-route`
`82`, and `pending-core` `7`.

Full non-coverage result after the `FT_Get_Kerning` null error route
classification:

```bash
make -C pillow-rs-freetype test
```

Result: `7144 / 7144` runnable rows passed, `0` failed, `90` pending. Route
audit: `real-parity` `3735`, `generic-error-fallback` `458`, `pending-route`
`82`, and `pending-core` `7`.

Full non-coverage result after the `FT_Load_Char` reserved-load-flag error route
classification:

```bash
make -C pillow-rs-freetype test
```

Result: `7144 / 7144` runnable rows passed, `0` failed, `90` pending. Route
audit: `real-parity` `3740`, `generic-error-fallback` `453`, `pending-route`
`82`, and `pending-core` `7`.

Result: `7110 / 7110` runnable rows passed, `0` failed, `124` pending.

Baseline: `37d7dde4`

Source artifacts:

- `pillow-rs-freetype/target/api-abi-audit/route_audit.json`
- `pillow-rs-freetype/target/api-abi-audit/route_audit.md`

Generated with:

```bash
make -C pillow-rs-freetype route-audit
```

This table treats green placeholder routes as missing real parity. A row counts
as real parity only when the audit classifies it as an explicit C oracle, Rust
FFI, C ABI, and WASM route, or as an intentionally maintained validation route.
The missing set below includes:

- `generic-fallback`
- `generic-error-fallback`
- `null-error-fallback`
- `void-fallback`
- `explicit-unsupported`
- `pending-core`

It excludes `compile-contract`, `real-parity`, `real-null-validation`,
`raw-slot-null-validation`, and `wrapper-null-validation`.

## Audit Totals

| Category | Rows |
|---|---:|
| real-parity | 3840 |
| compile-contract | 2229 |
| generic-fallback | 817 |
| generic-error-fallback | 129 |
| real-null-validation | 9 |
| null-error-fallback | 6 |
| explicit-unsupported | 6 |
| pending-core | 6 |
| raw-slot-null-validation | 4 |
| void-fallback | 2 |
| wrapper-null-validation | 1 |

Missing real-parity rows: 966.

## Missing Rows By Subject Group

| Subject group | Missing rows | Fallback | Error fallback | Pending | Representative operations |
|---|---:|---:|---:|---:|---|
| `ftcolor` | 130 | 119 | 11 | 0 | `ftcolor.get_paint_graph`, `ftcolor.traverse_paint_graph`, `ftcolor.get_paint`, `ftcolor.palette_data_get` |
| `ftcache` | 112 | 110 | 2 | 0 | `ftcache.image_cache_lookup_scaler`, `ftcache.cmap_cache_lookup`, `ftcache.manager_lookup_size`, `ftcache.sbit_cache_lookup_scaler` |
| `ftstroke` | 86 | 72 | 14 | 0 | `ftstroke.export_border`, `ftstroke.open_path_geometry`, `ftstroke.join_geometry`, `ftstroke.parse_outline` |
| `ftmm` | 84 | 67 | 14 | 3 | `ftmm.get_mm_var`, `ftmm.get_multi_master`, `ftmm.get_var_blend_coordinates`, `ftmm.get_var_design_coordinates` |
| `fterrdef` | 54 | 39 | 15 | 0 | `FT_Open_Face`, `FT_Outline_Render`, `FT_Add_Module`, `FT_New_Face` |
| `ftmodapi` | 47 | 29 | 18 | 0 | `ftmodapi.inspect_module_flags`, `ftmodapi.add_module`, `ftmodapi.set_debug_hook`, `ftmodapi.property_get` |
| `freetype` | 43 | 28 | 14 | 1 | `freetype.face_properties`, `freetype.attach_file`, `freetype.attach_stream`, `freetype.active_size_handle` |
| `ftgxval` | 41 | 35 | 6 | 0 | `FT_TrueTypeGX_Validate`, `ftgxval.truetype_gx_validate`, `ftgxval.classic_kern_validate`, `ftgxval.classic_kern_free` |
| `ftoutln` | 32 | 26 | 6 | 0 | `ftoutln.outline_reverse`, `ftoutln.outline_transform`, `ftoutln.outline_translate`, `ftoutln.outline_get_orientation` |
| `ftwinfnt` | 31 | 28 | 3 | 0 | `winfnt.get_header`, `ftwinfnt.get_winfnt_header`, `ftwinfnt.get_winfnt_header_abi`, `ftwinfnt.winfnt_header_type_import` |
| `t1tables` | 31 | 31 | 0 | 0 | `t1tables.get_ps_font_private_mm_blend`, `t1tables.get_ps_font_value`, `t1tables.mm_blend_dictionary`, `t1tables.get_ps_font_value_encoding` |
| `ftlist` | 29 | 27 | 2 | 0 | `ftlist.list_finalize`, `ftlist.list_find`, `ftlist.list_iterate`, `ftlist.list_remove` |
| `ftimage` | 26 | 20 | 6 | 0 | `ftoutln.outline_get_bitmap`, `renderer.raster_render`, `freetype.load_svg_glyph`, `ftimage.custom_renderer_lifecycle` |
| `ftdriver` | 25 | 23 | 2 | 0 | `ftdriver.property_set_get`, `ftdriver.glyph_to_script_map`, `ftdriver.hinting_engine_property`, `ftdriver.interpreter_version_property` |
| `ftglyph` | 24 | 20 | 4 | 0 | `ftglyph.done_glyph`, `ftglyph.glyph_transform`, `ftglyph.new_glyph`, `ftglyph.type_runtime` |
| `ftotval` | 17 | 15 | 2 | 0 | `ftotval.open_type_validate`, `ftotval.open_type_validate_then_free` |
| `ftparams` | 16 | 14 | 2 | 0 | `freetype.open_face_with_params`, `freetype.face_properties_then_render`, `freetype.open_face_incremental`, `freetype.face_properties` |
| `ftcid` | 15 | 14 | 1 | 0 | `ftcid.get_cid_from_glyph_index`, `ftcid.get_cid_is_internally_cid_keyed`, `ftcid.get_cid_registry_ordering_supplement` |
| `ftincrem` | 15 | 14 | 1 | 0 | `ftincrem.load_incremental_glyph`, `ftincrem.callback_handle_identity`, `ftincrem.client_lifetime_model`, `ftincrem.validate_callback_table` |
| `ftlogging` | 14 | 14 | 0 | 0 | `ftlogging.set_log_handler`, `ftlogging.trace_set_level`, `ftlogging.trace_set_default_level`, `ftlogging.set_default_log_handler_abi` |
| `ftpfr` | 13 | 11 | 2 | 0 | `ftpfr.get_pfr_advance`, `ftpfr.get_pfr_metrics`, `ftpfr.get_pfr_kerning` |
| `ftrender` | 13 | 11 | 2 | 0 | `ftrender.set_renderer_then_render`, `ftrender.get_renderer`, `ftrender.set_renderer`, `ftrender.render_mode_acceptance` |
| `ttnameid` | 12 | 12 | 0 | 0 | `sfnt.charmap_and_name_metadata`, `sfnt.enumerate_charmaps_and_names`, `face.enumerate_charmaps`, `freetype.enumerate_charmaps` |
| `ftbdf` | 11 | 9 | 2 | 0 | `ftbdf.get_bdf_property`, `ftbdf.get_bdf_charset_id` |
| `ftmoderr` | 10 | 8 | 2 | 0 | `smooth.render_error_probe`, `raster.module_error_probe`, `sfnt.face_load_error_probe`, `sdf.render_error_probe` |
| `ftgzip` | 9 | 2 | 7 | 0 | `ftgzip.gzip_uncompress`, `ftgzip.stream_open_gzip` |
| `ftbzip2` | 6 | 4 | 2 | 0 | `ftbzip2.stream_open_bzip2`, `ftbzip2.stream_read`, `ftbzip2.stream_close` |
| `otsvg` | 6 | 6 | 0 | 0 | `otsvg.svg_document_type_import`, `otsvg.svg_document_type_abi`, `otsvg.svg_renderer_callback_capture`, `otsvg.svg_document_rec_abi` |
| `ftlzw` | 5 | 2 | 3 | 0 | `ftlzw.stream_open_lzw`, `ftlzw.stream_open_lzw_abi` |
| `ftsystem` | 4 | 4 | 0 | 0 | `ftsystem.open_face_with_external_stream`, `ftsystem.new_library_with_custom_memory`, `ftsystem.memory_stream_probe` |
| `tttables` | 3 | 2 | 0 | 1 | `face.load_then_get_sfnt_table.maxp`, `face.new`, `sfnt.get_sfnt_table.record` |
| `ftbitmap` | 1 | 0 | 0 | 1 | `ftbitmap.glyphslot_own_bitmap` |
| `fttypes` | 1 | 1 | 0 | 0 | `winfnt.get_header` |

## Top Operation Buckets

These are the highest-count placeholder-style rows grouped by operation and
route category. The complete generated operation list remains in
`target/api-abi-audit/route_audit.md`; this table records the largest
repo-visible buckets for handoff and subagent selection.

| Rows | Category | Subject group | Operation | Example subject/case | Blocker note |
|---:|---|---|---|---|---|
| 24 | `generic-fallback` | `ftcache` | `ftcache.image_cache_lookup_scaler` | `ftcache.FTC_ImageCache_LookupScaler / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 20 | `generic-fallback` | `ftcolor` | `ftcolor.get_paint_graph` | `ftcolor.FT_COLR_COMPOSITE_EXCLUSION / paint_composite_mode_runtime` | no explicit maintained route classification |
| 18 | `generic-fallback` | `ftwinfnt` | `winfnt.get_header` | `ftwinfnt.FT_WinFNT_ID_CP1250 / charset_roundtrip_from_header` | no explicit maintained route classification |
| 18 | `generic-fallback` | `ftcache` | `ftcache.cmap_cache_lookup` | `ftcache.FTC_CMapCache_Lookup / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 16 | `generic-fallback` | `ftgxval` | `FT_TrueTypeGX_Validate` | `ftgxval.FT_VALIDATE_GX / validates_all_requested_tables` | no explicit maintained route classification |
| 14 | `generic-fallback` | `ftotval` | `ftotval.open_type_validate` | `ftotval.FT_OpenType_Validate / selected_tables_success` | no explicit maintained route classification |
| 12 | `generic-fallback` | `ftcolor` | `ftcolor.traverse_paint_graph` | `ftcolor.FT_COLR_COMPOSITE_CLEAR / paint_composite_runtime` | no explicit maintained route classification |
| 11 | `generic-fallback` | `t1tables` | `t1tables.get_ps_font_private_mm_blend` | `t1tables.T1_BLEND_BLUE_SCALE / private_blue_scale_runtime_value` | no explicit maintained route classification |
| 10 | `generic-fallback` | `ftcolor` | `ftcolor.get_paint` | `ftcolor.FT_Affine23 / root_transform_values` | no explicit maintained route classification |
| 9 | `generic-fallback` | `ftgxval` | `ftgxval.truetype_gx_validate` | `ftgxval.FT_TrueTypeGX_Validate / validates_selected_gx_tables` | no explicit maintained route classification |
| 8 | `generic-fallback` | `ftcache` | `ftcache.manager_lookup_size` | `ftcache.FTC_Manager_LookupSize / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 8 | `generic-fallback` | `ftcid` | `ftcid.get_cid_from_glyph_index` | `ftcid.FT_Get_CID_From_Glyph_Index / cid_face_returns_cid` | no explicit maintained route classification |
| 8 | `generic-fallback` | `ftcolor` | `ftcolor.get_color_glyph_paint_and_get_paint` | `ftcolor.FT_PaintRadialGradient / get_paint_radial_gradient_values` | no explicit maintained route classification |
| 8 | `generic-fallback` | `ftimage` | `ftimage.outline_decompose` | `ftimage.FT_Curve_Tag_Conic / curve_tag_classifies_conic_points` | no explicit maintained route classification |
| 8 | `generic-fallback` | `ftmm` | `ftmm.get_mm_var` | `ftmm.FT_Get_MM_Var / variable_font_descriptor_success` | no explicit maintained route classification |
| 8 | `generic-fallback` | `ftparams` | `freetype.open_face_with_params` | `ftparams.FT_PARAM_TAG_IGNORE_SBIX / open_face_ignores_sbix` | no explicit maintained route classification |
| 7 | `generic-fallback` | `ftcache` | `ftcache.sbit_cache_lookup_scaler` | `ftcache.FTC_SBitCache_LookupScaler / rejects_null_sbit_or_scaler` | no explicit maintained route classification |
| 7 | `generic-fallback` | `ftcolor` | `ftcolor.palette_data_get` | `ftcolor.FT_PALETTE_FOR_DARK_BACKGROUND / palette_flags_runtime` | no explicit maintained route classification |
| 7 | `generic-fallback` | `ftimage` | `ftoutln.outline_get_bitmap` | `ftimage.FT_Bitmap / empty_bitmap_is_valid` | no explicit maintained route classification |
| 7 | `generic-fallback` | `ftstroke` | `ftstroke.export_border` | `ftstroke.FT_STROKER_BORDER_LEFT / left_border_export_geometry` | no explicit maintained route classification |
| 6 | `explicit-unsupported` | `freetype` | `freetype.face_properties` | `freetype.FT_Face_Properties / success_supported_face_properties` | explicit Rust stub returns Unimplemented_Feature |
| 6 | `generic-fallback` | `ftmodapi` | `ftmodapi.inspect_module_flags` | `ftmodapi.FT_MODULE_DRIVER_HAS_HINTER / present_on_native_hinter_drivers` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftrender` | `ftrender.set_renderer_then_render` | `ftrender.FT_Set_Renderer / render_output_changes_with_current_renderer` | no explicit maintained route classification |
| 6 | `generic-fallback` | `t1tables` | `t1tables.get_ps_font_value` | `t1tables.FT_Get_PS_Font_Value / signature_and_behavior_matrix` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftimage` | `ftoutln.outline_get_bitmap` | `ftimage.FT_Bitmap / empty_bitmap_is_valid` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftcache` | `ftcache.image_cache_lookup` | `ftcache.FTC_ImageCache_Lookup / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftcache` | `ftcache.manager_lookup_face` | `ftcache.FTC_Manager_LookupFace / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftcache` | `ftcache.manager_new` | `ftcache.FTC_Manager_New / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftcache` | `ftcache.manager_remove_face_id` | `ftcache.FTC_Manager_RemoveFaceID / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftcolor` | `ftcolor.get_color_glyph_paint_then_get_paint` | `ftcolor.FT_COLOR_INCLUDE_ROOT_TRANSFORM / include_transform_runtime` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftcolor` | `ftcolor.get_gradient_paint_and_stops` | `ftcolor.FT_COLR_PAINTFORMAT_LINEAR_GRADIENT / paint_linear_gradient_payload` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftcolor` | `ftcolor.palette_select` | `ftcolor.FT_Color / palette_entries_preserve_bgra_order` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftcolor` | `ftcolor.get_colorline_stops` | `ftcolor.FT_ColorStop / iterator_output_values` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftmodapi` | `ftmodapi.inspect_module_flags` | `ftmodapi.FT_MODULE_DRIVER_HAS_HINTER / present_on_native_hinter_drivers` | no explicit maintained route classification |
| 6 | `generic-fallback` | `ftrender` | `ftrender.set_renderer_then_render` | `ftrender.FT_Set_Renderer / render_output_changes_with_current_renderer` | no explicit maintained route classification |
| 6 | `generic-fallback` | `t1tables` | `t1tables.get_ps_font_value` | `t1tables.FT_Get_PS_Font_Value / signature_matches_header` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftbdf` | `ftbdf.get_bdf_charset_id` | `ftbdf.FT_Get_BDF_Charset_ID / success_bdf_face_charset` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftcache` | `ftcache.cmap_cache_new` | `ftcache.FTC_CMapCache_New / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftcache` | `ftcache.image_cache_new` | `ftcache.FTC_ImageCache_New / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftcache` | `ftcache.manager_done` | `ftcache.FTC_Manager_Done / planned_cache_subsystem_not_out_of_scope` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftcolor` | `ftcolor.get_normalized_transform_paint` | `ftcolor.FT_COLR_PAINTFORMAT_ROTATE / paint_rotate_normalized_payload` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftcolor` | `ftcolor.get_color_glyph_clipbox` | `ftcolor.FT_ClipBox / color_glyph_clipbox_values` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftglyph` | `ftglyph.done_glyph` | `ftglyph.FT_BitmapGlyphRec / owns_bitmap_buffer` | no explicit maintained route classification |
| 5 | `generic-error-fallback` | `ftimage` | `ftoutln.outline_decompose` | `ftimage.FT_Outline / invalid_outline_errors` | no-asset expected-error row |
| 5 | `generic-fallback` | `ftmm` | `ftmm.get_var_design_coordinates` | `ftmm.FT_Get_Var_Design_Coordinates / success_default_design_coordinates` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftmm` | `ftmm.set_mm_blend_coordinates` | `ftmm.FT_Set_MM_Blend_Coordinates / success_set_normalized_coordinates` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftmm` | `ftmm.set_mm_weight_vector` | `ftmm.FT_Set_MM_WeightVector / success_set_weight_vector` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftmm` | `ftmm.set_var_blend_coordinates` | `ftmm.FT_Set_Var_Blend_Coordinates / success_aliases_mm_blend_setter` | no explicit maintained route classification |
| 5 | `generic-fallback` | `ftmm` | `ftmm.set_var_design_coordinates` | `ftmm.FT_Set_Var_Design_Coordinates / success_set_design_coordinates` | no explicit maintained route classification |

## Pending-Core Rows

| Subject | Operation | Case | Dependency blocking real route |
|---|---|---|---|
| `freetype.FT_Render_Glyph` | `render_glyph` | `error_unloaded_or_unsupported_slot_format.unrouted_slot_states` | Unloaded and unsupported synthetic glyph-slot states need explicit public runner support. |
| `ftbitmap.FT_GlyphSlot_Own_Bitmap` | `glyphslot_own_bitmap` | allocation failure | Deterministic allocator fault injection must be maintained before this row can run as real parity. |
| `ftmm.FT_Set_Named_Instance` | `ftmm.set_named_instance` | `success_adobe_mm_resets_default` | Adobe MM named-instance reset requires real Adobe MM support. |
| `ftmm.FT_Set_Named_Instance` | `ftmm.set_named_instance` | `output_changes_to_named_instance` | Named-instance glyph-output parity requires `gvar`/`HVAR` support. |
| `ftmm.FT_Var_Named_Style` | `ftmm.set_named_instance` | `selected_instance_matches_descriptor` | Named-style coordinate parity requires `FT_MM_Var` support. |
| `tttables.TT_VertHeader` | `sfnt.get_sfnt_table.record` | `sfnt_table_present_runtime.mvar_variation` | `MVAR` variation table behavior must be implemented before this SFNT table row can run. |

### Issue Set O: `FT_Set_Char_Size` null-face exact-error route

Previous blocker:

- `freetype.FT_Set_Char_Size.error_null_face` was classified as
  `generic-error-fallback` even though the fixture expected exact
  `Invalid_Face_Handle` behavior. The runtime harness accepted the expected
  error without exact C status/output comparison.

Verified progress:

- The pinned C oracle now has a maintained `--set-char-size null ...` path that
  calls `FT_Set_Char_Size(NULL, 768, 768, 72, 72)` and records the native
  `FT_Err_Invalid_Face_Handle` result.
- The unified harness now requires exact error status/output comparison for
  `set_char_size` rows with a null `face`.
- The route audit now classifies
  `freetype.FT_Set_Char_Size.error_null_face` as `real-parity`, validated
  through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Char_Size.error_null_face
make -C pillow-rs-freetype test-op OP=set_char_size
```

### Issue Set P: `FT_Select_Size` null-face exact-error route

Previous blocker:

- `freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face@null-face`
  stayed in `generic-error-fallback` even though the fixture expected exact
  `Invalid_Face_Handle` behavior. The runtime harness accepted the expected
  error without exact C status/output comparison.

Verified progress:

- The existing pinned C oracle `--select-size-null` route calls
  `FT_Select_Size(NULL, 0)` and records the native
  `FT_Err_Invalid_Face_Handle` result.
- The unified harness now requires exact error status/output comparison for
  `freetype.select_size` rows with a null `face`.
- The route audit now classifies only the null-face
  `freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face` variant as
  `real-parity`, validated through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI. The no-fixed-size and strike-index error variants remain
  separate rows and were not promoted by this change.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face@null-face
make -C pillow-rs-freetype test-op OP=freetype.select_size
```

### Issue Set Q: `FT_Set_Pixel_Sizes` null-face exact-error route

Previous blocker:

- `freetype.FT_Set_Pixel_Sizes.error_null_face` stayed in
  `generic-error-fallback` even though the fixture expected exact
  `Invalid_Face_Handle` behavior. The runtime harness accepted the expected
  error without exact C status/output comparison.
- The oracle argument builder used `0,0` for the null-face request instead of
  the fixture's public input `12,12`, and the pinned C helper rejected `null`
  as an unsupported source kind.

Verified progress:

- The pinned C oracle now calls `FT_Set_Pixel_Sizes(NULL, 12, 12)` for the
  exact fixture input and records the native `FT_Err_Invalid_Face_Handle`
  result.
- The unified harness now requires exact error status/output comparison for
  `set_pixel_sizes` rows with a null `face`.
- The route audit now classifies
  `freetype.FT_Set_Pixel_Sizes.error_null_face` as `real-parity`, validated
  through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Pixel_Sizes.error_null_face
make -C pillow-rs-freetype test-op OP=set_pixel_sizes
```

### Issue Set R: `FT_Select_Charmap` null-face exact-error route

Previous blocker:

- `freetype.FT_Select_Charmap.error_null_face` stayed in
  `generic-error-fallback` even though the fixture expected exact
  `Invalid_Face_Handle` behavior. The runtime harness accepted the expected
  error without exact C status/output comparison.

Verified progress:

- The pinned C oracle `--select-charmap-null-face` route calls
  `FT_Select_Charmap(NULL, FT_ENCODING_UNICODE)` for the exact fixture input
  and records the native `FT_Err_Invalid_Face_Handle` result.
- The unified harness now requires exact error status/output comparison for
  `freetype.select_charmap` rows with a null `face`.
- The route audit now classifies
  `freetype.FT_Select_Charmap.error_null_face` as `real-parity`, validated
  through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Select_Charmap.error_null_face
make -C pillow-rs-freetype test-op OP=freetype.select_charmap
```

### Issue Set S: `FT_Set_Charmap` null-face exact-error route

Previous blocker:

- `freetype.FT_Set_Charmap.error_null_face` stayed in
  `generic-error-fallback` even though the fixture expected exact
  `Invalid_Face_Handle` behavior. The runtime harness accepted the expected
  error without exact C status/output comparison.

Verified progress:

- The pinned C oracle `--set-charmap-null-face` route calls
  `FT_Set_Charmap(NULL, NULL)` and records the native
  `FT_Err_Invalid_Face_Handle` result plus the null-row output payload.
- The unified harness now requires exact error status/output comparison for
  `freetype.set_charmap` rows with a null `face`.
- The route audit now classifies
  `freetype.FT_Set_Charmap.error_null_face` as `real-parity`, validated
  through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Charmap.error_null_face
make -C pillow-rs-freetype test-op OP=freetype.set_charmap
```

### Issue Set T: `FT_Request_Size` null-face/null-request exact-error route

Previous blocker:

- `freetype.FT_Request_Size.error_null_face_or_request` stayed in
  `generic-error-fallback` even though the fixture expected exact pinned-C
  error behavior for both null `face` and null `request` variants. The runtime
  harness accepted the expected error without exact C status/output comparison.
- First divergence: the C oracle dispatcher only routed `request` and
  `requests` payloads through `--request-size`, while this public case used
  `variants`. Turning on exact comparison exposed the fallback immediately:
  the fallback returned status `7`, while the real Rust path returned the
  FreeType null-face status `35`.

Verified progress:

- The pinned C oracle `emit_request_size` route calls `FT_Request_Size` for
  each request row, including `FT_Request_Size(NULL, &request)` and
  `FT_Request_Size(face, NULL)`, and records the first native error plus each
  row's status/error payload.
- The oracle dispatcher now sends `variants` rows through that same
  `--request-size` route instead of falling back.
- The Rust FFI, thin C ABI, and WASM runners already route those same null
  input variants directly; the unified harness now requires exact error
  status/output comparison for this concrete public case.
- The route audit now classifies
  `freetype.FT_Request_Size.error_null_face_or_request` as `real-parity`,
  validated through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Request_Size.error_null_face_or_request
make -C pillow-rs-freetype test-op OP=freetype.request_size
```

### Issue Set U: `FT_Request_Size` ppem-overflow exact-error route

Previous blocker:

- `freetype.FT_Request_Size.error_ppem_overflow` stayed in
  `generic-error-fallback` even though the fixture expected exact
  `FT_Err_Invalid_Pixel_Size` behavior for a very large nominal request. The
  runtime harness accepted the expected error without exact C status/output
  comparison.

Verified progress:

- The pinned C oracle `emit_request_size` route calls `FT_Request_Size` with
  the oversized nominal request and records the native error plus the row
  status/metrics payload.
- FreeType 2.14.3 `FT_Request_Size` dispatches through
  `src/base/ftobjs.c:3438-3505`; the Rust implementation already matched the
  observed ppem-overflow error through Rust FFI, thin C ABI, and WASM ABI.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Request_Size.error_ppem_overflow` as `real-parity`, validated
  through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Request_Size.error_ppem_overflow
```

### Issue Set V: `FT_SIZE_REQUEST_TYPE_MAX` sentinel exact-error route

Previous blocker:

- `freetype.FT_SIZE_REQUEST_TYPE_MAX.request_size_rejects_sentinel` stayed in
  `generic-error-fallback` even though it exercises the public
  `FT_Request_Size` path with the sentinel enum value. The runtime harness
  accepted the expected error without exact pinned-C status/output comparison.

Verified progress:

- The pinned C oracle `emit_request_size` route calls `FT_Request_Size` with
  the sentinel `FT_SIZE_REQUEST_TYPE_MAX` request type and records the native
  error plus the row status/metrics payload.
- FreeType 2.14.3 `FT_Request_Size` rejects request types at or beyond
  `FT_SIZE_REQUEST_TYPE_MAX` before metrics mutation
  (`src/base/ftobjs.c:3438-3505`).
- Rust FFI, thin C ABI, and WASM ABI already matched the pinned C sentinel
  rejection output; the unified harness now requires exact error status/output
  comparison for this concrete public case.
- The route audit now classifies
  `freetype.FT_SIZE_REQUEST_TYPE_MAX.request_size_rejects_sentinel` as
  `real-parity`, validated through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_SIZE_REQUEST_TYPE_MAX.request_size_rejects_sentinel
```

### Issue Set W: `FT_Request_Size` BBox divide-by-zero exact-error route

Previous blocker:

- `fterrdef.FT_Err_Divide_By_Zero.invalid_size_transform_division_returns_error`
  stayed in `generic-error-fallback` even though the fixture targets the
  public `FT_Request_Size` BBox divide guards with an exact
  `FT_Err_Divide_By_Zero` expectation. The runtime harness accepted the
  expected error without exact pinned-C status/output comparison.

Verified progress:

- The pinned C oracle `emit_request_size` route calls `FT_Request_Size` with
  two BBox request rows against the compact malformed-matrix fixture and
  records each native error plus row metrics payload.
- FreeType 2.14.3 `FT_Request_Metrics` returns `Divide_By_Zero` when BBox
  width or height is zero before computing scales
  (`src/base/ftobjs.c:3264-3335`).
- Rust FFI, thin C ABI, and WASM ABI already matched the pinned C
  divide-by-zero output; the unified harness now requires exact error
  status/output comparison for this concrete public case.
- The route audit now classifies
  `fterrdef.FT_Err_Divide_By_Zero.invalid_size_transform_division_returns_error`
  as `real-parity`, validated through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Divide_By_Zero.invalid_size_transform_division_returns_error
```

### Issue Set X: `FT_Request_Size` invalid-request matrix exact-error route

Previous blocker:

- `freetype.FT_Request_Size.error_invalid_request_or_unavailable_strike`
  stayed in `generic-error-fallback` even though the fixture contains an exact
  pinned-C invalid-request matrix: sentinel request type, negative dimensions,
  zero ppem, and oversize ppem rows. The runtime harness accepted the expected
  error without exact pinned-C status/output comparison.

First divergence:

- Turning on exact comparison exposed the fourth row. Pinned C returned
  `FT_Err_Invalid_PPem` (`151`) for a nominal zero-width/zero-height TrueType
  request, while Rust returned success with zero metrics.

Verified progress:

- FreeType 2.14.3 `FT_Request_Size` dispatches TrueType faces through
  `tt_size_request` (`src/truetype/ttdriver.c:349-410`). That path first calls
  `FT_Request_Metrics`, then `tt_size_reset`, which rejects zero `x_ppem` or
  `y_ppem` as `Invalid_PPem` (`src/truetype/ttobjs.c:1247-1248`).
- Rust now preserves the existing `Invalid_Pixel_Size` overflow mapping and
  adds a distinct `Invalid_PPem` result for TrueType-style zero ppem requests.
- The unified harness now requires exact error status/output comparison for
  this concrete public matrix case.
- The route audit now classifies
  `freetype.FT_Request_Size.error_invalid_request_or_unavailable_strike` as
  `real-parity`, validated through pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Request_Size.error_invalid_request_or_unavailable_strike
```

### Issue Set Y: `FT_Request_Size` probe-face invalid-size-handle exact-error route

Previous blocker:

- `freetype.FT_Request_Size.error_probe_face_invalid_size_handle` stayed in
  `generic-error-fallback`, even though the fixture targets a concrete pinned-C
  `FT_Request_Size` call with `face_index = -1` probe-face loading and expects
  `FT_Err_Invalid_Size_Handle`.

Plan:

1. Confirm the row has a real font asset and pinned-C oracle output.
2. Enable exact status/output comparison for this one public row.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the request-size lane and route audit before committing.

Verified progress:

- The fixture row uses `input/fonts/DejaVuSans.ttf` with `face_index = -1`,
  which exercises FreeType's probe-face path before calling `FT_Request_Size`.
- The unified harness now requires exact error status/output comparison for
  this concrete public row.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Request_Size.error_probe_face_invalid_size_handle` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Request_Size.error_probe_face_invalid_size_handle
```

### Issue Set Z: `FT_Set_Pixel_Sizes` probe-face invalid-size-handle exact-error route

Previous blocker:

- `freetype.FT_Set_Pixel_Sizes.error_probe_face_invalid_size_handle` stayed in
  `generic-error-fallback`, even though the fixture targets a concrete pinned-C
  `FT_Set_Pixel_Sizes` call using `face_index = -1` probe-face loading and
  expects `FT_Err_Invalid_Size_Handle`.

Plan:

1. Confirm the row has a real font asset and exact-error expectation.
2. Enable exact status/output comparison for this one public row.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run route audit, full parity, and non-coverage gates before committing.

Verified progress:

- The fixture row uses `input/fonts/DejaVuSans.ttf` with `face_index = -1`,
  which exercises FreeType's probe-face path before `FT_Set_Pixel_Sizes`.
- The unified harness now requires exact error status/output comparison for
  this concrete public row.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Set_Pixel_Sizes.error_probe_face_invalid_size_handle` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Pixel_Sizes.error_probe_face_invalid_size_handle
```

### Issue Set AA: `FT_Set_Char_Size` probe-face invalid-size-handle exact-error route

Previous blocker:

- `freetype.FT_Set_Char_Size.error_probe_face_invalid_size_handle` stayed in
  `generic-error-fallback`, even though the fixture targets a concrete pinned-C
  `FT_Set_Char_Size` call using `face_index = -1` probe-face loading and
  expects `FT_Err_Invalid_Size_Handle`.

Plan:

1. Confirm the row has a real font asset and exact-error expectation.
2. Enable exact status/output comparison for this one public row.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the set-char-size lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row uses `input/fonts/DejaVuSans.ttf` with `face_index = -1`,
  which exercises FreeType's probe-face path before `FT_Set_Char_Size`.
- The unified harness now requires exact error status/output comparison for
  this concrete public row.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Set_Char_Size.error_probe_face_invalid_size_handle` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Char_Size.error_probe_face_invalid_size_handle
```

### Issue Set AB: `FT_Select_Size` past-end strike-index exact-error route

Previous blocker:

- `freetype.FT_Select_Size.error_strike_index_past_end_direct` stayed in
  `generic-error-fallback`, even though the fixture targets a concrete pinned-C
  `FT_Select_Size` call against a bitmap-strike face and expects
  `FT_Err_Invalid_Argument` for a strike index past the available strikes.

Plan:

1. Confirm the row has a real bitmap-strike font asset and exact-error
   expectation.
2. Enable exact status/output comparison for this one public row.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the select-size lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row uses `fixtures/assets/fonts/sbit_gray_format1.ttf` with a
  direct past-end strike index.
- The unified harness now requires exact error status/output comparison for
  this concrete public row.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Select_Size.error_strike_index_past_end_direct` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Select_Size.error_strike_index_past_end_direct
```

### Issue Set AC: `FT_Select_Size` strike-index range exact-error route

Previous blocker:

- `freetype.FT_Select_Size.error_strike_index_out_of_range` stayed in
  `generic-error-fallback`, even though the fixture targets concrete pinned-C
  `FT_Select_Size` calls against a bitmap-strike face and expects
  `FT_Err_Invalid_Argument` for negative and past-end strike indices while
  preserving the active size.

Plan:

1. Confirm the row has a real bitmap-strike font asset and exact-error
   expectation.
2. Enable exact status/output comparison for this one public row.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the select-size lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row uses `fixtures/assets/fonts/sbit_gray_format1.ttf` with
  negative and past-end strike-index variants.
- The unified harness now requires exact error status/output comparison for
  this concrete public row.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Select_Size.error_strike_index_out_of_range` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Select_Size.error_strike_index_out_of_range
```

### Issue Set AD: `FT_Select_Size` no-fixed-sizes exact-error route

Previous blocker:

- The `no-fixed-sizes` concrete row inside
  `freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face` stayed in
  `generic-error-fallback`, even though the fixture targets a concrete pinned-C
  `FT_Select_Size` call against a scalable face with no bitmap strikes. The
  sibling null-face variant was already classified as real parity.

Plan:

1. Confirm the row has a real scalable font asset and exact-error expectation.
2. Enable exact status/output comparison for this public case.
3. Keep the existing null-face exact classification and add the no-fixed-sizes
   concrete row to real parity.
4. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
5. Re-run the select-size lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row uses `input/fonts/DejaVuSans.ttf`, a scalable face without
  fixed bitmap strikes, and calls `FT_Select_Size(face, 0)`.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies the no-fixed-sizes concrete row for
  `freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face
```

### Issue Set AE: `FT_Reference_Face` null-face exact-error route

Previous blocker:

- `freetype.FT_Reference_Face.error_null_face` stayed in
  `generic-error-fallback`, even though the fixture targets the public
  `FT_Reference_Face(NULL)` error path and expects the pinned C
  `FT_Err_Invalid_Face_Handle` status.

Plan:

1. Confirm the row is a concrete null-face public call with exact-error
   expectation.
2. Enable exact status/output comparison for this public case.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the reference-face lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row calls `FT_Reference_Face(NULL)` directly.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Reference_Face.error_null_face` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Reference_Face.error_null_face
```

### Issue Set AF: `FT_Select_Charmap` missing-encoding exact-error route

Previous blocker:

- `freetype.FT_Select_Charmap.error_missing_encoding` stayed in
  `generic-error-fallback`, even though the fixture targets
  `FT_Select_Charmap(face, FT_ENCODING_SJIS)` on `DejaVuSans.ttf` and expects
  exact `FT_Err_Invalid_Argument` behavior with the selected charmap unchanged.

Plan:

1. Confirm the row is a concrete public call with an exact-error expectation.
2. Enable exact status/output comparison for this public case.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the select-charmap lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row calls `FT_Select_Charmap(face, FT_ENCODING_SJIS)` against
  `input/fonts/DejaVuSans.ttf`, which lacks that selected encoding.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Select_Charmap.error_missing_encoding` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Select_Charmap.error_missing_encoding
```

### Issue Set AG: `FT_Set_Charmap` null/foreign-charmap exact-error route

Previous blocker:

- `freetype.FT_Set_Charmap.error_null_or_foreign_charmap` stayed in
  `generic-error-fallback`, even though the fixture targets concrete public
  `FT_Set_Charmap` error calls for a null charmap and a charmap sourced from a
  different face, with the selected charmap required to remain unchanged.

Plan:

1. Confirm the row is a concrete public call set with exact-error expectation.
2. Enable exact status/output comparison for this public case.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the set-charmap lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row calls `FT_Set_Charmap` against `input/fonts/DejaVuSans.ttf`
  for both `null` and `from_other_face` charmap variants, using
  `input/fonts/LiberationSerif-Regular.ttf` as the foreign face.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Set_Charmap.error_null_or_foreign_charmap` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Charmap.error_null_or_foreign_charmap
```

### Issue Set AH: `FT_Select_Charmap` missing-Unicode-charmap exact-error route

Previous blocker:

- `freetype.FT_Select_Charmap.error_missing_unicode_charmap` stayed in
  `generic-error-fallback`, even though the fixture targets a concrete public
  `FT_Select_Charmap(face, FT_ENCODING_UNICODE)` error path on a non-Unicode
  charmap fixture and expects the selected charmap to remain unchanged.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Enable exact status/output comparison for this public case.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the select-charmap lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row calls `FT_Select_Charmap(face, FT_ENCODING_UNICODE)` against
  `fonts/charmap/cmap-nonunicode-format6.ttf`, which does not provide a
  Unicode charmap.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Select_Charmap.error_missing_unicode_charmap` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Select_Charmap.error_missing_unicode_charmap
```

### Issue Set AI: `FT_Err_Invalid_CharMap_Handle` set-charmap null-charmap exact-error route

Previous blocker:

- `fterrdef.FT_Err_Invalid_CharMap_Handle.set_charmap_rejects_foreign_or_null_charmap`
  stayed in `generic-error-fallback`, even though the fixture targets the
  concrete public `FT_Set_Charmap(face, NULL)` error path and expects
  `FT_Err_Invalid_CharMap_Handle` with the active charmap unchanged.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Enable exact status/output comparison for this public case.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the set-charmap lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row calls `FT_Set_Charmap(face, NULL)` against
  `input/fonts/DejaVuSans.ttf`.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `fterrdef.FT_Err_Invalid_CharMap_Handle.set_charmap_rejects_foreign_or_null_charmap`
  as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_CharMap_Handle.set_charmap_rejects_foreign_or_null_charmap
```

### Issue Set AJ: `FT_Set_Charmap` format-14-charmap exact-error route

Previous blocker:

- `freetype.FT_Set_Charmap.error_format14_charmap` stayed in
  `generic-error-fallback`, even though the fixture targets the concrete
  public `FT_Set_Charmap` error path for an active format-14-only cmap fixture
  and expects `FT_Err_Invalid_Argument` with the active charmap unchanged.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Enable exact status/output comparison for this public case.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the set-charmap lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row calls `FT_Set_Charmap` against
  `fonts/charmap/cmap-format14-only.ttf` and attempts to select all charmaps.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Set_Charmap.error_format14_charmap` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Charmap.error_format14_charmap
```

### Issue Set AK: `FT_Init_FreeType` null-output-pointer exact-error route

Previous blocker:

- `freetype.FT_Init_FreeType.error_null_output_pointer` stayed in
  `generic-error-fallback`, even though the fixture targets the concrete public
  `FT_Init_FreeType(NULL)` error path and expects exact
  `FT_Err_Invalid_Argument` behavior.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Enable exact status/output comparison for this public case.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the init-free-type lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The fixture row calls `FT_Init_FreeType(NULL)`.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Init_FreeType.error_null_output_pointer` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Init_FreeType.error_null_output_pointer
```

### Issue Set AL: `FT_Get_Track_Kerning` null-face/null-output exact-error route

Previous blocker:

- `freetype.FT_Get_Track_Kerning.error_null_face_or_output` stayed in
  `generic-error-fallback`, even though the fixture targets concrete public
  `FT_Get_Track_Kerning` error variants for a null face and a null output
  pointer.

Plan:

1. Confirm the row is a concrete public call set with exact-error expectation.
2. Enable exact status/output comparison for this public case.
3. Run the focused public case through Rust FFI, thin C ABI, and WASM ABI.
4. If exact comparison fails, fix the first Rust/core or ABI divergence. If it
   passes, classify the existing behavior as real parity.
5. Re-run the get-track-kerning lane, full parity, and non-coverage gates
   before committing.

Verified progress:

- The fixture row calls `FT_Get_Track_Kerning` for both `face: null` and
  `akerning: null` variants. The error variants do not require the future Type1
  track-kerning asset.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- Focused exact comparison passed for pinned C FreeType, Rust FFI, thin C ABI,
  and WASM ABI; no core Rust logic change was required.
- The route audit now classifies
  `freetype.FT_Get_Track_Kerning.error_null_face_or_output` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Get_Track_Kerning.error_null_face_or_output
```

### Issue Set AM: `FT_Get_Track_Kerning` SFNT/no-track-data exact-error route

Previous blocker:

- `freetype.FT_Get_Track_Kerning.sfnt_or_no_track_data_error` stayed in
  `generic-error-fallback`, even though the fixture targets the concrete public
  `FT_Get_Track_Kerning` error produced for an SFNT face with no AFM track
  kerning data.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case only.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the get-track-kerning lane, full parity, and non-coverage gates
   before committing.

Verified progress:

- The focused row already passed against pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Get_Track_Kerning.sfnt_or_no_track_data_error` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Get_Track_Kerning.sfnt_or_no_track_data_error
```

### Issue Set AN: `FT_Attach_File` null-face exact-error route

Previous blocker:

- `freetype.FT_Attach_File.error_null_face` stayed in
  `generic-error-fallback`, even though the fixture targets the concrete public
  `FT_Attach_File` error for a null face handle.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case only.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the attach-file lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The focused row already passed against pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Attach_File.error_null_face` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Attach_File.error_null_face
```

### Issue Set AO: `FT_Attach_File` null-pathname exact-error route

Previous blocker:

- `freetype.FT_Attach_File.error_null_pathname` stayed in
  `generic-error-fallback`, even though the fixture targets the concrete public
  `FT_Attach_File` error for a null pathname.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case only.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the attach-file lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The focused row already passed against pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Attach_File.error_null_pathname` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Attach_File.error_null_pathname
```

### Issue Set AP: `FT_Attach_File` missing/unsupported-file exact-error route

Previous blocker:

- `freetype.FT_Attach_File.error_missing_or_unsupported_file` stayed in
  `generic-error-fallback`, even though the fixture targets the concrete public
  `FT_Attach_File` error for a missing or unsupported attachment file.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case only.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the attach-file lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The focused row already passed against pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Attach_File.error_missing_or_unsupported_file` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Attach_File.error_missing_or_unsupported_file
```

### Issue Set AQ: `FT_Attach_Stream` null-face exact-error route

Previous blocker:

- `freetype.FT_Attach_Stream.error_null_face` stayed in
  `generic-error-fallback`, even though the fixture targets the concrete public
  `FT_Attach_Stream` error for a null face handle.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case only.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the attach-stream lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The focused row already passed against pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Attach_Stream.error_null_face` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Attach_Stream.error_null_face
```

### Issue Set AR: `FT_Attach_Stream` null-open-args exact-error route

Previous blocker:

- `freetype.FT_Attach_Stream.error_null_open_args` stayed in
  `generic-error-fallback`, even though the fixture targets the concrete public
  `FT_Attach_Stream` error for a null `FT_Open_Args` pointer.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case only.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the attach-stream lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The focused row already passed against pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Attach_Stream.error_null_open_args` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Attach_Stream.error_null_open_args
```

### Issue Set AS: `FT_Attach_Stream` invalid-open-args/unsupported-driver exact-error route

Previous blocker:

- `freetype.FT_Attach_Stream.error_invalid_open_args_or_unsupported_driver`
  stayed in `generic-error-fallback`, even though the fixture targets the
  concrete public `FT_Attach_Stream` error for invalid open arguments or an
  unsupported driver.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case only.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the attach-stream lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The focused row already passed against pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Attach_Stream.error_invalid_open_args_or_unsupported_driver` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Attach_Stream.error_invalid_open_args_or_unsupported_driver
```

### Issue Set AT: `FT_Face` null/done handle error-policy exact-error route

Previous blocker:

- `freetype.FT_Face.null_and_done_handle_errors` stayed in
  `generic-error-fallback`, even though the fixture targets concrete public
  face-handle error behavior for null and already-done handles.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case only.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the face-handle policy lane, full parity, and non-coverage gates
   before committing.

Verified progress:

- The focused row already passed against pinned C FreeType, Rust FFI, thin C
  ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case.
- The route audit now classifies
  `freetype.FT_Face.null_and_done_handle_errors` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Face.null_and_done_handle_errors
```

### Issue Set AU: `FT_Set_Char_Size` oversized-dimensions exact-error route

Previous blocker:

- `freetype.FT_Set_Char_Size.error_oversized_dimensions` stayed in
  `generic-error-fallback`, even though the fixture targets concrete public
  `FT_Set_Char_Size` errors for oversized width/height combinations.

Plan:

1. Confirm the row set is concrete public calls with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case ID.
4. Classify the concrete rows as real parity only if exact comparison passes.
5. Re-run the set-char-size lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The focused row set passed three concrete variants against pinned C FreeType,
  Rust FFI, thin C ABI, and WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case ID.
- The route audit now classifies
  `freetype.FT_Set_Char_Size.error_oversized_dimensions` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Char_Size.error_oversized_dimensions
```

### Issue Set AV: `FT_Set_Char_Size` invalid-pixel-size fterrdef exact-error route

Previous blocker:

- `fterrdef.FT_Err_Invalid_Pixel_Size.set_char_size_rejects_oversized_dimensions`
  stayed in `generic-error-fallback`, even though the fixture targets the
  public `FT_Set_Char_Size` invalid-pixel-size error for oversized dimensions.

Plan:

1. Confirm the row is a concrete public call with exact-error expectation.
2. Run the focused public case before classification to verify existing Rust
   FFI, thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact status/output comparison for this public case ID.
4. Classify the row as real parity only if exact comparison passes.
5. Re-run the set-char-size lane, full parity, and non-coverage gates before
   committing.

Verified progress:

- The focused row passed against pinned C FreeType, Rust FFI, thin C ABI, and
  WASM ABI before reclassification.
- The unified harness now requires exact error status/output comparison for
  this concrete public case ID.
- The route audit now classifies
  `fterrdef.FT_Err_Invalid_Pixel_Size.set_char_size_rejects_oversized_dimensions`
  as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Pixel_Size.set_char_size_rejects_oversized_dimensions
```

### Issue Set AW: `FT_IS_NAMED_INSTANCE` encoded face-index exact-error route

Previous blocker:

- `freetype.FT_IS_NAMED_INSTANCE.encoded_named_instance_face_index_returns_true`
  had a real-parity success variant, but the `instance-past-instance-count`
  variant stayed in `generic-error-fallback` because exact C error
  status/output comparison was not required for that public macro route.

Plan:

1. Confirm the fixture splits the valid named-instance face index from the
   invalid encoded instance index variant.
2. Run the focused public case before classification to verify current Rust FFI,
   thin C ABI, and WASM ABI behavior against pinned C FreeType.
3. Enable exact error status/output comparison for this public case ID.
4. Classify the expected-error variant as real parity only if exact comparison
   passes.
5. Re-run the focused face-macro case, full parity, and non-coverage gates
   before committing.

Verified progress:

- The focused case passed both concrete rows against pinned C FreeType, Rust
  FFI, thin C ABI, and WASM ABI before and after exact-error gating.
- The unified harness now requires exact error status/output comparison for the
  encoded face-index expected-error variant.
- The route audit now classifies the invalid encoded instance-index variant of
  `freetype.FT_IS_NAMED_INSTANCE.encoded_named_instance_face_index_returns_true`
  as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_IS_NAMED_INSTANCE.encoded_named_instance_face_index_returns_true
```

### Issue Set AX: `FT_Get_Advance` null-face/null-output exact probe route

Previous blocker:

- `ftadvanc.FT_Get_Advance.error_null_face_or_output` stayed in
  `generic-error-fallback`.
- The fixture requested `null_face` and `null_padvance` probes, but the
  maintained oracle and runtime runners were opening a normal face and passing
  a valid output pointer, so exact-error gating initially exposed that pinned C
  returned success for the wrong input.

Plan:

1. Keep the fixture intact; it is a valid public `FT_Get_Advance` error route.
2. Wire the pinned C oracle helper to execute the declared probe matrix:
   `FT_Get_Advance(NULL, ...)` and `FT_Get_Advance(face, ..., NULL)`.
3. Wire Rust FFI, thin C ABI, and WASM ABI runtime lanes to emit the same probe
   rows with sentinel `padvance` preservation.
4. Match FreeType `src/base/ftadvanc.c:116-120`: null face returns
   `Invalid_Face_Handle`; null `padvance` returns `Invalid_Argument`.
5. Require exact error status/output comparison and classify the row as real
   parity only after focused parity passes.

Verified progress:

- The focused case now passes exact comparison against pinned C FreeType, Rust
  FFI, thin C ABI, and WASM ABI for both null probes.
- Thin C and WASM wrappers now return `Invalid_Face_Handle` for null/missing
  face handles in `FT_Get_Advance`, matching FreeType's check order.
- The route audit now classifies
  `ftadvanc.FT_Get_Advance.error_null_face_or_output` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftadvanc.FT_Get_Advance.error_null_face_or_output
```

### Issue Set AY: `FT_Get_Advances` null-face/null-output exact probe route

Previous blocker:

- `ftadvanc.FT_Get_Advances.error_null_face_or_output` stayed in
  `generic-error-fallback`.
- The fixture requested `null_face` and `null_padvances` probes, but the
  maintained oracle and runtime runners were using a normal face and valid
  output array, so the row was not exercising the public C error path.

Plan:

1. Keep the fixture intact; it is a valid public `FT_Get_Advances` error route.
2. Wire the pinned C oracle helper to execute
   `FT_Get_Advances(NULL, ...)` and `FT_Get_Advances(face, ..., NULL)`.
3. Wire Rust FFI, thin C ABI, and WASM ABI runtime lanes to emit matching probe
   rows with sentinel `padvances` preservation.
4. Match FreeType `src/base/ftadvanc.c:158-164`: null face returns
   `Invalid_Face_Handle`; null `padvances` returns `Invalid_Argument`.
5. Require exact error status/output comparison and classify the row as real
   parity only after focused parity passes.

Verified progress:

- The focused case now passes exact comparison against pinned C FreeType, Rust
  FFI, thin C ABI, and WASM ABI for both null probes.
- Thin C and WASM wrappers now return `Invalid_Face_Handle` for null/missing
  face handles in `FT_Get_Advances`, matching FreeType's check order.
- The route audit now classifies
  `ftadvanc.FT_Get_Advances.error_null_face_or_output` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftadvanc.FT_Get_Advances.error_null_face_or_output
```

### Issue Set AZ: `FT_Get_Advance` invalid-glyph/invalid-flags exact-error route

Previous blocker:

- `ftadvanc.FT_Get_Advance.error_invalid_glyph_or_flags` stayed in
  `generic-error-fallback`.
- Exact-error gating exposed that pinned C returned `Invalid_Glyph_Index` for
  high-bit invalid flag variants with out-of-range glyph indices, while Rust
  returned `Unimplemented_Feature`.

Plan:

1. Keep the fixture intact; it exercises concrete same-input public errors.
2. Compare FreeType `src/base/ftadvanc.c` check order.
3. Move Rust `FT_Get_Advance` glyph-index validation before FAST_ONLY fallback
   and load-flag conversion.
4. Require exact error comparison for all concrete variants.
5. Classify the four concrete variants as real parity only after focused
   parity passes.

Verified progress:

- Rust now matches FreeType `src/base/ftadvanc.c:116-126` check order: glyph
  index is validated before FAST_ONLY fallback and load-flag conversion.
- The focused invalid-glyph/invalid-flags variants pass exact comparison
  against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies the four concrete
  `ftadvanc.FT_Get_Advance.error_invalid_glyph_or_flags` variants as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftadvanc.FT_Get_Advance.error_invalid_glyph_or_flags
```

### Issue Set BA: `FT_ADVANCE_FLAG_FAST_ONLY` exact-error route

Previous blocker:

- `ftadvanc.FT_ADVANCE_FLAG_FAST_ONLY.fast_only_error_behavior` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status and output comparison.

Plan:

1. Keep the fixture intact; it exercises public
   `FT_LOAD_DEFAULT | FT_ADVANCE_FLAG_FAST_ONLY` behavior through
   `FT_Get_Advance`.
2. Require exact error status and `padvance` sentinel comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused FAST_ONLY row passes exact comparison against pinned C FreeType,
  Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftadvanc.FT_ADVANCE_FLAG_FAST_ONLY.fast_only_error_behavior` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftadvanc.FT_ADVANCE_FLAG_FAST_ONLY.fast_only_error_behavior
```

### Issue Set BB: `FT_Get_Advances` invalid-range/invalid-flags exact-error route

Previous blocker:

- `ftadvanc.FT_Get_Advances.error_invalid_range_or_flags` stayed in
  `generic-error-fallback`.
- The row exercised real public `FT_Get_Advances` invalid range and invalid
  flag combinations, but the harness still accepted it as a generic
  expected-error row instead of requiring exact C/Rust/C-ABI/WASM comparison.

Plan:

1. Keep the fixture intact; it covers out-of-range `start`, overflowing
   `start + count`, invalid high-bit flags, and the `count == 0` ordering case.
2. Compare FreeType `src/base/ftadvanc.c:158-170`: null checks first, then the
   unsigned `start + count` range check, then `count == 0`.
3. Require exact error comparison for all concrete variants.
4. Classify the seven concrete variants as real parity only after focused
   parity passes.

Verified progress:

- Rust already matched FreeType `src/base/ftadvanc.c:158-170` range-check
  ordering, including invalid `start` before zero-count success.
- The focused invalid-range/invalid-flags variants pass exact comparison
  against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies the seven concrete
  `ftadvanc.FT_Get_Advances.error_invalid_range_or_flags` variants as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftadvanc.FT_Get_Advances.error_invalid_range_or_flags
```

### Issue Set BC: `FT_Get_Advance` probe-face exact-error route

Previous blocker:

- `fterrdef.FT_Err_Invalid_Size_Handle.null_or_detached_size_rejected` stayed
  in `generic-error-fallback`.
- Exact-error gating exposed that pinned C returned `Invalid_Glyph_Index` for
  the negative face-index probe passed to `FT_Get_Advance`, while Rust returned
  `Invalid_Size_Handle`.

Plan:

1. Keep the fixture intact; it exercises a real negative face-index probe with
   no active `FT_Size`.
2. Compare FreeType `src/base/ftadvanc.c:116-126` and the TrueType fast
   advance path used by `FT_LOAD_NO_HINTING`.
3. Preserve normal no-size-face `Invalid_Size_Handle` behavior, but match the
   pinned probe-face C route for `FT_Get_Advance`.
4. Require exact error comparison before route-audit classification.

Verified progress:

- Rust now matches pinned C for this `FT_Get_Advance` probe-face route:
  negative face-index probe returns `Invalid_Glyph_Index` instead of
  short-circuiting to `Invalid_Size_Handle`.
- The focused row passes exact comparison against pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI.
- The route audit now classifies
  `fterrdef.FT_Err_Invalid_Size_Handle.null_or_detached_size_rejected` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Size_Handle.null_or_detached_size_rejected
```

### Issue Set BD: `FT_Done_Library` null-library exact-error route

Previous blocker:

- `ftmodapi.FT_Done_Library.rejects_null_library` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_Done_Library(NULL)`.
2. Require exact error status comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused null-library row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_Done_Library.rejects_null_library` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Done_Library.rejects_null_library
```

### Issue Set BF: `FT_Reference_Library` null-library exact-error route

Previous blocker:

- `ftmodapi.FT_Reference_Library.rejects_null_library` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_Reference_Library(NULL)`.
2. Require exact error status comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused null-library row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_Reference_Library.rejects_null_library` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Reference_Library.rejects_null_library
```

### Issue Set BG: `FT_Done_MM_Var` null-library exact-error route

Previous blocker:

- `ftmm.FT_Done_MM_Var.null_library_error` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status/free-event comparison.

Plan:

1. Keep the fixture intact; it exercises public
   `FT_Done_MM_Var(NULL, sentinel_non_owned_pointer)`.
2. Require exact error status and free-event comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused null-library row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Done_MM_Var.null_library_error` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Done_MM_Var.null_library_error
```

### Issue Set BH: `FT_New_Face` null-library/null-output-pointer oracle route blocker

Current blocker:

- `freetype.FT_New_Face.error_null_library_or_aface` stayed in
  `generic-error-fallback`.
- Exact-error gating was tested and correctly rejected classification: the
  pinned oracle path returned success for the current maintained route instead
  of executing the fixture's null-library/null-`aface` public probes.

Plan:

1. Keep the fixture intact; it exercises public `FT_New_Face` error behavior
   for null library and null `aface`.
2. Add or repair the pinned C oracle route so it executes both same-input null
   variants and records exact status/output observations.
3. Wire the Rust FFI, thin C ABI, and WASM ABI runners through the same public
   variants.
4. Only then require exact error status/output comparison and classify the row
   as `real-parity`.

Failed classification attempt:

- Adding exact-error gating caused the focused case to fail because the oracle
  returned `ok`:
  `rust ffi: freetype.FT_New_Face.error_null_library_or_aface requires an exact C error, but the oracle returned ok`.
- The row must remain in `generic-error-fallback` until the oracle route is
  repaired.

Diagnostic command:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_New_Face.error_null_library_or_aface
```

### Issue Set BI: `FT_Remove_Module` null-library exact-error route

Previous blocker:

- `ftmodapi.FT_Remove_Module.rejects_null_library` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_Remove_Module(NULL, ...)`.
2. Require exact error status comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused null-library row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_Remove_Module.rejects_null_library` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Remove_Module.rejects_null_library
```

### Issue Set BJ: `FT_Add_Module` null-library exact-error route

Previous blocker:

- `ftmodapi.FT_Add_Module.rejects_null_library` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_Add_Module(NULL, ...)`.
2. Require exact error status comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused null-library row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_Add_Module.rejects_null_library` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Add_Module.rejects_null_library
```

### Issue Set BK: `FT_Remove_Module` missing/foreign-module exact-error route

Previous blocker:

- `ftmodapi.FT_Remove_Module.rejects_missing_or_foreign_module` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status and module-table
  comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_Remove_Module` behavior for
   missing/null and foreign module handles.
2. Require exact error status and unchanged-module-table comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused missing/foreign-module row passes exact comparison against pinned
  C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_Remove_Module.rejects_missing_or_foreign_module` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Remove_Module.rejects_missing_or_foreign_module
```

### Issue Set BL: `FT_Add_Module` null-class exact-error route

Previous blocker:

- `ftmodapi.FT_Add_Module.rejects_null_class` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status and module-count
  comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_Add_Module` behavior for a
   null `FT_Module_Class` pointer with a valid library.
2. Require exact error status and unchanged-module-count comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused null-class row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_Add_Module.rejects_null_class` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Add_Module.rejects_null_class
```

### Issue Set BM: `FT_Add_Module` future-required-version exact-error route

Previous blocker:

- `ftmodapi.FT_Add_Module.rejects_future_required_version` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status, module-count, and
  lookup-result comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_Add_Module` behavior for a
   module class requiring a newer FreeType version than the pinned oracle.
2. Require exact error status, unchanged-module-count, and null lookup-result
   comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused future-required-version row passes exact comparison against
  pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_Add_Module.rejects_future_required_version` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Add_Module.rejects_future_required_version
```

### Issue Set BN: `FT_Add_Module` duplicate-name/version exact route

Previous blocker:

- `ftmodapi.FT_Add_Module.duplicate_name_version_rules` stayed in
  `generic-error-fallback`.
- The fixture is marked `expect_error=true` because the call sequence includes
  an intentional lower-version duplicate error, but the declared expectation is
  `status: ok` with exact comparison of the full status sequence, module count,
  destructor calls, and installed version.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the route audit still treated the row as a
  generic expected-error fallback.

Plan:

1. Keep the fixture intact; it exercises public `FT_Add_Module` duplicate
   module-name/version replacement rules.
2. Keep the exact comparison over `status_sequence`, `module_count`,
   `destructor_calls`, and `installed_version`.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused duplicate-name/version row passes exact comparison against pinned
  C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_Add_Module.duplicate_name_version_rules` as `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Add_Module.duplicate_name_version_rules
```

### Issue Set BO: `FT_New_Library` null-input exact-error route

Previous blocker:

- `ftmodapi.FT_New_Library.rejects_null_inputs_preserving_output` stayed in
  `generic-error-fallback`.
- The focused same-input runtime already matched pinned C FreeType, Rust FFI,
  thin C ABI, and WASM ABI, but the harness still allowed it as a generic
  expected-error row instead of enforcing exact status, output-pointer, and
  allocator-call comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_New_Library` behavior for
   null memory and null output-library pointer inputs.
2. Require exact error status, preserved output pointer, and no allocator-call
   comparison.
3. Classify the concrete row as real parity only after focused exact parity
   passes.

Verified progress:

- The focused null-input row passes exact comparison against pinned C FreeType,
  Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_New_Library.rejects_null_inputs_preserving_output` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_New_Library.rejects_null_inputs_preserving_output
```

### Issue Set BP: `FT_Add_Module` fterrdef exact-error routes

Previous blocker:

- The fterrdef `FT_Add_Module` rows for `FT_Err_Invalid_Version`,
  `FT_Err_Lower_Module_Version`, and `FT_Err_Too_Many_Drivers` stayed in
  `generic-error-fallback`.
- The fixtures name exact C behavior from `freetype/src/base/ftobjs.c`, but
  the harness still accepted them as generic expected-error rows instead of
  enforcing exact status, error symbol, and module-table observation parity.

Plan:

1. Keep the fterrdef fixtures intact; they exercise public `FT_Add_Module`
   failure behavior for future required FreeType versions, duplicate lower
   module versions, and module registry exhaustion.
2. Require exact error comparison for all three case IDs.
3. Classify each row as real parity only after focused same-input parity
   passes through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The three focused `FT_Add_Module` fterrdef rows pass exact comparison against
  pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `fterrdef.FT_Err_Invalid_Version.module_requires_newer_freetype`
  - `fterrdef.FT_Err_Lower_Module_Version.duplicate_module_not_newer`
  - `fterrdef.FT_Err_Too_Many_Drivers.module_registry_limit`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Version.module_requires_newer_freetype
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Lower_Module_Version.duplicate_module_not_newer
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Too_Many_Drivers.module_registry_limit
```

### Issue Set BQ: `FT_New_Library` allocator-failure exact-error route

Previous blocker:

- `ftmodapi.FT_New_Library.allocation_failure_preserves_output` stayed in
  `generic-error-fallback`.
- The fixture requires exact allocator-failure behavior from
  `freetype/src/base/ftobjs.c:FT_New_Library`, but the harness still accepted
  it as a generic expected-error row instead of enforcing exact status,
  preserved output pointer, and allocator-call comparison.

Plan:

1. Keep the fixture intact; it exercises public `FT_New_Library` behavior with
   a failing `FT_MemoryRec` allocator and a sentinel output pointer.
2. Require exact error status, output-pointer preservation, and allocator-call
   comparison.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused allocator-failure row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmodapi.FT_New_Library.allocation_failure_preserves_output` as
  `real-parity`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_New_Library.allocation_failure_preserves_output
```

### Issue Set BR: `FT_Property_Get` exact-error routes

Previous blocker:

- The `FT_Property_Get` rows for null arguments, missing or unsupported
  property services, and invalid property names stayed in
  `generic-error-fallback`.
- The fixtures require exact behavior from
  `freetype/src/base/ftobjs.c:FT_Property_Get`, but the harness still accepted
  these rows as generic expected-error rows instead of enforcing exact status
  and output-value preservation.

Plan:

1. Keep the fixtures intact; they exercise public `FT_Property_Get` error
   behavior for invalid handles, missing modules, unsupported property services,
   and module callback errors.
2. Require exact error status and preserved output value comparison for all
   three case IDs.
3. Classify each row as real parity only after focused same-input parity
   passes through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The three focused `FT_Property_Get` rows pass exact comparison against pinned
  C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `ftmodapi.FT_Property_Get.rejects_null_arguments`
  - `ftmodapi.FT_Property_Get.missing_or_unsupported_property_service`
  - `ftmodapi.FT_Property_Get.invalid_property_name`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Get.rejects_null_arguments
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Get.missing_or_unsupported_property_service
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Get.invalid_property_name
```

### Issue Set BS: `FT_Property_Set` exact-error routes

Previous blocker:

- The `FT_Property_Set` rows for null arguments, missing or unsupported
  property services, and invalid property names or values stayed in
  `generic-error-fallback`.
- The fixtures require exact behavior from
  `freetype/src/base/ftobjs.c:FT_Property_Set`, but the harness still accepted
  these rows as generic expected-error rows instead of enforcing exact status
  and property-preservation comparison.

Plan:

1. Keep the fixtures intact; they exercise public `FT_Property_Set` error
   behavior for invalid handles, missing modules, unsupported property services,
   and module callback errors.
2. Require exact error status and property preservation comparison for all
   three case IDs.
3. Classify each row as real parity only after focused same-input parity
   passes through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The three focused `FT_Property_Set` rows pass exact comparison against pinned
  C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `ftmodapi.FT_Property_Set.rejects_null_arguments`
  - `ftmodapi.FT_Property_Set.missing_or_unsupported_property_service`
  - `ftmodapi.FT_Property_Set.invalid_property_or_value`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Set.rejects_null_arguments
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Set.missing_or_unsupported_property_service
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Set.invalid_property_or_value
```

### Issue Set BT: `FT_Property_Get/Set` supported-property exact routes

Previous blocker:

- `ftmodapi.FT_Property_Get.gets_supported_property` and
  `ftmodapi.FT_Property_Set.sets_supported_property` stayed in
  `generic-fallback`.
- The fixtures require exact successful behavior from
  `freetype/src/base/ftobjs.c:FT_Property_Get`,
  `freetype/src/base/ftobjs.c:FT_Property_Set`, and the TrueType driver
  `interpreter-version` property path, but the route audit had no explicit
  maintained classification for these successful property rows.

Plan:

1. Keep the fixtures intact; they exercise public property get and set/get
   behavior for the TrueType `interpreter-version` property.
2. Require the existing exact fixture comparison for status and returned
   property values.
3. Classify the rows as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused supported-property get and set/get rows pass exact comparison
  against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `ftmodapi.FT_Property_Get.gets_supported_property`
  - `ftmodapi.FT_Property_Set.sets_supported_property`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Get.gets_supported_property
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Set.sets_supported_property
```

### Issue Set BU: `FT_Library_SetLcdFilterWeights` exact-error routes

Previous blocker:

- `ftlcdfil.FT_Library_SetLcdFilterWeights.error_null_library` and
  `ftlcdfil.FT_Library_SetLcdFilterWeights.error_null_weights` stayed in
  `generic-error-fallback`.
- The fixtures require exact behavior from `freetype/src/base/ftlcdfil.c`, but
  the harness still accepted these rows as generic expected-error rows instead
  of enforcing exact error output and unchanged-weight observation where
  applicable.

Plan:

1. Keep the fixtures intact; they exercise public
   `FT_Library_SetLcdFilterWeights` error behavior for null library and null
   weight pointer inputs.
2. Require exact error comparison for both case IDs.
3. Classify each row as real parity only after focused same-input parity
   passes through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The two focused `FT_Library_SetLcdFilterWeights` rows pass exact comparison
  against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `ftlcdfil.FT_Library_SetLcdFilterWeights.error_null_library`
  - `ftlcdfil.FT_Library_SetLcdFilterWeights.error_null_weights`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_Library_SetLcdFilterWeights.error_null_library
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_Library_SetLcdFilterWeights.error_null_weights
```

### Issue Set BV: `FT_Library_SetLcdGeometry` exact-error routes

Previous blocker:

- `ftlcdfil.FT_Library_SetLcdGeometry.error_null_library` and
  `ftlcdfil.FT_Library_SetLcdGeometry.error_null_geometry` stayed in
  `generic-error-fallback`.
- The fixtures require exact behavior from `freetype/src/base/ftlcdfil.c`, but
  the harness still accepted these rows as generic expected-error rows instead
  of enforcing exact error output and unchanged-geometry observation where
  applicable.

Plan:

1. Keep the fixtures intact; they exercise public
   `FT_Library_SetLcdGeometry` error behavior for null library and null
   geometry pointer inputs.
2. Require exact error comparison for both case IDs.
3. Classify each row as real parity only after focused same-input parity
   passes through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The two focused `FT_Library_SetLcdGeometry` rows pass exact comparison
  against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `ftlcdfil.FT_Library_SetLcdGeometry.error_null_library`
  - `ftlcdfil.FT_Library_SetLcdGeometry.error_null_geometry`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_Library_SetLcdGeometry.error_null_library
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_Library_SetLcdGeometry.error_null_geometry
```

### Issue Set BW: `FT_Library_SetLcdFilter` exact-error routes

Previous blocker:

- The enabled-branch `FT_Library_SetLcdFilter` error rows stayed in
  `generic-error-fallback`.
- The fixtures require exact behavior from `freetype/src/base/ftlcdfil.c`, but
  the harness still accepted these rows as generic expected-error rows instead
  of enforcing exact error output and unchanged-weight observation where
  applicable.

Plan:

1. Keep the fixtures intact; they exercise public `FT_Library_SetLcdFilter`
   behavior for null libraries and rejected filter values.
2. Require exact error comparison for the focused enabled-branch case IDs.
3. Classify each row as real parity only after focused same-input parity
   passes through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
4. Leave the build-dependent
   `ftlcdfil.FT_Library_SetLcdFilter.unimplemented_without_subpixel_filtering`
   row outside this promotion until it is separately proven.

Verified progress:

- The six focused `FT_Library_SetLcdFilter` rejection rows pass exact
  comparison against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `ftlcdfil.FT_Library_SetLcdFilter.error_null_library`
  - `ftlcdfil.FT_Library_SetLcdFilter.error_invalid_filter`
  - `ftlcdfil.FT_LcdFilter.rejected_filter_values`
  - `ftlcdfil.FT_LCD_FILTER_LEGACY.rejected_by_set_lcd_filter`
  - `ftlcdfil.FT_LCD_FILTER_LEGACY1.rejected_by_set_lcd_filter`
  - `ftlcdfil.FT_LCD_FILTER_MAX.rejected_by_set_lcd_filter`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_Library_SetLcdFilter.error_null_library
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_Library_SetLcdFilter.error_invalid_filter
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_LcdFilter.rejected_filter_values
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_LCD_FILTER_LEGACY.rejected_by_set_lcd_filter
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_LCD_FILTER_LEGACY1.rejected_by_set_lcd_filter
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_LCD_FILTER_MAX.rejected_by_set_lcd_filter
```

### Issue Set BX: `FT_Get_Var_Design_Coordinates` null-output route

Previous blocker:

- `ftmm.FT_Get_Var_Design_Coordinates.error_null_coords` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:362-388`,
  where the public wrapper returns `FT_Err_Invalid_Argument` before service
  lookup when the output coordinate pointer is null.

Plan:

1. Keep the fixture intact; it exercises public
   `FT_Get_Var_Design_Coordinates` behavior for a valid variable face with a
   null coordinate output pointer.
2. Require exact error comparison for return status and the `coords_written`
   observation.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused `FT_Get_Var_Design_Coordinates` null-coords row passes exact
  comparison against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_Var_Design_Coordinates.error_null_coords` as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Var_Design_Coordinates.error_null_coords
```

### Issue Set BY: `FT_Set_Var_*_Coordinates` null-input routes

Previous blocker:

- `ftmm.FT_Set_Var_Design_Coordinates.error_null_coords_with_nonzero_count`
  and `ftmm.FT_Set_Var_Blend_Coordinates.error_null_coords_with_nonzero_count`
  stayed in `generic-error-fallback`.
- The fixtures require exact behavior from `freetype/src/base/ftmm.c`, where
  the public wrappers return `FT_Err_Invalid_Argument` before service lookup
  when `num_coords` is nonzero and the coordinate pointer is null.

Plan:

1. Keep the fixtures intact; they exercise public variation-coordinate setter
   behavior for valid variable faces with null coordinate input pointers.
2. Require exact error comparison for return status and the
   `variation_state_changed` observation.
3. Classify each row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused `FT_Set_Var_Design_Coordinates` and
  `FT_Set_Var_Blend_Coordinates` null-coordinate rows pass exact comparison
  against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `ftmm.FT_Set_Var_Design_Coordinates.error_null_coords_with_nonzero_count`
  - `ftmm.FT_Set_Var_Blend_Coordinates.error_null_coords_with_nonzero_count`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_Var_Design_Coordinates.error_null_coords_with_nonzero_count
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_Var_Blend_Coordinates.error_null_coords_with_nonzero_count
```

### Issue Set BZ: `FT_Set_MM_*_Coordinates` null-input routes

Previous blocker:

- `ftmm.FT_Set_MM_Blend_Coordinates.error_null_coords_with_nonzero_count`
  and `ftmm.FT_Set_MM_Design_Coordinates.error_null_coords_with_nonzero_count`
  stayed in `generic-error-fallback`.
- The fixtures require exact behavior from `freetype/src/base/ftmm.c`, where
  the public wrappers return `FT_Err_Invalid_Argument` before service lookup
  when `num_coords` is nonzero and the coordinate pointer is null.

Plan:

1. Keep the fixtures intact; they exercise public multiple-master coordinate
   setter behavior for valid variable/MM faces with null coordinate input
   pointers.
2. Require exact error comparison for return status and the
   `variation_state_changed` observation.
3. Classify each row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused `FT_Set_MM_Blend_Coordinates` and
  `FT_Set_MM_Design_Coordinates` null-coordinate rows pass exact comparison
  against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies these case IDs as `real-parity`:
  - `ftmm.FT_Set_MM_Blend_Coordinates.error_null_coords_with_nonzero_count`
  - `ftmm.FT_Set_MM_Design_Coordinates.error_null_coords_with_nonzero_count`

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_Blend_Coordinates.error_null_coords_with_nonzero_count
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_Design_Coordinates.error_null_coords_with_nonzero_count
```

### Issue Set CA: `FT_Get_Var_Blend_Coordinates` null-output route

Previous blocker:

- `ftmm.FT_Get_Var_Blend_Coordinates.error_null_coords` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:574-600`,
  where the public wrapper returns `FT_Err_Invalid_Argument` before service
  lookup when the output coordinate pointer is null.

Plan:

1. Keep the fixture intact; it exercises public
   `FT_Get_Var_Blend_Coordinates` behavior for a valid variable face with a
   null coordinate output pointer.
2. Require exact error comparison for return status and the `coords_written`
   observation.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
4. Keep sibling invalid/non-variable-face scenarios outside this promotion
   until separately proven.

Verified progress:

- The focused `FT_Get_Var_Blend_Coordinates` null-coords row passes exact
  comparison against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_Var_Blend_Coordinates.error_null_coords` as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Var_Blend_Coordinates.error_null_coords
```

### Issue Set CB: `FT_Get_Var_Blend_Coordinates` invalid/non-variable-face route

Previous blocker:

- `ftmm.FT_Get_Var_Blend_Coordinates.error_non_variable_or_invalid_face`
  stayed in `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:574-600`
  and `freetype/src/base/ftmm.c:31-52`: null faces return
  `FT_Err_Invalid_Face_Handle`, while static faces without a multiple-master
  service return `FT_Err_Invalid_Argument`.
- Both scenarios preserve the sentinel output coordinate buffer.

Plan:

1. Keep the fixture intact; it exercises the public
   `FT_Get_Var_Blend_Coordinates` wrapper for a null face and a static
   non-variable face.
2. Require exact error comparison for scenario, return status, and preserved
   coordinate output.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused invalid/non-variable-face row passes exact comparison against
  pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_Var_Blend_Coordinates.error_non_variable_or_invalid_face` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Var_Blend_Coordinates.error_non_variable_or_invalid_face
```

### Issue Set CC: `FT_Get_MM_Blend_Coordinates` invalid-input route

Previous blocker:

- `ftmm.FT_Get_MM_Blend_Coordinates.invalid_face_or_coords_error` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:545-571`:
  null coordinate output returns `FT_Err_Invalid_Argument` before service
  lookup, null face is rejected by `ft_face_get_mm_service`, and non-variable
  static faces return `FT_Err_Invalid_Argument`.
- Valid output buffers are preserved on the error scenarios.

Plan:

1. Keep the fixture intact; it exercises the public
   `FT_Get_MM_Blend_Coordinates` wrapper over null-coordinate, null-face, and
   non-variable-face argument rows.
2. Require exact error comparison for each row status and preserved coordinate
   output.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused invalid-input row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_MM_Blend_Coordinates.invalid_face_or_coords_error` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_Blend_Coordinates.invalid_face_or_coords_error
```

### Issue Set CD: `FT_Get_MM_Var` null-output route

Previous blocker:

- `ftmm.FT_Get_MM_Var.null_output_error` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:123-143`:
  the public wrapper returns `FT_Err_Invalid_Argument` before service lookup
  when the `FT_MM_Var**` output pointer is null.

Plan:

1. Keep the fixture intact; it exercises the public `FT_Get_MM_Var` wrapper
   with a valid variable face and null output pointer.
2. Require exact error comparison for return status and absence of descriptor
   output.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused null-output row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies `ftmm.FT_Get_MM_Var.null_output_error` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_Var.null_output_error
```

### Issue Set CE: `FT_Get_MM_Var` invalid/non-variable-face route

Previous blocker:

- `ftmm.FT_Get_MM_Var.invalid_or_non_variable_face_error` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:38-58`
  and `freetype/src/base/ftmm.c:123-143`: null faces return
  `FT_Err_Invalid_Face_Handle`, non-variable static faces return
  `FT_Err_Invalid_Argument`, and the caller's output pointer is preserved.

Plan:

1. Keep the fixture intact; it exercises the public `FT_Get_MM_Var` wrapper
   over null-face and non-variable-face argument rows.
2. Require exact error comparison for row status and preserved descriptor
   output pointer.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused invalid/non-variable-face row passes exact comparison against
  pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_MM_Var.invalid_or_non_variable_face_error` as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_Var.invalid_or_non_variable_face_error
```

### Issue Set CF: `FT_Get_MM_WeightVector` len-without-buffer route

Previous blocker:

- `ftmm.FT_Get_MM_WeightVector.len_without_buffer_error` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:253-274`:
  when `len` is non-null but `weightvector` is null, the public wrapper returns
  `FT_Err_Invalid_Argument` before service lookup and preserves the length
  value.

Plan:

1. Keep the fixture intact; it exercises the public
   `FT_Get_MM_WeightVector` wrapper with a valid length pointer and null output
   buffer.
2. Require exact error comparison for return status and preserved `len` output.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused len-without-buffer row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_MM_WeightVector.len_without_buffer_error` as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_WeightVector.len_without_buffer_error
```

### Issue Set CG: `FT_Get_MM_WeightVector` unsupported-face route

Previous blocker:

- `ftmm.FT_Get_MM_WeightVector.unsupported_face_error` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:38-58`
  and `freetype/src/base/ftmm.c:253-274`: null faces return
  `FT_Err_Invalid_Face_Handle`, non-MM/static faces return
  `FT_Err_Invalid_Argument`, and valid output buffers are preserved on errors.
- The fixture source reference previously grouped this with the setter area;
  this promotion is specifically for the public getter wrapper.

Plan:

1. Keep the fixture intact; it exercises the public
   `FT_Get_MM_WeightVector` wrapper over unsupported face rows.
2. Require exact error comparison for row status, `len`, and preserved
   `weightvector` output.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused unsupported-face row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_MM_WeightVector.unsupported_face_error` as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_WeightVector.unsupported_face_error
```

### Issue Set CH: `FT_Get_Multi_Master` TrueType/OpenType variation route

Previous blocker:

- `ftmm.FT_Get_Multi_Master.true_type_or_opentype_variation_error` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:88-111`:
  the public wrapper asks the multiple-master service for the legacy
  `get_mm` entry, initializes the result to `FT_Err_Invalid_Argument`, and
  preserves the sentinel `FT_Multi_Master` descriptor when that legacy Adobe MM
  service entry is unavailable on TrueType/OpenType variation faces.

Plan:

1. Keep the fixture intact; it exercises the public `FT_Get_Multi_Master`
   wrapper against a TrueType/OpenType variation face.
2. Require exact error comparison for return status and preserved descriptor
   output.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused TrueType/OpenType variation row passes exact comparison against
  pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_Multi_Master.true_type_or_opentype_variation_error` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Multi_Master.true_type_or_opentype_variation_error
```

### Issue Set CI: `FT_Set_MM_Design_Coordinates` non-Adobe variation route

Previous blocker:

- `ftmm.FT_Set_MM_Design_Coordinates.error_non_adobe_variation_face` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:169-184`:
  the public wrapper rejects nonzero null coordinates before service lookup,
  otherwise asks for the multiple-master service, initializes the result to
  `FT_Err_Invalid_Argument`, and only calls `service->set_mm_design` when the
  legacy Adobe MM design-coordinate setter exists. TrueType/OpenType variation
  faces use the variation design-coordinate setter instead, and static faces do
  not expose the Adobe MM setter.

Plan:

1. Keep the fixture intact; it exercises both variable and static non-Adobe
   faces through the public `FT_Set_MM_Design_Coordinates` wrapper.
2. Require exact error comparison for the return status on the same input.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused non-Adobe variation/static-face row passes exact comparison
  against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Set_MM_Design_Coordinates.error_non_adobe_variation_face` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_Design_Coordinates.error_non_adobe_variation_face
```

### Issue Set CJ: `FT_Set_MM_WeightVector` null-weightvector route

Previous blocker:

- `ftmm.FT_Set_MM_WeightVector.error_null_weightvector_with_nonzero_len`
  stayed in `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:212-223`:
  the public wrapper returns `FT_Err_Invalid_Argument` before multiple-master
  service lookup when `len` is nonzero and `weightvector` is null.

Plan:

1. Keep the fixture intact; it exercises the public `FT_Set_MM_WeightVector`
   wrapper with a real Adobe MM face and a null weight-vector pointer.
2. Require exact error comparison for the return status on the same input.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused null-weightvector row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Set_MM_WeightVector.error_null_weightvector_with_nonzero_len` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_WeightVector.error_null_weightvector_with_nonzero_len
```

### Issue Set CK: `FT_Set_MM_WeightVector` unsupported variation route

Previous blocker:

- `ftmm.FT_Set_MM_WeightVector.error_unsupported_on_true_type_variations`
  stayed in `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:212-231`:
  after the null-pointer precheck, the public wrapper asks for the
  multiple-master service, initializes the result to `FT_Err_Invalid_Argument`,
  and only calls `service->set_mm_weightvector` when that legacy Adobe MM
  setter exists. TrueType/OpenType variation faces expose variation
  coordinates but do not provide the Adobe MM weight-vector setter.

Plan:

1. Keep the fixture intact; it exercises the public `FT_Set_MM_WeightVector`
   wrapper against a real TrueType/OpenType variation face.
2. Require exact error comparison for the return status on the same input.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused unsupported-variation row passes exact comparison against pinned
  C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Set_MM_WeightVector.error_unsupported_on_true_type_variations` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_WeightVector.error_unsupported_on_true_type_variations
```

### Issue Set CL: `FT_Get_Multi_Master` invalid/non-variable route

Previous blocker:

- `ftmm.FT_Get_Multi_Master.invalid_or_non_variable_face_error` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftmm.c:96-111`:
  the public wrapper rejects null `amaster` before service lookup, otherwise
  delegates face validation to `ft_face_get_mm_service`; invalid faces or
  non-variable faces return the public FreeType error while preserving the
  caller's sentinel `FT_Multi_Master` descriptor.

Plan:

1. Keep the fixture intact; it exercises both null-face and concrete
   non-variable-face inputs through the public `FT_Get_Multi_Master` wrapper.
2. Require exact error comparison for return status and preserved descriptor
   output on the same input.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused invalid/non-variable row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `ftmm.FT_Get_Multi_Master.invalid_or_non_variable_face_error` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Multi_Master.invalid_or_non_variable_face_error
```

### Issue Set CM: `FT_Render_Glyph` invalid render-mode route

Previous blocker:

- `freetype.FT_Render_Glyph.invalid_render_mode` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftobjs.c:4983-4994`
  and `freetype/src/base/ftobjs.c:4733-4855`: the public wrapper rejects null
  or detached slots before dispatch, then delegates loaded slots to
  `FT_Render_Glyph_Internal`, where renderers reject unsupported render modes
  with the public FreeType error.

Plan:

1. Keep the fixture intact; it exercises three concrete local fonts with the
   same invalid render-mode value.
2. Require exact error comparison for the return status on the same input.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused invalid-render-mode row passes exact comparison for all three
  variants against pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies the three concrete
  `freetype.FT_Render_Glyph.invalid_render_mode` rows as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Render_Glyph.invalid_render_mode
```

### Issue Set CN: `FT_RENDER_MODE_MAX` render rejection route

Previous blocker:

- `freetype.FT_RENDER_MODE_MAX.render_glyph_rejects_sentinel` stayed in
  `generic-error-fallback`.
- The fixture requires exact behavior from `freetype/src/base/ftobjs.c:4983-4994`
  and `freetype/src/base/ftobjs.c:4733-4855`: `FT_RENDER_MODE_MAX` is a
  sentinel outside the renderable mode range, so a loaded outline slot reaches
  renderer dispatch and is rejected with the public FreeType error.

Plan:

1. Keep the fixture intact; it exercises a concrete local font with
   `render_mode` set to `FT_RENDER_MODE_MAX`.
2. Require exact error comparison for the return status on the same input.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused sentinel render-mode row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `freetype.FT_RENDER_MODE_MAX.render_glyph_rejects_sentinel` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_RENDER_MODE_MAX.render_glyph_rejects_sentinel
```

### Issue Set CO: `FT_Render_Glyph` unsupported slot-format route

Previous blocker:

- `freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format` stayed
  in `generic-error-fallback`.
- The routed fixture variant requires exact behavior from
  `freetype/src/base/ftobjs.c:4983-4994` and
  `freetype/src/base/ftobjs.c:4733-4855`: a loaded composite glyph slot
  produced by `FT_LOAD_NO_RECURSE` reaches renderer dispatch, where renderers
  return `FT_Err_Cannot_Render_Glyph` for unsupported glyph image formats.

Plan:

1. Keep the fixture intact; classify only the concrete routed composite-slot
   variant and leave the separate unrouted slot-state variant pending until a
   public runner exists.
2. Require exact error comparison for the return status on the same input.
3. Classify the routed row as real parity only after focused same-input parity
   passes through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused composite-slot row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format` as
  `real-parity` for the routed concrete row.
- The harness still reports the unrouted slot-state row as pending.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format
```

### Issue Set CP: `FT_LOAD_TARGET_MODE` invalid render-target route

Previous blocker:

- `freetype.FT_LOAD_TARGET_MODE.render_rejects_invalid_target_mode` stayed in
  `generic-error-fallback`.
- The fixture already pins exact C behavior for `FT_Load_Glyph` with
  `FT_LOAD_RENDER | FT_LOAD_TARGET_(6)`: `FT_LOAD_TARGET_MODE` extracts the
  four-bit render mode from bits 16-19
  (`freetype/include/freetype/freetype.h:3617-3636`), and
  `FT_Load_Glyph` passes that mode into `FT_Render_Glyph` when
  `FT_LOAD_RENDER` is set (`freetype/src/base/ftobjs.c:1168-1177`). The
  unsupported render mode returns `FT_Err_Cannot_Render_Glyph`.

Plan:

1. Keep the fixture intact; it already records the exact public error expected
   for the same input.
2. Require exact error comparison for the return status on the same input.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused invalid render-target row passes exact comparison against pinned
  C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `freetype.FT_LOAD_TARGET_MODE.render_rejects_invalid_target_mode` as
  `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_LOAD_TARGET_MODE.render_rejects_invalid_target_mode
```

### Issue Set CQ: `FT_New_Memory_Face` null file-base route

Previous blocker:

- `freetype.FT_New_Memory_Face.error_null_file_base` stayed in
  `generic-error-fallback`.
- The fixture already pins exact C behavior for a valid library, null
  `file_base`, nonzero `file_size`, and `face_index=0`: `FT_New_Memory_Face`
  returns `FT_Err_Invalid_Argument` before constructing `FT_Open_Args` or
  delegating to `ft_open_face_internal`
  (`freetype/src/base/ftobjs.c:1629-1647`).

Plan:

1. Keep the fixture intact; it already records the exact public error expected
   for the same input.
2. Require exact error comparison for the return status on the same input.
3. Classify the row as real parity only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The focused null file-base row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `freetype.FT_New_Memory_Face.error_null_file_base` as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_New_Memory_Face.error_null_file_base
```

### Issue Set CR: `FT_New_Memory_Face` null library/aface route

Previous blocker:

- `freetype.FT_New_Memory_Face.error_null_library_or_aface` stayed in
  `generic-error-fallback`.
- The fixture pins two same-input variants for valid font bytes:
  null `library` and valid `aface`, then valid `library` and null `aface`.
  `FT_New_Memory_Face` delegates non-null `file_base` inputs to
  `ft_open_face_internal` (`freetype/src/base/ftobjs.c:1629-1647`); the null
  library variant fails through `FT_Stream_New`
  (`freetype/src/base/ftobjs.c:199-211`), while the null `aface` variant is
  rejected after stream creation in `ft_open_face_internal`
  (`freetype/src/base/ftobjs.c:2568-2586`).
- Exact-error gating was tested and correctly rejected classification: the
  maintained oracle route returned `Ok` for the current row shape, so the
  strict comparator reported `rust ffi:value` instead of proving exact C error
  parity.

Plan:

1. Preserve the fixture and teach the maintained `new_memory_face` variant row
   encoding to carry null `library` and null `aface` flags to the C oracle.
2. Make the Rust FFI and WASM lanes apply the same public wrapper input policy
   for null `library`/`aface` variants.
3. Fix the thin C ABI `FT_New_Memory_Face` validation order to match C
   FreeType: null `file_base` returns `Invalid_Argument`, then null `library`
   returns `Invalid_Library_Handle`, then null `aface` returns
   `Invalid_Argument`.
4. Require exact error comparison only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The C oracle now receives and executes the row's null `library` and null
  `aface` variants instead of collapsing them into normal face opens.
- The thin C ABI now returns `FT_Err_Invalid_Library_Handle` for null
  `library`, matching pinned C FreeType.
- The focused null library/aface row passes exact comparison against pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `freetype.FT_New_Memory_Face.error_null_library_or_aface` as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_New_Memory_Face.error_null_library_or_aface
```

### Issue Set CS: `FT_Open_Face` null library/args/aface route

Previous blocker:

- `freetype.FT_Open_Face.error_null_library_args_or_aface` stayed in
  `generic-error-fallback`.
- The fixture pins three same-input variants: null `library`, null `args`, and
  null `aface`. `FT_Open_Face` delegates to `ft_open_face_internal`
  (`freetype/src/base/ftobjs.c:2514-2525`); null `args` is rejected there,
  null `library` is rejected by `FT_Stream_New`, and null `aface` is rejected
  after stream creation (`freetype/src/base/ftobjs.c:2568-2586`).
- Exact-error gating was tested and correctly rejected classification: the
  maintained oracle route returned `Ok` for the current row shape, so the
  strict comparator reported `rust ffi:value` instead of proving exact C error
  parity.

Plan:

1. Preserve the fixture and route this row through a maintained
   `FT_Open_Face` oracle command instead of the `FT_New_Memory_Face` variant
   command.
2. Add a thin C ABI `FT_Open_Face` export with the public `FT_Open_Args`
   layout and memory-source delegation needed for same-input parity.
3. Preserve C validation order: null `args` is rejected before stream
   creation, null `library` is rejected by `FT_Stream_New`, and null `aface`
   is rejected after stream creation.
4. Require exact error comparison only after focused same-input parity passes
   through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Verified progress:

- The C oracle now executes this row through `FT_Open_Face` with null
  `library`, null `args`, and null `aface` variants.
- The thin C ABI now exposes `FT_Open_Face` and the `FT_Open_Args` record,
  delegating the memory-source subset to the already verified
  `FT_New_Memory_Face` path.
- The focused null library/args/aface row passes exact comparison against
  pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The route audit now classifies
  `freetype.FT_Open_Face.error_null_library_args_or_aface` as `real-parity`.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Open_Face.error_null_library_args_or_aface
```

### Issue Set BE: `FT_Outline_Get_BBox` null probe route blocker

Current blocker:

- `ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output` is a valid public
  `FT_Outline_Get_BBox` null-input fixture, but current maintained runtime
  runners for `ftbbox.outline_get_bbox` observe a loaded glyph slot's stored
  `outline_bbox` instead of invoking a public Rust FFI / thin C ABI / WASM ABI
  `FT_Outline_Get_BBox` endpoint.
- Exact-error gating was tested and correctly rejected classification: the
  pinned oracle path returned success for the normal glyph-outline route rather
  than executing the fixture's `null_outline` / `null_abbox` probes.

Plan:

1. Do not classify this row as real parity until the harness calls an actual
   `FT_Outline_Get_BBox` endpoint for pinned C, Rust FFI, C ABI, and WASM ABI.
2. Implement the real public endpoint, not a null-only shortcut.
3. Reuse the `FT_Outline_Get_CBox` null-input runner structure where possible,
   but preserve `FT_Outline_Get_BBox`'s distinct return/error behavior from
   FreeType `src/base/ftbbox.c:474-486`.
4. Re-enable exact-error gating only after focused same-input comparison
   passes.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output
```

## Coverage Bulk Context

Current condition-coverage bulk context from
`target/coverage/unified-condition-summary.json`:

| File | Lines | Branches | Functions |
|---|---:|---:|---:|
| `src/tt/sbit.rs` | 514 / 638 | 60 / 60 | 34 / 87 |
| `src/grays.rs` | 727 / 740 | 177 / 178 | 32 / 33 |
| `src/autohint/latin.rs` | 2608 / 2844 | 1079 / 1286 | 70 / 73 |
| `src/render.rs` | 2112 / 2222 | 379 / 434 | 135 / 140 |
| `src/scaler.rs` | 1135 / 1276 | 190 / 200 | 49 / 63 |

Use condition coverage as secondary prioritization only: route audit category
and route shape decide whether a row is real parity, while condition coverage
helps pick the first implementation branch inside a chosen bucket.

### Render Bucket Route Findings

`src/render.rs` is at 379 / 434 branches after the current
`FT_Render_Glyph` route sweep.  The following exact public rows were tested in
an isolated worktree and removed because they preserved exact parity but did
not move the `src/render.rs` branch total:

- `render-coverage-subpixel-conic-mono`
- `render-coverage-empty-notdef-normal`
- `render-coverage-top-edge-dropout-mono`

Current render branch blockers should be treated as dependencies, not as
green-row opportunities:

| Render miss bucket | Public route status | Dependency / reason |
|---|---|---|
| `OUTLINE_SINGLE_PASS` mono branch in `render_mono` | Existing `ftimage.FT_OUTLINE_SINGLE_PASS` rows route through `FT_Outline_Render` and the gray rasterizer, not `render.rs`. | FreeType `ttgload.c` clears `FT_OUTLINE_SINGLE_PASS` during glyph loading, and TrueType scan-control maps only dropout/include flags. A real `FT_Render_Glyph` route needs a public glyph-slot path that can carry this flag into mono rendering. |
| Removed obsolete mono helper family | No public route entered the old `MonoProfileBuilder`, intersection rasterizer, horizontal center-edge pass, or low-precision line/bezier wrappers. | These private helpers were duplicate/no-call code beside the active `MonoOutlineProfileBuilder` path and were removed after exact runtime parity stayed `7045 / 7045`. Do not re-add helper-only rows for them. |
| `MonoOutlineProfileBuilder` residuals | Public `FT_Render_Glyph` mono routes reach this family, but many residual subconditions are exact topology dependencies. | Existing `hinter-control-matrix.ttf` and `render-coverage.ttf` rows cover scan types, folded dropout, zero-height sweep, and horizontal/vertical alternate-set dropout. Add only a compact glyph proven by focused condition deltas; several prior mono topologies passed parity without moving coverage and are listed in `FONT_FIXTURE_COVERAGE_PLAN.md`. |
| Empty loaded outline second operands (`render_normal`, SDF/LCD cbox helpers) | DejaVu space and empty-outline candidates cover only the already-hit `points.is_empty()` side. | The remaining operands require a public loaded outline with non-empty points and `n_contours == 0` or empty contour vectors. Valid C FreeType glyph loading does not produce that state; malformed synthetic outlines need a separate C-oracle public route. |
| Invalid contour/cubic outline errors | Public font loading rejects or normalizes malformed contours before render helpers. | Branches in `MonoOutlineProfileBuilder::decompose` and SDF orientation require invalid contour ordering, first-point cubic, broken conic/cubic tag sequences, or out-of-range contour ends after a slot is already loaded. Keep as pending/unsupported unless runner support can pass a real public malformed `FT_Outline` to the C oracle. |
| SDF negative saturation in `map_fixed_to_sdf` | Large/reverse/edge SDF probes and the current SDF fixture rows do not move it. | `rasterize_sdf_outline` clamps distance to `fixed_spread` before mapping, so a valid negative normalized distance reaches `udist == 128`, not `> 128`. |
| Removed `winding_contains` SDF helper | No current public route entered it. | The current SDF implementation uses edge distances plus outline orientation; the private winding helper had no caller and was removed with the obsolete render cleanup. |

## Candidate Conversion Buckets

These are the highest-value route families to convert from placeholder success
to real C/Rust/C-ABI/WASM parity. They are intentionally scoped as subagent
units, not as a single monolithic implementation task.

| Bucket | Rows | Owned routes | Likely owned files | Main dependency |
|---|---:|---|---|---|
| COLR/color/palette traversal | 130 | `ftcolor.*`, `otsvg.*`, SVG/color glyph load probes | `src/tables.rs`, new color table module if added, `src/font.rs`, `src/ffi/*`, `ffi-c/src/lib.rs`, `ffi-wasm/src/lib.rs`, `tests/fixtures/inputs/public-api/ftcolor.*.json` | COLR/CPAL/SVG data model and iterator ABI, then C/Rust/ABI fixture runner routes. |
| FTC cache subsystem | 112 | `ftcache.*` manager, cmap/image/sbit cache, node lifecycle | new cache module if added, `src/font.rs`, `src/tt/sbit.rs`, `src/ffi/*`, C/WASM wrappers, `ftcache.*.json` inputs | Manager-owned face/size/cache-node handles with exact FreeType error/null behavior. |
| Stroker geometry | 86 | `ftstroke.*` parse, export, glyph stroke/border, counts | new stroker module if added, `src/outline.rs`, `src/render.rs`, `src/ffi/*`, C/WASM wrappers, `ftstroke.*.json` inputs | Pure-Rust stroker path construction and exact border/count/export geometry. |
| Multiple-master and variable fonts | 84 | `ftmm.*`, named instances, variation table rows | `src/tt/fvar.rs`, `src/tables.rs`, `src/font.rs`, `src/scaler.rs`, C/WASM wrappers, `ftmm.*.json` inputs | Complete `FT_MM_Var`, Adobe MM, `gvar`/`HVAR`/`MVAR` behavior before pending rows can become real. |
| Error-path asset routing | 54 | `fterrdef.*` error rows across face load, render, module, stream paths | `tests/unified_fixture_parity.rs`, public-api input rows, runner/oracle routing, then relevant core modules | Replace no-asset expected-error placeholders with concrete C oracle inputs and Rust route execution. |
| Outline/image/raster callbacks | 88 | `ftimage.*`, `ftoutln.*`, `ftrender.*` decompose/render/raster routes | `src/outline.rs`, `src/render.rs`, `src/grays.rs`, `src/ffi/*`, C/WASM wrappers | Callback-compatible outline decomposition, bitmap extraction, renderer mode state, and exact error propagation. |
| Module/property APIs | 72 | `ftmodapi.*`, `ftdriver.*`, `ftparams.*`, `freetype.face_properties*` | `src/api.rs`, `src/font.rs`, `src/autohint/*`, `src/tt/hinter/*`, `src/ffi/*`, C/WASM wrappers | Decide exact supported-vs-unsupported module surface, then route properties through real core state. |
| Glyph object APIs | 25 | `ftglyph.*` plus the allocator-fault `ftbitmap.glyphslot_own_bitmap` pending row | `src/render.rs`, `src/font.rs`, `src/outline.rs`, `src/ffi/*`, C/WASM wrappers | Glyph object handles, bitmap glyph ownership, transform/copy/done semantics, and maintained allocator fault injection for the remaining bitmap pending row. |
| GX/OpenType validation | 58 | `ftgxval.*`, `ftotval.*` validate/free rows | `src/tables.rs`, new validator modules if added, `src/ffi/*`, C/WASM wrappers | Validation buffer ownership and exact selected-table success/error behavior. |
| Legacy format/stream families | 100 | `t1tables.*`, `ftwinfnt.*`, `ftbdf.*`, `ftpfr.*`, `ftcid.*`, compressed stream rows | new format/stream modules if added, `src/font.rs`, `src/tables.rs`, `src/ffi/*`, C/WASM wrappers | Decide supported pure-Rust parsers vs exact unsupported/error policy, then add real oracle inputs. |

## Recommended Subagent Slices

1. `ftcache` cache-manager real parity: own `ftcache.*` routes only. Start with
   `FTC_Manager_New`, `FTC_Manager_Done`, and `FTC_CMapCache_New` before lookup
   rows.
2. `ftcolor` COLR/CPAL iteration: own color paint graph, palette, colorline,
   and layer routes. Keep SVG document rows out unless the implementation
   reaches SVG glyph loading.
3. `ftstroke` stroker core: own stroker create/configure/parse/count/export
   routes. Do not mix with generic outline decomposition fixes.
4. `ftmm` variable-font descriptors and named instances: own `ftmm.*` and the
   three pending named-instance rows; leave `MVAR` SFNT row as a follow-up if
   `MVAR` is not implemented.
5. Error-path concrete assets: own `generic-error-fallback`,
   `null-error-fallback`, and `void-fallback` rows, converting placeholders to
   concrete C/Rust route checks without changing expected outputs.
6. Outline/image/raster callbacks: own `ftimage.*`, `ftoutln.*`, and
   `ftrender.*` routes that require callback or renderer state.
7. Glyph object lifecycle: own `ftglyph.*` rows and the remaining
   `ftbitmap.glyphslot_own_bitmap` allocator-fault pending row. The public
   bitmap copy/convert/done/embolden/blend routes are already real parity.
8. Module/property behavior: own `ftmodapi.*`, `ftdriver.*`, `ftparams.*`, and
   `freetype.face_properties*`; first classify exact unsupported behavior vs
   real stateful support.
9. Validation APIs: own `ftgxval.*` and `ftotval.*`; route validate/free buffer
   lifetimes through all ABI surfaces.
10. Legacy formats and streams: own BDF, PFR, CID, Type 1, WinFNT, gzip, bzip2,
    and LZW rows only after the supported-vs-unsupported policy is explicit.
