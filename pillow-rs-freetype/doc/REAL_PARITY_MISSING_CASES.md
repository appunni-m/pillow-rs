# Real-Parity Missing Cases

Baseline: `7d7c1d23`

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
| real-parity | 3838 |
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
| 10 | `generic-fallback` | `ftimage` | `ftoutln.outline_decompose` | `ftimage.FT_CURVE_TAG / classifies_outline_tags` | no explicit maintained route classification |
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

## Coverage Bulk Context

Current condition-coverage bulk context from
`target/coverage/unified-condition-summary.json`:

| File | Lines | Branches | Functions |
|---|---:|---:|---:|
| `src/tt/sbit.rs` | 514 / 638 | 60 / 60 | 34 / 87 |
| `src/grays.rs` | 727 / 740 | 177 / 178 | 32 / 33 |
| `src/autohint/latin.rs` | 2608 / 2844 | 1079 / 1286 | 70 / 73 |
| `src/render.rs` | 2099 / 2597 | 379 / 434 | 142 / 183 |
| `src/scaler.rs` | 1182 / 1305 | 200 / 218 | 53 / 65 |

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
| Mono intersection fallback helpers (`rasterize_mono_intersections`, `segment_intersection`, `apply_horizontal_center_edges`) | No current public route can enter them. | These helpers are currently definition-only in `src/render.rs`; coverage requires restoring a C-equivalent caller path, not adding more fixture rows. |
| SDF negative saturation in `map_fixed_to_sdf` | Large/reverse/edge SDF probes and the current SDF fixture rows do not move it. | `rasterize_sdf_outline` clamps distance to `fixed_spread` before mapping, so a valid negative normalized distance reaches `udist == 128`, not `> 128`. |
| `winding_contains` SDF helper | No current public route can enter it. | The helper is definition-only under the current SDF implementation path; treat as a missing/unused algorithm path unless a C-equivalent caller is restored. |

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
