# Real-Parity Missing Cases

### Issue Set Current: `FT_ORIENTATION_FILL_LEFT` reverse observation route

Status: completed on 2026-07-20 for one composite outline reverse/orientation
row.

Implemented real parity row:

- `ftoutln.FT_ORIENTATION_FILL_LEFT.reverse_toggles_orientation_fixture`

Finding:

- The fixture requires the public effect of `FT_Outline_Reverse` to be observed
  through orientation, points, tags, contours, flags, cbox, bbox, outline
  decomposition callbacks, bitmap rendering, and invalid-control rows.
- The previous audit kept the row `pending-route` because only smaller
  individual outline reverse/orientation helpers were maintained; promoting the
  row through those partial helpers would have skipped required public outputs.
- The new route calls pinned FreeType 2.14.3, Rust FFI, C ABI, and WASM ABI on
  equivalent synthetic outlines and compares the full combined output exactly.

Impact:

- `real-parity`: `4532 -> 4533`
- `pending-route`: `429 -> 428`
- `compile-contract`: stays `2266`
- `real-null-validation`: stays `8`

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_ORIENTATION_FILL_LEFT.reverse_toggles_orientation_fixture
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype fmt
python3 -m py_compile pillow-rs-freetype/scripts/check_public_api_inputs.py
git diff --check
```

### Issue Set Current: `FT_Get_SubGlyph_Info` null-output wrapper contract

Status: classified on 2026-07-20; no route promoted.

Rejected candidate:

- `freetype.FT_Get_SubGlyph_Info.error_null_outputs`

Finding:

- The row is useful ABI evidence, but it is not same-input native C parity.
- Pinned FreeType 2.14.3 `FT_Get_SubGlyph_Info` checks the slot and subglyph
  first, then writes through every output pointer.  For a valid composite slot,
  passing a null `p_index`, `p_flags`, `p_arg1`, `p_arg2`, or `p_transform`
  would dereference null memory instead of returning a public `FT_Error`.
- The unified oracle intentionally avoids the crash by first calling pinned C
  with every output pointer non-null to prove the selected subglyph is
  native-C-callable, then records `FT_Err_Invalid_Argument` as the Rust FFI,
  C ABI, and WASM ABI null-output policy.
- Promoting this row to `real-parity` would make the audit claim exact
  same-input C/Rust/C-ABI/WASM behavior for inputs that pinned C cannot safely
  execute.

Required fix plan:

1. Keep this row explicit `pending-route` unless the fixture is split into two
   separately named behaviors: native C valid-subglyph success and wrapper
   null-output validation.
2. If a wrapper-contract row is kept, classify it separately from real parity
   and keep the reason tied to `ftobjs.c` `FT_Get_SubGlyph_Info` output writes.
3. Promote only a same-input row where pinned C, Rust FFI, C ABI, and WASM ABI
   all call `FT_Get_SubGlyph_Info` with equivalent slot, sub-index, and output
   pointer nullness and compare the same public result.

### Issue Set Current: OpenType validation absent BASE table expectation mismatch

Status: classified on 2026-07-20; no route promoted.

Rejected candidate:

- `ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output`

Finding:

- The fixture declares success for `FT_OpenType_Validate` with
  `FT_VALIDATE_BASE` on `input/fonts/DejaVuSans.ttf`, expecting the absent
  BASE table to produce an OK status with a null `BASE_table` output.
- A pinned FreeType 2.14.3 probe against this worktree returned
  `FT_Err_Unimplemented_Feature` (`7`) for the exact public call:
  `FT_OpenType_Validate(face, FT_VALIDATE_BASE, &base, &gdef, &gpos, &gsub,
  &jstf)`.
- The same probe initialized all output pointers to non-null sentinels before
  the call; pinned C left those sentinels untouched rather than writing null.
- Promoting the current fixture expectation as OK/null-output would contradict
  the pinned C oracle and would be a green placeholder.

Required fix plan:

1. Decide whether this row is meant to cover the active pinned build behavior
   or a future FreeType build with OpenType validation support enabled.
2. If the pinned build is authoritative, update or replace the row so it
   expects exact `FT_Err_Unimplemented_Feature` output and sentinel
   preservation for the same DejaVuSans input.
3. If a successful absent-table route is required, add a C-openable fixture and
   build configuration where pinned FreeType actually returns OK and writes a
   null BASE output for this public call.
4. Promote only after the pinned C oracle, Rust FFI, C ABI, and WASM ABI all
   compare the same validation flags, output pointer initialization, and table
   pointer classes.

### Issue Set Current: property-route pending rows with fixture/input mismatches

Status: classified on 2026-07-20; no route promoted.

Rejected candidate:

- `fterrdef.FT_Err_Missing_Property.known_property_success`

Finding:

- The fixture input currently says `module_name="svg"` and
  `property_name="svg-hooks"`, with expected success.
- FreeType 2.14.3 documents and implements SVG renderer hooks on module
  `ot-svg`, not `svg`; see `include/freetype/ftdriver.h` `svg-hooks` example
  and `src/svg/ftsvg.c:ft_svg_property_get`.
- A pinned-build probe against this worktree's FreeType oracle returned:
  - `FT_Property_Get(library, "svg", "svg-hooks", &hooks) -> 11`
    (`FT_Err_Missing_Module`)
  - `FT_Property_Get(library, "ot-svg", "svg-hooks", &hooks) -> 0`
- Promoting the existing row through the scalar `truetype:interpreter-version`
  helper would use a different public input and would be a green placeholder.

Required fix plan:

1. Add a maintained typed property route for `svg-hooks` that preserves the
   public input module name and hook-record shape.
2. Correct the public input row or add a replacement row that uses
   `module_name="ot-svg"` for the SVG hook success control; keep the current
   `module_name="svg"` behavior visible as an exact `Missing_Module` case if
   the manifest intends to exercise that spelling.
3. Implement the behavior in core Rust first, then expose it through thin C ABI
   and WASM ABI helpers without parsing hook semantics in the wrappers.
4. Promote only after the pinned C oracle, Rust FFI, C ABI, and WASM ABI all
   compare the same module/property input and output.

Related property rows still pending:

- `ftdriver.FT_Prop_IncreaseXHeight.property_set_get_round_trips_limit`
- `ftdriver.FT_Prop_IncreaseXHeight.limit_changes_autohint_x_height`
- `ftdriver.FT_Prop_GlyphToScriptMap.map_mutation_affects_autohint_script`

Reason:

- These are not scalar `FT_UInt` properties like
  `truetype:interpreter-version`.  They require typed `FT_Prop_*` records,
  face-specific autohinter globals, and observable glyph-load behavior owned by
  the Rust core before the C/WASM ABI wrappers can be considered thin and exact.

### Issue Set Current: post-zero-table route probes that must not be promoted as generic

Status: probed on 2026-07-20 after `real-parity=4531`; no route promoted.

Probe commands:

```bash
make -C pillow-rs-freetype test-case CASE=tttables.TT_MaxProfile.malformed_table_error_source
make -C pillow-rs-freetype test-case CASE=ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output
make -C pillow-rs-freetype test-case CASE=ftdriver.FT_AUTOHINTER_SCRIPT_LATIN.default_script_property_roundtrip
```

Findings:

- `tttables.TT_MaxProfile.malformed_table_error_source` is not an asset-only
  problem.  The declared assets `input/fonts/sfnt/truncated-maxp.ttf` and
  `input/fonts/sfnt/invalid-maxp.ttf` are missing, but the operation
  `face.load_then_get_sfnt_table.maxp` also has no maintained runtime runner.
  Required next fix: extend `scripts/build_sfnt_fixtures.py` to generate the
  declared malformed `maxp` assets, then add an explicit pinned-C/Rust/C
  ABI/WASM route that compares face-open error, nullness, and any loaded
  `TT_MaxProfile` fields.  Do not alias this to `sfnt.get_sfnt_table.maxp`,
  which only covers already-open valid faces.
- `ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output` is a fixture
  expectation mismatch in the pinned build.  The audit records pinned
  FreeType 2.14.3 returning `FT_Err_Unimplemented_Feature` (`7`) for
  `FT_VALIDATE_BASE` and leaving non-null output sentinels untouched, while
  the row declares OK/null-output behavior.  Required next fix: either change
  the fixture contract through the maintained generator/input workflow to
  assert the build's actual unsupported-service behavior, or provide a
  C-openable build/asset path where BASE validation is supported.  Do not
  promote the current row as absent-table success.
- `ftdriver.FT_AUTOHINTER_SCRIPT_LATIN.default_script_property_roundtrip` is
  not equivalent to the existing `truetype:interpreter-version` scalar
  property route.  Removing the row from the driver pending set only exposes
  the generic property-service guard:
  `FT_Property_Get/Set route requires maintained Rust FFI, C ABI, and WASM ABI property APIs`.
  Required next fix: implement `autofitter:default-script` and related
  typed autohinter property storage in the Rust core, then expose only thin
  C/WASM calls for the same public property surface.

### Issue Set Current: malformed SFNT constructor errors with stale declared expectations

Status: partially fixed on 2026-07-20; remaining stale-error fixtures stay
pending.

Fixed candidate:

- `fterrdef.FT_Err_Invalid_File_Format.new_memory_face_rejects_broken_sfnt`

Finding:

- The fixture asset `fonts/synthetic/sfnt/recognized-broken-sfnt.ttf` is a
  12-byte SFNT header with version `0x00010000` and `numTables=0`.
- FreeType 2.14.3 returns public error `85`
  (`FT_Err_Invalid_Stream_Operation`) for the exact asset.
- Rust previously returned `3` (`FT_Err_Invalid_File_Format`) after treating
  the zero-table directory as a generic missing-table font error.
- The fix adds a narrow Rust constructor error for TrueType SFNT directories
  with `numTables=0` and maps it to public error `85` through Rust FFI, C ABI,
  and WASM.
- Focused verification:
  `make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_File_Format.new_memory_face_rejects_broken_sfnt`
  now reports `runtime_parity: passed=1 failed=0 total=1`.
- FreeType 2.14.3 `src/sfnt/ttload.c:tt_face_load_font_dir`/
  `check_table_dir` detects no valid table records before public face-open
  status is surfaced.

### Issue Set Current: `FT_PARAM_TAG_UNPATENTED_HINTING` no-effect open params

Status: two-row runtime route completed on 2026-07-20 for pinned FreeType
2.14.3 `FT_Open_Face` handling of deprecated
`FT_PARAM_TAG_UNPATENTED_HINTING`.

Implemented real parity rows:

- `ftparams.FT_PARAM_TAG_UNPATENTED_HINTING.open_face_no_effect`
- `ftparams.FT_PARAM_TAG_UNPATENTED_HINTING.null_data_accepted_or_ignored`

Finding:

- Pinned FreeType exposes `FT_PARAM_TAG_UNPATENTED_HINTING` for historical
  compatibility, but current `FT_Open_Face` source does not consume the tag.
  The parameter is therefore accepted and ignored, including null `data`.
- The new oracle route opens the same font through `FT_Open_Face` with
  `FT_OPEN_PARAMS`, observes `open_error`, `face_flags`, size setup, glyph load
  error, and public glyph-slot fields, and compares that output with Rust FFI,
  thin C ABI, and WASM ABI.
- The C ABI path passes two real `FT_Parameter` records through
  `FT_Open_Face`: one with null `data`, one with non-null ignored `data`.
  Rust FFI and WASM do not expose arbitrary `FT_Open_Args`; for this deprecated
  ignored tag their exact observable output is normal face opening and glyph
  observation.

Rejected candidates:

- `FT_PARAM_TAG_IGNORE_SBIX` remains pending until real sbix fixtures and sbix
  outline/bitmap behavior are C-openable.
- `FT_PARAM_TAG_INCREMENTAL` remains pending until incremental font callbacks
  and metrics override behavior are routed.
- `FT_PARAM_TAG_STEM_DARKENING` remains pending until the CFF/Type1
  stem-darkening property path and output effects are implemented.

Impact:

- `real-parity`: `4455 -> 4457`
- `compile-contract`: stays `2265`
- `pending-route`: `500 -> 498`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=FT_PARAM_TAG_UNPATENTED_HINTING
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: FTC CMap null-cache exact-error route ordering

Status: three-concrete-row audit cleanup completed on 2026-07-20 for pinned
FreeType 2.14.3 `FTC_CMapCache_Lookup` null-cache behavior.

Implemented real parity rows:

- `ftcache.FTC_CMapCache_Lookup.error_null_cache_returns_zero` (3 concrete
  variants)

Finding:

- The unified runtime harness already executes these rows through a maintained
  cache exact-error/null-result lane and compares pinned C oracle, Rust FFI, C
  ABI, and WASM ABI output.
- The static route audit still parked the rows behind the broad FTC cache
  subsystem pending bucket because the broad pending classifier ran before the
  existing exact-error/null-route promotion.
- The route audit now checks concrete exact-error and focused real-parity
  reasons before broad subsystem pending buckets, while unresolved future assets
  and non-runnable success/lifecycle rows remain pending.

Rejected candidates:

- `FTC_Manager_Done.success_null_or_invalid_library_noop`,
  `FTC_Manager_RemoveFaceID.success_null_manager_noop`, and
  `FTC_Node_Unref.null_or_invalid_inputs_noop` remain pending because focused
  runtime filters currently select them as `runnable=0`.
- `FTC_Node_Unref.null_or_invalid_inputs_noop` is not a safe null-only
  promotion candidate.  Its fixture includes `{node: "foreign_or_bad_cache_index",
  manager: "live_empty"}` in addition to null-node/null-manager variants.  Pinned
  FreeType 2.14.3 `src/cache/ftcmanag.c:FTC_Node_Unref` reads
  `node->cache_index` whenever both `node` and `manager` are non-null, then
  compares that value with `manager->num_caches`.  A maintained route therefore
  needs an explicit FTC node/manager layout facade that can model the foreign
  bad-index input; treating the whole row as a generic void no-op would be a
  green placeholder.
- Palette non-SFNT/null-output rows and OpenType absent-table rows also remain
  pending for the same reason; promoting them would be placeholder accounting.
- `ftparams` unpatented/sbix/incremental/stem-darkening rows remain pending
  because they require `FT_Open_Args` parameter semantics, not an unrelated
  existing property or face-toggle route.

Impact:

- `real-parity`: `4452 -> 4455`
- `compile-contract`: stays `2265`
- `pending-route`: `503 -> 500`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=error_null_cache_returns_zero
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Open_Args` typographic-name null-data route

Status: two-row runtime route completed on 2026-07-20 for pinned FreeType
2.14.3 `FT_Open_Face` name-selection parameters whose `data` pointer is null.

Implemented real parity rows:

- `ftparams.FT_PARAM_TAG_IGNORE_SBIX.unsupported_or_non_sbix_no_spurious_failure`
- `ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY.open_face_uses_legacy_family_name`
- `ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY.null_data_accepted`
- `ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY.open_face_uses_legacy_subfamily_name`
- `ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY.null_data_accepted`

Finding:

- The Rust, C ABI, and WASM backends already had a maintained
  `FT_Open_Face` name-options route, but it was only selected for the older
  `FT_PARAM_TAG_IGNORE_PREFERRED_*` case IDs and for `new_memory_face`
  operation dispatch.
- These two `FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_*` constants are aliases of the
  preferred-family/subfamily tags in pinned FreeType. Pinned C checks the tag
  and does not dereference the `data` pointer for these name-selection
  parameters, so null data is accepted.
- The harness now dispatches the exact `freetype.open_face_with_params` rows to
  the same maintained name-options route for pinned C oracle, Rust FFI, C ABI,
  and WASM ABI.

Remaining related blockers:

- Other `ftparams` rows remain pending because they require real sbix,
  incremental font, random seed, or stem-darkening behavior rather than
  name-selection tag plumbing.
- Real SBIX-table behavior is still pending: existing `fonts/color/sbix-*`
  files are symlinks to DejaVu, so they cannot prove `FT_PARAM_TAG_IGNORE_SBIX`
  bitmap/outline selection. The non-SBIX no-spurious-failure row is now split
  and verified; the bitmap-only SBIX branch remains visible as a pending
  fixture/core task.
- The `open_face_uses_legacy_*` rows were normalized on 2026-07-20 to the
  maintained `scenarios[]` route and now verify exact family/style strings
  through pinned C, Rust FFI, C ABI, and WASM ABI.

Current exact pending `ftparams` fix plan:

- `ftparams.FT_PARAM_TAG_IGNORE_SBIX.open_face_ignores_sbix`: add a maintained
  real SBIX font fixture with an outline/default-strike distinction, then route
  `FT_Open_Face` with `FT_OPEN_PARAMS` through pinned C FreeType
  `sfnt/sfobjs.c` SBIX dispatch, Rust FFI, thin C ABI params, and WASM ABI.
  The verified non-SBIX no-effect row cannot prove this branch.
- `ftparams.FT_PARAM_TAG_IGNORE_SBIX.bitmap_only_requires_real_sbix_fixture`:
  add a real bitmap-only or missing-outline SBIX fixture and compare exact
  public open/load result across C/Rust/C-ABI/WASM. The current sbix-named test
  assets are not sufficient proof, so this must remain pending.
- `ftparams.FT_PARAM_TAG_INCREMENTAL.incremental_interface_used_for_glyph_load`:
  build a maintained incremental-font route that stores the client callback
  interface at `FT_Open_Face`, invokes glyph-data callbacks, releases glyph
  data, applies metrics overrides, and compares callback event logs plus public
  glyph output across all ABI lanes.
- `ftparams.FT_PARAM_TAG_INCREMENTAL.missing_or_null_interface_matches_c`:
  prove null or incomplete callback-interface behavior through the same
  `FT_Open_Face` parameter-table branch from pinned C FreeType
  `src/base/ftobjs.c`; do not count a generic open-face or exact-error route as
  parity for this parameter branch.
- `ftparams.FT_PARAM_TAG_RANDOM_SEED.valid_seed_sets_face_property`: route a
  valid seed through a driver-visible public CFF/Type1/CID output, or document
  and verify with pinned C that the seed is intentionally not observable for the
  maintained fixture set. Scalar error/null-size handling alone is not output
  parity.
- `ftparams.FT_PARAM_TAG_STEM_DARKENING.cff_type1_toggle_changes_supported_output`:
  add a C-openable CFF/Type1/CID fixture where toggling stem darkening changes
  or provably preserves public metrics, outline, or bitmap output across
  C/Rust/C-ABI/WASM. The existing null-data/scalar property route is not enough.

Latest impact for the 2026-07-20 SBIX non-SBIX no-op route:

- `real-parity`: `4527 -> 4528`
- `pending-route`: stays `428` because
  `ftparams.FT_PARAM_TAG_IGNORE_SBIX.bitmap_only_requires_real_sbix_fixture`
  preserves the unresolved bitmap-only SBIX requirement.

Latest impact for the 2026-07-20 legacy-name route normalization:

- `real-parity`: `4525 -> 4527`
- `pending-route`: `430 -> 428`

Earlier null-data impact:

- `real-parity`: `4450 -> 4452`
- `compile-contract`: stayed `2265`
- `pending-route`: `505 -> 503`
- `pending-core`: stayed `1`
- `generic-fallback`: stayed `0`

Verification:

```bash
make -C pillow-rs-freetype test-op OP=freetype.open_face_with_params
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: WinFNT output mutation route promotion

Status: one-row audit cleanup completed on 2026-07-20 for the
`FT_Get_WinFNT_Header` caller-owned output record mutation contract.

Promoted case:

- `FT_WinFNT_Header.mutable_output_handle_contract` now runs through the pinned
  C FreeType oracle, Rust FFI, thin C ABI, and WASM ABI. FreeType
  `src/base/ftwinfnt.c` copies the WinFNT header into the caller-owned record
  on success and leaves the caller's sentinel record unchanged for non-WinFNT
  and null-face error rows; all three mutation rows now compare exactly.

Impact:

- `real-parity`: `4514 -> 4515`
- `pending-route`: `442 -> 441`

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftwinfnt.FT_WinFNT_Header.mutable_output_handle_contract
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: WinFNT MAC charset charmap route promotion

Status: one-row audit cleanup completed on 2026-07-20 for the WinFNT MAC
charset runtime charmap contract.

Promoted case:

- `FT_WinFNT_ID_MAC.mac_charset_selects_apple_roman_charmap` now runs through
  the pinned C FreeType oracle, Rust FFI, thin C ABI, and WASM ABI. FreeType
  `src/winfonts/winfnt.c:858-876` creates the WinFNT face charmap with
  `FT_ENCODING_APPLE_ROMAN` and `TT_PLATFORM_MACINTOSH` when the parsed WinFNT
  header charset byte is `FT_WinFNT_ID_MAC` (`77`); the fixture now compares
  exact `FT_Get_WinFNT_Header` charset and active charmap public fields.

Impact:

- `real-parity`: `4515 -> 4516`
- `pending-route`: `441 -> 440`

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftwinfnt.FT_WinFNT_ID_MAC.mac_charset_selects_apple_roman_charmap
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: CID signature and Type1 sentinel contract cleanup

Status: two-row audit cleanup completed on 2026-07-20 for public signature and
sentinel contracts that were incorrectly parked behind runtime subsystem
pending buckets.

Additional audit corrections:

- `ftcid.FT_Get_CID_Registry_Ordering_Supplement.public_header_signature` is a
  public `ftcid.h` function signature/import contract, not CID runtime face
  data behavior.
- `t1tables.T1_BLEND_MAX.sentinel_not_runtime_field` is a public
  `t1tables.h` enum sentinel/non-field contract, not Type1 blend dictionary
  runtime extraction.

Rejected candidates:

- FTC cache opaque cache rows remain pending because their expectations require
  manager-owned cache lifecycle and post-done invalidation behavior.
- `FT_StreamRec` memory/callback stream rows remain pending because they require
  exact stream field mutation and callback event parity.

Impact:

- `real-parity`: stays `4450`
- `compile-contract`: `2263 -> 2265`
- `pending-route`: `507 -> 505`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Verification:

```bash
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: header/layout contract audit cleanup

Status: five-row audit cleanup completed on 2026-07-20 for public header,
macro, enum, and layout contracts that were incorrectly parked behind runtime
subsystem pending buckets.

Additional audit corrections:

- `ftglyph.FT_Glyph_BBox_Mode.enum_variants_match_header` is an enum/header
  value contract, not glyph object runtime behavior.
- `ftglyph.FT_Glyph_BBox_Mode.deprecated_lowercase_aliases_match` is a macro
  alias/header contract, not glyph object runtime behavior.
- `ftimage.FT_IMAGE_TAG.override_contract_matches_c` is a C macro override
  compile contract, not raster/image runtime behavior.
- `ftmm.T1_MAX_MM_AXIS.record_array_capacity` is a public
  `FT_Multi_Master` layout/capacity contract, not MM runtime descriptor
  behavior.
- `ftmm.T1_MAX_MM_MAP_POINTS.axis_map_capacity` is a public header/layout
  absence contract for `FT_MM_Axis`, not MM runtime behavior.

Rejected probe:

- The declared non-SFNT color fixture
  `fonts/bdf/properties-atoms-integers-cardinals.bdf` is still not usable for
  runtime parity. A temporary generated BDF proved pinned C returns `Ok` for
  the non-SFNT palette rows, but Rust core currently returns error `7`
  (`FT_Err_Invalid_File_Format`) when opening that BDF input. The color rows
  remain pending until BDF/non-SFNT face loading is implemented or the exact
  declared input is otherwise C-openable and Rust-openable.

Impact:

- `real-parity`: stays `4450`
- `compile-contract`: `2258 -> 2263`
- `pending-route`: `512 -> 507`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Verification:

```bash
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: GX null free and palette data without CPAL

Status: three-row runtime route completed on 2026-07-20 for pinned FreeType
2.14.3 GX/classic-kern null-face free behavior and SFNT palette-data behavior
when no CPAL table exists.

Implemented real parity rows:

- `ftgxval.FT_TrueTypeGX_Free.null_face_noop`
- `ftgxval.FT_ClassicKern_Free.null_face_noop`
- `ftcolor.FT_Palette_Data_Get.success_sfnt_without_cpal`

Finding:

- Pinned C `FT_TrueTypeGX_Free` (`src/base/ftgxval.c`) and
  `FT_ClassicKern_Free` return immediately when `face` is null; the sentinel
  validation table pointer is not freed or inspected. Rust FFI, C ABI, and WASM
  ABI now expose the same null-face no-op route.
- Pinned C `FT_Palette_Data_Get` (`src/base/ftcolor.c`) copies the face palette
  data record. For an SFNT face without a CPAL table, that record remains
  zero/null initialized, and the call succeeds. The maintained route now uses
  the existing DejaVu Sans fixture alias and compares the zero/null palette
  fields through pinned C, Rust FFI, C ABI, and WASM ABI.

Remaining related blockers:

- BDF/non-SFNT palette rows remain pending because the declared fixture
  `fonts/bdf/properties-atoms-integers-cardinals.bdf` is unresolved in this
  worktree. No substitute fixture was promoted.
- CPAL/COLR palette rows remain pending until there is a maintained color
  subsystem route.
- `ftotval.FT_OpenType_Validate` BASE-absent success behavior remains pending:
  the attempted same-input probe returned pinned C error `7`
  (`FT_Err_Invalid_File_Format`) for the referenced fixture, so parity is not
  proven for that row.

Impact:

- `real-parity`: `4447 -> 4450`
- `compile-contract`: stays `2258`
- `pending-route`: `515 -> 512`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftgxval.truetype_gx_free
make -C pillow-rs-freetype test-op OP=ftgxval.classic_kern_free
make -C pillow-rs-freetype test-op OP=ftcolor.palette_data_get
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Set_Default_Properties` environment route

Status: three-row runtime route completed on 2026-07-20 for pinned FreeType
2.14.3 default property environment parsing.

Implemented real parity rows:

- `ftmodapi.FT_Set_Default_Properties.no_environment_noop`
- `ftmodapi.FT_Set_Default_Properties.parses_supported_environment_property`
- `ftmodapi.FT_Set_Default_Properties.ignores_malformed_or_failed_properties`

Finding:

- The supported environment-property row was already classified as
  `real-parity`, but it did not have an explicit maintained operation dispatch
  in the unified harness. The route now explicitly compares all three rows
  through pinned C oracle, Rust FFI, C ABI support path, and WASM ABI support
  path.
- Pinned C `FT_Set_Default_Properties` (`src/base/ftinit.c`) reads
  `FREETYPE_PROPERTIES`, parses whitespace-separated
  `module:property=value` tokens with a 128-byte component limit, calls
  `ft_property_string_set`, and deliberately ignores all setter errors.
- For the currently supported public property, `truetype:interpreter-version`,
  pinned C parses the string value with `ft_strtol`; value `35` applies, while
  malformed tokens, missing modules/properties, and null library calls leave
  observable interpreter-version state unchanged or unobservable.

Impact:

- `real-parity`: `4442 -> 4444`
- `compile-contract`: stays `2258`
- `pending-route`: `520 -> 518`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftmodapi.set_default_properties
```

### Issue Set Current: late ABI scalar/header contract cleanup

Status: eight-row audit cleanup completed on 2026-07-20 for pinned FreeType
2.14.3 public header, opaque-handle, enum, and scalar sentinel contracts.

Additional audit corrections:

- `ftlzw.FT_Stream_OpenLZW.import_contract` is a public `ftlzw.h` symbol import
  contract, not LZW stream runtime behavior.
- `ftstroke.FT_Stroker.alias_defined` and
  `ftstroke.FT_Stroker.opaque_handle_import_contract` are opaque handle
  compile/layout contracts, not stroker path lifecycle behavior.
- `ftwinfnt.FT_WinFNT_HeaderRec.field_order_matches_header` is a public header
  field-order contract, not WinFNT runtime header extraction.
- `otsvg.FT_SVG_Document.alias_defined` is a public `otsvg.h` typedef import
  contract, not SVG renderer callback behavior.
- `t1tables.T1_Blend_Flags.enum_variants_match_header`,
  `t1tables.T1_Blend_Flags.max_tracks_variant_count`, and
  `t1tables.T1_EncodingType.enum_variants_match_header` are scalar/header value
  contracts, not Type1 runtime table extraction.

Impact:

- `real-parity`: stays `4442`
- `compile-contract`: `2250 -> 2258`
- `pending-route`: `528 -> 520`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Verification:

```bash
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Get_Module` runtime route and ABI contract cleanup

Status: twenty-two-row parity/audit batch completed on 2026-07-20 for pinned
FreeType 2.14.3 module lookup behavior and public header ABI/import contracts.

Implemented real parity rows:

- `ftmodapi.FT_Get_Module.lookup_existing_and_missing_module`
- `ftmodapi.FT_Get_Module.null_inputs_return_null`

Additional audit corrections:

- 8 `ftlist`/`ftlogging` callback-provider import rows are header ABI contracts,
  not runtime callback/provider behavior, so they now classify as
  `compile-contract`.
- 6 `ftmm` import rows are public `ftmm.h` symbol contracts, not MM descriptor
  runtime behavior, so they now classify as `compile-contract`.
- 6 `ftwinfnt`/`otsvg` header, pointer, and record-layout rows are ABI/layout
  contracts, not specialized runtime record behavior, so they now classify as
  `compile-contract`.

Finding:

- The two `FT_Get_Module` rows only require initialized-library module lookup
  and null-input behavior. They no longer depend on the broader module
  lifecycle/provider route.
- Pinned C `FT_Get_Module` returns `NULL` when either `library` or
  `module_name` is null, and returns the module whose class name matches the
  registered default module for known names. The parity route compares module
  nullness and public class name, not unstable pointer addresses.

Impact:

- `real-parity`: `4440 -> 4442`
- `compile-contract`: `2230 -> 2250`
- `pending-route`: `550 -> 528`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftmodapi.get_module
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: module flags and interpreter-version property batch

Status: ten-row parity/audit batch completed on 2026-07-20 for pinned FreeType
2.14.3 default module metadata and TrueType interpreter-version property
behavior.

Implemented real parity rows:

- `ftdriver.TT_INTERPRETER_VERSION_35.interpreter_version_property_roundtrip`
- `ftdriver.TT_INTERPRETER_VERSION_38.interpreter_version_property_normalizes_to_40`
- `ftdriver.TT_INTERPRETER_VERSION_40.interpreter_version_property_roundtrip`
- `ftmodapi.FT_MODULE_DRIVER_HAS_HINTER.present_on_native_hinter_drivers`
- `ftmodapi.FT_MODULE_DRIVER_HINTS_LIGHTLY.cff_driver_sets_light_hint_flag`
- `ftmodapi.FT_MODULE_DRIVER_NO_OUTLINES.bitmap_driver_flags_match_c`
- `ftmodapi.FT_MODULE_DRIVER_SCALABLE.scalable_driver_flags_match_c`
- `ftmodapi.FT_MODULE_FONT_DRIVER.font_driver_modules_set_bit`
- `ftmodapi.FT_MODULE_HINTER.autofitter_module_sets_hinter_bit`

Additional audit correction:

- `ftmm.FT_Done_MM_Var.import_contract` is an ABI import/signature contract, not
  MM runtime descriptor behavior, so it now classifies as `compile-contract`.

Finding:

- The module-flag rows were blocked by declared future font/module assets even
  though the maintained route compares only default module-table metadata via
  `FT_Get_Module(...)->clazz->module_flags`.
- The interpreter-version property rows were blocked for the same reason; their
  maintained public route only needs `FT_Property_Set`/`FT_Property_Get` with
  the `truetype` module and scalar `FT_UInt` values.
- Running all six module-flag rows exposed a real Rust divergence: Rust
  registered the CID module in the default library, but the pinned FreeType
  build returns no `cid` module from `FT_Get_Module`. Core now leaves CID out
  of the default module list while preserving `type42`, which pinned C does
  register.

C behavior verified:

- Pinned C returns exact module presence and `module_flags` for `truetype`,
  `type1`, `type42`, `cff`, bitmap drivers, `autofitter`, renderers, and
  non-driver helpers from the initialized default library.
- Pinned C accepts and reads back the three public
  `TT_INTERPRETER_VERSION_*` scalar values through the TrueType property
  service. The Rust FFI, C ABI, and WASM ABI now run the same exact property
  cases instead of relying on the broader generic property-set row.

Impact:

- `real-parity`: `4431 -> 4440`
- `compile-contract`: `2229 -> 2230`
- `pending-route`: `560 -> 550`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Remaining related blockers:

- `ftdriver.interpreter_version_glyph_output` remains pending until the
  bytecode-sensitive runtime glyph fixtures are C-openable.
- Face-scoped autohinter property records such as
  `FT_Prop_GlyphToScriptMap` and `FT_Prop_IncreaseXHeight` remain pending
  because they require typed face/global state, not scalar interpreter-version
  property plumbing.

### Issue Set Current: TrueType interpreter-version glyph-output fixture blocker

Status: still pending.  The scalar `truetype:interpreter-version` property
routes are real parity, but the runtime glyph-output rows cannot be promoted
from the current inputs.

Case IDs:

- `ftdriver.TT_INTERPRETER_VERSION_35.glyph_hinting_runtime_effect`
- `ftdriver.TT_INTERPRETER_VERSION_38.glyph_hinting_runtime_effect`
- `ftdriver.TT_INTERPRETER_VERSION_40.glyph_hinting_runtime_effect`

Current blocker:

- Each row references `fonts/truetype/backward-compat-phantom-points.ttf` as
  the `control_font`, and that maintained fixture is not present.
- The existing `fonts/truetype/bytecode-interpreter-version.ttf` scalar
  property route is not a substitute.  It proves `FT_Property_Set/Get` state,
  including FreeType's `38 -> 40` normalization in the pinned build, but it
  does not prove that interpreter-version selection changes public glyph
  loading, metrics, outlines, or rendered bytes.

Required real-parity route:

1. Add or generate a reproducible, C-openable TrueType control fixture whose
   bytecode exercises `GETINFO`, backward-compatible phantom-point behavior,
   component offsets, and a glyph with interpreter-version-sensitive public
   output.
2. For versions `35`, `38`, and `40`, set the pinned C FreeType TrueType
   `interpreter-version` property, then run `FT_Load_Glyph` and
   `FT_Render_Glyph` for the fixture's named probe glyphs at the declared ppem
   sizes and load flags.
3. Compare the same input through pinned C, Rust FFI, thin C ABI, and WASM ABI
   for exact `property_error`, per-glyph load error, 26.6 metrics, outline
   point/tag/contour hash, bitmap mode/placement/stride, and bitmap byte hash.
4. Promote the rows only after the focused cases pass with those exact
   comparisons.  A constant-value assertion or scalar property roundtrip is a
   green placeholder for these rows.

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftdriver.interpreter_version_property
make -C pillow-rs-freetype test-op OP=ftmodapi.inspect_module_flags
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Glyph_To_Bitmap` malformed invalid-argument route

Status: one malformed glyph facade row promoted to real parity on 2026-07-20
for the pinned FreeType 2.14.3 early invalid-argument behavior in
`src/base/ftglyph.c`.

Implemented real parity row:

- `ftglyph.FT_Glyph_To_Bitmap.error_invalid_arguments_or_unrenderable_format`

Finding:

- The Rust, C ABI, WASM ABI, and pinned C oracle runners already had exact
  invalid-input probes for:
  - null `FT_Glyph*` argument;
  - non-null `FT_Glyph*` pointing to a null glyph;
  - glyph record with null `library`;
  - glyph record with null `clazz`;
  - glyph class without a prepare hook.
- The row still stayed `pending-route` because it referenced the missing
  shared malformed glyph facade
  `facades/glyph/malformed-slot-and-class-cases.json`.
- The new facade marks this exact invalid-input row as routed and explicitly
  marks the other malformed glyph rows as `pending-route`. The harness validates
  that the routed row is present in the facade before running the Rust, C ABI,
  and WASM invalid-input comparisons.

C behavior verified:

- `FT_Glyph_To_Bitmap` returns exact public `FT_Error` values for each early
  invalid-argument path and preserves the caller glyph-handle class according
  to the pinned C behavior before bitmap allocation.

Impact:

- `real-parity`: `4430 -> 4431`
- `pending-route`: `561 -> 560`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Remaining malformed glyph blockers:

- `ftglyph.FT_Get_Glyph.error_unsupported_format_or_bad_slot_payload`
- `ftglyph.FT_Get_Glyph.error_advance_out_of_16_16_range`
- `ftglyph.FT_GlyphRec.clazz_is_private_identity_only`
- `ftglyph.FT_Glyph_To_Bitmap.error_render_failure_preserves_original`

These remain pending until maintained synthetic slot, class-hook, cleanup, and
renderer-failure routes exist across Rust, C ABI, WASM, and pinned C.

Per-case blocker detail:

- `ftglyph.FT_Get_Glyph.error_unsupported_format_or_bad_slot_payload` requires
  a synthetic `FT_GlyphSlot` route matching pinned FreeType 2.14.3
  `freetype/src/base/ftglyph.c:633-682`. The route must build C-observable
  `slot->library`, `slot->format`, and payload fields, call `FT_New_Glyph` and
  the selected glyph-class `glyph_init` hook, then compare the public
  `FT_Error` and `*aglyph` null/preservation behavior through pinned C, Rust
  FFI, thin C ABI, and WASM.
- `ftglyph.FT_Get_Glyph.error_advance_out_of_16_16_range` requires synthetic
  slot advances at the exact C overflow boundaries from
  `freetype/src/base/ftglyph.c:651-667`: `slot->advance.x/y >= 0x8000 * 64`
  and `<= -0x8000 * 64`. Exact parity must prove allocation cleanup and
  `*aglyph = NULL`; null-slot coverage does not exercise this path.
- `ftglyph.FT_GlyphRec.clazz_is_private_identity_only` must not compare raw
  private pointers. `freetype/include/freetype/ftglyph.h:93-120` exposes
  `clazz` as a private glyph-class pointer; the maintained route must create
  outline, bitmap, and SVG glyphs through public operations and classify the
  private class only by public behavior and glyph format across all ABI lanes.
- `ftglyph.FT_Glyph_To_Bitmap.error_render_failure_preserves_original` requires
  a real `glyph_prepare` or renderer-failure route matching
  `freetype/src/base/ftglyph.c:771-874`: dummy slot construction, bitmap-glyph
  allocation, optional origin apply/restore, partial bitmap free on render
  error, and original handle preservation even when `destroy` is true. The
  already-routed early invalid-argument checks do not prove this path.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=FT_Glyph_To_Bitmap
make -C pillow-rs-freetype test-case CASE=FT_Get_Glyph
make -C pillow-rs-freetype test-case CASE=FT_GlyphRec
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_List_Insert/Remove/Up` topology facade route

Status: implemented as real parity on 2026-07-20 for the pinned FreeType
2.14.3 list-topology behavior in `src/base/ftutil.c:282-365`.

Implemented real parity rows:

- `ftlist.FT_List_Insert.insert_empty_list`
- `ftlist.FT_List_Insert.insert_non_empty_list`
- `ftlist.FT_List_Insert.null_list_or_node_noop`
- `ftlist.FT_List_Remove.remove_head_middle_tail`
- `ftlist.FT_List_Remove.remove_only_node`
- `ftlist.FT_List_Remove.null_list_or_node_noop`
- `ftlist.FT_List_Remove.membership_not_checked`
- `ftlist.FT_List_Up.move_tail_or_middle_to_head`
- `ftlist.FT_List_Up.already_head_noop`
- `ftlist.FT_List_Up.null_list_or_node_noop`

Finding:

- The Rust, C ABI, WASM ABI, and pinned C oracle runners already implemented
  exact list topology comparison for these rows, but the declared facade input
  `facades/list/ft-list-topologies.json` was missing.
- Counting those rows without a maintained facade would have hidden an
  unresolved input dependency. The fixture now describes the shared topology
  vocabulary and source references, and the harness validates that every
  runnable list-topology case is explicitly present in that facade before
  executing Rust, C ABI, or WASM output comparison.

C behavior verified:

- `FT_List_Insert` prepends the node, updates the old head's `prev`, preserves
  the existing tail, and treats null list or node arguments as no-op.
- `FT_List_Remove` patches links using the node's own `prev`/`next`, clears
  list head/tail around the removed node, preserves node data, and does not
  perform membership validation.
- `FT_List_Up` moves a non-head node to list head, updates tail only when the
  old tail moves, preserves the relative order of the other nodes, and treats
  null list or node arguments as no-op.

Impact:

- `real-parity`: `4420 -> 4430`
- `pending-route`: `571 -> 561`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Remaining list-route blocker:

- The three `import_contract` rows for `FT_List_Insert`, `FT_List_Remove`, and
  `FT_List_Up` remain `pending-route`; they are part of the callback/provider
  and public import-contract route surface, not the topology mutation surface.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=FT_List_Insert
make -C pillow-rs-freetype test-case CASE=FT_List_Remove
make -C pillow-rs-freetype test-case CASE=FT_List_Up
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Face_Properties` scalar face-property route

Status: scalar `FT_Face_Properties` route implemented on 2026-07-20 for the
C-callable pinned FreeType 2.14.3 behavior in `src/base/ftobjs.c:4010-4069`.

Implemented real parity rows:

- `freetype.FT_Face_Properties.success_supported_face_properties`
- `freetype.FT_Face_Properties.success_zero_properties_noop`
- `freetype.FT_Face_Properties.error_invalid_property_tag_or_value`
- `ftparams.FT_PARAM_TAG_LCD_FILTER_WEIGHTS.malformed_data_does_not_read_as_weights`
- `ftparams.FT_PARAM_TAG_RANDOM_SEED.null_or_wrong_size_errors`
- `ftparams.FT_PARAM_TAG_STEM_DARKENING.unsupported_or_null_data_matches_c_error`

C behavior verified:

- `FT_PARAM_TAG_STEM_DARKENING` writes
  `face->internal->no_stem_darkening` as `0` for input true, `1` for input
  false, and `-1` for null data.
- `FT_PARAM_TAG_RANDOM_SEED` writes `face->internal->random_seed`, clamps
  negative values to `0`, and resets to `-1` for null data.
- `FT_PARAM_TAG_LCD_FILTER_WEIGHTS` returns
  `FT_Err_Unimplemented_Feature` before dereferencing `data`.
- `FT_PARAM_TAG_STEM_DARKENING` with null data resets
  `face->internal->no_stem_darkening` to `-1` and returns `FT_Err_Ok`.
- Unknown tags return `FT_Err_Invalid_Argument`.

Important blocker retained:

- `freetype.FT_Face_Properties.error_null_face` is intentionally not counted
  as parity. Pinned C dereferences `face` when `num_properties > 0` and
  segfaults for a null face. Counting Rust `Invalid_Face_Handle` as parity
  would be a green placeholder, so the row is `pending-route` with that reason.
- `ftparams.FT_PARAM_TAG_STEM_DARKENING.cff_type1_toggle_changes_supported_output`
  remains pending until there is a C-openable CFF/Type1/CID fixture where stem
  darkening changes a public metric, outline, or bitmap result.

Latest impact for the 2026-07-20 stem-darkening null-data route:

- `real-parity`: `4528 -> 4529`
- `pending-route`: `428 -> 427`

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=FT_Face_Properties
make -C pillow-rs-freetype test-case CASE=FT_PARAM_TAG_RANDOM_SEED
make -C pillow-rs-freetype test-case CASE=FT_PARAM_TAG_LCD_FILTER_WEIGHTS
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Property_Get/Set` scalar TrueType property slice

Status: scalar `truetype:interpreter-version` route implemented on
2026-07-20; autohinter and face-global driver properties remain pending-route.

Current route-audit ledger after the scalar property implementation:

- `real-parity=4415`
- `pending-core=1`
- `pending-route=570`
- `generic-fallback=0`

Finding:

- Focused runtime parity for
  `ftmodapi.FT_Property_Get.gets_supported_property` previously passed because
  both the pinned oracle route and all three backends returned the generic
  fallback `FT_Err_Unimplemented_Feature` (`7`).
- The same false-green pattern was observed for
  `ftmodapi.FT_Property_Set.sets_supported_property`.
- `ftmodapi.FT_Property_Get.rejects_null_arguments` and
  `ftmodapi.FT_Property_Get.invalid_property_name` previously passed through a
  generic null/error fallback (`FT_Err_Invalid_Library_Handle`, `35`) rather
  than a maintained `FT_Property_Get` public route.
- `fterrdef.FT_Err_Missing_Property.driver_property_unknown_name` previously
  passed as generic `Unimplemented_Feature` (`7`), not the intended
  `FT_Err_Missing_Property` (`12`) public behavior.
- The scalar TrueType property route now has maintained public behavior in
  core Rust, C ABI, WASM ABI, and the pinned C oracle for:
  - all 8 `ftmodapi.FT_Property_Get/Set` manifest rows;
  - `fterrdef.FT_Err_Missing_Property.driver_property_unknown_name`;
  - `ftdriver.TT_INTERPRETER_VERSION_40.default_interpreter_version`.
- The implementation matches FreeType 2.14.3
  `src/base/ftobjs.c:ft_property_do` dispatch and
  `src/truetype/ttdriver.c:tt_property_get/set` for
  `interpreter-version`: default `40`, set `35`, normalize `38` to `40`,
  accept `40`, reject invalid values with `FT_Err_Unimplemented_Feature`, and
  preserve the previous value on rejection.

Impact:

- 10 scalar property-service rows moved from explicit `pending-route` to
  `real-parity`.
- `real-parity`: `4405 -> 4415`
- `pending-route`: `580 -> 570`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

Remaining property implementation plan:

1. Keep autohinter `glyph-to-script-map` pending until the Rust core owns the
   face-global glyph-style map and exposes it through thin ABI records.
2. Keep autohinter `increase-x-height` pending until face-global x-height state
   affects actual auto-hinted glyph output.
3. Keep CFF/Type1 hinting-engine properties pending until the corresponding
   driver property storage and glyph behavior are implemented in core.

Verification before promotion:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Get.gets_supported_property
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Property_Set.sets_supported_property
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Missing_Property.driver_property_unknown_name
make -C pillow-rs-freetype test-case CASE=ftdriver.TT_INTERPRETER_VERSION_40.default_interpreter_version
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-ffi-compat
make -C pillow-rs-freetype test
```

### Issue Set Current: future-batch unresolved-asset correction

Status: classified as explicit pending-route on 2026-07-20.

Current route-audit ledger after the strict unresolved-asset correction:

- `real-parity=4417`
- `pending-core=1`
- `pending-route=568`
- `generic-fallback=0`
- full runtime parity `6601/6601`, `pending=633`

Finding:

- Focused runtime parity for
  `ftmm.FT_Set_Var_Design_Coordinates.success_set_design_coordinates` reports
  `runnable=0`, `pending=1` because the row references unresolved fixture
  assets such as `fonts/variable/inter-wght.ttf`.
- Before this correction, the static route audit promoted future-batch success
  rows for FTMM, FTDRIVER, and FTMODAPI when they declared an asset key, even
  if that asset was not present under `tests/fixtures`.
- That made 54 rows look like `real-parity` even though no C-openable fixture
  existed for exact pinned C oracle, Rust FFI, C ABI, and WASM ABI output
  comparison. These rows are now explicit `pending-route` with the missing
  asset named in the reason.
- Exact error rows that do not require the missing success asset remain real
  when focused runtime parity proves the public error route. For example,
  `ftmm.FT_Set_Var_Design_Coordinates.error_null_coords_with_nonzero_count`
  remains `real-parity`.
- Follow-up on 2026-07-20: the single-axis
  `ftmm.FT_Set_Var_Design_Coordinates.success_set_design_coordinates` row now
  has a maintained C oracle route and validates through Rust FFI, C ABI, and
  WASM ABI.  The promoted route sets design coordinates, then reads back
  `FT_Get_Var_Design_Coordinates`, `FT_Get_Var_Blend_Coordinates`, and
  `face_flags` from the same face.  The first divergence was face flags after
  the setter: C reported `FT_FACE_FLAG_VARIATION` after explicit coordinate
  selection, while Rust only reported the default variable-font
  `FT_FACE_FLAG_MULTIPLE_MASTERS` state.  Rust now tracks explicit variation
  coordinate selection separately so default getter rows keep C's open-time
  flags and set-coordinate rows expose the post-set variation flag.
- The remaining `FT_Set_Var_Design_Coordinates` success rows stay pending:
  `success_partial_extra_and_reset` still needs the three-axis fixture, and
  the metrics/glyph-output rows require exact public output comparison beyond
  coordinate state.

Impact:

- `real-parity`: `4471 -> 4417`
- `pending-route`: `514 -> 568`
- `pending-core`: stays `1`
- `generic-fallback`: stays `0`

The remaining `pending-core` row is not green-placeholder cleanup. It is a
larger Adobe multiple-master surface:

| Case | Current blocker | Required first implementable slice |
|---|---|---|
| `ftmm.FT_Set_Named_Instance.success_adobe_mm_resets_default` | No maintained Adobe MM fixture is present under `tests/fixtures`; the row also requires real Adobe multiple-master state, not only OpenType `fvar` named instances. | Add or generate a maintained Adobe MM Type 1 fixture, parse the MM design space in pure Rust, implement default design reset semantics for `FT_Set_Named_Instance(0)`, then compare face flags, face index, and design coordinates through Rust FFI, C ABI, and WASM ABI. |

Execution order:

1. Do not promote future-batch success rows unless every declared runtime asset
   resolves and the focused row executes through pinned C FreeType, Rust FFI,
   C ABI, and WASM ABI.
2. Treat Adobe MM as a separate Type 1 multiple-master parser/fixture slice.
   Do not fake it with an OpenType variable font.

2026-07-20 progress:

- Core now parses `fvar` axis min/default/max values and derives active
  normalized coordinates for named-instance faces.
- Core now parses `gvar` tuple-variation data, including shared tuples,
  embedded tuples, intermediate regions, shared/private point lists, and packed
  X/Y deltas, then applies point deltas to simple glyph outlines before hinting.
- This diagnostic was resolved in later commits. The named-instance output row
  is now real parity through Rust FFI, C ABI, and WASM ABI; it must not be
  re-counted as pending work.

2026-07-20 historical named-instance diagnostic:

- Added a maintained C oracle/harness route for
  `ftmm.FT_Set_Named_Instance.output_changes_to_named_instance`. At this point
  in the history the row was still pending, so an explicit focused run reported
  the fractional-gvar blocker instead of a false green.
- Unguarded focused comparison first failed at `FT_GlyphSlot.advance.x`
  (`expected=1152`, Rust `actual=1216`). Applying `gvar` phantom deltas to the
  core scaler and autohint advance path fixed that mismatch.
- The next unguarded failure is `bitmap.buffer_hex` only. Metrics, advance,
  placement, and route serialization match; the remaining blocker is fractional
  point precision. For glyph `a.sc` in `named-instances.ttf`, `fontTools`
  instantiation shows third-font-unit point deltas while the current
  `GlyphOutline` stores integer font units.

2026-07-20 HVAR/fractional diagnostic update:

- Added shared item-variation-store evaluation and HVAR advance-width delta
  support following `ttgxvar.c` (`tt_var_load_item_variation_store`,
  `tt_var_load_delta_set_index_mapping`, `tt_var_get_item_delta`, and
  `tt_hvadvance_adjust`).
- The scaler now prefers HVAR advance-width deltas over gvar phantom advance
  deltas when HVAR is present, matching FreeType's double-adjustment avoidance.
- FreeType's autofit loader reloads glyphs with `FT_LOAD_NO_SCALE` before
  `af_glyph_hints_reload`; the native TrueType unrounded sidecar must not be
  applied directly to that autohint reload path.
- At this historical point the unguarded focused comparison still failed only
  at `bitmap.buffer_hex`; HVAR was no longer part of the observed pending
  reason for this row. Later commits resolved and promoted the named-instance
  output row.

Verification required before the remaining Adobe MM row moves to `real-parity`:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_Named_Instance.success_adobe_mm_resets_default
make -C pillow-rs-freetype test-ffi-compat
make -C pillow-rs-freetype test-harness
```

### Issue Set Current: `FT_Var_Named_Style` selected instance descriptor route

Status: implemented as real parity on 2026-07-20.

Finding:

- `ftmm.FT_Var_Named_Style.selected_instance_matches_descriptor` was blocked
  in `pending-core` because the harness had no maintained way to compare the
  selected named instance against the `FT_MM_Var.namedstyle` coordinate array.
- Pinned C FreeType obtains namedstyle coordinates from `FT_Get_MM_Var`, then
  `FT_Set_Named_Instance` updates the public face index and
  `FT_Get_Var_Design_Coordinates` reports the selected design coordinates. For
  the fixture row, the selected design coordinates equal the zero-based
  namedstyle descriptor for the one-based public instance index.

Implementation:

- Added a feature-gated Rust FFI helper that exposes only fvar namedstyle
  coordinates for parity testing. It does not claim full `FT_MM_Var` ownership,
  axis flags, Adobe MM, or gvar/HVAR/MVAR support.
- Added thin C ABI and WASM ABI test-support wrappers over the same core helper.
- Added a pinned C oracle route that calls real `FT_Get_MM_Var`,
  `FT_Set_Named_Instance`, and `FT_Get_Var_Design_Coordinates`, then compares
  `namedstyle_coords`, `selected_design_coords`, and `face_index` across Rust
  FFI, C ABI, and WASM ABI.

Impact:

- `ftmm.FT_Var_Named_Style.selected_instance_matches_descriptor` moved from
  `pending-core` to `real-parity`.
- Route audit count target after this batch: `real-parity=4469`,
  `pending-core=3`, `pending-route=514`, `generic-fallback=0`.

Verification:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Var_Named_Style.selected_instance_matches_descriptor
```

### Issue Set Current: `FT_GlyphSlot_Own_Bitmap` allocation-failure route

Status: implemented as real parity on 2026-07-20.

Finding:

- `ftbitmap.FT_GlyphSlot_Own_Bitmap.error_copy_allocation_failure` was blocked
  in `pending-core` because the row required deterministic allocator fault
  injection at the bitmap deep-copy allocation.
- Pinned C FreeType `src/base/ftbitmap.c:1084-1102` calls
  `FT_Bitmap_Copy` only for bitmap slots whose internal flags do not contain
  `FT_GLYPH_OWN_BITMAP`. If that copy allocation fails, C returns
  `FT_Err_Out_Of_Memory` without replacing the slot bitmap or setting the
  ownership flag. Already-owned, outline-format, and null-slot variants remain
  no-op success paths.

Implementation:

- Added a pinned C oracle custom `FT_MemoryRec` fail-after allocator and enabled
  it only after face creation, sizing, and glyph loading, so the forced failure
  occurs at `FT_GlyphSlot_Own_Bitmap`'s bitmap-copy allocation.
- Added feature-gated Rust FFI, thin C ABI, and WASM ABI test-support helpers
  that simulate the same failed copy allocation while preserving slot state.
- Routed all fixture-declared variants (`bitmap_borrowed`, `bitmap_owned`,
  `outline_format`, `null_slot`) through the unified parity runner.

Impact:

- `ftbitmap.FT_GlyphSlot_Own_Bitmap.error_copy_allocation_failure` moved from
  `pending-core` to `real-parity`.
- Route audit count target after this batch: `real-parity=4468`,
  `pending-core=4`, `pending-route=514`, `generic-fallback=0`.

Verification:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftbitmap.FT_GlyphSlot_Own_Bitmap
```

### Issue Set Current: `FT_Glyph_Copy` copy-hook failure cleanup route

Status: still pending. The current maintained `FT_Glyph_Copy` route covers
null source, null target, and null class error rows only.

Finding:

- `ftglyph.FT_Glyph_Copy.error_copy_hook_failure_cleans_target` declares two
  future facades: `facades/memory/allocation-failure.json` and
  `facades/glyph/malformed-slot-and-class-cases.json`.
- The row is not an asset-only problem. It needs a maintained glyph-copy
  failure route that forces allocator or class copy-hook failure after partial
  target allocation, then observes whether FreeType destroys the partial target
  and leaves the caller-visible target pointer/class state exactly as pinned C
  does.
- The existing null-input route proves only early argument validation for
  `FT_Glyph_Copy`; it does not exercise `src/base/ftglyph.c` cleanup after a
  copy hook returns an error.

Required fix plan:

1. Add a reproducible allocation-failure facade and malformed glyph-class
   facade under the maintained fixture workflow.
2. Add a same-input `ftglyph.glyph_copy` route that drives the three declared
   failure modes: allocator failure, bitmap copy failure, and SVG zero-length
   source behavior.
3. Compare exact `FT_Error`, target pointer class/nullness, partial-copy
   destruction, and cleanup-event order through pinned C, Rust FFI, thin C ABI,
   and WASM ABI.
4. Promote only after the focused case is runnable and proves those exact
   cleanup semantics; do not fold it into the existing null source/target/class
   real-parity route.

Verification while pending:

```bash
make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_Copy.error_copy_hook_failure_cleans_target
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Render_Glyph` unloaded and unsupported slot-state route

Status: implemented as real parity on 2026-07-20.

Finding:

- `freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format.unrouted_slot_states`
  was blocked in `pending-core` because the harness had no public runner for
  rendering a just-opened unloaded glyph slot or a synthetic unknown public slot
  format.
- Pinned C FreeType returns `FT_Err_Cannot_Render_Glyph` (`19`) for both
  variants and leaves the slot fields unchanged. The synthetic unknown-format
  probe preserves `format=0x12345678`, `glyph_index=77`, and
  `advance=(11,22)`.

Implementation:

- Added a public Rust FFI `FT_Render_Glyph` guard for non-outline/non-bitmap
  slot formats so unloaded, composite, and unknown public formats return
  `Cannot_Render_Glyph` before renderer mutation.
- Added feature-gated ABI test-support helpers to install the synthetic
  unsupported slot in thin C ABI and WASM ABI handles without exporting new
  public C symbols.
- Added the maintained `--render-glyph-slot-states` pinned C oracle route and
  unified fixture runners for Rust FFI, C ABI, and WASM ABI.

Impact:

- `freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format.unrouted_slot_states`
  moved from `pending-core` to `real-parity`.
- Route audit count target after this batch: `real-parity=4467`,
  `pending-core=5`, `pending-route=514`, `generic-fallback=0`.

Verification:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format
```

### Issue Set Current: `FT_GLYPH_FORMAT_NONE` empty-slot route

Status: implemented as real parity on 2026-07-20.

Finding:

- `ftimage.FT_GLYPH_FORMAT_NONE.reset_slot_uses_none` was blocked in
  `pending-core` because new-face and failed-load glyph-slot state was not
  exposed consistently through Rust FFI, thin C ABI, and WASM ABI.
- Pinned C FreeType creates a live `face->glyph` slot when a face opens. Before
  any successful glyph load, that public slot uses `FT_GLYPH_FORMAT_NONE` and
  zero public metrics/advance/bitmap fields. A failed invalid glyph load does
  not replace that slot.

Implementation:

- Added a pure-Rust empty glyph-slot constructor with `GlyphFormat::None` and
  zero public fields.
- Initialized C ABI and WASM face handles with that empty slot instead of a
  missing slot.
- Added a maintained `freetype.slot_format_probe` route that compares
  `new_face_before_load` and `failed_load_invalid_glyph_index` rows through
  pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

Impact:

- `ftimage.FT_GLYPH_FORMAT_NONE.reset_slot_uses_none` moved from
  `pending-core` to `real-parity`.
- Route audit count target after this batch: `real-parity=4466`,
  `pending-core=6`, `pending-route=514`, `generic-fallback=0`.

Verification:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftimage.FT_GLYPH_FORMAT_NONE
```

### Issue Set Current: residual public-surface route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `da78c660c`: `real-parity=4465`,
  `generic-fallback=11`, `pending-route=503`, `pending-core=7`.

Finding:

- The remaining generic rows cover `FT_Get_CID_Registry_Ordering_Supplement`,
  `FT_Err_Missing_Property`, `FT_Err_Ok`, `FT_OpenType_Free`,
  `FT_VALIDATE_BASE`, `FT_ORIENTATION_FILL_LEFT`, `FT_Get_PFR_Kerning`,
  `FT_Get_PFR_Metrics`, and malformed `TT_MaxProfile` table behavior.
- There are 11 concrete rows and 10 unique case IDs because
  `ftpfr.FT_Get_PFR_Metrics.pfr_metrics_success` appears in two concrete audit
  rows.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There are no maintained
  residual public-surface routes that drive matching CID, property/status,
  OpenType validation/free, outline orientation, PFR metric/kerning, and
  malformed table inputs across all ABI lanes.

Classification change:

- 1 `ftcid`, 3 `fterrdef`, 2 `ftotval`, 1 `ftoutln`, 3 `ftpfr`, and
  1 `tttables` rows moved from `generic-fallback` to `pending-route`.
- New route audit counts: `real-parity=4465`, `generic-fallback=0`,
  `pending-route=514`, `pending-core=7`.

Required fix plan:

1. Add maintained residual public-surface routes instead of per-row expected
   output shortcuts. Each route must run the same input through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust CID registry/ordering/supplement behavior first,
   including public header signature expectations and non-CID fallback paths.
3. Implement pure-Rust property/status behavior first: known property success,
   `FT_Err_Ok` lifecycle behavior, and success status without masking output
   mismatches.
4. Implement pure-Rust OpenType validation/free behavior first: absent table
   null output, validated table ownership, and face-memory lifetime semantics.
5. Implement pure-Rust outline orientation and PFR behavior first: reverse
   orientation toggling, PFR metrics, and non-PFR kerning fallback semantics.
6. Implement pure-Rust malformed TrueType table behavior first: `maxp` parse
   errors must preserve the same public error source as C FreeType.
7. Compare exact return codes, output records, nullness, ownership/free events,
   orientation state, metric values, kerning values, and error classifications
   for the same input.
8. Promote rows only after focused `ftcid`, `fterrdef`, `ftotval`, `ftoutln`,
   `ftpfr`, and `tttables` runtime proves exact C oracle, Rust FFI, C ABI, and
   WASM ABI output.

Follow-up promoted rows:

- `fterrdef.FT_Err_Ok.successful_face_lifecycle`
- `fterrdef.FT_Err_Ok.successful_constant_status_does_not_mask_output`

These rows now use the maintained `FT_Load_Glyph` + `FT_Render_Glyph` slot
route instead of status-only output.  The route compares the same DejaVu input
through pinned FreeType 2.14.3, Rust FFI, thin C ABI, and WASM ABI, including
full glyph slot metrics and bitmap bytes.  Other residual status/property rows
remain pending until they have similarly maintained public-output routes.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftcid
make -C pillow-rs-freetype test-case CASE=fterrdef
make -C pillow-rs-freetype test-case CASE=ftotval
make -C pillow-rs-freetype test-case CASE=ftoutln
make -C pillow-rs-freetype test-case CASE=ftpfr
make -C pillow-rs-freetype test-case CASE=tttables
```

### Issue Set Current: callback/provider route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `22a97f383`: `real-parity=4465`,
  `generic-fallback=21`, `pending-route=493`, `pending-core=7`.

Finding:

- The remaining callback/provider rows cover `FT_List_Insert`,
  `FT_List_Iterate`, `FT_List_Remove`, `FT_List_Up`,
  `FT_Set_Default_Log_Handler`, `FT_Set_Log_Handler`,
  `FT_Trace_Set_Default_Level`, `FT_Trace_Set_Level`,
  `FT_Renderer_Class`, and `FT_Set_Renderer`.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There are no maintained
  callback/provider routes that build equivalent lists, install logging and
  trace handlers, select renderers, drive callbacks, and compare callback event
  order, provider selection, mutation side effects, and public return codes
  across all ABI lanes.

Classification change:

- 4 `ftlist`, 4 `ftlogging`, and 2 `ftrender` rows moved from
  `generic-fallback` to `pending-route`.
- New route audit counts: `real-parity=4465`, `generic-fallback=11`,
  `pending-route=503`, `pending-core=7`.

Required fix plan:

1. Add maintained list, logging, trace, and renderer-provider routes instead of
   per-row expected output shortcuts. They must run equivalent operation
   sequences through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust list behavior first: insert/remove/up ordering, iterator
   callback invocation, node ownership, mutation side effects, and empty-list
   boundary behavior.
3. Implement pure-Rust logging and trace behavior first: handler installation,
   default handler restoration, component-level selection, trace-level state,
   callback payload shape, and lifecycle behavior.
4. Implement pure-Rust renderer-provider behavior first: renderer class
   selection, render-mode acceptance, callback dispatch, and observable
   `FT_Set_Renderer` side effects without moving logic into C or WASM wrappers.
5. Compare exact return codes, list order, callback event sequences,
   provider/renderer identity, trace-level state, and ABI-visible records for
   the same input.
6. Promote rows only after focused `ftlist`, `ftlogging`, and `ftrender`
   runtime proves exact C oracle, Rust FFI, C ABI, and WASM ABI output.

Renderer-specific blocker detail:

- `ftrender.FT_Renderer_Class.render_mode_acceptance_matches_callbacks` is not
  proven by `FT_Renderer_Class` layout or `FT_Get_Renderer` class metadata. A
  maintained renderer-behavior route must select each public renderer class and
  compare `FT_Render_Glyph` return codes plus bitmap descriptor/byte output for
  `NORMAL`, `LIGHT`, `MONO`, `LCD`, `LCD_V`, SDF, and SVG modes through pinned
  C, Rust FFI, thin C ABI, and WASM.
- `ftrender.FT_Set_Renderer.set_outline_renderer_success` is not proven by the
  current invalid-library/invalid-renderer or set-mode-error rows. Pinned
  FreeType 2.14.3 `freetype/src/base/ftobjs.c:4676-4732` validates that the
  renderer belongs to `library->renderers`, moves that list node with
  `FT_List_Up`, updates `cur_renderer` only for outline renderers, and calls
  `clazz->set_mode` for each supplied parameter until the first error. Exact
  parity requires a route that compares those state transitions and the
  subsequent rendered output across pinned C, Rust FFI, C ABI, and WASM.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftlist
make -C pillow-rs-freetype test-case CASE=ftlogging
make -C pillow-rs-freetype test-case CASE=ftrender
```

### Issue Set Current: compressed/external stream route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `d36af6e5f`: `real-parity=4465`,
  `generic-fallback=33`, `pending-route=481`, `pending-core=7`.

Finding:

- The remaining compressed/external stream rows cover `FT_Stream_OpenBzip2`,
  `FT_Gzip_Uncompress`, `FT_Stream_OpenGzip`, `FT_Stream_OpenLZW`,
  `FT_Memory`, `FT_Stream`, and `FT_StreamRec`.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There are no maintained
  stream subsystem routes that open matching compressed buffers and callback
  streams, observe decompressed bytes, validate close ownership, inspect public
  stream record fields, and compare custom allocator/callback events across all
  ABI lanes.
- The BZIP2 success/lifecycle rows are blocked by concrete missing declared
  byte fixtures as well as missing stream behavior.  The public input manifests
  reference `streams/bzip2/valid-pcf-header.pcf.bz2` for open/read/close
  lifecycle rows and `streams/bzip2/valid-pcf-header.raw` for decompressed byte
  reads; neither fixture exists in the maintained fixture tree.  The future
  route must compare pinned C `freetype/include/freetype/ftbzip2.h:63-91` and
  `freetype/src/bzip2/ftbzip2.c:471-515` open behavior, plus
  `freetype/src/bzip2/ftbzip2.c:371-466` read, seek-backwards reset, and close
  ownership behavior, against Rust FFI, C ABI, and WASM for the same bytes.
- The gzip stream success row is also blocked by a concrete missing declared
  stream manifest.  `ftgzip.FT_Stream_OpenGzip.opens_valid_gzip_stream`
  references `compressed/gzip/small-and-large-streams.json`, which must include
  one stream below and one above FreeType's 40KiB in-memory threshold.  The
  future route must compare pinned C `freetype/include/freetype/ftgzip.h:63-91`
  and `freetype/src/gzip/ftgzip.c:608-708` stream field classes and
  decompressed byte reads against Rust FFI, C ABI, and WASM for the same bytes.
- The LZW stream success row is blocked by missing declared byte/facade
  fixtures.  `ftlzw.FT_Stream_OpenLZW.opens_valid_lzw_stream` references
  `streams/lzw/small-valid-pcf.Z` and
  `facades/stream/memory-ft-stream.json`; neither exists in the maintained
  fixture tree.  The future route must compare pinned C
  `freetype/include/freetype/ftlzw.h:47-82`,
  `freetype/src/lzw/ftlzw.c:337-383`, and
  `freetype/src/lzw/ftlzw.c:221-308` open/read/backward-seek/close behavior
  against Rust FFI, C ABI, and WASM for the same bytes.
- The `ftsystem` external stream and custom allocator rows are blocked by
  missing harness assets and missing callback-event routing.  The public input
  manifests reference `memory/harnesses/custom-allocator-events.json`,
  `streams/harnesses/external-stream-errors.json`, and
  `streams/harnesses/external-stream-callbacks.json`; none exists in the
  maintained fixture tree.  The future routes must compare pinned C
  `freetype/src/base/ftobjs.c:5472` `FT_New_Library` custom-memory behavior,
  `freetype/src/base/ftobjs.c:2514` `FT_Open_Face` with `FT_OPEN_STREAM`, and
  public `FT_StreamRec` field/callback shape from
  `freetype/include/freetype/ftsystem.h:325-340` against Rust FFI, C ABI, and
  WASM for the same callback event sequences.
- `ftsystem.FT_StreamRec.memory_stream_field_contract` uses the maintained
  `input/fonts/DejaVuSans.ttf` font asset, but it is still not real runtime
  parity.  It needs a maintained memory-stream probe that opens those same
  bytes with `FT_New_Memory_Face`, observes `base`, `size`, `pos`, `cursor`,
  `limit`, and frame-read events, and compares those outputs across pinned C,
  Rust FFI, C ABI, and WASM.  Reusing the layout-only `FT_StreamRec` ABI route
  would be a green placeholder.

Classification change:

- 4 `ftbzip2`, 2 `ftgzip`, 2 `ftlzw`, and 4 `ftsystem` rows moved from
  `generic-fallback` to `pending-route`.
- New route audit counts: `real-parity=4465`, `generic-fallback=21`,
  `pending-route=493`, `pending-core=7`.

Required fix plan:

1. Add maintained compressed-stream and external-stream routes instead of
   per-row expected output shortcuts. They must run equivalent buffer/stream
   operation sequences through pinned C FreeType, Rust FFI, thin C ABI, and
   WASM ABI.
2. Implement pure-Rust gzip, bzip2, and LZW stream behavior first: open/read
   sequencing, decompressed byte output, source stream ownership, close behavior,
   and build-policy classification for unavailable compressors.
3. Implement pure-Rust external stream and memory callback behavior first:
   stream record field population, read/close callback dispatch, custom
   allocator event ordering, and ownership/lifetime semantics.
4. Compare exact return codes, output bytes, public `FT_StreamRec` fields,
   callback event sequences, nullness/ownership behavior, and ABI-visible
   records for the same input.
5. Promote rows only after focused `ftbzip2`, `ftgzip`, `ftlzw`, and `ftsystem`
   runtime proves exact C oracle, Rust FFI, C ABI, and WASM ABI output.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftbzip2
make -C pillow-rs-freetype test-case CASE=ftgzip
make -C pillow-rs-freetype test-case CASE=ftlzw
make -C pillow-rs-freetype test-case CASE=ftsystem
```

### Issue Set Current: `ftwinfnt`/`otsvg` specialized public-record route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `7fa157c05`: `real-parity=4465`,
  `generic-fallback=47`, `pending-route=467`, `pending-core=7`.

Finding:

- The remaining `ftwinfnt` rows cover `FT_Get_WinFNT_Header`,
  `FT_WinFNT_Header`, `FT_WinFNT_HeaderRec`, `FT_WinFNT_ID_DEFAULT`, and
  `FT_WinFNT_ID_MAC`.
- The remaining `otsvg` rows cover `FT_SVG_Document`,
  `FT_SVG_DocumentRec`, SVG renderer callback capture, document byte ranges,
  payload pointers, transforms, and metrics fields.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There are no maintained
  specialized public-record routes that open the required WinFNT/SVG-backed
  fixtures, call the relevant APIs/callbacks, and compare exact header fields,
  charset/charmap behavior, SVG document records, callback payloads, pointer
  shapes, and layout/ABI contracts across all ABI lanes.

Classification change:

- 8 `ftwinfnt` rows and 6 `otsvg` rows moved from `generic-fallback` to
  `pending-route`.
- New route audit counts: `real-parity=4465`, `generic-fallback=33`,
  `pending-route=481`, `pending-core=7`.

Required fix plan:

1. Add maintained WinFNT and OTSVG public-record routes instead of per-row
   expected output shortcuts. They must run the same operation sequence through
   pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust WinFNT behavior first: header extraction, pointer/output
   mutation semantics, record layout/field order, charset validity, and Mac
   charset charmap selection.
3. Implement pure-Rust OTSVG behavior first: document record population,
   byte-range and payload pointer fields, transform and metrics fields, and
   renderer callback document capture.
4. Compare exact return codes, public struct fields, pointer/nullness behavior,
   callback payloads, layout/ABI values, charset/charmap choices, and
   build-dependent SVG classifications.
5. Promote rows only after focused `ftwinfnt` and `otsvg` runtime proves exact
   C oracle, Rust FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftwinfnt
make -C pillow-rs-freetype test-case CASE=otsvg
```

### Issue Set Current: `freetype` core face/size/slot route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `e615ce588`: `real-parity=4465`,
  `generic-fallback=67`, `pending-route=447`, `pending-core=7`.

Finding:

- The remaining core `freetype` rows cover `FT_Attach_File`,
  `FT_Attach_Stream`, `FT_Bitmap_Size`, external-stream face ownership,
  variation face flags after selection, `FT_Face`/`FT_FaceRec` public record
  ownership and fields, `FT_GlyphSlot` reuse, `FT_LOAD_SVG_ONLY`,
  `FT_Open_Args`, `FT_Parameter`, `FT_RENDER_MODE_NORMAL`,
  `FT_STYLE_FLAG_BOLD`, `FT_Size`, `FT_SizeRec`, and Type1 AFM track kerning.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  core route that exercises the same attach/open/load/size/property sequence
  through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI, then compares
  exact public record fields, handle ownership/nullness, slot reuse side
  effects, load-target/SVG behavior, and track-kerning results.

Classification change:

- 20 core `freetype` concrete rows moved from `generic-fallback` to
  `pending-route`; the active-size and size-record cases each expand to three
  concrete rows.
- Other non-core generic rows remain untouched; this classifier is exact-case
  scoped.
- New route audit counts: `real-parity=4465`, `generic-fallback=47`,
  `pending-route=467`, `pending-core=7`.

Required fix plan:

1. Add a maintained core face/size/slot route instead of per-row expected
   output shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust behavior first: attach-file/stream dispatch, external
   stream ownership flags, face record population, owned slot/size/charmap
   handles, active size switching, slot reuse mutation, available bitmap-size
   records, load target/render mode mapping, SVG-only load behavior,
   style/variation flags, parameter pass-through, and Type1 AFM track kerning.
3. Compare exact return codes, public struct fields, string/pointer nullness,
   handle identity/stability, ownership flags, active size record values,
   slot mutation effects, load/render output differences, and track-kerning
   scalar outputs.
4. Keep already-routed core exact-error and real runtime rows real; do not
   demote them while building the broader core route.
5. Promote rows only after focused `freetype` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=freetype
```

### Issue Set Current: `ftimage` image/raster route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `4e054d85a`: `real-parity=4465`,
  `generic-fallback=85`, `pending-route=429`, `pending-core=7`.

Finding:

- The remaining `ftimage` rows cover `FT_GLYPH_FORMAT_PLOTTER`,
  `FT_GLYPH_FORMAT_SVG`, `FT_IMAGE_TAG`, mono outline dropout flags,
  `FT_OUTLINE_OWNER`, `FT_PIXEL_MODE_NONE`, `FT_Pos`, and raster callback
  records/function pointers (`FT_Raster`, `FT_Raster_Funcs`,
  `FT_Raster_New_Func`, `FT_Raster_Reset_Func`, `FT_Raster_Set_Mode_Func`,
  `FT_Raster_Span_Func`, and `FT_Raster_Done_Func`).
- Those rows had stayed in `generic-fallback` with either
  `no explicit maintained route classification` or a shared Rust fallback
  reason.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  image/raster route that loads or constructs the relevant glyph/image states,
  invokes outline bitmap/direct rendering and custom renderer lifecycle
  callbacks, observes SVG/build-dependent glyph formats, and compares exact
  public records and callback side effects across all ABI lanes.
- The `ftimage.FT_Pos.coordinate_outputs_use_ft_pos` fixture specifically
  declares `outlines/synthetic/negative-and-large-coordinates.json`, but that
  maintained synthetic outline asset is absent. Its fixture also marks this as
  a future requirement for negative and large coordinates. Promoting this row
  before adding the asset/generator and a real coordinate route would be a
  green placeholder, not same-input C/Rust/C-ABI/WASM parity.
- Follow-up on 2026-07-20: the fixture tree already has several maintained
  `outlines/synthetic/*.json` assets, but there is still no maintained
  `coordinate_endpoint_parity` runtime branch in the unified runner. Adding
  only `negative-and-large-coordinates.json` would change the blocker without
  proving parity. The row needs both the synthetic outline asset/generator and
  a runner that compares `FT_Load_Glyph` outline points,
  `FT_Outline_Get_CBox`, `FT_Vector_Transform`, and
  `FT_Outline_Decompose` callback coordinates through pinned C, Rust FFI, thin
  C ABI, and WASM ABI.

Classification change:

- 18 `ftimage` concrete rows moved from `generic-fallback` to `pending-route`;
  the SVG glyph-format success case expands to two concrete rows.
- Other generic FreeType core rows remain untouched; this classifier is
  exact-case scoped.
- New route audit counts: `real-parity=4465`, `generic-fallback=67`,
  `pending-route=447`, `pending-core=7`.

Required fix plan:

1. Add a maintained image/raster route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust image/raster behavior first: SVG/build-dependent glyph
   format reporting, plotter/source emitter inventory, outline dropout flags,
   owner destruction semantics, empty bitmap pixel mode, `FT_Pos` coordinate
   outputs, custom raster lifecycle callbacks, set-mode observability, and
   direct span emission.
   - For `FT_Pos`, first add a maintained synthetic outline asset or generator
     for negative, large-positive, large-negative, and transformed coordinate
     cases, then route the same outline through `FT_Load_Glyph`/outline point
     capture, `FT_Outline_Get_CBox`, `FT_Vector_Transform`, and
     `FT_Outline_Decompose` callback capture across all ABI lanes.
3. Compare exact return codes, glyph format values, outline flag effects,
   bitmap state, coordinate widths/signs, callback invocation counts/order,
   raster handles, set-mode results, emitted spans, and ownership/destruction
   side effects.
4. Keep already-routed image/raster exact rows real; do not demote them while
   building the broader image/raster route.
5. Promote rows only after focused `ftimage` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Current exact image/raster pending split:

- `FT_GLYPH_FORMAT_PLOTTER.source_emitter_inventory`: add a maintained glyph
  format emitter inventory route, or pinned C evidence that the versioned build
  cannot produce plotter glyphs through any shipped module. Scalar tag equality
  is not runtime parity.
- `FT_GLYPH_FORMAT_SVG.produced_by_svg_glyph_load_when_enabled`: add an
  SVG-enabled C-openable glyph fixture and pure-Rust SVG glyph-slot route that
  compares load error, slot format, SVG document fields, and C/WASM ABI output.
- `FT_GLYPH_FORMAT_SVG.unsupported_svg_build_classification`: compare the same
  SVG glyph input across pinned C, Rust FFI, C ABI, and WASM ABI for the
  build-feature-disabled case; do not treat arbitrary load errors as equal.
- `FT_OUTLINE_IGNORE_DROPOUTS.mono_dropout_behavior`: route the flag into the
  mono rasterizer through `FT_Outline_Get_Bitmap` or glyph rendering and
  compare exact bitmap bytes.
- `FT_OUTLINE_INCLUDE_STUBS.mono_stub_dropout_behavior`: add a dropout fixture
  with stubs and compare FreeType mono rasterizer bytes across all ABI lanes.
- `FT_OUTLINE_SMART_DROPOUTS.mono_smart_dropout_behavior`: add a smart-dropout
  mono fixture and exact bitmap comparison. Smooth-raster checks are not proof.
- `FT_OUTLINE_OWNER.destruction_ownership_behavior`: prove owner-bit
  allocation/free semantics and allocator ownership through a maintained
  lifecycle route.
- `FT_Raster.lifecycle_callback_contract`: add a custom renderer facade that
  records raster allocation, reset, render, set-mode, and done callback order.
- `FT_Raster_New_Func.renderer_lifecycle_calls_new`,
  `FT_Raster_Reset_Func.renderer_lifecycle_calls_reset`,
  `FT_Raster_Set_Mode_Func.set_mode_result_is_observable`, and
  `FT_Raster_Done_Func.renderer_lifecycle_calls_done`: verify each callback's
  arguments, ordering, return-code behavior, and side effects against pinned C.
- `FT_Raster_Funcs.callback_slots_match_registered_renderers`: register
  synthetic renderers and compare public callback-table identity/availability.
- `FT_Raster_Span_Func.direct_render_emits_spans`: compare direct-render span
  count, y/x/len/coverage tuples, clipping, and callback ordering.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftimage
```

### Issue Set Current: `ftparams` open-face parameter route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `7bdbf4a7c`: `real-parity=4465`,
  `generic-fallback=98`, `pending-route=416`, `pending-core=7`.

Finding:

- The remaining `ftparams` rows cover real SBIX bitmap/outline selection,
  `FT_PARAM_TAG_INCREMENTAL`, `FT_PARAM_TAG_RANDOM_SEED`, and
  `FT_PARAM_TAG_STEM_DARKENING`.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  open-face parameter route that passes identical `FT_Open_Args` parameter
  arrays through pinned C FreeType, Rust FFI, C ABI, and WASM, then compares
  exact face metadata, glyph-load behavior, property side effects, accepted
  null data, unsupported/build-dependent behavior, and parameter dispatch
  semantics.

Classification change:

- 13 `ftparams` rows moved from `generic-fallback` to `pending-route`.
- Other generic `freetype.open_face*` rows remain untouched; this classifier is
  exact-case scoped.
- New route audit counts: `real-parity=4465`, `generic-fallback=85`,
  `pending-route=429`, `pending-core=7`.

Required fix plan:

1. Add a maintained open-face parameter route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust parameter dispatch first: real sbix ignore behavior,
   incremental interface routing, random-seed face property effects,
   stem-darkening toggles, and null-data handling.
3. Compare exact return codes, face flags, family/subfamily strings, glyph-load
   outputs after parameter mutation, build-dependent support classifications,
   accepted null-data behavior, and preservation of unsupported inputs.
4. Keep already-routed open-face and exact-error rows real; do not demote them
   while building the broader parameter route.
5. Promote rows only after focused `ftparams` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftparams
```

### Issue Set Current: `ftglyph` glyph-object route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `438e6ce12`: `real-parity=4465`,
  `generic-fallback=111`, `pending-route=403`, `pending-core=7`.

Finding:

- Existing glyph exact-error and null-lifecycle rows are classified separately
  as real parity when they have pinned C/Rust/C-ABI/WASM proof.
- The remaining `ftglyph` rows cover glyph object pointer aliases, caller-owned
  glyph lifetime, glyph class identity, bbox mode constants and lowercase
  aliases, `FT_Glyph_Transform` for outline/SVG glyphs, `FT_New_Glyph` with a
  renderer-supported custom format, and SVG glyph feature availability records.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  glyph-object route that allocates glyph objects, proves ownership and alias
  semantics, applies matrix/delta transforms, checks bbox mode public constants,
  and handles SVG glyph feature availability consistently across all ABI lanes.

Classification change:

- 13 `ftglyph` rows moved from `generic-fallback` to `pending-route`.
- Existing exact glyph rows remain separately classified; this classifier is
  exact-case scoped.
- New route audit counts: `real-parity=4465`, `generic-fallback=98`,
  `pending-route=416`, `pending-core=7`.

Required fix plan:

1. Add a maintained glyph-object route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust glyph object state first: glyph allocation, class/type
   identity, caller-owned lifetime, bitmap/outline/SVG alias records, matrix
   and delta transform accumulation, bbox mode constants, and custom-format
   renderer support.
3. Compare exact return codes, pointer/nullness behavior, class identity,
   transform output, bbox mode values and aliases, SVG feature availability,
   ownership/free behavior, and unsupported/build-dependent classifications.
4. Keep already-routed glyph exact-error and null-lifecycle rows real; do not
   demote them while building the broader glyph-object route.
5. Promote rows only after focused `ftglyph` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Current exact glyph-object pending split:

- `FT_BitmapGlyph.pointer_alias_matches_record`: create a real bitmap glyph
  through `FT_Get_Glyph` or `FT_Glyph_To_Bitmap`, cast it to `FT_BitmapGlyph`,
  and compare root fields plus `FT_BitmapGlyphRec` payload across C/Rust/C-ABI/WASM.
- `FT_Glyph.caller_owned_lifetime`: add allocation/free event proof for
  `FT_New_Glyph`, `FT_Get_Glyph`, `FT_Glyph_Copy`, `FT_Glyph_To_Bitmap`, and
  `FT_Done_Glyph`. Non-null handle existence is not ownership parity.
- `FT_Glyph_Class.opaque_class_identity_only`: classify the private class
  pointer only through stable public behavior after creating outline, bitmap,
  and SVG glyphs. Do not compare raw private pointers or private fields.
- `FT_Glyph_Transform.success_outline_matrix_delta`: compare fixed-point
  matrix math, delta application, root advance, and transformed outline arrays
  against pinned C across all ABI lanes.
- `FT_Glyph_Transform.success_outline_delta_only_or_matrix_only`: cover null
  matrix/null delta public inputs and exact output for delta-only and matrix-only
  outline transforms.
- `FT_Glyph_Transform.success_svg_transform_accumulates`: use an SVG-enabled
  glyph fixture and prove `FT_SvgGlyphRec` transform/delta accumulation. Outline
  transform parity does not prove SVG record mutation.
- `FT_New_Glyph.success_renderer_supported_custom_format`: register a synthetic
  renderer whose glyph format is accepted by pinned C and compare initialized
  root fields, payload class, and ownership behavior.
- `FT_OutlineGlyph.pointer_alias_matches_record`: create a real outline glyph,
  cast it to `FT_OutlineGlyph`, and compare root record plus outline arrays.
- `FT_SvgGlyph.pointer_alias_matches_record_when_enabled`: with SVG enabled,
  prove `FT_GLYPH_FORMAT_SVG` can be cast to `FT_SvgGlyph` and exposes matching
  `FT_SvgGlyphRec` fields.
- `FT_SvgGlyph.feature_availability_recorded` and
  `FT_SvgGlyphRec.svg_feature_disabled_classification`: add a build-feature
  route that distinguishes enabled SVG glyph records from unsupported builds for
  the same public SVG glyph input.
- `FT_Done_Glyph` non-null lifecycle rows are now split by exact ownership
  obligation instead of sharing a broad generic blocker:
  - `fterrdef.FT_Err_Invalid_Handle.generic_object_handle_validation`: distinguish
    valid glyphs, null no-op, and foreign or stale handles.
  - `FT_BitmapGlyphRec.owns_bitmap_buffer`: prove bitmap buffers are
    glyph-owned and released by `FT_Done_Glyph`.
  - `FT_Done_Glyph.success_releases_owned_glyph`: prove a real owned glyph is
    released exactly once across C/Rust/C-ABI/WASM.
  - `FT_Done_Glyph.lifetime_before_library_done`: prove glyph release before
    `FT_Done_Library` uses the same allocator and invalidation behavior as C.
  - `FT_OutlineGlyphRec.owns_outline_arrays`: prove contour, point, and tag
    arrays are glyph-owned and released by `FT_Done_Glyph`.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftglyph
```

### Issue Set Current: `ftincrem` incremental-font callback route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `9675ced82`: `real-parity=4465`,
  `generic-fallback=125`, `pending-route=389`, `pending-core=7`.

Finding:

- The remaining `ftincrem` rows cover `FT_Incremental`,
  `FT_Incremental_Interface`, `FT_Incremental_InterfaceRec`,
  `FT_Incremental_FuncsRec`, `FT_Incremental_Metrics`, and
  `FT_Incremental_MetricsRec`.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  incremental-font route that opens a face through `FT_PARAM_TAG_INCREMENTAL`,
  stores the client interface, calls the glyph-data callbacks, releases glyph
  data, seeds metrics callback input, applies horizontal and vertical metrics
  overrides, and compares callback identity/lifetime behavior across all ABI
  lanes.

Classification change:

- 14 `ftincrem` concrete rows moved from `generic-fallback` to
  `pending-route`; the glyph-data success case expands to two concrete rows.
- Existing exact incremental/error rows remain separately classified.
- New route audit counts: `real-parity=4465`, `generic-fallback=111`,
  `pending-route=403`, `pending-core=7`.

Required fix plan:

1. Add a maintained incremental-font route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust incremental state first: client-owned handle storage,
   callback table validation, open-face parameter dispatch, glyph-data callback
   invocation, release callback ordering, metrics callback input seeding, and
   horizontal/vertical metrics override application.
3. Compare exact return codes, callback invocation counts/order, passed object
   identity, glyph-data buffer lifetimes, release behavior, metrics seed values,
   modified metrics output, null/absent interface behavior, and embedded-data
   fallback behavior.
4. Keep already-routed exact incremental/error rows real; do not demote them
   while building the broader incremental-font route.
5. Promote rows only after focused `ftincrem` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Current exact incremental-font pending split:

- `FT_Incremental.handle_passed_without_deref`: pass a client object through
  `FT_PARAM_TAG_INCREMENTAL` and prove pinned C forwards the opaque handle to
  callbacks without dereferencing it.
- `FT_Incremental.lifetime_owned_by_client`: prove FreeType stores only the
  client-owned interface/object for the face lifetime and does not free it.
- `FT_Incremental_FuncsRec.required_and_optional_callbacks`: compare required
  `get_glyph_data` and `get_glyph_metrics`, optional `free_glyph_data`, null
  entries, and open/load error timing.
- `FT_Incremental_FuncsRec.glyph_data_success_and_release`: record
  `get_glyph_data`, glyph-byte ownership, release callback ordering, and public
  glyph output for success rows.
- `FT_Incremental_Interface.parameter_data_cast_shape`: prove
  `FT_Parameter.data` is interpreted as `FT_Incremental_InterfaceRec*` with
  exact null/bad-shape behavior.
- `FT_Incremental_Interface.null_or_absent_interface_behavior`: compare null
  data, missing parameter, and incomplete interface without fabricating
  callbacks.
- `FT_Incremental_InterfaceRec.open_face_stores_interface`: prove
  `FT_Open_Face` stores the interface on the face and uses it during glyph
  loading.
- `FT_Incremental_InterfaceRec.object_round_trips_to_callbacks`: compare
  callback event logs showing client object identity round-trips into glyph-data
  and metrics callbacks.
- `FT_Incremental_InterfaceRec.absent_parameter_uses_embedded_data`: prove the
  same face uses embedded font data and does not call incremental callbacks when
  `FT_PARAM_TAG_INCREMENTAL` is absent.
- `FT_Incremental_Metrics.null_not_passed_by_c`: prove C never passes a null
  metrics pointer when requesting overrides.
- `FT_Incremental_MetricsRec.input_metrics_seed_matches_c`: capture callback
  input metrics before mutation and compare horizontal/vertical seed values.
- `FT_Incremental_MetricsRec.horizontal_override_applied`: compare public
  advance/bearing output after callback-written horizontal metrics.
- `FT_Incremental_MetricsRec.vertical_override_applied_where_c_calls_it`: use a
  fixture where pinned C requests vertical metrics and compare public vertical
  advances/bearings.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftincrem
```

### Issue Set Current: `ftdriver` driver/autohinter property route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `299c28d2f`: `real-parity=4465`,
  `generic-fallback=141`, `pending-route=373`, `pending-core=7`.

Finding:

- Existing driver rows with runtime assets are classified separately as real
  parity when they have pinned C/Rust/C-ABI/WASM proof.
- The remaining `ftdriver` rows cover `FT_AUTOHINTER_SCRIPT_*`,
  `FT_Prop_GlyphToScriptMap`, `FT_Prop_IncreaseXHeight`,
  `FT_HINTING_*`, `FT_CFF_HINTING_*`, and
  `TT_INTERPRETER_VERSION_40` default-property behavior.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  driver-property route that applies `FT_Property_Set`/`FT_Property_Get`, loads
  glyphs after property mutation, compares autohinter script map effects,
  x-height changes, hinting-engine choices, and default interpreter version
  behavior across all ABI lanes.

Classification change:

- 16 `ftdriver` rows moved from `generic-fallback` to `pending-route`.
- Existing exact driver rows remain `real-parity`; the classifier is
  intentionally exact-case scoped.
- New route audit counts: `real-parity=4465`, `generic-fallback=125`,
  `pending-route=389`, `pending-core=7`.

Required fix plan:

1. Add a maintained driver-property route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust property state first: autohinter script map
   get/set semantics, glyph-to-script-map mutation effects, increase-x-height
   property storage and glyph-output effects, CFF/TT hinting-engine properties,
   and default interpreter-version reporting.
3. Compare exact return codes, property values, pointer/nullness behavior,
   property persistence, glyph-output deltas after mutation, script-selection
   effects, and build-dependent hinting-engine classifications.
4. Keep already-routed exact driver rows real; do not demote them while building
   the broader driver-property route.
5. Promote rows only after focused `ftdriver` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Current exact broad-driver pending split:

- `FT_AUTOHINTER_SCRIPT_CJK.fallback_script_property_roundtrip`: implement
  `autofitter:fallback-script` `FT_Property_Set/Get` routing and verify CJK
  acceptance/readback plus invalid-script preservation against pinned C.
- `FT_AUTOHINTER_SCRIPT_CJK.glyph_to_script_map_runtime`: add a CJK-control
  font and maintained `FT_Prop_GlyphToScriptMap` route that compares per-glyph
  script values before/after auto-hinted load.
- `FT_AUTOHINTER_SCRIPT_INDIC.fallback_script_property_validation`: prove the
  pinned build's Indic fallback-script acceptance or `Invalid_Argument`
  behavior through the same property route.
- `FT_AUTOHINTER_SCRIPT_INDIC.glyph_to_script_map_runtime`: add Indic cmap
  coverage and compare script-map plus auto-hinted glyph output across
  C/Rust/C-ABI/WASM.
- `FT_AUTOHINTER_SCRIPT_LATIN.default_script_property_roundtrip`: implement
  `autofitter:default-script` directly. Do not reuse the scalar
  `truetype:interpreter-version` property route because that is a different
  public input.
- `FT_AUTOHINTER_SCRIPT_LATIN.glyph_to_script_map_runtime`: compare Basic
  Latin, Greek, and Cyrillic script-map values and subsequent auto-hinted glyph
  output through the maintained map route.
- `FT_AUTOHINTER_SCRIPT_NONE.default_and_fallback_property_roundtrip`: prove
  default-script and fallback-script NONE readback, invalid controls, and
  output preservation through all ABI lanes.
- `FT_AUTOHINTER_SCRIPT_NONE.glyph_to_script_map_runtime`: compare map mutation
  side effects and before/after auto-hinted glyph output.
- `FT_CFF_HINTING_ADOBE.hinting_engine_property_runtime` and
  `FT_CFF_HINTING_FREETYPE.hinting_engine_property_runtime`: route CFF driver
  hinting-engine property set/get and compare metrics, outline, or bitmap
  behavior on a C-openable CFF fixture. Macro values alone are not runtime
  parity.
- `FT_HINTING_ADOBE.hinting_engine_property_runtime` and
  `FT_HINTING_FREETYPE.hinting_engine_property_runtime`: route TrueType driver
  hinting-engine property set/get and compare bytecode-sensitive hinted output.
  A property-set no-op must stay pending.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftdriver
```

### Issue Set Current: `ftmodapi` module/library lifecycle route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `8a5599e28`: `real-parity=4465`,
  `generic-fallback=160`, `pending-route=354`, `pending-core=7`.

Finding:

- Existing exact module API rows and runtime-asset rows are classified
  separately as real parity when they have pinned C/Rust/C-ABI/WASM proof.
- The remaining `ftmodapi` rows cover `FT_New_Library`,
  `FT_Reference_Library`, `FT_Done_Library`, `FT_Add_Module`,
  `FT_Remove_Module`, `FT_Get_Module`, `FT_FACE_DRIVER_NAME`,
  `FT_Module_Class`, `FT_Module_Interface`, `FT_MODULE_*` flags, and
  `FT_Set_Default_Properties` no-op/malformed-property behavior.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  module API route that constructs libraries, installs/removes modules, tracks
  reference counts and destruction side effects, queries module classes and
  driver names, and compares default-property parsing behavior across all ABI
  lanes.

Classification change:

- 19 `ftmodapi` rows moved from `generic-fallback` to `pending-route`.
- Existing exact module API rows remain `real-parity`; the classifier is
  intentionally exact-case scoped.
- New route audit counts: `real-parity=4465`, `generic-fallback=141`,
  `pending-route=373`, `pending-core=7`.

Required fix plan:

1. Add a maintained module API route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust library/module state first: module registration/removal,
   reference counting, final library destruction, face/module cleanup, module
   requester interface output, driver names, and module flag metadata.
3. Compare exact return codes, pointer/nullness behavior, module identity,
   module class fields, reference-count effects, final-destroy effects,
   driver-name strings, default-property environment parsing/no-op behavior,
   and malformed-property handling.
4. Keep already-routed exact module API rows real; do not demote them while
   building the broader module route.
5. Promote rows only after focused `ftmodapi` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Residual module-lifecycle blocker detail:

- `ftmodapi.FT_Add_Module.add_minimal_module_success` requires a maintained
  synthetic module-class route matching pinned FreeType 2.14.3
  `freetype/src/base/ftobjs.c:5058-5168`: version/name checks, allocation,
  `module->library`/`memory` initialization, renderer/hinter/driver side
  effects, `module_init`, table insertion, and `FT_Get_Module` lookup.
  Existing null/future-version/duplicate error rows do not prove success
  installation.
- `ftmodapi.FT_Done_Library.final_destroy_closes_faces_and_modules` requires a
  final-destroy route matching `freetype/src/base/ftobjs.c:5542-5620`:
  refcount reaches zero, driver-owned faces are closed in C order, modules are
  removed in reverse table order, destructors run, and the library becomes
  unusable across Rust FFI, thin C ABI, and WASM.
- `ftmodapi.FT_MODULE_RENDERER.renderer_module_registration` requires
  `FT_Add_Module` coverage that proves `ft_add_renderer` runs before
  `module_init`, mutates the renderer list/current renderer, and cleans raster
  state on initialization failure. Header constant parity is insufficient.
- `ftmodapi.FT_MODULE_STYLER.styler_module_registration` requires a synthetic
  module-class route proving the styler bit is stored and observable while not
  triggering renderer, hinter, or driver setup side effects.
- `ftmodapi.FT_Module_Class.fields_drive_module_lifecycle` requires a
  class-field facade that exercises name/version/requires/flags/size,
  `module_interface`, `module_init`, and `module_done` through add, interface
  lookup, remove, and final library destruction. Layout/import checks alone are
  not lifecycle parity.
- `ftmodapi.FT_Module_Interface.requester_return_type` requires a route matching
  `FT_Get_Module_Interface` in `freetype/src/base/ftobjs.c:5199-5207`: named
  module lookup returns exactly `clazz->module_interface`, with null and
  missing-module cases still visible.
- `ftmodapi.FT_Remove_Module.removes_installed_module` requires an
  add-get-remove route matching `freetype/src/base/ftobjs.c:5261-5298`: exact
  pointer lookup, table compaction, tail nulling, `Destroy_Module`/`module_done`,
  and later lookup failure. Null or foreign module errors do not prove success
  removal.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftmodapi
```

### Issue Set Current: `ftmm` MM/variation descriptor route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `3ac19a70e`: `real-parity=4465`,
  `generic-fallback=182`, `pending-route=332`, `pending-core=7`.

Finding:

- Existing FTMM exact error rows and runtime-asset success rows are already
  classified separately as real parity when they have pinned C/Rust/C-ABI/WASM
  proof.
- The remaining FTMM rows cover `FT_Get_MM_Var`, `FT_Done_MM_Var`,
  `FT_Get_MM_Blend_Coordinates`, `FT_Get_MM_WeightVector`,
  `FT_Get_Multi_Master`, `FT_Get_Var_Axis_Flags`,
  `FT_Get_Default_Named_Instance`, `FT_Multi_Master`, `T1_MAX_MM_*`, and
  related ABI/import/layout-capacity contracts.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  MM/variation route that opens the required variable and Adobe MM faces,
  reads descriptor records, axis flags, design/blend coordinates, named style
  defaults, and Type1 MM layout capacities, and then compares exact public
  fields and lifecycle side effects across all ABI lanes.

Classification change:

- 22 `ftmm.*` rows moved from `generic-fallback` to `pending-route`.
- Existing exact FTMM error rows and runtime-asset success rows remain
  `real-parity`; the classifier is intentionally applied after those real
  promotions.
- New route audit counts: `real-parity=4465`, `generic-fallback=160`,
  `pending-route=354`, `pending-core=7`.

Required fix plan:

1. Add a maintained MM/variation descriptor route instead of per-row expected
   output shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust support for the missing descriptor and coordinate
   surfaces first: `FT_MM_Var`, Adobe `FT_Multi_Master`, axis maps, hidden-axis
   flags, default named instance, design/blend coordinate conversion, and
   descriptor ownership/free behavior.
3. Compare exact return codes, descriptor counts, axis names/tags/min/default/max
   values, hidden flags, named-style defaults, coordinate arrays, partial/excess
   count behavior, Type1 MM capacity constants, and post-`FT_Done_MM_Var`
   lifecycle state.
4. Keep the already-routed FTMM exact error and runtime success rows real; do
   not demote them while building the broader MM route.
5. Promote rows only after focused `ftmm` runtime proves exact C oracle, Rust
   FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftmm
```

### Issue Set Current: `ftgxval` GX/classic kern validation route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `c8e6ce7cf`: `real-parity=4465`,
  `generic-fallback=200`, `pending-route=314`, `pending-core=7`.

Finding:

- The remaining `ftgxval` rows cover `FT_TrueTypeGX_Validate`,
  `FT_TrueTypeGX_Free`, `FT_ClassicKern_Validate`,
  `FT_ClassicKern_Free`, `FT_VALIDATE_*` selector constants, output slot
  indexes, and validation-buffer lifetime/free behavior.
- Those rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`, then under one broad
  `ftgxval.*` pending reason.  The current classifier names each blocked row
  explicitly so future `ftgxval` rows cannot be hidden by a subsystem-wide
  placeholder.
- They are not same-input C/Rust/C-ABI/WASM parity. There is no maintained GX
  validation route that opens a real GX/classic-kern font, invokes the pinned C
  validator and the Rust implementation with the same validation flags, compares
  exact output table byte slices and lengths, and proves that the returned table
  buffers remain valid until the corresponding FreeType-style free call.

Classification change:

- 16 `ftgxval.*` rows are explicit `pending-route` records with case-specific
  blockers instead of a subsystem-wide pending reason.
- Existing exact `ftgxval` error rows and already-promoted selector/index rows
  remain real through their normal exact routes.
- The route audit count remains stable for this refinement; it changes the
  blocker granularity, not the number of accepted parity rows.

Case-specific blockers:

- `ftgxval.FT_ClassicKern_Free.frees_classic_kern_validation_buffer` needs a
  maintained classic-kern validate-then-free route proving C allocation
  ownership and free semantics for non-null validation buffers.
- `ftgxval.FT_ClassicKern_Validate.validates_ms_classic_kern` needs a
  C-openable Microsoft classic kern fixture and exact output pointer/length
  bytes plus error comparison across all ABI lanes.
- `ftgxval.FT_ClassicKern_Validate.validates_apple_classic_kern` needs a
  C-openable Apple classic kern fixture and exact validation buffer, error, and
  lifetime comparison; MS-kern success does not prove Apple selector behavior.
- `ftgxval.FT_TrueTypeGX_Free.frees_gx_validation_buffer` needs a maintained GX
  validate-then-free route proving ownership/free semantics for table buffers
  returned by `FT_TrueTypeGX_Validate`.
- `ftgxval.FT_TrueTypeGX_Validate.validates_selected_gx_tables` needs a
  C-openable GX/AAT fixture and exact selected output slots across pinned C,
  Rust FFI, C ABI, and WASM ABI.
- `ftgxval.FT_TrueTypeGX_Validate.validates_all_gx_tables` needs the same
  fixture with all requested output slots, errors, and lifetimes checked.
- `ftgxval.FT_TrueTypeGX_Validate.respects_table_length` needs malformed or
  truncated GX tables proving pinned C length validation and exact error/output
  pointer handling.
- `ftgxval.FT_VALIDATE_APPLE.runtime_selects_apple_classic_kern` needs a
  classic-kern route proving the selector chooses Apple kern validation/output
  rather than MS behavior.
- `ftgxval.FT_VALIDATE_CKERN.runtime_accepts_ms_or_apple` needs a maintained
  classic-kern route proving the selector accepts the correct MS/Apple variant
  and returns exact buffer/error output.
- `ftgxval.FT_VALIDATE_CKERN.output_table_lifetime` needs a validate/free route
  proving returned table buffers stay valid until `FT_ClassicKern_Free` and are
  freed exactly once.
- `ftgxval.FT_VALIDATE_opbd.gx_validate_selects_opbd_table`,
  `ftgxval.FT_VALIDATE_prop.gx_validate_selects_prop_table`, and
  `ftgxval.FT_VALIDATE_trak.gx_validate_selects_trak_table` each need a GX/AAT
  fixture with the named table and exact selected-table output slot/error
  comparison.
- `ftgxval.FT_VALIDATE_opbd_INDEX.indexes_gx_validate_output_slot`,
  `ftgxval.FT_VALIDATE_prop_INDEX.indexes_gx_validate_output_slot`, and
  `ftgxval.FT_VALIDATE_trak_INDEX.indexes_gx_validate_output_slot` each need
  maintained proof that the public index maps to the same
  `FT_TrueTypeGX_Validate` output slot as pinned C.

Required fix plan:

1. Add a maintained GX/classic kern validation route instead of per-row expected
   output shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement the pure-Rust GX validator first: selected/all table flag handling,
   output slot mapping, returned table byte ownership, and classic kern MS/Apple
   validation behavior. C and WASM wrappers may only copy records and manage
   handles/buffers.
3. Compare exact return codes, selected table nullness, table lengths, table
   bytes, output slot indexes, repeated validate/free behavior, null-face
   no-op behavior, and post-free lifecycle state.
4. Keep exact rejection/error rows real; do not demote them while building the
   success/lifecycle route.
5. Promote rows only after focused `ftgxval` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftgxval
```

### Issue Set Current: future-batch strict route triage

Scope:

- The requested future batch was handled with coverage disabled and exact
  runtime parity only.
- Rows are promoted only when they pass after strict classifier promotion. A
  focused pass while a row is still `generic-fallback` is not sufficient,
  because `allow_oracle_errors` can hide pinned C errors.

Promoted rows:

- `ftbdf.FT_Get_BDF_Charset_ID`: two charset rows moved from
  `generic-fallback` to `real-parity`.
- `ftcid.FT_Get_CID_From_Glyph_Index`: nine CID glyph-index rows moved from
  `generic-fallback` to `real-parity`.
- `ftcid.FT_Get_CID_Is_Internally_CID_Keyed`: three CID-keyed rows moved from
  `generic-fallback` to `real-parity`.
- `ftcid.FT_Get_CID_Registry_Ordering_Supplement.success_cid_keyed_face` moved
  from `generic-fallback` to `real-parity`.
- `ftpfr.FT_Get_PFR_Advance`: three PFR advance rows moved from
  `generic-fallback` to `real-parity`.
- `ftpfr.FT_Get_PFR_Kerning.pfr_pair_kerning_success` moved from
  `generic-fallback` to `real-parity`.
- `ftmodapi.FT_Set_Default_Properties.parses_supported_environment_property`
  moved from `generic-fallback` to `real-parity`.
- `fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error` moved
  from `pending-route` to `real-parity` after `generated/sfnt/missing-hmtx.ttf`
  was replaced by a maintained generated SFNT with `hhea` present and `hmtx`
  absent. Pinned FreeType 2.14.3 `FT_New_Memory_Face` returns
  `FT_Err_Hmtx_Table_Missing` (`147`), and the focused parity row passes across
  Rust FFI, C ABI, and WASM.
- `fterrdef.FT_Err_Array_Too_Large.ttc_header_overflow_returns_error` moved
  from `pending-route` to `real-parity` after the maintained SFNT fixture
  generator added `malformed/ttc/count-overflows-offset-array.ttc`. The fixture
  is a 12-byte TTC header with a face count whose offset array cannot fit in
  the stream. Pinned FreeType 2.14.3 `FT_New_Memory_Face` returns
  `FT_Err_Array_Too_Large` (`10`), and Rust now preserves that exact public
  error through Rust FFI, C ABI, and WASM.
- `freetype.FT_HAS_COLOR.color_font_semantics` moved from `pending-route` to
  `real-parity` by reusing the maintained `fonts/color/colr-cpal-v0.ttf`
  fixture. The row now proves the public `FT_HAS_COLOR` face-flag behavior for
  a C-openable COLR/CPAL face through pinned C, Rust FFI, C ABI, and WASM.

Route audit impact:

- `real-parity`: `4436 -> 4457`.
- `generic-fallback`: `519 -> 501`.
- `pending-route`: `24 -> 21`.

Rejected or blocked during the same pass:

- `ftpfr.FT_Get_PFR_Metrics.pfr_metrics_success` must remain explicit
  `pending-route`. A focused operation run can pass while generic fallback is
  allowed, but the raw pinned oracle cache for the selected case contains
  `FT_Err_Unimplemented_Feature` (`7`) and the row's PFR font remains
  `required_future_asset`. Treating it as `real-parity` would be a green
  placeholder. Current route audit reports two concrete pending rows for this
  case: `before_size` and `after_size`.
- Required fix for `ftpfr.FT_Get_PFR_Metrics.pfr_metrics_success`: add a
  maintained C-openable `input/fonts/pfr/basic-metrics.pfr` fixture exposing
  the PFR metrics service, implement the pure-Rust PFR metrics route in core
  first, then compare exact `outline_resolution`, `metrics_resolution`,
  `metrics_x_scale`, and `metrics_y_scale` before and after setting size
  through pinned C, Rust FFI, thin C ABI, and WASM ABI. The already-real
  non-PFR metrics/error rows are not a substitute for this service-success
  route.
- A classifier-only 14-row `ftglyph` batch was rejected. After strict
  promotion, `CASE=ftglyph` reported pinned oracle error `7` for the promoted
  success/introspection rows. These rows need maintained public runner/facade
  support before they can be called real parity.
- `freetype.attach_file`, `freetype.attach_stream`,
  `freetype.face_owned_handles`, `freetype.inspect_face_rec`,
  `freetype.glyph_slot_reuse`, `freetype.open_face_args`,
  `freetype.parameter_dispatch`, and `ftbzip2.stream_open_bzip2` were rejected
  as classifier-only promotions. Each passed while generic fallback was
  allowed, but strict success classification exposed pinned oracle error `7`.
- `t1tables.get_ps_font_private_mm_blend` was rejected: route audit would move
  eleven rows, but focused strict parity had zero runnable cases and eleven
  unresolved runtime font assets.
- `ftcache.cmap_cache_lookup` was rejected: route audit would move twelve rows,
  but focused strict parity left fifteen unresolved runtime font assets and the
  runnable oracle cache contained error `7`.
- `ftstroke.set`, `ftstroke.open_path_geometry`, `ftstroke.join_geometry`, and
  `ftstroke.parse_outline` are not valid strict promotions yet. Focused runs
  passed while the rows were still generic fallback, but after strict
  classification the full refreshed parity gate reported pinned C error `7`
  for 18 stroker rows.
- `ftmodapi.FT_Set_Default_Properties.no_environment_noop` and
  `ftmodapi.FT_Set_Default_Properties.ignores_malformed_or_failed_properties`
  also reported pinned C error `7` under strict full parity.
- `ftmodapi.FT_Add_Module.*` and `ftmodapi.FT_Get_Module.*` probe rows
  reported pinned C error `7` under strict focused parity, so they remain
  fallback-classified.
- `ftcid.FT_Get_CID_Registry_Ordering_Supplement.public_header_signature`
  reported pinned C error `7` under strict focused parity; only the runtime
  `success_cid_keyed_face` row was promoted.
- `ftpfr.FT_Get_PFR_Kerning.non_pfr_falls_back_to_unscaled_kerning` has since
  been promoted through the maintained non-PFR fallback route.  The remaining
  PFR kerning success fixture with a true PFR service font remains unresolved.
- Other `generated/sfnt/*` future rows are still missing generated assets:
  `missing-cmap.ttf`, `missing-hmtx-incremental.ttf`,
  `invalid-post-format.ttf`, `truncated-png-bitmap.ttf`, and
  `invalid-target-table.ttf`.
- `fterrdef.FT_Err_Invalid_File_Format.new_memory_face_rejects_broken_sfnt`
  has since been promoted by matching the exact pinned-C zero-table SFNT public
  error `85` (`FT_Err_Invalid_Stream_Operation`) across Rust FFI, C ABI, and
  WASM.
- `ftcolor.get_paint_graph` and `ftcolor.traverse_paint_graph` stayed
  unpromoted because focused parity reported unresolved runtime font assets.
- `FT_HAS_COLOR` is not evidence for every color font flavor. SVG and sbix
  face flags remain covered by their dedicated public rows
  (`FT_HAS_SVG`, `FT_HAS_SBIX`); CBDT/CBLC color bitmap behavior still needs a
  maintained C-openable fixture before it should be claimed as real parity.
- `ftgxval.truetype_gx_validate`, `ftgxval.classic_kern_validate`, and
  `ftmodapi.inspect_module_flags` stayed only partially runnable because
  related rows still report unresolved runtime font assets.

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftcid
make -C pillow-rs-freetype test-op OP=ftpfr
make -C pillow-rs-freetype test-op OP=ftbdf.get_bdf_charset_id
make -C pillow-rs-freetype test-op OP=ftmodapi.set_default_properties
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Array_Too_Large.ttc_header_overflow_returns_error
make -C pillow-rs-freetype test-case CASE=freetype.FT_HAS_COLOR.color_font_semantics
FONTDONE_UNIFIED_ORACLE_REFRESH=1 make fontdone-parity
python3 pillow-rs-freetype/scripts/check_public_api_inputs.py --route-audit --route-audit-json /tmp/fontdone-route-audit-final.json
```

### Issue Set Current: rejected size/cache/list/face strict probes

Scope:

- Continued strict-promotion triage after the `real-parity` `4454` checkpoint.
- Temporary operation-level probes were used only to expose pinned C behavior;
  every probe rule was removed before commit because the rows were not valid
  strict promotions as a family.

Rejected probes:

- `freetype.active_size_handle`: all three `FT_Size` active-size rows failed
  strict focused parity with pinned C error `7`.
- `freetype.size_record_state`: all three `FT_SizeRec` active-size record rows
  failed strict focused parity with pinned C error `7`.
- `ftcache.manager_lookup_size`: mixed route. `FTC_ScalerRec` pixel/point
  descriptor rows failed with pinned C error `7`; the apparent route-audit
  movement also included unresolved runtime-asset rows when checked by exact
  case filter, so no cache row was promoted from this probe.
- `ftcache.node_unref` and `ftcache.type_contract`: all probed rows failed
  strict focused parity with pinned C error `7`.
- `ftcache.cmap_cache_new`, `ftcache.image_cache_new`,
  `ftcache.image_cache_lookup`, `ftcache.manager_lookup_face`, and
  `ftcache.manager_new`: operation filters had some runnable successes, but
  exact case filters for the route-audit candidate rows reported unresolved
  runtime font assets. These remain fallback-classified until the fixture/route
  split is explicit enough to promote only runnable concrete rows.
- `freetype.open_face_with_params`, `freetype.face_properties_then_render`,
  `freetype.open_face_args`, and `freetype.inspect_face_rec`: strict focused
  parity exposed pinned C error `7` on the public rows; the operation-level
  classifier would be a green placeholder.
- `ftlist.list_insert_abi`, `ftlist.list_iterate_abi`,
  `ftlist.list_remove_abi`, and `ftlist.list_up_abi`: each import-contract row
  failed strict focused parity with pinned C error `7`.

Required follow-up plan:

1. For cache rows, split fixture variants so route-audit candidate IDs map to
   runnable concrete cases before promotion; do not promote unresolved asset
   rows based on operation-level successes.
2. For size/list/face-param rows, inspect the pinned C oracle route first: the
   first divergence is the oracle returning `7`, not Rust/C-ABI/WASM output.
3. Only retry strict promotion after the pinned C route returns success for the
   same concrete input, then verify with focused strict parity and full
   `make fontdone-parity`.

Rejected verification:

```bash
make -C pillow-rs-freetype test-op OP=freetype.active_size_handle
make -C pillow-rs-freetype test-op OP=freetype.size_record_state
make -C pillow-rs-freetype test-op OP=ftcache.manager_lookup_size
make -C pillow-rs-freetype test-op OP=ftcache.node_unref
make -C pillow-rs-freetype test-op OP=ftcache.type_contract
make -C pillow-rs-freetype test-op OP=freetype.open_face_with_params
make -C pillow-rs-freetype test-op OP=freetype.face_properties_then_render
make -C pillow-rs-freetype test-op OP=freetype.open_face_args
make -C pillow-rs-freetype test-op OP=freetype.inspect_face_rec
make -C pillow-rs-freetype test-op OP=ftlist.list_insert_abi
make -C pillow-rs-freetype test-op OP=ftlist.list_iterate_abi
make -C pillow-rs-freetype test-op OP=ftlist.list_remove_abi
make -C pillow-rs-freetype test-op OP=ftlist.list_up_abi
```

### Issue Set Current: future-batch exact route triage

Scope:

- The requested future batch was treated as a promotion pass over pending
  `required_future_asset` and generic fallback rows, with coverage disabled.
- Rows were promoted only when a focused refreshed parity run compared pinned C
  FreeType, Rust FFI, C ABI, and WASM ABI exactly.

Rejected future-asset probes:

- `ftdriver.FT_AUTOHINTER_SCRIPT_*` property-set/get and glyph-to-script-map
  rows were tested for exact promotion.  The focused `ftdriver` run exposed
  pinned C oracle error `7` for eight autohinter property/map success rows, so
  they remain `generic-fallback` until those public property routes are made
  exact.
- `freetype.FT_IS_SCALABLE.bitmap_only_face_returns_false`: temporarily
  removing `required_future_asset` made the row runnable, but pinned C returned
  error `85`; the tracked `fonts/bitmap/bitmap-only.pcf` file is still not a
  C-openable bitmap-only face for this public macro route.
- Follow-up on 2026-07-20: the row now uses the existing C-openable
  `fonts/no-encoding/bdf-or-pcf-encoding-none.bdf` bitmap face for this macro
  route only.  Direct pinned C `--face-macro` output is `FT_Err_Ok`,
  `face_flags=18`, and `FT_IS_SCALABLE=false`.  Rust previously returned
  `FT_Err_Invalid_Argument` for structurally valid BDF because the constructor
  only classified malformed BDF errors and ended valid BDF with "not
  implemented."  Rust now constructs a narrow bitmap-only `FaceKind::Bdf`
  public face record with FreeType-compatible `FIXED_SIZES | HORIZONTAL`
  flags.  This does not promote BDF glyph rendering or the separate
  `FT_ENCODING_NONE` charmap row.
- `ftcache.FTC_SBitCache_Lookup.missing_bitmap_has_null_buffer`: temporarily
  removing `required_future_asset` made the row runnable, but pinned C returned
  error `6`; the current `input/fonts/cache/bitmap-strike-small-sbits.ttf`
  symlink does not satisfy the missing-SBit success contract.
- `ftglyph.FT_Glyph_Transform.success_svg_transform_accumulates`: the row still
  references an unresolved SVG glyph asset and remains unpromoted.
- `ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output`: exact promotion
  made the row strict and exposed pinned C error `7`; it remains fallback until
  the absent-BASE-table fixture/route is made public-exact.
- Remaining unresolved `ftotval.FT_VALIDATE_*` rows stay asset-pending where
  the focused operation reports unresolved runtime fonts.
- `ftstroke.FT_Stroker_ConicTo` success rows, `ftstroke.FT_Stroker_CubicTo`
  success rows, `ftstroke.FT_Stroker_LineTo` success rows, and
  `ftstroke.FT_Stroker_GetCounts` success rows initially passed under generic
  fallback classification. After exact promotion, the refreshed C oracle
  returned error `7` for the success rows, so they remain generic fallback
  until the stroker success route is fixed at the public endpoint.
- `ftimage.FT_OUTLINE_IGNORE_DROPOUTS.mono_dropout_behavior`,
  `ftimage.FT_OUTLINE_INCLUDE_STUBS.mono_stub_dropout_behavior`, and
  `ftimage.FT_OUTLINE_SMART_DROPOUTS.mono_smart_dropout_behavior`: exact
  promotion failed with C error `7`; keep fallback-classified until the
  outline-bitmap dropout route is made public-exact.
- `ftlist.FT_List_Iterate.iterates_all_nodes_success` and
  `ftlist.FT_List_Iterate.iterator_can_mutate_current_node`: exact promotion
  now passes through the public list endpoint across pinned C FreeType, Rust
  FFI, thin C ABI, and WASM ABI.
- `fterrdef.FT_Err_Missing_Startfont_Field.bdf_first_line_not_startfont` was
  rechecked in the current future-batch pass.  The pinned public
  `FT_New_Memory_Face` result for the BDF-like fixture is
  `FT_Err_Invalid_Stream_Operation` (`85`) with a null face; the pure-Rust
  constructor now detects the same BDF-like missing-STARTFONT probe before
  SFNT fallback and routes that exact public error.

Promoted rows:

- `ftrender.FT_Get_Renderer` renderer lookup batch: `2 / 3` rows moved out
  of `generic-fallback` after adding a maintained oracle/runtime route for
  renderer class metadata.  The promoted rows validate outline, bitmap, SVG,
  and unknown-format renderer lookup through pinned C FreeType, Rust FFI, thin
  C ABI, and WASM ABI.  C behavior observed in FreeType 2.14.3:
  `FT_GLYPH_FORMAT_OUTLINE` resolves to `smooth`, `FT_GLYPH_FORMAT_BITMAP`
  resolves to `bsdf`, `FT_GLYPH_FORMAT_SVG` resolves to `ot-svg`, and an
  unknown glyph format returns no renderer.  The null-library row remains
  `pending-route` because pinned C returns `FT_Err_Invalid_Library_Handle`
  (`35`), not the fixture's declared success/null observation.
- Current route audit after this batch: `real-parity` `4436`,
  `generic-fallback` `519`, `pending-route` `24`.
- `ftrender` renderer-selection plus `ftlogging` debug logging behavior batch:
  `16 / 16` rows promoted from `generic-fallback` to `real-parity`.  The
  promoted rows are limited to strict-success public behavior rows:
  `ftrender.set_renderer_then_render`, `ftlogging.set_default_log_handler`,
  `ftlogging.set_log_handler`, `ftlogging.set_log_handler_then_default`,
  `ftlogging.trace_set_default_level`, and `ftlogging.trace_set_level`.
  Focused refreshed parity passed `ftrender` `27 / 27` runnable rows and
  `ftlogging` `14 / 14` runnable rows.  Strict promotion attempts rejected
  `ftlogging` ABI import-contract rows because the pinned oracle returned
  error `7` for those cases, so those rows remain generic.
- Current route audit after this batch: `real-parity` `4434`,
  `generic-fallback` `522`, `pending-route` `23`.
- Rejected in this pass: broad `ftstroke` promotion (`64` strict failures with
  pinned-oracle error `7`) and `freetype.open_face_with_params` / `ftparams`
  promotion (`6` strict failures with pinned-oracle error `7`).  They remain
  `generic-fallback` until the underlying route/oracle behavior is made exact.
- Rejected in the follow-up probe pass: `ftglyph` ownership/type/transform
  behavior rows (`11` strict failures with pinned-oracle error `7`) and
  no-asset `ftmodapi` module/property-management rows (`9` strict failures
  with pinned-oracle error `7`).  These rows pass only through generic fallback
  today; promoting them would reward an oracle-error placeholder rather than
  exact C/Rust/C ABI/WASM output.
- `ftmodapi` module flags plus SFNT/charmap metadata batch: `14 / 14`
  runtime-asset rows promoted from `generic-fallback` to `real-parity`.  The
  promoted rows are limited to existing runtime assets and existing exact
  routes: `ftmodapi` module-flag rows and SFNT/charmap platform/encoding/name
  metadata rows.  Focused refreshed parity passed `ftmodapi` `62 / 62`
  runnable rows and `sfnt` `62 / 62` runnable rows; unresolved `ftmodapi` and
  SFNT fixtures remain pending/generic until their runtime assets are present.
  `ftmodapi.done_library` and
  `ftmodapi.face_driver_name` were tested and left generic because strict
  promotion exposed pinned-oracle error `7` for the current fixtures.
- Current route audit after this batch: `real-parity` `4418`,
  `generic-fallback` `538`, `pending-route` `23`.
- `ftdriver` interpreter-version batch: `6 / 6` runtime-asset rows promoted
  from `generic-fallback` to `real-parity`.  Focused refreshed parity for the
  `ftdriver` filter passed `26 / 26` runnable rows; `12` driver rows remain
  pending for unresolved runtime assets, and the autohinter property/map rows
  remain generic as noted above.
- Current route audit after this batch: `real-parity` `4404`,
  `generic-fallback` `552`, `pending-route` `23`.
- `ftmm` multiple-master/variation success batch: `34 / 34` runtime-asset
  rows promoted from `generic-fallback` to `real-parity`.  The promoted rows
  are limited to existing runtime assets and existing exact routes; unresolved
  future MM fixtures remain pending/generic.  Focused refreshed parity for the
  `ftmm` filter passed `47 / 47` runnable rows, with `52` rows still pending
  for unresolved MM runtime fonts.
- Current route audit after this batch: `real-parity` `4398`,
  `generic-fallback` `558`, `pending-route` `23`.
- `fterrdef` load-glyph exact-error batch: `26 / 26` promoted from
  `pending-route` to `real-parity`.  These rows now execute refreshed pinned C
  FreeType, Rust FFI, thin C ABI, and WASM ABI exact-error comparisons instead
  of remaining blocked by the generic "any error would be a green placeholder"
  route guard:
  - `fterrdef.FT_Err_Bad_Argument.bytecode_invalid_jump_returns_error`
  - `fterrdef.FT_Err_Code_Overflow.bytecode_jump_past_range_returns_error`
  - `fterrdef.FT_Err_Code_Overflow.push_instruction_truncation_returns_error`
  - `fterrdef.FT_Err_Corrupted_Font_Header.autohint_zero_units_per_em_returns_error`
  - `fterrdef.FT_Err_Could_Not_Find_Context.truetype_context_allocation_failure_returns_error`
  - `fterrdef.FT_Err_DEF_In_Glyf_Bytecode.glyph_program_fdef_returns_error`
  - `fterrdef.FT_Err_Debug_OpCode.debug_opcode_returns_error`
  - `fterrdef.FT_Err_Divide_By_Zero.bytecode_div_zero_returns_error`
  - `fterrdef.FT_Err_ENDF_In_Exec_Stream.stray_endf_returns_error`
  - `fterrdef.FT_Err_Execution_Too_Long.opcode_counter_limit_returns_error`
  - `fterrdef.FT_Err_Execution_Too_Long.negative_jump_limit_returns_error`
  - `fterrdef.FT_Err_Glyph_Too_Big.ps_builder_large_outline_returns_error`
  - `fterrdef.FT_Err_Invalid_Opcode.tt_bytecode_invalid_opcode`
  - `fterrdef.FT_Err_Invalid_Reference.tt_bytecode_invalid_point_reference`
  - `fterrdef.FT_Err_Nested_DEFS.truetype_nested_fdef`
  - `fterrdef.FT_Err_Nested_DEFS.truetype_nested_idef`
  - `fterrdef.FT_Err_Stack_Overflow.tt_interpreter_stack_overflow`
  - `fterrdef.FT_Err_Stack_Overflow.cff_charstring_stack_overflow`
  - `fterrdef.FT_Err_Stack_Underflow.cff_charstring_missing_operands`
  - `fterrdef.FT_Err_Syntax_Error.charstring_or_afm_syntax_error`
  - `fterrdef.FT_Err_Too_Few_Arguments.tt_interpreter_argument_underflow`
  - `fterrdef.FT_Err_Too_Few_Arguments.cff_decoder_underflow_translation`
  - `fterrdef.FT_Err_Too_Many_Function_Defs.tt_fdef_limit_exceeded`
  - `fterrdef.FT_Err_Too_Many_Hints.tt_glyph_hint_limit`
  - `fterrdef.FT_Err_Too_Many_Instruction_Defs.tt_idef_limit_exceeded`
  - `fterrdef.FT_Err_Unimplemented_Feature.unsupported_font_feature`
- Current route audit after this batch: `real-parity` `4364`,
  `pending-route` `23`, `generic-fallback` `592`.
- Remaining `fterrdef` route-pending rows are not included in this promotion:
  five need missing malformed SFNT/name/post fixtures, and
  `fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error` still
  needs an exact `new_memory_face` error route.
- `freetype.FT_Init_FreeType.creates_library_handle`
- `freetype.FT_Init_FreeType.created_library_reports_version_and_modules`
- `freetype.FT_Init_FreeType.error_null_output_pointer`: pinned C FreeType
  2.14.3 returns `FT_Err_Invalid_Face_Handle` (`35`) for a null `alibrary`
  output pointer; the C ABI wrapper validates this pointer before creating the
  Rust library handle.
- `ftmodapi.FT_Add_Default_Modules.installs_default_module_table`
- `ftmm.FT_Done_MM_Var.null_descriptor_success`
- `ftotval.FT_OpenType_Validate.selected_tables_success`
- `ftotval.FT_VALIDATE_BASE.validate_selects_base_table`
- `ftotval.FT_VALIDATE_GDEF.validate_selects_gdef_table`
- `ftotval.FT_VALIDATE_GPOS.validate_selects_gpos_table`
- `ftotval.FT_VALIDATE_GSUB.validate_selects_gsub_table`
- `ftotval.FT_VALIDATE_JSTF.validate_selects_jstf_table`
- `ftotval.FT_VALIDATE_MATH.validate_selects_math_table`
- `ftotval.FT_VALIDATE_OT.validate_all_requested_tables`
- `ftwinfnt.FT_Get_WinFNT_Header.*`: `7 / 7` runnable rows passed after adding
  deterministic WinFNT fixtures and exact pinned-C/Rust FFI/C ABI/WASM routing.
- `ftwinfnt.FT_WinFNT_ID_*.charset_roundtrip_from_header`: `18 / 18` concrete
  charset rows now pass exact pinned-C/Rust FFI/C ABI/WASM comparison through
  `winfnt.get_header`.
- `fttypes.FT_UShort.winfnt_header_field_contract`: exact pinned-C/Rust FFI/C
  ABI/WASM comparison now passes after adding a deterministic
  `fonts/winfnt/ushort-fields-known.fnt` fixture through the maintained WinFNT
  generator.
- `new_memory_face` malformed BDF constructor errors: `10 / 10` BDF-specific
  rows now compare exact pinned-C/Rust FFI/C ABI/WASM error output after adding
  deterministic fixtures and explicit Rust BDF constructor error classification.
- `FT_PARAM_TAG_IGNORE_PREFERRED_FAMILY` /
  `FT_PARAM_TAG_IGNORE_PREFERRED_SUBFAMILY`: `4 / 4` behavioral rows now
  compare exact pinned-C/Rust FFI/C ABI/WASM face-name output after adding
  deterministic preferred-vs-legacy name fixtures and a dedicated
  `FT_Open_Face` parameter route. The proof compares `family_name` and
  `style_name` C-string bytes, not just face-open success.
- `ftlist.FT_List_Insert.*`: `3 / 3` topology rows now compare exact pinned-C
  FreeType, Rust FFI, thin C ABI, and WASM ABI output. The proof records
  empty-list insertion, insertion before existing head for one-node and
  three-node lists, and null list/node no-ops.
- `ftlist.FT_List_Remove.*`: `4 / 4` topology rows now compare exact pinned-C
  FreeType, Rust FFI, thin C ABI, and WASM ABI output. The proof records
  `FT_List_Remove` head/middle/tail unlinking, only-node removal, null
  list/node no-ops, and FreeType's membership-unchecked neighbor patching.
- `ftlist.FT_List_Up.*`: `3 / 3` topology rows now compare exact pinned-C
  FreeType, Rust FFI, thin C ABI, and WASM ABI output. The proof records tail
  and middle movement to head, already-head no-op behavior, and null
  list/node no-ops.
- `ftlist.FT_List_Finalize.*`: `4 / 4` callback/free rows now compare exact
  pinned-C FreeType, Rust FFI, thin C ABI, and WASM ABI output. The proof
  records destructor call order, `(memory, data, user)` identity, caller
  `memory->free(memory, node)` invocation order, null-destructor behavior, and
  null list/memory no-ops. C/WASM wrappers own raw traversal; safe `fontdone`
  receives explicit nodes plus `&FT_MemoryRec`.
- `ftlist.FT_List_Iterate.iterates_all_nodes_success`: `1 / 1` traversal row
  now compares exact pinned-C FreeType, Rust FFI, thin C ABI, and WASM ABI
  output for empty, one-node, and three-node lists. The proof records
  head-to-tail visited data tokens, user pointer identity, unchanged final
  topology, and FreeType's `next` snapshot step via the safe Rust helper.
- `ftlist.FT_List_Iterate.iterator_can_mutate_current_node`: `1 / 1` callback
  mutation row now compares exact pinned-C FreeType, Rust FFI, thin C ABI, and
  WASM ABI output. The proof records that FreeType snapshots `cur->next`
  before invoking the iterator callback, so removing the current node, moving
  it to the head, finding its data, or finalizing a side list inside the
  callback does not change the main traversal order.
- `ftmodapi.FT_MODULE_FONT_DRIVER.font_driver_modules_set_bit`: `1 / 1`
  module-class flag row now compares exact pinned-C FreeType, Rust FFI, thin
  C ABI, and WASM ABI output. The proof records default module presence and
  `module_flags` values for driver and non-driver modules. The same exact route
  code can inspect other module flags, but the five asset-backed
  `ftmodapi.inspect_module_flags` rows remain generic/pending until their
  runtime font assets resolve; they are not counted as real parity.
- `ftbdf.FT_Get_BDF_Property.*` success rows: `3 / 3` runnable rows now compare
  exact pinned-C FreeType, Rust FFI, thin C ABI, and WASM ABI output for BDF
  string/integer/cardinal properties, PCF signed properties, and SFNT embedded
  BDF strike properties. Three additional BDF-property error rows were already
  exact; three unresolved asset-backed rows remain pending.

Focused non-coverage proof before promotion:

```bash
make -C pillow-rs-freetype test-op OP=ftotval.open_type_validate
make -C pillow-rs-freetype test-op OP=ftstroke.conic_to
make -C pillow-rs-freetype test-op OP=ftstroke.cubic_to
make -C pillow-rs-freetype test-op OP=ftstroke.line_to
make -C pillow-rs-freetype test-op OP=ftstroke.get_counts
make -C pillow-rs-freetype test-op OP=ftwinfnt.get_winfnt_header
make -C pillow-rs-freetype test-op OP=new_memory_face
make -C pillow-rs-freetype test-op OP=freetype.init_free_type
make -C pillow-rs-freetype test-op OP=ftmodapi.add_default_modules
make -C pillow-rs-freetype test-op OP=ftmodapi.inspect_module_flags
make -C pillow-rs-freetype test-op OP=ftmm.done_mm_var
make -C pillow-rs-freetype test-case CASE=ftlist
make -C pillow-rs-freetype test-op OP=ftbdf.get_bdf_property
```

Results:

- `ftotval.open_type_validate`: `11 / 11` runnable rows passed, with nine
  existing asset-pending rows left visible.
- `ftwinfnt.get_winfnt_header`: `7 / 7` runnable rows passed.
- `winfnt.get_header`: `19 / 19` runnable rows passed, including the 18
  charset rows and the `fttypes.FT_UShort.winfnt_header_field_contract` row.
- `new_memory_face`: `96 / 96` runnable rows passed; this includes the 10 newly
  exact malformed-BDF constructor rows. Ten unrelated constructor rows remain
  pending by fixture/route: preferred-family/subfamily SFNT assets, name-table
  malformed SFNT assets, invalid `post`, TTC offset overflow, broken SFNT, and
  the non-promoted `Missing_Startfont` row described above.
- `freetype.init_free_type`: `3 / 3` runnable rows passed after adding the
  explicit pinned-C, Rust FFI, C ABI, and WASM route.
- `ftmodapi.add_default_modules`: `2 / 2` runnable rows passed after adding
  the exact module-table mutation route.
- `ftmm.done_mm_var`: `3 / 3` runnable rows passed after adding the exact
  library/null-descriptor route.
- `ftlist`: `29 / 29` focused runtime rows passed with `0` pending after
  promoting the ten `FT_List_Insert`, `FT_List_Remove`, and `FT_List_Up`
  topology rows, the four `FT_List_Finalize` callback/free rows, and the
  two `FT_List_Iterate` traversal/mutation rows.
- `ftmodapi.inspect_module_flags`: `1 / 1` focused runtime row passed with
  five unresolved asset-backed rows left pending.
- `ftbdf.get_bdf_property`: `3 / 3` focused runtime rows passed with three
  unresolved asset-backed rows left pending. A broader probe also checked
  `freetype.attach_file`, `freetype.attach_stream`,
  `freetype.active_size_handle`, and `freetype.size_record_state`; those rows
  passed only in the narrow operation filter but failed full strict parity with
  pinned C error `7`, so they were not promoted.
- Each selected stroker operation passed `4 / 4` only while fallback-classified;
  exact promotion failed with C error `7`, so those rows were not retained.
- No fixture input, oracle output, expected value, threshold, or runtime logic
  was changed.

Latest route proof after the FT_List follow-up:

- Route audit moved sixteen FT_List rows from `generic-fallback` to
  `real-parity` across the topology, Finalize, and Iterate follow-ups.
- Current route audit after the change: `real-parity` `4334`,
  `generic-fallback` `596`, `pending-route` `49`, `pending-core` `7`.
- Full refreshed parity remains green: `6802 / 6802` runnable rows passed with
  `432` pending. The global runnable total did not increase; these rows are
  now maintained exact routes instead of fallback-classified/pending focused
  list rows.

Latest route proof after the module-flag follow-up:

- Route audit moved one `ftmodapi.inspect_module_flags` row from
  `generic-fallback` to `real-parity`.
- Current route audit after the change: `real-parity` `4335`,
  `generic-fallback` `595`, `pending-route` `49`, `pending-core` `7`.

Latest route proof after the BDF-property follow-up:

- Route audit moved three `ftbdf.get_bdf_property` rows from
  `generic-fallback` to `real-parity`.
- Current route audit after the change: `real-parity` `4338`,
  `generic-fallback` `592`, `pending-route` `49`, `pending-core` `7`.

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

### Issue Set Current: outline orientation mutation plus stroker border helpers

Previous blockers:

- `ftoutln.FT_Outline_Get_Orientation.transformed_and_reversed_outlines` was
  classified through generic fallback even though the public behavior is
  deterministic: FreeType mutates outline point order/flags through
  `FT_Outline_Reverse` and matrix sign through `FT_Outline_Transform`, then
  reports orientation from the mutated outline.
- `FT_Outline_GetInsideBorder` and `FT_Outline_GetOutsideBorder` rows were
  generic fallback or stale `required_future_asset` rows. Their native C
  behavior is a thin delegation to `FT_Outline_Get_Orientation` in
  `freetype/src/base/ftstroke.c`.
- The shared `ftstroke.outline_border_orientation_pair` parity helper initially
  selected the inside-border endpoint from the operation name; the outside
  delegation row uses the same shared operation name, so endpoint dispatch must
  use the subject/case id as well.

Promoted rows:

- `ftoutln.FT_Outline_Get_Orientation.transformed_and_reversed_outlines`
- `ftstroke.FT_Outline_GetInsideBorder.truetype_orientation_returns_right`
- `ftstroke.FT_Outline_GetInsideBorder.non_truetype_orientation_returns_left`
- `ftstroke.FT_Outline_GetInsideBorder.orientation_delegation`
- `ftstroke.FT_Outline_GetOutsideBorder.truetype_orientation_returns_left`
- `ftstroke.FT_Outline_GetOutsideBorder.non_truetype_orientation_returns_right`
- `ftstroke.FT_Outline_GetOutsideBorder.orientation_delegation`

Verified progress:

- Exact parity passed for all seven promoted rows through Rust FFI, thin C ABI,
  WASM ABI, and pinned C FreeType.
- Route audit moved seven rows from `generic-fallback` to `real-parity`.
- Current route audit after the change: `real-parity` `4191`,
  `generic-fallback` `674`, `pending-route` `63`, `pending-core` `13`.
- Full refreshed parity remains green: `7157 / 7157` runnable rows passed with
  `77` pending. The runtime total did not increase because these rows were
  already runnable as generic fallback; this change makes them exact maintained
  real-parity routes.

Focused non-coverage results:

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Outline_Get_Orientation.transformed_and_reversed_outlines
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Outline_GetInsideBorder
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Outline_GetOutsideBorder
```

Results:

- `FT_Outline_Get_Orientation.transformed_and_reversed_outlines`: `1 / 1`
  focused runtime row passed, `0` failed, `0` pending.
- `FT_Outline_GetInsideBorder`: `3 / 3` focused runtime rows passed, `0`
  failed, `0` pending.
- `FT_Outline_GetOutsideBorder`: `3 / 3` focused runtime rows passed, `0`
  failed, `0` pending.

Resolved related probe:

- `FT_List_Add`, `FT_List_Find`, `FT_List_Remove`, and `FT_List_Up` now have
  maintained exact routes. The safe `fontdone` helper layer receives explicit
  neighboring node references from thin ABI wrappers/tests instead of
  dereferencing arbitrary raw list topology in core, preserving
  `#![deny(unsafe_code)]`. Remaining list work should focus on
  `FT_List_Insert`, `FT_List_Iterate`, and `FT_List_Finalize` route semantics.

### Issue Set Current: MoveTo callback exact-error route

Previous blocker:

- `ftimage.FT_Outline_MoveTo_Func.decompose_propagates_callback_error` was a
  generic expected-error fallback. Marking it exact initially exposed the real
  blocker: the runtime classified it as pending because the maintained
  `ftoutln.outline_decompose` callback trace route did not include the
  `FT_Outline_MoveTo_Func` error case.

Promoted row:

- `ftimage.FT_Outline_MoveTo_Func.decompose_propagates_callback_error`

Verified progress:

- The pinned native oracle now calls `FT_Outline_Decompose` with a `move_to`
  callback that returns `123` before recording an event, matching FreeType
  `src/base/ftoutln.c:99-102` immediate callback-error propagation.
- Rust FFI, thin C ABI, and WASM ABI parity runners now emit the same exact
  public error output for that row.
- Focused exact parity passed: `1 / 1` runtime row passed, `0` failed,
  `0` pending.
- Route audit moved one row from `generic-error-fallback` to `real-parity`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Outline_MoveTo_Func.decompose_propagates_callback_error
```

Result: `1 / 1` focused runtime row passed, `0` failed, `0` pending.

Rejected exact-error probes:

- `freetype.FT_New_Face.error_null_library_or_aface`: exact-error promotion
  failed because the current pinned oracle route returns a top-level `Ok`
  wrapper with per-variant error rows, so the exact-error guard rejects it.
- `fterrdef.FT_Err_Invalid_Argument.null_output_or_bad_flag_arguments`:
  exact-error promotion failed for the same top-level `Ok` wrapper shape.
- `ftlcdfil.FT_Library_SetLcdGeometry.unimplemented_with_subpixel_filtering`:
  exact-error promotion failed for the same top-level `Ok` wrapper shape.
- The adjacent `FT_Outline_LineTo_Func`, `FT_Outline_ConicTo_Func`, and
  `FT_Outline_CubicTo_Func` callback-error rows were promoted separately by
  routing them through `ftoutln.outline_decompose`, reusing standard synthetic
  fixtures, and comparing the exact callback-return matrix output.

### Issue Set Current: named-instance memory-face stale route plus rejected exact-error batch

Previous blocker:

- `freetype.FT_New_Memory_Face.success_named_instance_index` was still
  classified as `pending-route` because its manifest assets carried stale
  `required_future_asset` metadata for both `font` and `font_bytes`.
- The actual file already exists as
  `tests/fixtures/input/fonts/variable/named-instances.ttf`, and the focused
  parity row executes through the pinned C oracle, Rust FFI, thin C ABI, and
  WASM ABI.
- The macOS oracle build path also emitted `nproc: command not found` every
  time `scripts/build_ft.sh` ran from parity Make targets.

Promoted row:

- `freetype.FT_New_Memory_Face.success_named_instance_index`

Verified progress:

- Focused exact parity passed for the promoted row.
- Route audit moved one row from `pending-route` to `real-parity`.
- Current route audit after the change: `real-parity` `4167`,
  `pending-route` `71`, `generic-error-fallback` `39`,
  `generic-fallback` `696`.
- Previous full refreshed parity for this row passed as part of
  `7155 / 7155` runnable rows with `79` pending.
- `scripts/build_ft.sh` now resolves build parallelism via
  `FONTDONE_BUILD_JOBS`, `nproc`, `sysctl -n hw.ncpu`, or `getconf`, so the
  oracle build remains portable without changing parity semantics.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_New_Memory_Face.success_named_instance_index
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending. Route
audit: `real-parity` `4167`, `pending-route` `71`.

Rejected batch probes:

- `ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output`: exact-error probe
  failed because pinned C returned `Ok`; do not promote as an expected-error
  row.
- `ftcache.FTC_SBitCache_Lookup.clears_outputs_before_lookup`: exact-error
  probe failed because pinned C returned `Ok`; do not promote as an
  expected-error row.
- `fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error`:
  exact-error probe failed because pinned C returned `Ok`; do not promote as
  an expected-error row.
- `fterrdef.FT_Err_Invalid_Library_Handle.library_api_rejects_null_library`:
  exact-error probe failed because pinned C returned `Ok`; do not promote as
  an expected-error row.
- `ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs`: rechecked with
  strict error output in the current route batch and promoted to exact runtime
  parity through pinned C, Rust FFI, C ABI, and WASM.

### Issue Set Current: stale existing-asset route batch

Previous blocker:

- Several rows still had `required_future_asset` route blockers even though
  the referenced assets now exist in `tests/fixtures`.
- `freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes`
  needed the runner to honor `non_unicode_if_fixture_present`; otherwise it
  opened the primary Unicode `font` and did not exercise the Apple Roman
  fixture requested by the row.
- `freetype.FT_New_Face.success_negative_face_index_probe` and
  `freetype.FT_Open_Face.error_unknown_format_or_out_of_range_face` also
  referenced existing assets that were still classified as future work.

Promoted rows:

- `freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes@cp0`
- `freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes@cp65`
- `freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes@cp90`
- `freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes@cp1114111`
- `freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes@cp4294967295`
- `freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes@cp4294967296`
- `freetype.FT_New_Face.success_negative_face_index_probe`
- `freetype.FT_Open_Face.error_unknown_format_or_out_of_range_face`

Verified progress:

- Focused exact parity passed for all eight promoted concrete rows.
- The `FT_Get_Char_Index` runner now opens `non_unicode_charmap_font` only when
  the case explicitly requests `non_unicode_if_fixture_present`, so the C
  oracle, Rust FFI, C ABI, and WASM ABI compare the same Apple Roman input.
- Route audit moved eight rows from `pending-route` to `real-parity`.
- Current route audit after the change: `real-parity` `4175`,
  `pending-route` `63`, `generic-error-fallback` `39`,
  `generic-fallback` `696`.

Focused non-coverage results:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes
make -C pillow-rs-freetype test-case CASE=freetype.FT_New_Face.success_negative_face_index_probe
make -C pillow-rs-freetype test-case CASE=freetype.FT_Open_Face.error_unknown_format_or_out_of_range_face
```

Results:

- `FT_Get_Char_Index.active_charmap_present_and_missing_codes`: `14 / 14`
  focused runtime rows passed, `0` failed, `0` pending.
- `FT_New_Face.success_negative_face_index_probe`: `1 / 1` focused runtime row
  passed, `0` failed, `0` pending.
- `FT_Open_Face.error_unknown_format_or_out_of_range_face`: `1 / 1` focused
  runtime row passed, `0` failed, `0` pending.

Rejected probes:

- `freetype.FT_ENCODING_NONE.representative_runtime_observation`: exact probe
  failed because pinned C returned error `23`; keep pending until the fixture
  or route is corrected against C behavior.
- `freetype.FT_HAS_HORIZONTAL.no_horizontal_metrics_control`: exact probe
  failed because pinned C returned error `85`; keep pending.
- Rechecked on 2026-07-20 by scanning every local fixture font with the pinned
  C `--face-macro ... FT_HAS_HORIZONTAL` oracle.  The scan found 755
  C-openable font files across `.ttf`, `.otf`, `.ttc`, `.otb`, `.pfb`, `.pfa`,
  `.bdf`, `.pcf`, and `.fnt`; all 755 returned `FT_HAS_HORIZONTAL=true`.
  There is currently no maintained local replacement for the invalid 8-byte
  `input/fonts/no-horizontal/no-hhea-metrics.pcf` placeholder.  Keep the row
  pending until a purpose-built C-openable no-horizontal control face exists.
- `freetype.FT_IS_SCALABLE.bitmap_only_face_returns_false`: exact probe failed
  because pinned C returned error `85`; keep pending.
- `ftcache.FTC_SBitCache_Lookup.missing_bitmap_has_null_buffer`: exact probe
  failed because pinned C returned error `6`; keep pending.

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
- `ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs`: rechecked with
  strict error output in the current route batch and promoted to exact runtime
  parity.
- `ftbdf.FT_Get_BDF_Charset_ID.error_sfnt_bdf_without_selected_strike`:
  rechecked with strict error output in the current route batch and promoted
  to exact runtime parity.
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
- `freetype.FT_IS_SCALABLE.bitmap_only_face_returns_false`: later promoted by
  switching the macro row from the invalid 8-byte `.pcf` placeholder to the
  C-openable BDF bitmap face and adding a narrow valid-BDF face-open path in
  Rust.
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
- `ftbbox.FT_Outline_Get_BBox.error_malformed_outline`: closed by the
  maintained malformed-outline route below. The row now reuses
  `outlines/synthetic/malformed-outline-cases.json` and requires exact C error
  output comparison instead of the obsolete missing
  `input/outlines/malformed-outline.bin` asset.
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

### Issue Set Promoted: `FT_Get_BDF_Charset_ID` exact-error route

Previous blocker:

- `ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs` and
  `ftbdf.FT_Get_BDF_Charset_ID.error_sfnt_bdf_without_selected_strike` were
  classified as `pending-route` because exact public BDF charset error
  comparison was not enabled.

Fix:

1. Add both concrete BDF charset error rows to the exact-error promotion list.
2. Remove their route-audit `pending-route` override.
3. Keep the unresolved success fixture rows pending; this promotion covers only
   the two exact error rows that now run through pinned C, Rust FFI, C ABI, and
   WASM.

Non-coverage probe:

```bash
make -C pillow-rs-freetype test-op OP=ftbdf.get_bdf_charset_id
```

Result: `3 / 3` runnable rows passed, `0` failed, `2` pending for the
unresolved BDF charset success fixtures. Route audit moved `real-parity`
`4288 -> 4290` and `pending-route` `52 -> 50`.

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
- The broader `ftbdf.get_bdf_charset_id` operation lane now has three exact
  error rows promoted and passing. Two success fixture rows remain pending for
  unresolved BDF charset assets.

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

### Issue Set Current: `FT_Get_PFR_Kerning` non-PFR fallback route

Previous blocker:

- `ftpfr.FT_Get_PFR_Kerning.non_pfr_falls_back_to_unscaled_kerning` was in
  `pending-route` because the harness had no maintained route for the
  `ftpfr.c` fallback path.
- Pinned FreeType 2.14.3 `src/base/ftpfr.c:98-120` validates face/vector, then
  calls `FT_Get_Kerning(face, left, right, FT_KERNING_UNSCALED, avector)` when
  no PFR metrics service is present.

Fix plan:

1. Implement pure-Rust `FT_Get_PFR_Kerning` as the exact non-PFR fallback,
   without adding PFR service support or changing fixture expectations.
2. Expose the same behavior through thin C ABI and WASM ABI wrappers.
3. Route the existing `kern_font` fixture through the pinned C oracle, Rust
   FFI, C ABI, and WASM ABI exact comparison.

Verified progress:

- Focused row passed exact parity: `1 / 1`, `0` failed, `0` pending.
- `ftpfr` filter passed `7 / 7`, with `6` rows still pending for unresolved
  true-PFR assets/metrics routes.
- Full parity passed `6718 / 6718`, pending `516`.
- Route audit moved `real-parity` `4513 -> 4514` and
  `pending-route` `443 -> 442`.

Focused non-coverage result:

```bash
make -C pillow-rs-freetype test-case CASE=ftpfr.FT_Get_PFR_Kerning.non_pfr_falls_back_to_unscaled_kerning
make -C pillow-rs-freetype test-case CASE=ftpfr
```

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
  `ftcache.FTC_SBitCache_Lookup.clears_outputs_before_lookup` remained
  unpromoted in this older batch: exact-error classification required an
  error, but the then-current runner returned `Ok`. This was later resolved by
  re-running the maintained SBit cache route and promoting both rows after
  exact Rust FFI, thin C ABI, and WASM ABI comparison passed.

Fix plan:

1. Promote only the concrete rows that pass focused exact comparison:
   - `ftcache.FTC_SBitCache_LookupScaler.rejects_null_sbit_or_scaler`
   - `ftcache.FTC_SBitCache_LookupScaler.clears_outputs_before_lookup`
   - `ftcache.FTC_SBitCache_New.error_outputs_null_cache`
   - `ftcache.FTC_SBitCache_New.invalid_arguments_match_c`
2. Superseded: the two `ftcache.FTC_SBitCache_Lookup` rows are handled by the
   later SBit exact-error issue set once the maintained route verifies exact
   errors for the current same-input fixtures.
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

### Issue Set Current: `FTC_SBitCache_Lookup` direct exact-error rows

Previous blocker:

- `ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output` and
  `ftcache.FTC_SBitCache_Lookup.clears_outputs_before_lookup` stayed in
  `generic-error-fallback`.
- Older exact probes reported top-level `Ok` from the runner, so those rows
  could not be promoted without faking the public error route.

Plan:

1. Keep both fixture rows intact.
2. Re-run the current maintained `ftcache.sbit_cache_lookup` route against
   pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
3. Require exact error status/output comparison for the two concrete case IDs
   only after the focused SBit cache lane passes.
4. Add explicit route-audit reasons for the null-output and output-clearing
   public error contracts.

Verified progress:

- Focused `ftcache.FTC_SBitCache_Lookup` parity passes after exact-error
  gating: `runtime_parity: passed=12 failed=0 total=12`, with one unrelated
  required-future-asset row still pending.
- The route audit now classifies both direct SBit lookup error rows as
  `real-parity`.
- Route audit moved `real-parity` `4202 -> 4204` and
  `generic-error-fallback` `36 -> 34`.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftcache.FTC_SBitCache_Lookup
```

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
  promotion in this older batch because the then-current runner returned `Ok`.
  This was later resolved by re-running the maintained bbox route and promoting
  the row only after exact Rust FFI, thin C ABI, and WASM ABI comparison passed.
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
- `ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output` was not promoted
  in this older batch; superseded by the later direct bbox null exact-error
  issue set.
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

Probe result:

```bash
make -C pillow-rs-freetype test-ffi-compat
make -C pillow-rs-freetype test-op OP=load_glyph
```

Temporarily enabling exact-error comparison for these 26 rows moved route audit
from `pending-route` `71` to `45` and `real-parity` `4236` to `4262`, but the
focused runtime lane correctly failed `26` rows.  The route promotion was not
kept.

Observed blockers:

- `25` rows returned `Ok` from the pinned C oracle for the current maintained
  fixture inputs even though the row expects an `FT_Err_*` load error.  These
  fixture/oracle assumptions need repair before the rows can become exact
  parity.
- `fterrdef.FT_Err_Corrupted_Font_Header.autohint_zero_units_per_em_returns_error`
  did produce a C error, but Rust returned error code `7` where pinned C
  returned error code `8`.  The row name/expectation mentions
  `FT_Err_Corrupted_Font_Header`, but pinned C did not return the generated
  `0xB9` value in this probe; this needs exact fixture/oracle repair before
  implementation work can be trusted.

### Issue Set Current: open-face route family promotion probe

Problem:

- Open-face route rows had dedicated pinned-C, Rust FFI, C ABI, and WASM
  runners, but several operations were still classified as `generic-fallback`
  because they were missing from the maintained real-parity operation set.
- The related surface covers `FT_Open_Args`, external stream ownership,
  style-flag pair opening, parameter-tag opening, incremental nullness/default
  behavior, and wasm/C wrapper handling of the same public open-face routes.

Probe:

- Temporarily promoting these operations to `REAL_PARITY_OPERATIONS` was tested:
  `freetype.open_face_args`, `freetype.open_face_stream`,
  `freetype.open_face_pair`, `freetype.open_face_with_params`,
  `freetype.open_face_incremental`, `ftsystem.open_face_with_external_stream`,
  `ftincrem.open_face_incremental_nullness`,
  `ftincrem.open_face_parameter_cast`,
  `ftincrem.open_face_with_incremental_parameter`, and
  `ftincrem.open_face_without_incremental_parameter`.
- The promotion was not kept because real comparison exposed route/input
  mismatches instead of passing parity.

Commands:

```bash
make -C pillow-rs-freetype test-op OP=freetype.open_face_with_params
make -C pillow-rs-freetype test-op OP=freetype.open_face_args
make -C pillow-rs-freetype test-op OP=freetype.open_face_stream
make -C pillow-rs-freetype test-op OP=freetype.open_face_pair
make -C pillow-rs-freetype test-op OP=ftsystem.open_face_with_external_stream
make -C pillow-rs-freetype test-op OP=ftincrem.open_face_incremental_nullness
make -C pillow-rs-freetype test-op OP=ftincrem.open_face_without_incremental_parameter
make -C pillow-rs-freetype test-op OP=freetype.open_face_incremental
```

Generic fallback comparison reported `15` runnable rows passing, but exact route
classification exposed real failures:

- `freetype.open_face_stream` produced pinned C `FT_Err_Invalid_File_Format`
  (`7`) for a row currently declared success.
- Several `freetype.open_face_with_params`, `freetype.open_face_pair`,
  `ftincrem.open_face_incremental_nullness`,
  `ftincrem.open_face_without_incremental_parameter`, and
  `freetype.open_face_incremental` rows failed value comparison once promoted.
- Four related rows stayed pending due unresolved future assets.

Status: not promoted.  The rows need fixture/oracle repair or implementation
fixes before they can be moved out of `generic-fallback`.

Follow-up failed probe:

- A smaller ten-row subset was tested next:
  `freetype.open_face_pair`, `freetype.open_face_stream`,
  `freetype.parameter_dispatch`, `freetype.face_owned_handles`,
  `freetype.active_size_handle`, and `freetype.size_record_state`.
- Focused operation runs passed under generic fallback, but a full strict route
  run after temporary promotion exposed the real issue: pinned C returned
  `FT_Err_Invalid_File_Format` (`7`) for all ten rows.  The generic route had
  accepted those shared errors, so promoting them would be a green placeholder.
- The promotion was reverted.  These rows require fixture/oracle repair before
  route classification can become real parity.
- `freetype.attach_file`, `freetype.attach_stream`, `freetype.open_face_args`,
  `freetype.inspect_face_rec`, `freetype.inspect_available_sizes`, and
  `freetype.enumerate_charmaps` also remain unpromoted because their focused
  lanes or route audit expose unresolved attachment/font assets.

### Issue Set Current: FT_List route promotion probe

Problem:

- `ftlist.list_add`, `ftlist.list_finalize`, and `ftlist.list_find` looked like
  a clean eleven-row future batch because they do not depend on font assets and
  their focused operation runs pass under generic fallback.

Probe:

- Temporarily adding those three operations to `REAL_PARITY_OPERATIONS` moved
  eleven concrete rows from `generic-fallback` to `real-parity` with no
  `pending-route` increase in the route audit.
- Full strict runtime parity rejected the promotion: the pinned C oracle
  returned `FT_Err_Invalid_File_Format` (`7`) for all eleven `FT_List_*`
  success/null-shape rows once they were classified as real routes.

Status: not promoted.  The `FT_List_*` rows need real C oracle fixture routing
or generator repair before they can count as public-route parity.  Focused
generic fallback success is not sufficient evidence.

### Issue Set Current: future-batch route promotion audit

Problem:

- A 10+ row future-batch promotion was requested to avoid delaying future
  surfaces.  Several raw `generic-fallback` rows looked runnable from focused
  operation filters, but strict full parity showed that most were still
  accepting C-oracle fallback errors.

Failed probes:

- Face/open/lifecycle rows:
  `freetype.open_face_pair`, `freetype.open_face_stream`,
  `freetype.parameter_dispatch`, `freetype.face_owned_handles`,
  `freetype.active_size_handle`, and `freetype.size_record_state`.
  Temporary promotion produced ten strict failures; pinned C returned
  `FT_Err_Invalid_File_Format` (`7`) for every row.
- `FT_List_Add`, `FT_List_Finalize`, and `FT_List_Find` looked like an
  eleven-row asset-free batch.  Temporary operation promotion produced eleven
  strict failures; pinned C returned `FT_Err_Invalid_File_Format` (`7`) for
  every row.
- `FT_Stroker` primitive success/no-op rows
  (`ConicTo.coincident_control_and_end_noop`,
  `CubicTo.coincident_controls_and_end_noop`, `LineTo.zero_length_line_noop`,
  and `Stroker_New.valid_library_allocates_stroker`) produced four strict
  failures; pinned C returned `FT_Err_Invalid_File_Format` (`7`) for every row.
- A mixed thirteen-row batch across `FT_Outline_Get_Bitmap`, `FT_List_Iterate`,
  `FT_New_Glyph`, `FT_Add_Module`, and module lifecycle produced twelve strict
  failures for the same C-oracle fallback reason.
- A broader case-specific strict probe then temporarily classified 56
  runtime-referenced `generic-fallback` rows.  Full strict parity rejected 36
  rows with pinned C `FT_Err_Invalid_File_Format` (`7`); those rows remain
  generic fallback and must not be promoted by operation.

Verified progress:

- Twenty case-specific rows survived strict full parity and are now classified
  as real parity through pinned C oracle, Rust FFI, C ABI, and WASM ABI routes:
  `ftglyph.FT_New_Glyph.success_bitmap_outline_svg_empty_glyph`,
  `ftdriver.FT_Prop_GlyphToScriptMap.property_get_returns_face_map`,
  `ftmodapi.FT_DEBUG_HOOK_TRUETYPE.debug_hook_index_import_contract`, and
  seventeen GX/classic-kern validation rows:
  `ftgxval.FT_VALIDATE_GX.validates_all_requested_tables`,
  `ftgxval.FT_VALIDATE_GX_LENGTH.controls_output_slot_initialization`,
  `ftgxval.FT_VALIDATE_MS.validates_ms_classic_kern`,
  `ftgxval.FT_VALIDATE_bsln.validates_bsln_table_slot`,
  `ftgxval.FT_VALIDATE_bsln_INDEX.indexes_bsln_output_slot`,
  `ftgxval.FT_VALIDATE_feat.validates_feat_table_slot`,
  `ftgxval.FT_VALIDATE_feat_INDEX.indexes_feat_output_slot`,
  `ftgxval.FT_VALIDATE_just.validates_just_table_slot`,
  `ftgxval.FT_VALIDATE_just_INDEX.indexes_just_output_slot`,
  `ftgxval.FT_VALIDATE_kern.validates_gx_kern_table_slot`,
  `ftgxval.FT_VALIDATE_kern_INDEX.indexes_kern_output_slot`,
  `ftgxval.FT_VALIDATE_lcar.validates_lcar_table_slot`,
  `ftgxval.FT_VALIDATE_lcar_INDEX.indexes_lcar_output_slot`,
  `ftgxval.FT_VALIDATE_mort.validates_mort_table_slot`,
  `ftgxval.FT_VALIDATE_mort_INDEX.indexes_mort_output_slot`,
  `ftgxval.FT_VALIDATE_morx.validates_morx_table_slot`, and
  `ftgxval.FT_VALIDATE_morx_INDEX.indexes_morx_output_slot`.
- Route audit moved twenty rows from `generic-fallback` to `real-parity`:
  `real-parity` `4291 -> 4311`, `generic-fallback` `639 -> 619`, with
  `pending-route` unchanged at `49`.
- Full non-coverage parity passes with `6802 / 6802` runtime rows and `432`
  pending rows.

Next required work:

- Do not promote the failed batches by operation.  Add real C-oracle dispatch
  and matching Rust/C-ABI/WASM route support first, then re-run full strict
  parity.  Focused generic fallback success is not sufficient evidence.

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
- Follow-up route audit promotion: `ftimage.FT_PIXEL_MODE_NONE.empty_bitmap_state`
  now explicitly reuses the maintained empty-outline `FT_Outline_Get_Bitmap`
  route across pinned C oracle, Rust FFI, C ABI, and WASM ABI.  This compares
  the real initialized empty bitmap fields instead of treating pixel-mode `0`
  as a standalone constant or status-only shortcut.
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
- Added real maintained outline/module fixtures for the future outline-render
  batch:
  `outlines/synthetic/dropout-thin-stems-scantype.json`,
  `outlines/synthetic/overlapping-contours.json`,
  `outlines/synthetic/cw-ccw-orientation-pairs.json`,
  `outlines/synthetic/span-wide-overflow.json`,
  `outlines/render/cbox-beyond-render-limit.json`, and
  `modules/raster/params-logging-renderer.json`.
- Pinned C oracle now emits exact outputs for ten previously pending
  `ftoutln.outline_render` rows covering dropout flags, even-odd fill,
  reverse-fill orientation, raster parameter forwarding, renderer fallback
  errors, and wide `FT_Span` callback behavior.
- Rust FFI, C ABI, and WASM ABI now match pinned FreeType for
  `ftimage.FT_Span.wide_outline_span_limit`: the C smooth direct sweep assigns
  `FT_Span.len` with a plain `(unsigned short)` cast, so a 66559-pixel span
  wraps to `1023` rather than saturating to `65535` or returning an error.
- The previous `wide_outline_span_limit` expected-error assumption was wrong
  for pinned FreeType 2.14.3 and the maintained fixture; the row now expects
  success and compares the exact callback stream.

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

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Span.wide_outline_span_limit
```

Result: `1 / 1` runtime parity row passed, `0` failed, `0` pending.

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_render
```

Result: `89 / 89` runtime parity rows passed, `0` failed, `0` pending. Route
audit: `real-parity` `4236`, `pending-route` `71`,
`generic-error-fallback` absent (`0`).

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

## Pending-Core Rows

| Subject | Operation | Case | Dependency blocking real route |
|---|---|---|---|
| `ftmm.FT_Set_Named_Instance` | `ftmm.set_named_instance` | `success_adobe_mm_resets_default` | Adobe MM named-instance reset requires real Adobe MM support. |
| `ftmm.FT_Set_Named_Instance` | `ftmm.set_named_instance` | `output_changes_to_named_instance` | Named-instance glyph-output parity requires `gvar`/`HVAR` support. |
| `tttables.TT_VertHeader` | `sfnt.get_sfnt_table.record` | `sfnt_table_present_runtime.mvar_variation` | `MVAR` variation table behavior must be implemented before this SFNT table row can run. |

### Issue Set Current: FT_List public route implementation plan

Status:

- `FT_List_Add` and `FT_List_Find` are now strict real parity. The pinned C
  oracle emits tokenized list topology/identity JSON from actual FreeType
  `src/base/ftutil.c` calls, and the Rust FFI, C ABI, and WASM ABI lanes call
  their own list APIs before emitting the same tokenized graph. Route audit
  impact: `real-parity` 4311 -> 4318 and `generic-fallback` 619 -> 612.
- `FT_List_Remove`, `FT_List_Up`, `FT_List_Iterate` success/mutation rows, and
  `FT_List_Finalize` remain unpromoted. They need the same strict oracle/backend
  topology serializers before route classification changes. `FT_List_Finalize`
  additionally needs allocator/free-event modeling because C frees nodes through
  `FT_Memory`.

Previous blocker:

- `FT_List_Add`, `FT_List_Finalize`, `FT_List_Find`,
  `FT_List_Iterate`, `FT_List_Remove`, and `FT_List_Up` were present in the
  public manifest and fixtures, but the C ABI and WASM layers did not export
  these list functions, and `fontdone::ffi` only defined the raw
  `FT_ListRec`/`FT_ListNodeRec` layout types.
- Temporary route promotions for `FT_List_Add`, `FT_List_Finalize`,
  `FT_List_Find`, and the two `FT_List_Iterate` success rows failed strict
  parity because the C oracle fell through to `FT_Err_Invalid_File_Format`
  (`7`). The two existing `FT_List_Iterate` error rows only exercise generic
  invalid-argument behavior; they are not enough to prove list traversal.

Completed fix:

1. Add safe core-owned list operations in `fontdone::ffi` that take Rust
   references or typed callback adapters. This is the behavior owner. Done for
   Add/Find/Remove/Up/Iterate-next helpers; Finalize remains allocator-gated.
2. Add thin C ABI exports that only validate raw pointers, adapt callbacks, and
   call the core functions. Done for Add/Find/Remove/Up/Iterate.
3. Add equivalent WASM ABI test-support exports only if the public route audit
   expects JS/WASM coverage for these rows; they must delegate to the same core
   list operations. Done for Add/Find/Remove/Up/Iterate.
4. Extend the pinned C oracle dispatch in `scripts/gen_unified_oracle.c` for
   the synthetic list fixtures instead of accepting the fallback `error 7`.
   Done for Add/Find only.
5. Add the missing facade fixtures under `tests/fixtures/facades/list/` with
   input topology/callback descriptions only. Do not embed oracle outputs.
   Still pending for broader Remove/Up/Iterate/Finalize expansion.
6. Promote rows case-by-case only after strict full parity passes. Done for
   `ftlist.list_add` and `ftlist.list_find` only; do not add broad `ftlist.*`
   operation classification.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftlist
make -C pillow-rs-freetype test-ffi-compat
make fontdone-lint
make fontdone-parity
```

Rejected shortcuts:

- Do not mark `ftlist.*` operations as `REAL_PARITY_OPERATIONS` before real C
  oracle dispatch exists.
- Do not satisfy the success rows by accepting matching errors. Exact success
  rows must return the same topology/callback output as pinned C.
- Do not put list mutation behavior solely in C ABI or WASM wrappers; wrappers
  must stay thin.

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

### Issue Set Deferred 10+: rejected route-only batches and next real surfaces

Previous blocker:

- The user requested at least ten related issues per batch, but the largest
  remaining placeholder-style buckets are not valid route-label promotions.
- Broad route-audit promotion was tested and rejected for these buckets because
  it changed generic fallback rows into exact runtime rows that the maintained
  oracle or runtime lanes do not currently implement.

Rejected route-only batches:

1. `ftlist` mutation/lifecycle rows: 27 rows look related, but promoting the
   family makes the focused `ftlist.FT_List` run fail because the pinned C
   oracle returns harness error `7` for 27 rows. Required work is a maintained
   C oracle route plus Rust FFI, thin C ABI, and WASM list-state runners.
2. `load_glyph` public error rows: 26 rows use malformed-glyph fixtures, but
   most specify `glyph_index: "fixture_defined_error_glyph"`. The current
   harness maps that symbolic selector to glyph 0, and direct C oracle probes
   over sampled generated fonts returned `FT_Err_Ok` for tested glyphs instead
   of the declared bytecode/CFF errors. Required work is to fix fixture
   generation/metadata so the malformed glyph is source-backed and selected
   deterministically, then fix any Rust core divergence exposed by exact C
   comparison.
3. `ftdriver` autohinter/driver property rows: 12 rows pass only while generic
   fallback is allowed. Promoting `ftdriver.property_set_get`,
   `ftdriver.glyph_to_script_map`, and `ftdriver.hinting_engine_property`
   makes the `ftdriver.FT_AUTOHINTER_SCRIPT_` focused run fail with C oracle
   harness error `7` for the runtime rows. Required work is real
   `FT_Property_Set`/`FT_Property_Get` property runners plus matching Rust FFI,
   thin C ABI, and WASM observations for property readback, glyph-to-script-map
   pointers, and glyph output side effects.
4. `ftmm` descriptor/coordinate success rows: enough rows exist numerically,
   but most remaining rows reference missing or `required_future_asset`
   variation/MM fonts such as `fonts/variable/inter-wght.ttf` and
   `fonts/type1-mm/adobe-mm-two-axis.pfb`. Required work is source-backed
   fixture generation or checked-in active assets before those rows can become
   real parity.

Plan:

1. Do not promote any of the above buckets through route audit until the exact
   focused lane fails without generic fallback for the right C-vs-Rust reason.
2. For `ftdriver`, start with one representative `FT_AUTOHINTER_SCRIPT_LATIN`
   property row and implement the pinned C oracle command first; then add Rust
   FFI, C ABI, and WASM outputs only if they observe the same public fields.
3. For `load_glyph`, add explicit fixture metadata for the malformed glyph
   index or regenerate the font with a deterministic named malformed glyph;
   never keep `fixture_defined_error_glyph -> 0` as proof of error parity.
4. For `ftmm`, add or generate the missing active variable/MM fixtures before
   promoting success descriptor rows.
5. For `ftlist`, implement maintained list-state runners before changing the
   route audit category.

### Issue Set Current: `FT_Library_SetLcdGeometry` unavailable-subpixel exact route

Previous blocker:

- `ftlcdfil.FT_Library_SetLcdGeometry.unimplemented_with_subpixel_filtering`
  remained in `generic-error-fallback` even though the existing runtime route
  compared pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI successfully.

Plan:

1. Keep the fixture intact.
2. Require exact error status/output comparison for this concrete public case
   ID.
3. Add a route-audit reason specific to `FT_Library_SetLcdGeometry` unavailable
   subpixel support.
4. Re-run the focused row and route audit before committing.

Verified progress:

- Focused parity passed before reclassification:
  `runtime_parity: passed=1 failed=0 total=1`.
- The route audit now classifies
  `ftlcdfil.FT_Library_SetLcdGeometry.unimplemented_with_subpixel_filtering`
  as a real exact route.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftlcdfil.FT_Library_SetLcdGeometry.unimplemented_with_subpixel_filtering
make -C pillow-rs-freetype test-ffi-compat
FONTDONE_UNIFIED_ORACLE_REFRESH=1 make -C pillow-rs-freetype test
make -C pillow-rs-freetype fmt
make -C pillow-rs-freetype clippy
make -C pillow-rs-freetype build
git diff --check
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

### Issue Set BE: `FT_Outline_Get_BBox` null probe route

Previous blocker:

- `ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output` is a valid public
  `FT_Outline_Get_BBox` null-input fixture, but older maintained runtime
  runners for `ftbbox.outline_get_bbox` observed a loaded glyph slot's stored
  `outline_bbox` instead of invoking a public Rust FFI / thin C ABI / WASM ABI
  `FT_Outline_Get_BBox` endpoint.
- Exact-error gating was previously tested and correctly rejected
  classification: the pinned oracle path returned success for the normal
  glyph-outline route rather than executing the fixture's `null_outline` /
  `null_abbox` probes.

Plan:

1. Keep the fixture row intact.
2. Require exact error status/output comparison only after focused same-input
   comparison passes.
3. Add a route-audit reason specific to the public
   `FT_Outline_Get_BBox` null-outline/output contract.

Verified progress:

- Focused exact parity passes:
  `runtime_parity: passed=1 failed=0 total=1`.
- The route audit now classifies
  `ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output` as `real-parity`.
- Route audit moved `real-parity` `4204 -> 4205` and
  `generic-error-fallback` `34 -> 33`.

2026-07-19 update:

- `FT_Outline_Get_BBox` is now a public Rust FFI helper with thin C ABI and
  WASM ABI wrappers. The helper matches pinned FreeType
  `src/base/ftbbox.c:474-547`: null output returns `Invalid_Argument`, null
  outline returns `Invalid_Outline`, empty outlines write zero, and non-trivial
  outlines use the same decompose fallback for conic/cubic extrema.
- The unified harness now calls the real Rust FFI, C ABI, and WASM ABI BBox
  endpoints for loaded glyph BBox rows and for the null probe row.
- Focused subject result:
  `make -C pillow-rs-freetype test-case CASE=ftbbox.FT_Outline_Get_BBox`
  reported `runtime_parity: passed=8 failed=0 total=8`, with one pending row
  remaining for the missing malformed-outline fixture asset.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output
```

### Issue Set BF: rejected 10+ route-only batches

Current blocker:

- `ftlist.list_add`, `ftlist.list_find`, and `ftlist.list_finalize` rows passed
  through the unified runtime harness but stayed classified as
  `generic-fallback` because the route audit had no explicit maintained-route
  classification for these public operations.
- Promoting those operations proved this was not a safe route-classification
  gap: the exact public C oracle route returned error `7` for all 11 rows.

Rejected adjacent batches:

- An 11-row `ftlist.list_add` / `ftlist.list_find` / `ftlist.list_finalize`
  route probe was attempted and rejected. Before promotion, focused generic
  fallback probes passed `3/3`, `4/4`, and `4/4`; after adding the operations
  to `REAL_PARITY_OPERATIONS`, focused exact route probes failed `3/3`, `4/4`,
  and `4/4` with oracle error `7`.
- A 26-row generated `load_glyph` exact-error probe was attempted and rejected.
  Focused `make -C pillow-rs-freetype test-op OP=load_glyph` failed
  `runtime_parity: passed=561 failed=26 total=587` after temporarily requiring
  exact error output.
- Most rows still reached an oracle `Ok` path instead of the fixture-declared
  error. One row,
  `fterrdef.FT_Err_Corrupted_Font_Header.autohint_zero_units_per_em_returns_error`,
  returned mismatched public errors (`oracle=8`, Rust FFI `=7`).
- Those generated `load_glyph` rows remain real blockers and must not be
  promoted until their fixture assets/glyph selectors reach the intended C
  error path and Rust matches the same error.
- A 50-row `ftcolor.get_paint_graph` / `ftcolor.traverse_paint_graph` /
  `FT_TrueTypeGX_Validate` route probe was also rejected after
  `test-ffi-compat` moved the rows to `pending-route`, not `real-parity`.
  Focused runtime probes passed, but the audit correctly identified missing
  or `required_future_asset` inputs for the declared public rows.
- A 14-row driver/module route probe was attempted and rejected:
  `ftmodapi.add_module`, `ftmodapi.set_default_properties`,
  `ftdriver.glyph_to_script_map`, and `ftdriver.property_set_get` moved through
  the route audit, but focused exact probes failed with oracle error `7` for
  the success rows. The operation-level routes are not maintained enough for
  full promotion.

Do not promote yet:

- `ftlist.list_add`
- `ftlist.list_find`
- `ftlist.list_finalize`
- `ftcolor.get_paint_graph`
- `ftcolor.traverse_paint_graph`
- `FT_TrueTypeGX_Validate`
- `ftmodapi.add_module`
- `ftmodapi.set_default_properties`
- `ftdriver.glyph_to_script_map`
- `ftdriver.property_set_get`

Rejected commands:

```bash
make -C pillow-rs-freetype test-op OP=load_glyph
make -C pillow-rs-freetype test-op OP=ftlist.list_add
make -C pillow-rs-freetype test-op OP=ftlist.list_find
make -C pillow-rs-freetype test-op OP=ftlist.list_finalize
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint_graph
make -C pillow-rs-freetype test-op OP=ftcolor.traverse_paint_graph
make -C pillow-rs-freetype test-op OP=FT_TrueTypeGX_Validate
make -C pillow-rs-freetype test-op OP=ftmodapi.add_module
make -C pillow-rs-freetype test-op OP=ftmodapi.set_default_properties
make -C pillow-rs-freetype test-op OP=ftdriver.glyph_to_script_map
make -C pillow-rs-freetype test-op OP=ftdriver.property_set_get
```

### Issue Set BG: WinFNT header rows promoted to exact parity

Current status:

- The 18 concrete `winfnt.get_header` charset rows now run against pinned C
  FreeType, Rust FFI, C ABI, and WASM ABI and compare exact `error` plus
  `header.charset` output.
- The deterministic fixtures live under `tests/fixtures/fonts/winfnt/charset/`
  and encode the concrete WinFNT header charset byte for each public
  `FT_WinFNT_ID_*` constant.
- The separate `fttypes.FT_UShort.winfnt_header_field_contract` row now uses
  `tests/fixtures/fonts/winfnt/ushort-fields-known.fnt`, generated by
  `make -C pillow-rs-freetype font-fixture-winfnt`, and compares exact copied
  `FT_WinFNT_HeaderRec` `FT_UShort` fields through pinned C, Rust FFI, C ABI,
  and WASM.

Promoted rows:

- `ftwinfnt.FT_WinFNT_ID_CP1250.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1251.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1252.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1253.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1254.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1255.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1256.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1257.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1258.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP1361.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP874.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP932.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP936.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP949.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_CP950.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_MAC.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_OEM.charset_roundtrip_from_header`
- `ftwinfnt.FT_WinFNT_ID_SYMBOL.charset_roundtrip_from_header`

Additional promoted row:

- `fttypes.FT_UShort.winfnt_header_field_contract`

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=charset_roundtrip_from_header
make -C pillow-rs-freetype test-case CASE=fttypes.FT_UShort.winfnt_header_field_contract
make -C pillow-rs-freetype test-op OP=winfnt.get_header
```

Result after exact route and fixture generation:

```text
runtime_parity: passed=19 failed=0 total=19 covered_manifest_cases=19 failure_buckets=
```

### Issue Set BG: `FT_Outline_Check` invalid matrix needs exact error-output support

Current blocker:

- `ftoutln.FT_Outline_Check.invalid_null_or_count_mismatch` describes a
  per-scenario invalid-input matrix: null outline, zero points with one
  contour, and non-increasing contour endpoints.
- A real native C/Rust/C-ABI/WASM runner can produce matching per-scenario
  return codes, but the current unified exact-error guard treats
  `expect_error=true` rows as requiring a top-level error status from the
  oracle.
- Returning a top-level generic `Invalid_Outline` would hide the matrix and
  would not prove the declared same-input per-scenario output.

Required fix:

1. Add exact error-output comparison support for rows whose declared output is a
   scenario matrix of `FT_Error` values.
2. Keep the row pending-core until focused exact comparison checks all three
   invalid scenarios through pinned C, Rust FFI, C ABI, and WASM ABI.
3. Do not count this row as real parity based only on a top-level error code.

Rejected verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Outline_Check
```

Result before moving the invalid row to pending-core:

```text
runtime_parity: passed=2 failed=1 total=3
ftoutln.FT_Outline_Check.invalid_null_or_count_mismatch requires an exact C error,
but the oracle returned ok with per-scenario output
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

### Issue Set BI: `ftimage` `FT_Outline_Get_Bitmap` route audit batch

Plan:

1. Limit the batch to rows sharing `operation=ftoutln.outline_get_bitmap`.
2. Run every candidate row through the focused unified parity target before
   changing route-audit classification.
3. Promote only rows that already compare through pinned C oracle, Rust FFI,
   C ABI, and WASM ABI.
4. Keep unrelated bytecode, BDF, and `FT_Outline_Render` rows out of this
   batch until their runner dependencies are fixed.

Promoted rows:

- `ftimage.FT_Bitmap.empty_bitmap_is_valid`
- `ftimage.FT_RASTER_FLAG_DEFAULT.default_monochrome_target_path`

Focused verification before promotion:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Bitmap.empty_bitmap_is_valid
make -C pillow-rs-freetype test-case CASE=ftimage.FT_RASTER_FLAG_DEFAULT.default_monochrome_target_path
```

Each focused row passed exact runtime parity with `passed=1 failed=0 total=1`.

Rejected probes:

- Bytecode `load_glyph` error rows for invalid jump, jump past range, divide by
  zero, invalid opcode, stack overflow, too few arguments, and nested
  `FDEF`/`IDEF` were not promoted.  The current oracle argument resolver maps
  symbolic `glyph_index=fixture_defined_error_glyph` to glyph `0`, so the C
  oracle returns top-level success.  These need maintained fixture-defined
  glyph-index resolution before exact-error classification can be real.
- BDF success rows were not promoted.  The route-audit classifier has real
  handling for BDF error rows only; success rows still need explicit runtime
  runner/oracle support.
- Dropout rows were not promoted:
  `ftimage.FT_OUTLINE_IGNORE_DROPOUTS.mono_dropout_behavior`,
  `ftimage.FT_OUTLINE_INCLUDE_STUBS.mono_stub_dropout_behavior`, and
  `ftimage.FT_OUTLINE_SMART_DROPOUTS.mono_smart_dropout_behavior` still
  reference missing future fixtures
  `outlines/synthetic/dropout-thin-stems-scantype.json` and
  `outlines/synthetic/dropout-stubs-scantype.json`.  Mapping them to the
  generic square route would be a green placeholder, not real dropout parity.
- `FT_Outline_Render` rows with missing render fixtures were not promoted.
  They need fixture loading plus runtime support for the specific render case,
  not just route-audit metadata.

### Issue Set BK: `FT_New_Face` invalid output matrix route

Plan:

1. Probe a batch of generic expected-error rows and keep only rows whose C
   oracle, Rust FFI, C ABI lane, and WASM lane produce exact public error
   observations.
2. Do not promote rows whose oracle arguments still reach a normal success
   path or missing fixture path.
3. Add a maintained C oracle command for path-based `FT_New_Face` variants
   instead of reusing the generic fallback.
4. Compare the `outputs[*].status` and `outputs[*].error` matrix exactly.

First divergence:

- `freetype.FT_New_Face.error_null_library_or_aface` had a fixture declaring
  exact invalid `library`/`aface` variants, but `oracle_args` ignored
  `params.variants` for `freetype.new_face`.
- Pinned C oracle therefore opened the normal DejaVuSans path and returned
  `{"opened": true}`.
- Rust FFI had equivalent explicit null-handle behavior available, but the
  unified runner also ignored the variant matrix.

Fix:

- Added `--new-face-variants` to the C oracle.  It calls pinned C
  `FT_New_Face` once per variant and emits the same `outputs[]` shape used by
  the existing memory/open-face variant runners.
- Added a path-only `font_pathname` helper so `FT_New_Face` cannot accidentally
  consume inline bytes as a pathname.
- Routed `freetype.new_face` variant cases through a Rust-side variant runner
  and promoted only this verified row to exact-error real parity.

Promoted row:

- `freetype.FT_New_Face.error_null_library_or_aface`

Rejected rows from this probe batch:

- `ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output` — superseded by the
  later direct bbox null exact-error issue set; the maintained route now
  validates exact pinned C, Rust FFI, thin C ABI, and WASM ABI behavior.
- `ftlcdfil.FT_Library_SetLcdGeometry.unimplemented_with_subpixel_filtering`
  — current fixture routes to `{"error": 0}` on this build, not the declared
  exact unimplemented-feature error.
- `ftbdf.FT_Get_BDF_Charset_ID.error_sfnt_bdf_without_selected_strike` and
  `ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs` — both reference
  missing BDF/OTB assets in this worktree.
- `ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output` and
  `ftcache.FTC_SBitCache_Lookup.clears_outputs_before_lookup` — superseded by
  the later direct SBit exact-error issue set; the current maintained route now
  verifies exact errors for both rows.
- `fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error`,
  `fterrdef.FT_Err_Invalid_Argument.null_output_or_bad_flag_arguments`,
  `fterrdef.FT_Err_Invalid_Glyph_Format.render_or_load_rejects_unsupported_glyph_format`,
  `fterrdef.FT_Err_Invalid_Library_Handle.library_api_rejects_null_library`,
  `fterrdef.FT_Err_Missing_SVG_Hooks.svg_render_without_hooks`, and
  `fterrdef.FT_Err_Unimplemented_Feature.unsupported_font_feature` — focused
  exact promotion showed the oracle still returned top-level success, so these
  need concrete runner/asset routing before promotion.

Focused verification:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_New_Face.error_null_library_or_aface
```

Result: `runtime_parity: passed=1 failed=0 total=1 covered_manifest_cases=1`.

### Issue Set BJ: `FT_Outline_*` exact invalid matrix parity

Plan:

1. Keep the batch limited to `ftoutln` invalid-case matrices whose runners
   already produce `results[*].return` for pinned C, Rust FFI, C ABI, and
   WASM ABI.
2. Treat top-level `Ok` plus non-zero per-scenario returns as an exact error
   matrix, not as a generic success.
3. Exclude these operations from the no-font/null-param shortcut so the Rust
   lane reaches the maintained outline matrix runners.
4. Keep exact JSON comparison for every scenario return.

First divergence:

- Pinned C oracle returned top-level `Ok` with exact per-scenario error rows,
  for example `FT_Outline_Check.invalid_null_or_count_mismatch` produced
  three `FT_Err_Invalid_Outline` scenario returns.
- Rust FFI lane returned a single top-level null-param shortcut error before
  reaching `rust_outline_check_runtime_output`.

Promoted rows:

- `ftoutln.FT_Outline_Check.invalid_null_or_count_mismatch`
- `ftoutln.FT_Outline_Copy.invalid_pointer_or_size_mismatch`
- `ftoutln.FT_Outline_Done.invalid_library_or_outline_errors`
- `ftoutln.FT_Outline_Embolden.invalid_or_indeterminate_orientation_errors`
- `ftoutln.FT_Outline_EmboldenXY.invalid_orientation_or_null_errors`
- `ftoutln.FT_Outline_New.invalid_arguments_and_limits`

Focused verification before promotion:

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_check
make -C pillow-rs-freetype test-op OP=ftoutln.outline_copy
make -C pillow-rs-freetype test-op OP=ftoutln.outline_done
make -C pillow-rs-freetype test-op OP=ftoutln.outline_embolden
make -C pillow-rs-freetype test-op OP=ftoutln.outline_embolden_xy
make -C pillow-rs-freetype test-op OP=ftoutln.outline_new
```

Each focused operation passed exact runtime parity with no pending rows.

### Issue Set BH: FT_Outline lifecycle/mutation invalid matrices need exact error-output support

Resolved by Issue Set BJ.  The success rows for `FT_Outline_Copy`,
`FT_Outline_New`, `FT_Outline_Done`, `FT_Outline_Embolden`, and
`FT_Outline_EmboldenXY` already had real Rust/C-ABI/WASM/native-oracle routes.
Their multi-scenario invalid rows now use exact error-matrix comparison instead
of remaining `pending-core`:

- `ftoutln.FT_Outline_Copy.invalid_pointer_or_size_mismatch`
- `ftoutln.FT_Outline_Done.invalid_library_or_outline_errors`
- `ftoutln.FT_Outline_Embolden.invalid_or_indeterminate_orientation_errors`
- `ftoutln.FT_Outline_EmboldenXY.invalid_orientation_or_null_errors`
- `ftoutln.FT_Outline_New.invalid_arguments_and_limits`

Do not regress these rows by returning top-level generic errors or by weakening
the exact-error guard.  They require maintained per-scenario exact-error output
support in the unified harness.

These are the highest-value route families to convert from placeholder success
to real C/Rust/C-ABI/WASM parity. They are intentionally scoped as subagent
units, not as a single monolithic implementation task.

| Bucket | Rows | Owned routes | Likely owned files | Main dependency |
|---|---:|---|---|---|
| COLR/color/palette traversal | 130 | `ftcolor.*`, `otsvg.*`, SVG/color glyph load probes | `src/tables.rs`, new color table module if added, `src/font.rs`, `src/ffi/*`, `ffi-c/src/lib.rs`, `ffi-wasm/src/lib.rs`, `tests/fixtures/inputs/public-api/ftcolor.*.json` | COLR/CPAL/SVG data model and iterator ABI, then C/Rust/ABI fixture runner routes. |
| FTC cache subsystem | 112 | `ftcache.*` manager, cmap/image/sbit cache, node lifecycle | new cache module if added, `src/font.rs`, `src/tt/sbit.rs`, `src/ffi/*`, C/WASM wrappers, `ftcache.*.json` inputs | Manager-owned face/size/cache-node handles with exact FreeType error/null behavior. |
| Stroker geometry | 86 | `ftstroke.*` parse, export, glyph stroke/border, counts | new stroker module if added, `src/outline.rs`, `src/render.rs`, `src/ffi/*`, C/WASM wrappers, `ftstroke.*.json` inputs | Pure-Rust stroker path construction and exact border/count/export geometry. |
| Multiple-master and variable fonts | 84 | `ftmm.*`, named instances, variation table rows | `src/tt/fvar.rs`, `src/tt/varstore.rs`, `src/tt/mvar.rs`, `src/tables.rs`, `src/font.rs`, `src/scaler.rs`, C/WASM wrappers, `ftmm.*.json` and `tttables.*.json` inputs | Complete Adobe MM reset semantics for the remaining pending-core row. |
| Error-path asset routing | 54 | `fterrdef.*` error rows across face load, render, module, stream paths | `tests/unified_fixture_parity.rs`, public-api input rows, runner/oracle routing, then relevant core modules | Replace no-asset expected-error placeholders with concrete C oracle inputs and Rust route execution. |
| Outline/image/raster callbacks | 88 | `ftimage.*`, `ftoutln.*`, `ftrender.*` decompose/render/raster routes | `src/outline.rs`, `src/render.rs`, `src/grays.rs`, `src/ffi/*`, C/WASM wrappers | Callback-compatible outline decomposition, bitmap extraction, renderer mode state, and exact error propagation. |
| Module/property APIs | 72 | `ftmodapi.*`, `ftdriver.*`, `ftparams.*`, `freetype.face_properties*` | `src/api.rs`, `src/font.rs`, `src/autohint/*`, `src/tt/hinter/*`, `src/ffi/*`, C/WASM wrappers | Decide exact supported-vs-unsupported module surface, then route properties through real core state. |
| Glyph object APIs | 25 | `ftglyph.*` routes | `src/render.rs`, `src/font.rs`, `src/outline.rs`, `src/ffi/*`, C/WASM wrappers | Glyph object handles, bitmap glyph ownership, transform/copy/done semantics. The `ftbitmap.glyphslot_own_bitmap` allocator-fault row is already real parity. |
| GX/OpenType validation | 58 | `ftgxval.*`, `ftotval.*` validate/free rows | `src/tables.rs`, new validator modules if added, `src/ffi/*`, C/WASM wrappers | Validation buffer ownership and exact selected-table success/error behavior. |
| Legacy format/stream families | 100 | `t1tables.*`, `ftwinfnt.*`, `ftbdf.*`, `ftpfr.*`, `ftcid.*`, compressed stream rows | new format/stream modules if added, `src/font.rs`, `src/tables.rs`, `src/ffi/*`, C/WASM wrappers | Decide supported pure-Rust parsers vs exact unsupported/error policy, then add real oracle inputs. |

## Recommended Subagent Slices

### Issue Set Current: output-status public error routes

Problem:

- Some public API error fixtures report the C `FT_Error` inside
  `output.status` rather than as the top-level runtime status. Treating them
  as top-level exact-error rows is wrong: focused promotion attempts failed
  because the oracle command itself returns top-level `Ok` while the public
  API return value is carried in the output payload.
- `FT_Get_Glyph_Name` also used direct string comparisons for lifecycle
  handles, so fixture values like `"NULL"` did not reach the intended null
  buffer path in the Rust/C/WASM lanes or oracle args.

Fix:

- Normalize `freetype.get_glyph_name` `face` and `buffer` lifecycle params
  through the case-insensitive null helper.
- Classify the following rows as real parity through exact
  `output.status` comparison, not through the top-level exact-error guard:
  - `fterrdef.FT_Err_Invalid_Argument.null_output_or_bad_flag_arguments`
  - `fterrdef.FT_Err_Invalid_Library_Handle.library_api_rejects_null_library`
  - `fterrdef.FT_Err_Invalid_Glyph_Format.render_or_load_rejects_unsupported_glyph_format`
  - `fterrdef.FT_Err_Missing_SVG_Hooks.svg_render_without_hooks`

Verified progress:

- Route audit moved `real-parity` `4205 -> 4209` and
  `generic-error-fallback` `33 -> 29`.
- Focused runtime parity passed for all four rows with pinned C oracle, Rust
  FFI, thin C ABI, and WASM ABI.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Argument.null_output_or_bad_flag_arguments
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Library_Handle.library_api_rejects_null_library
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Glyph_Format.render_or_load_rejects_unsupported_glyph_format
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Missing_SVG_Hooks.svg_render_without_hooks
```

### Issue Set Current: validation and Type 1 table green-placeholder guard

Problem:

- Several validation and Type 1 table operations appeared to pass focused
  runtime parity when selected by operation filter.
- The route audit proved those green rows were placeholders: their declared
  runtime font assets are missing or marked `required_future_asset`, so the
  harness had silently substituted the default DejaVuSans fallback instead of
  executing the row's declared C fixture.

Fix:

- Non-error runtime cases that declare a font-like asset but have no resolved
  runtime font source are now selected as pending instead of falling back to
  DejaVuSans.
- Header/scalar compile-contract operations are exempt from this guard.  Their
  public comparison is constant/layout/macro output, not runtime font data.
  This prevents module-error `constant_map` compile-contract rows from being
  incorrectly moved to pending only because they also document future public
  route assets.
- This keeps the full parity metric honest: missing declared validation/table
  assets are not counted as green runtime parity.

Verified compile-contract refinement:

```bash
FONTDONE_UNIFIED_OPERATION_FILTER=constant_map \
  FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
  cargo test --manifest-path pillow-rs-freetype/Cargo.toml \
    --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture
```

Result: `constant_map` passed `46/46` with `0` pending rows.  Full selection
after the refinement moved from `6754` runnable / `480` pending to `6766`
runnable / `468` pending while preserving unresolved asset-backed rows as
pending.

Rejected promotion probes:

```bash
make -C pillow-rs-freetype test-op OP=t1tables.get_ps_font_private_mm_blend
make -C pillow-rs-freetype test-op OP=ftgxval.truetype_gx_validate
make -C pillow-rs-freetype test-op OP=ftotval.open_type_validate
make -C pillow-rs-freetype test-op OP=t1tables.get_ps_font_value
make -C pillow-rs-freetype test-op OP=FT_TrueTypeGX_Validate
make -C pillow-rs-freetype test-op OP=ftgxval.classic_kern_validate
```

Before the guard, these operation filters reported green results:

- `t1tables.get_ps_font_private_mm_blend`: `11/11`
- `ftgxval.truetype_gx_validate`: `12/12`
- `ftotval.open_type_validate`: `20/20`
- `t1tables.get_ps_font_value`: `8/8`
- `FT_TrueTypeGX_Validate`: `16/16`
- `ftgxval.classic_kern_validate`: `9/9`

These are not valid parity promotions until their declared assets exist and
the focused operation filters execute those assets without default-font
fallback.

Rejected larger batch:

- Honoring fixture `compare.mode: exact_error` globally would move the 26
  `load_glyph` error rows out of `generic-error-fallback` in the route audit,
  but focused exact runtime execution failed. Most generated bytecode rows
  currently make the C oracle return top-level `Ok`; one maps C
  `FT_Err_Corrupted_Font_Header` (`8`) while Rust returns
  `FT_Err_Unimplemented_Feature` (`7`); the existing reserved load-flag row
  maps C `6` while Rust returns `7`.
- The `fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error`
  exact probe also failed because the oracle returns top-level `Ok`.
- The remaining BDF charset error rows are blocked by missing runtime assets:
  `input/fonts/bdf/sfnt-bdf-table.otb` and
  `input/fonts/bdf/charset-registry.bdf`.
- Do not promote these exact-error rows until their oracle route/fixture assets
  and Rust error mapping are fixed.

### Issue Set Current: null-error fallback public routes

Problem:

- Six null/invalid-handle public error rows were still classified as
  `null-error-fallback` even though each row has an executable maintained route
  through the pinned C oracle, Rust FFI, thin C ABI, and WASM ABI.
- The route audit caught these before the real-parity hook because the shape
  fallback classifier ran first.

Fix:

- Add a scoped real-parity override for the six focused-proven case IDs before
  shape fallback classification:
  - `freetype.FT_Done_Face.error_invalid_or_foreign_face_handle`
  - `freetype.FT_Done_FreeType.error_invalid_or_foreign_library_handle`
  - `freetype.FT_New_Face.error_null_pathname`
  - `freetype.FT_Render_Glyph.error_null_or_unowned_slot`
  - `freetype.FT_Set_Char_Size.error_invalid_or_unscalable_face`
  - `freetype.FT_Set_Pixel_Sizes.error_invalid_or_unscalable_face`

Focused verification before promotion:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_New_Face.error_null_pathname
make -C pillow-rs-freetype test-case CASE=freetype.FT_Done_Face.error_invalid_or_foreign_face_handle
make -C pillow-rs-freetype test-case CASE=freetype.FT_Done_FreeType.error_invalid_or_foreign_library_handle
make -C pillow-rs-freetype test-case CASE=freetype.FT_Render_Glyph.error_null_or_unowned_slot
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Char_Size.error_invalid_or_unscalable_face
make -C pillow-rs-freetype test-case CASE=freetype.FT_Set_Pixel_Sizes.error_invalid_or_unscalable_face
```

Each focused case passed exact runtime parity with `0` pending rows.
Route audit after promotion: `real-parity` `4215`; `null-error-fallback`
removed.

1. `ftcache` cache-manager real parity: own `ftcache.*` routes only. Start with
   `FTC_Manager_New`, `FTC_Manager_Done`, and `FTC_CMapCache_New` before lookup
   rows.
2. `ftcolor` COLR/CPAL iteration: own color paint graph, palette, colorline,
   and layer routes. Keep SVG document rows out unless the implementation
   reaches SVG glyph loading.
3. `ftstroke` stroker core: own stroker create/configure/parse/count/export
   routes. Do not mix with generic outline decomposition fixes.
4. `ftmm` variable-font descriptors and named instances: own `ftmm.*` and the
   remaining Adobe MM reset row.  The named-instance `gvar`/`HVAR` glyph-output
   row is real parity as of `49a539875`; do not re-add its pending guard.
   Track `MVAR` SFNT table mutation as Issue Set BK below.
5. Error-path concrete assets: own `generic-error-fallback`,
   `null-error-fallback`, and `void-fallback` rows, converting placeholders to
   concrete C/Rust route checks without changing expected outputs.
6. Outline/image/raster callbacks: own `ftimage.*`, `ftoutln.*`, and
   `ftrender.*` routes that require callback or renderer state.
7. Glyph object lifecycle: own `ftglyph.*` rows. The
   `ftbitmap.glyphslot_own_bitmap` allocator-fault row and the public bitmap
   copy/convert/done/embolden/blend routes are already real parity.
8. Module/property behavior: own `ftmodapi.*`, `ftdriver.*`, `ftparams.*`, and
   `freetype.face_properties*`; first classify exact unsupported behavior vs
   real stateful support.
9. Validation APIs: own `ftgxval.*` and `ftotval.*`; route validate/free buffer
   lifetimes through all ABI surfaces.
10. Legacy formats and streams: own BDF, PFR, CID, Type 1, WinFNT, gzip, bzip2,
    and LZW rows only after the supported-vs-unsupported policy is explicit.

### Issue Set Current: exact-error placeholders made pending

Problem:

- The route audit still had `29` `generic-error-fallback` rows. These rows were
  green placeholders because the runtime accepted an expected error without an
  exact C status/output comparison.
- `26` rows were `load_glyph` malformed bytecode/charstring/font error cases.
  Prior exact probing showed these are not yet promotable: most symbolic
  `fixture_defined_error_glyph` rows still make the C oracle load glyph `0` and
  return top-level success; one row maps C `8` while Rust returns `7`; the
  reserved load-flag row maps C `6` while Rust returns `7`.
- One `new_memory_face` HMTX-missing row is also not promotable because the
  current oracle route returns top-level success.
- Two BDF charset exact-error rows are blocked by unresolved runtime assets and
  missing exact route support.

Fix:

- Classify those exact rows as `pending-route` with an explicit reason:
  accepting any error would be a green placeholder.
- This does not claim new real parity. It removes false-green fallback coverage
  so the missing exact routes remain visible.

Verification:

```bash
make -C pillow-rs-freetype test-ffi-compat
make -C pillow-rs-freetype test-op OP=load_glyph
make -C pillow-rs-freetype test-op OP=ftbdf.get_bdf_charset_id
```

Observed route audit after the change:

- `generic-error-fallback`: removed from the category counts.
- `pending-route`: `92`.
- `real-parity`: unchanged at `4215`.

Focused runtime:

- `load_glyph`: `561 / 561` runnable rows passed, `26` pending with exact-error
  route placeholder reason.
- `ftbdf.get_bdf_charset_id`: `3 / 3` runnable rows passed, `2` pending for
  unresolved BDF charset success assets.

### Issue Set Current: ftimage outline-decompose callback aliases

Problem:

- Eight `ftimage.*` outline-decompose rows were still `pending-route` even
  though they exercise the same public `FT_Outline_Decompose` route already
  covered by `ftoutln.outline_decompose`.
- The rows used the stale operation name `ftimage.outline_decompose` and stale
  non-standard fixture IDs such as `outlines/conic-consecutive-and-closing.json`
  instead of the maintained canonical fixtures under `outlines/synthetic/`.
- The harness only loaded outline assets from `outline`/`synthetic_outline`
  keys, so rows using the explicit `outline_fixture` key silently fell back to
  the default square until the row became executable.

Fix:

- Normalize the eight non-error rows onto `ftoutln.outline_decompose`.
- Point them to existing standard fixtures:
  - `outlines/synthetic/conic-single-and-consecutive.json`
  - `outlines/synthetic/cubic-paired-controls.json`
  - `outlines/synthetic/on-curve-lines-multicontour.json`
  - `outlines/synthetic/tags-with-touch-and-scan-bits.json`
- Teach the runtime harness to consume `outline_fixture`.
- Add exact C oracle aliases for the new public-input case IDs while preserving
  the older canonical case behavior.
- Use the public-input `(shift=1, delta=32)` matrix for these alias rows.

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_decompose
```

Result: `21 / 21` runnable rows passed, `2` pending. Route audit moved eight
rows from `pending-route` to `real-parity`: `real-parity` `4223`,
`pending-route` `84`.

Follow-up fix:

- The three `ftimage.FT_Outline_{Conic,Cubic,Line}To_Func.decompose_propagates_callback_error`
  rows now use the same standard fixtures and public `ftoutln.outline_decompose`
  route.  They require `compare_error_output=true` because accepting "any error"
  would hide callback-return parity bugs.
- The dedicated matrix route runs return values `7` and `1234`, fails at the
  first target line/conic/cubic callback after the initial `move_to`, and
  compares `rows[*].status`, `rows[*].events_before_abort`, and
  `rows[*].failing_callback` across the pinned C oracle, Rust FFI, C ABI, and
  WASM ABI.

Verification:

```bash
make -C pillow-rs-freetype test-op OP=ftoutln.outline_decompose
```

Result: `24 / 24` runnable rows passed, `2` pending. Route audit moved three
more rows from `pending-route` to `real-parity`: `real-parity` `4226`,
`pending-route` `81`.

### Issue Set Current: raw `FT_Outline` invalid internal pointers

Problem:

- `ftimage.FT_Outline.invalid_outline_errors` declares a five-scenario invalid
  outline matrix for `FT_Outline_Decompose`.
- Three scenarios are ordinary malformed outline data:
  `first_point_cubic`, `unpaired_cubic`, and
  `last_contour_not_n_points_minus_one`.
- Two scenarios are raw C-record states with nonzero counts and null internal
  pointers: `null_points_nonzero_count` and
  `null_contours_nonzero_count`.

Finding:

- Added the reusable fixture set
  `outlines/synthetic/malformed-outline-cases.json` for the three non-null
  malformed outline models.
- An attempted exact route that called pinned FreeType 2.14.3
  `FT_Outline_Decompose` on the raw null-internal-pointer records crashed the
  oracle process with `SIGSEGV` before producing a public `FT_Error` matrix.
- Therefore the current fixture expectation that those raw internal-pointer
  cases return `FT_Err_Invalid_Outline` is not proven for
  `FT_Outline_Decompose` by the pinned C oracle.
- Rust/C-ABI/WASM wrappers can defensively reject the raw descriptors before
  building a safe snapshot, but counting that as same-input C parity would be a
  green placeholder while the C oracle crashes.

Required fix:

1. Keep `ftimage.FT_Outline.invalid_outline_errors` pending-route until the raw
   null-pointer scenarios are reclassified or isolated behind an oracle-safe
   public API that actually returns `FT_Err_Invalid_Outline`.
2. If the fixture is wrong for `FT_Outline_Decompose`, update the fixture note
   or split the row instead of deleting the test.
3. Only promote the non-null malformed outline scenarios when they can be
   compared as their own exact row, or after the raw pointer expectation is
   corrected with pinned C evidence.

Rejected verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftimage.FT_Outline.invalid_outline_errors
```

Observed failure during attempted promotion:

```text
runtime oracle comparison failed: exit=signal: 11 (SIGSEGV)
runtime_parity: passed=0 failed=1 total=1
```

### Issue Set Current: `FT_ENCODING_NONE` representative BDF fixture

Problem:

- `freetype.FT_ENCODING_NONE.representative_runtime_observation` still marks
  `fonts/no-encoding/bdf-or-pcf-encoding-none.bdf` as `required_future_asset`.
- The file exists in the repository, but existence alone is not sufficient for
  parity because the row requires a C-openable face whose charmap reports
  `FT_ENCODING_NONE`.

Finding:

- Temporarily removing the `required_future_asset` marker made the row runnable
  and moved the route audit as if it were real parity.
- Focused runtime comparison failed because the pinned C oracle returned error
  `23` for the row instead of the expected successful charmap observation.
- The marker was restored. Promoting this row based only on the existing file
  would be a green placeholder.
- Rechecked on 2026-07-20 after adding narrow valid-BDF face-open support for
  the `FT_IS_SCALABLE` bitmap macro row.  The same BDF can open through the
  `--face-macro` oracle path, but the generated `--inspect-charmaps` route
  still returns `FT_Err_Invalid_Pixel_Size` (`23`) with the default
  `FT_Set_Pixel_Sizes(face, 0, 0)` setup.  Preserving the initial size avoids
  that setup error and proves the deeper mismatch: pinned C exposes one BDF
  charmap with encoding `1094995778`, platform `7`, encoding ID `0`; selecting
  `FT_ENCODING_NONE` (`0`) returns `FT_Err_Invalid_Argument` (`6`).  Therefore
  the existing BDF is a valid bitmap macro control but not a valid
  `FT_ENCODING_NONE` runtime-observation fixture.

Required fix:

1. Generate or select a deterministic BDF/PCF fixture that the pinned C oracle
   opens successfully for this operation.
2. Verify the selected charmap reports `FT_ENCODING_NONE` through pinned C,
   Rust FFI, C ABI, and WASM ABI.
3. Only then remove `required_future_asset`.

Rejected verification:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_ENCODING_NONE.representative_runtime_observation
```

Observed failure during attempted promotion:

```text
runtime_parity: passed=0 failed=1 total=1
rust ffi: freetype.FT_ENCODING_NONE.representative_runtime_observation oracle returned unexpected error 23
```

### Issue Set Current: `FT_Err_Name_Table_Missing` via `FT_New_Memory_Face`

Problem:

- `fterrdef.FT_Err_Name_Table_Missing.sfnt_name_storage_out_of_bounds` and
  `fterrdef.FT_Err_Name_Table_Missing.sfnt_without_name_table` are pending-route
  because their declared `new_memory_face` fixtures do not exist.
- The existing maintained name fixture generator can create SFNT name-table
  controls, but fixture existence alone is not enough; the public
  `FT_New_Memory_Face` route must produce the same pinned C classification as
  Rust FFI, C ABI, and WASM.

Finding:

- A rejected attempt added generator outputs at the declared public fixture IDs:
  `fixtures/assets/fonts/name_table_bad_storage.ttf` and
  `fixtures/assets/fonts/name_table_missing.ttf`, then enabled
  `compare_error_output=true` on both rows.
- The route audit then classified both rows as real parity, but focused runtime
  parity failed.
- For `sfnt_name_storage_out_of_bounds`, pinned C did not produce
  `FT_Err_Name_Table_Missing`; the runtime observed `FT_Err_Invalid_File_Format`
  (`error_code: 3`) for the generated short name-table payload.
- For `sfnt_without_name_table`, pinned C `FT_New_Memory_Face` opened the face
  successfully (`error_code: 0`). Missing `name` table behavior is not proven by
  this endpoint alone.
- Therefore adding these two assets and exact flags would be a green placeholder
  unless the fixture generator is adjusted to hit the exact C source branch, or
  the rows are moved to a public operation that actually observes the missing
  name table.

Required fix:

1. Keep both rows pending-route until a deterministic fixture produces the exact
   pinned C error through the declared public route, or the rows are split and
   assigned to the correct public name-table operation.
2. If `FT_New_Memory_Face` is not the public endpoint that observes
   `FT_Err_Name_Table_Missing`, update the fixture definition instead of
   changing expected outputs to match Rust.
3. Only promote the rows after focused parity passes with
   `compare_error_output=true` across pinned C, Rust FFI, C ABI, and WASM.

Rejected verification:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Name_Table_Missing
```

Observed failure during attempted promotion:

```text
runtime_parity: passed=1 failed=2 total=3
failure_buckets=rust ffi:value:2
rust ffi: fterrdef.FT_Err_Name_Table_Missing.sfnt_name_storage_out_of_bounds requires an exact C error, but the oracle returned ok (backend=Status { kind: Error, error_code: 3 })
rust ffi: fterrdef.FT_Err_Name_Table_Missing.sfnt_without_name_table requires an exact C error, but the oracle returned ok (backend=Status { kind: Ok, error_code: 0 })
```

### Issue Set Current: `FT_Err_Invalid_Post_Table_Format` via `FT_New_Memory_Face`

Problem:

- `fterrdef.FT_Err_Invalid_Post_Table_Format.sfnt_post_format_rejected` is
  pending-route because it references missing fixture
  `generated/sfnt/invalid-post-format.ttf`.
- The repository already has `fonts/metadata/post-format-unsupported.ttf`,
  documented as an unsupported `post` format 4.0 control for glyph-name and
  SFNT-table behavior, so it looked like a possible standard replacement.

Finding:

- A rejected attempt changed the row to use
  `fonts/metadata/post-format-unsupported.ttf` and enabled
  `compare_error_output=true`.
- Focused runtime parity failed: pinned C `FT_New_Memory_Face` opened that
  fixture successfully for this endpoint (`error_code: 0`).
- Therefore the existing unsupported-post fixture proves later public metadata
  behavior, not a face-open `FT_Err_Invalid_Post_Table_Format` route.

Required fix:

1. Keep the `generated/sfnt/invalid-post-format.ttf` row pending-route until a
   deterministic face-open fixture is created that makes pinned C return
   `FT_Err_Invalid_Post_Table_Format` from the declared `new_memory_face`
   operation.
2. Do not reuse `fonts/metadata/post-format-unsupported.ttf` for this row; it is
   a valid face-open input for pinned C and would make the exact route fail.
3. If FreeType only reports this error from a later `post`-table access path for
   the available fixtures, split or move the fixture to the correct public
   endpoint instead of changing expected outputs.

Rejected verification:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Post_Table_Format.sfnt_post_format_rejected
```

Observed failure during attempted promotion:

```text
runtime_parity: passed=0 failed=1 total=1
failure_buckets=rust ffi:value:1
rust ffi: fterrdef.FT_Err_Invalid_Post_Table_Format.sfnt_post_format_rejected requires an exact C error, but the oracle returned ok (backend=Status { kind: Ok, error_code: 0 })
```

### Issue Set Current: generated SFNT future-asset batch

Baseline before this batch:

- Route audit at `02dd868a1`: `real-parity=4462`,
  `pending-route=16`, `pending-core=7`, `generic-fallback=501`.

Maintained generators now emit reproducible assets for these previously missing
paths:

- `generated/sfnt/invalid-post-format.ttf`
- `fixtures/assets/fonts/name_table_bad_storage.ttf`
- `fixtures/assets/fonts/name_table_missing.ttf`
- `fonts/synthetic/sfnt/recognized-broken-sfnt.ttf`

Exact promotion findings:

- `fterrdef.FT_Err_Invalid_Post_Table_Format.sfnt_post_format_rejected`:
  generated unsupported-post-format SFNT opens successfully in pinned C
  (`FT_Err_Ok`), so this face-open row remains pending.
- `fterrdef.FT_Err_Name_Table_Missing.sfnt_name_storage_out_of_bounds`:
  generated bad-storage `name` table returns pinned-C public error `3`, not
  `FT_Err_Name_Table_Missing`.
- `fterrdef.FT_Err_Name_Table_Missing.sfnt_without_name_table`: generated
  no-name-table SFNT opens successfully in pinned C (`FT_Err_Ok`).
- `fterrdef.FT_Err_Invalid_File_Format.new_memory_face_rejects_broken_sfnt`
  has since been fixed by matching pinned C public error `85` for the exact
  zero-table SFNT asset.

The route audit intentionally keeps the remaining cases as `pending-route` until a fixture
hits the declared pinned-C public error path, or the fixture rows are moved to
the public endpoint that actually observes the condition. Counting these as
`generic-error-fallback` or `real-parity` would be a green placeholder.

Rejected exact checks:

```bash
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_Post_Table_Format.sfnt_post_format_rejected
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Name_Table_Missing
make -C pillow-rs-freetype test-case CASE=fterrdef.FT_Err_Invalid_File_Format.new_memory_face_rejects_broken_sfnt
```

### Issue Set Current: generic-fallback route promotion audit

Baseline before this batch:

- Route audit at `c26dccc96`: `real-parity=4462`,
  `generic-fallback=501`, `pending-route=16`, `pending-core=7`.

Attempted route promotions:

- `ftoutln.outline_get_orientation` (`ftoutln.FT_Orientation`): exact
  classification exposed two missing synthetic orientation fixtures as
  explicit pending-route rows. The focused runtime selected two runnable rows
  and passed both.
- `freetype.active_size_handle` / `freetype.size_record_state` and
  `ftglyph.glyph_transform` were rejected for promotion. They passed while
  generic fallback tolerated oracle errors, but failed once exact route
  classification required the pinned-C oracle to produce success output.

Rejected exact checks:

```text
make -C pillow-rs-freetype test-case CASE=freetype.FT_Size
runtime_parity: passed=36 failed=6 total=42
rust ffi: freetype.FT_Size.active_size_handle_runtime@s10 oracle returned unexpected error 7
rust ffi: freetype.FT_Size.active_size_handle_runtime@s16 oracle returned unexpected error 7
rust ffi: freetype.FT_Size.active_size_handle_runtime@s24 oracle returned unexpected error 7
rust ffi: freetype.FT_SizeRec.active_size_record_runtime@s1 oracle returned unexpected error 7
rust ffi: freetype.FT_SizeRec.active_size_record_runtime@s2 oracle returned unexpected error 7
rust ffi: freetype.FT_SizeRec.active_size_record_runtime@s3 oracle returned unexpected error 7

make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_Transform
runtime_parity: passed=1 failed=2 total=3
rust ffi: ftglyph.FT_Glyph_Transform.success_outline_matrix_delta oracle returned unexpected error 7
rust ffi: ftglyph.FT_Glyph_Transform.success_outline_delta_only_or_matrix_only oracle returned unexpected error 7
```

Route audit after accepted promotion:

- `real-parity=4462`
- `generic-fallback=499`
- `pending-route=18`
- `pending-core=7`

The pending-route increase was intentional de-placeholdering. The two
orientation rows listed below were later closed by reusing the standard
`outlines/orientation/*` fixture tree and routing
`ftoutln.outline_get_orientation` through the exact C/Rust/C-ABI/WASM
orientation matrix:

- `ftoutln.FT_Orientation.orientation_algorithm_contract`
- `ftoutln.FT_ORIENTATION_FILL_LEFT.returned_for_positive_area`

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftoutln.FT_Orientation
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Outline_Get_BBox` malformed outline parity

Status: partially closed on 2026-07-20.

Closed:

- `ftbbox.FT_Outline_Get_BBox.error_malformed_outline` now routes three
  non-null, array-backed malformed outline models through pinned FreeType
  `FT_Outline_Get_BBox`, Rust FFI, thin C ABI, and WASM ABI.
- The public input now uses the standard maintained fixture
  `outlines/synthetic/malformed-outline-cases.json` instead of the obsolete
  missing binary fixture `input/outlines/malformed-outline.bin`.
- The row requires `compare_error_output: true`, so route audit counts it only
  when the same row-level C error/status/bbox output is compared exactly.
- First divergence found while promoting the row: Rust called
  `FT_Outline_Check` before bbox computation and rejected
  `last_contour_not_n_points_minus_one`, while pinned FreeType
  `src/base/ftbbox.c:474-547` does not call `FT_Outline_Check` and can return
  success through the `cbox == bbox` fast path. Rust now keeps validation local
  to the bbox algorithm and lets the decompose path report malformed off-curve
  sequences.

Remaining:

- Raw `FT_Outline` records with nonzero counts and null internal pointers are
  still not counted as C parity. The pinned C `FT_Outline_Decompose` probe for
  those shapes can segfault instead of returning a public `FT_Error`, so those
  cases remain facade/oracle-safety work under the raw invalid-pointer issue
  set. Do not fold them into `FT_Outline_Get_BBox` real parity without a
  same-input C oracle result.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftbbox.FT_Outline_Get_BBox
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_Stroker` success/lifecycle route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `c5775ac82`: `real-parity=4465`,
  `generic-fallback=494`, `pending-route=20`, `pending-core=7`.

Finding:

- The route audit already has exact real parity for selected `ftstroke`
  invalid-argument/error rows and for
  `FT_Outline_GetInsideBorder` / `FT_Outline_GetOutsideBorder`.
- The remaining `FT_Stroker` object/path success, no-op, lifecycle, export,
  count, cap, join, and glyph-stroking rows had stayed in `generic-fallback`
  with the reason `no explicit maintained route classification`.
- Those rows are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  stroker object/path route that constructs a real `FT_Stroker`, applies
  `FT_Stroker_Set`, `FT_Stroker_BeginSubPath`, `FT_Stroker_LineTo`,
  `FT_Stroker_ConicTo`, `FT_Stroker_CubicTo`, `FT_Stroker_EndSubPath`,
  `FT_Stroker_ParseOutline`, exports borders, and compares the produced
  outline geometry and counts exactly.

Classification change:

- 65 `ftstroke.*` rows moved from `generic-fallback` to `pending-route`.
- 21 existing exact `ftstroke` rows remain `real-parity`.
- New route audit counts: `real-parity=4465`, `generic-fallback=429`,
  `pending-route=85`, `pending-core=7`.

Current route-audit breakdown:

- Current route audit has 60 `ftstroke` pending rows in this bucket. Later
  exact-error and null/no-op promotions reduced the remaining set from the
  original 65-row classification batch; do not use the old count as the
  implementation target.
- The route classifier now names these rows through explicit case sets grouped
  by behavior surface. Future `ftstroke.*` rows must add a concrete blocker or
  a real parity route; they are not hidden by a wildcard subsystem reason.
- The blocker families are allocation/lifecycle, Set/Rewind state,
  BeginSubPath/LineTo path state, conic/cubic curves, line caps, line joins,
  EndSubPath/ParseOutline, counts, border/export behavior,
  Glyph_Stroke/Glyph_StrokeBorder ownership, and non-null Done cleanup.

| Operation | Count | Pending case IDs |
| --- | ---: | --- |
| `ftstroke.begin_subpath` | 2 | `ftstroke.FT_Stroker_BeginSubPath.closed_subpath_initial_state`<br>`ftstroke.FT_Stroker_BeginSubPath.open_subpath_initial_state` |
| `ftstroke.begin_subpath_wide_stroke` | 1 | `ftstroke.FT_Stroker_BeginSubPath.wide_stroke_mode_depends_on_cap_and_join` |
| `ftstroke.conic_to` | 3 | `ftstroke.FT_Stroker_ConicTo.coincident_control_and_end_noop`<br>`ftstroke.FT_Stroker_ConicTo.conic_curve_success`<br>`ftstroke.FT_Stroker_ConicTo.first_segment_starts_subpath` |
| `ftstroke.cubic_to` | 3 | `ftstroke.FT_Stroker_CubicTo.coincident_controls_and_end_noop`<br>`ftstroke.FT_Stroker_CubicTo.cubic_curve_success`<br>`ftstroke.FT_Stroker_CubicTo.first_segment_starts_subpath` |
| `ftstroke.end_subpath` | 2 | `ftstroke.FT_Stroker_EndSubPath.closed_subpath_closes_two_borders`<br>`ftstroke.FT_Stroker_EndSubPath.open_subpath_emits_caps_and_single_border` |
| `ftstroke.export` | 3 | `ftstroke.FT_Stroker_Export.append_to_existing_outline`<br>`ftstroke.FT_Stroker_Export.exports_left_then_right`<br>`ftstroke.FT_Stroker_Export.invalid_inputs_noop` |
| `ftstroke.export_border` | 7 | `ftstroke.FT_STROKER_BORDER_LEFT.left_border_export_geometry`<br>`ftstroke.FT_STROKER_BORDER_RIGHT.right_border_export_geometry`<br>`ftstroke.FT_StrokerBorder.border_selection_runtime_shape`<br>`ftstroke.FT_Stroker_ExportBorder.append_to_existing_outline`<br>`ftstroke.FT_Stroker_ExportBorder.invalid_inputs_or_border_noop`<br>`ftstroke.FT_Stroker_ExportBorder.open_path_right_border_empty`<br>`ftstroke.FT_Stroker_ExportBorder.valid_left_and_right_export` |
| `ftstroke.get_border_counts` | 3 | `ftstroke.FT_Stroker_GetBorderCounts.closed_path_border_counts`<br>`ftstroke.FT_Stroker_GetBorderCounts.open_path_single_border_counts`<br>`ftstroke.FT_Stroker_GetBorderCounts.optional_output_pointers` |
| `ftstroke.get_counts` | 3 | `ftstroke.FT_Stroker_GetCounts.combined_closed_path_counts`<br>`ftstroke.FT_Stroker_GetCounts.combined_open_path_counts`<br>`ftstroke.FT_Stroker_GetCounts.optional_output_pointers` |
| `ftstroke.glyph_stroke` | 2 | `ftstroke.FT_Glyph_Stroke.destroy_original_option`<br>`ftstroke.FT_Glyph_Stroke.outline_glyph_stroked_success` |
| `ftstroke.glyph_stroke_border` | 3 | `ftstroke.FT_Glyph_StrokeBorder.destroy_original_option`<br>`ftstroke.FT_Glyph_StrokeBorder.inside_border_success`<br>`ftstroke.FT_Glyph_StrokeBorder.outside_border_success` |
| `ftstroke.join_geometry` | 4 | `ftstroke.FT_STROKER_LINEJOIN_BEVEL.bevel_join_geometry`<br>`ftstroke.FT_STROKER_LINEJOIN_MITER_FIXED.fixed_miter_limit_geometry`<br>`ftstroke.FT_STROKER_LINEJOIN_MITER_VARIABLE.variable_miter_limit_geometry`<br>`ftstroke.FT_Stroker_LineJoin.join_geometry_and_miter_limit` |
| `ftstroke.join_geometry_alias` | 1 | `ftstroke.FT_STROKER_LINEJOIN_MITER.alias_matches_variable_join_geometry` |
| `ftstroke.line_to` | 3 | `ftstroke.FT_Stroker_LineTo.first_segment_starts_subpath`<br>`ftstroke.FT_Stroker_LineTo.line_segment_success`<br>`ftstroke.FT_Stroker_LineTo.zero_length_line_noop` |
| `ftstroke.open_path_geometry` | 4 | `ftstroke.FT_STROKER_LINECAP_BUTT.butt_cap_open_line_geometry`<br>`ftstroke.FT_STROKER_LINECAP_ROUND.round_cap_open_line_geometry`<br>`ftstroke.FT_STROKER_LINECAP_SQUARE.square_cap_open_line_geometry`<br>`ftstroke.FT_Stroker_LineCap.open_path_cap_geometry` |
| `ftstroke.parse_outline` | 4 | `ftstroke.FT_Stroker_EndSubPath.no_segment_after_begin`<br>`ftstroke.FT_Stroker_ParseOutline.degenerate_contours_skipped`<br>`ftstroke.FT_Stroker_ParseOutline.line_conic_cubic_success`<br>`ftstroke.FT_Stroker_ParseOutline.opened_outline_success` |
| `ftstroke.rewind` | 2 | `ftstroke.FT_Stroker_Rewind.attributes_preserved`<br>`ftstroke.FT_Stroker_Rewind.clears_previous_path` |
| `ftstroke.set` | 3 | `ftstroke.FT_Stroker_Set.attributes_affect_geometry`<br>`ftstroke.FT_Stroker_Set.clears_existing_path`<br>`ftstroke.FT_Stroker_Set.miter_limit_clamped_to_one` |
| `ftstroke.set_then_rewind_observed` | 1 | `ftstroke.FT_Stroker_Rewind.set_calls_rewind` |
| `ftstroke.stroke_manual_path` | 1 | `ftstroke.FT_STROKER_LINEJOIN_ROUND.round_join_geometry` |
| `ftstroke.stroke_wide_curve` | 1 | `ftstroke.FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration` |
| `ftstroke.stroker_done` | 1 | `ftstroke.FT_Stroker_Done.valid_stroker_releases_buffers` |
| `ftstroke.stroker_done_after_export` | 1 | `ftstroke.FT_Stroker_Done.after_export_cleanup` |
| `ftstroke.stroker_lifecycle` | 1 | `ftstroke.FT_Stroker.lifecycle_contract` |
| `ftstroke.stroker_new` | 1 | `ftstroke.FT_Stroker_New.valid_library_allocates_stroker` |

2026-07-21 export/export-border blocker split:

- Export and export-border rows are split by exact obligation instead of
  sharing a broad border/export blocker:
  - `FT_STROKER_BORDER_LEFT.left_border_export_geometry`: left-border outline
    points, tags, contours, and orientation.
  - `FT_STROKER_BORDER_RIGHT.right_border_export_geometry`: right-border
    outline points, tags, contours, and orientation.
  - `FT_StrokerBorder.border_selection_runtime_shape`: public border enum
    values selecting the same left/right border geometry and output shape as
    pinned C.
  - `FT_Stroker_Export.exports_left_then_right`: combined export appending left
    then right border geometry in pinned-C point/tag/contour order.
  - `FT_Stroker_Export.append_to_existing_outline`: combined export appending
    to existing outline contents with exact point, tag, contour, and
    contour-index offsets.
  - `FT_Stroker_Export.invalid_inputs_noop`: null stroker or null outline
    inputs preserving the existing outline and return/no-op behavior.
  - `FT_Stroker_ExportBorder.valid_left_and_right_export`: separate left and
    right border exports producing exact pinned-C outline geometry.
  - `FT_Stroker_ExportBorder.open_path_right_border_empty`: right border of an
    open path being empty or preserved exactly like pinned C.
  - `FT_Stroker_ExportBorder.invalid_inputs_or_border_noop`: invalid border
    values, null stroker, or null outline preserving output and no-op behavior.
  - `FT_Stroker_ExportBorder.append_to_existing_outline`: border export
    appending to existing outline contents with exact contour-index offsets.

2026-07-21 conic/cubic curve blocker split:

- Conic, cubic, and wide-curve stroker rows are split by exact obligation
  instead of sharing a broad curve blocker:
  - `FT_Stroker_ConicTo.conic_curve_success`: quadratic curve subdivision and
    generated border points, tags, and contours matching pinned C.
  - `FT_Stroker_ConicTo.first_segment_starts_subpath`: a conic segment
    initializing an otherwise empty subpath with the same border state and
    output geometry.
  - `FT_Stroker_ConicTo.coincident_control_and_end_noop`: a control point equal
    to the current point and end point preserving state or no-oping exactly
    like pinned C.
  - `FT_Stroker_CubicTo.cubic_curve_success`: cubic curve subdivision and
    generated border points, tags, and contours matching pinned C.
  - `FT_Stroker_CubicTo.first_segment_starts_subpath`: a cubic segment
    initializing an otherwise empty subpath with the same border state and
    output geometry.
  - `FT_Stroker_CubicTo.coincident_controls_and_end_noop`: both controls and
    end point equal to the current point preserving state or no-oping exactly
    like pinned C.
  - `FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration`: FreeType's
    wide-curve join restoration emitting the same round-join geometry after
    curve subdivision.

2026-07-21 line-cap blocker split:

- Open-path line-cap rows are split by exact obligation instead of sharing a
  broad cap-geometry blocker:
  - `FT_STROKER_LINECAP_BUTT.butt_cap_open_line_geometry`: butt caps
    terminating at the endpoint with exact border geometry, tags, and contours.
  - `FT_STROKER_LINECAP_ROUND.round_cap_open_line_geometry`: round cap arc
    subdivision and emitted outline geometry matching pinned C.
  - `FT_STROKER_LINECAP_SQUARE.square_cap_open_line_geometry`: square cap
    endpoint extension and emitted outline geometry matching pinned C.
  - `FT_Stroker_LineCap.open_path_cap_geometry`: public cap enum selection for
    butt, round, and square geometry exactly matching pinned C.

2026-07-21 path-construction and line-join blocker split:

- `BeginSubPath`, `LineTo`, and line-join rows are split by exact obligation
  instead of sharing broad path-construction and join-geometry blockers:
  - `FT_Stroker_BeginSubPath.closed_subpath_initial_state`: opened flag, first
    point, and left/right border initial state for a closed path.
  - `FT_Stroker_BeginSubPath.open_subpath_initial_state`: opened flag, first
    point, and cap-dependent border initial state for an open path.
  - `FT_Stroker_BeginSubPath.wide_stroke_mode_depends_on_cap_and_join`:
    FreeType wide-stroke setup selected from cap and join attributes.
  - `FT_Stroker_LineTo.line_segment_success`: line joins, emitted border
    points, tags, contours, and current point advancement.
  - `FT_Stroker_LineTo.first_segment_starts_subpath`: a line segment
    initializing an otherwise empty subpath with the same border state and
    output geometry.
  - `FT_Stroker_LineTo.zero_length_line_noop`: a line to the current point
    preserving state or no-oping exactly like pinned C.
  - `FT_STROKER_LINEJOIN_BEVEL.bevel_join_geometry`: bevel join points, tags,
    contours, and cutover behavior.
  - `FT_STROKER_LINEJOIN_MITER.alias_matches_variable_join_geometry`: public
    miter alias selecting variable-miter geometry.
  - `FT_STROKER_LINEJOIN_MITER_FIXED.fixed_miter_limit_geometry`: fixed-miter
    intersection, miter-limit fallback, and output geometry.
  - `FT_STROKER_LINEJOIN_MITER_VARIABLE.variable_miter_limit_geometry`:
    variable-miter intersection, miter-limit fallback, and output geometry.
  - `FT_STROKER_LINEJOIN_ROUND.round_join_geometry`: round join arc
    subdivision and emitted points, tags, and contours.
  - `FT_Stroker_LineJoin.join_geometry_and_miter_limit`: public join enum
    values and miter-limit inputs selecting the same output geometry.

2026-07-21 end-subpath, parse-outline, and count blocker split:

- `EndSubPath`, `ParseOutline`, `GetBorderCounts`, and `GetCounts` rows are
  split by exact obligation instead of sharing broad outline/finalization and
  count blockers:
  - `FT_Stroker_EndSubPath.closed_subpath_closes_two_borders`: close emission
    joining left/right borders and preserving contour order.
  - `FT_Stroker_EndSubPath.open_subpath_emits_caps_and_single_border`: cap
    emission and single-border finalization for open paths.
  - `FT_Stroker_EndSubPath.no_segment_after_begin`: ending immediately after
    `BeginSubPath` preserving state or no-oping exactly like pinned C.
  - `FT_Stroker_ParseOutline.line_conic_cubic_success`: line, conic, and cubic
    contour decomposition feeding the stroker and emitting exact geometry.
  - `FT_Stroker_ParseOutline.opened_outline_success`: opened-outline flag
    selecting cap/finalization behavior.
  - `FT_Stroker_ParseOutline.degenerate_contours_skipped`: zero-length or
    malformed contours being skipped or preserved exactly like pinned C.
  - `FT_Stroker_GetBorderCounts.closed_path_border_counts`: left/right point
    and contour counts after closing a path.
  - `FT_Stroker_GetBorderCounts.open_path_single_border_counts`: open-path
    single-border or empty-border counts.
  - `FT_Stroker_GetBorderCounts.optional_output_pointers`: null output
    pointers preserved while non-null outputs receive exact counts.
  - `FT_Stroker_GetCounts.combined_closed_path_counts`: combined left/right
    point and contour totals for closed paths.
  - `FT_Stroker_GetCounts.combined_open_path_counts`: combined open-path totals
    including empty-border handling.
  - `FT_Stroker_GetCounts.optional_output_pointers`: null output pointers
    preserved while non-null outputs receive exact combined counts.

2026-07-21 set/rewind and glyph-stroke blocker split:

- `Set`, `Rewind`, `Glyph_Stroke`, and `Glyph_StrokeBorder` rows are split by
  exact obligation instead of sharing broad state and glyph-object blockers:
  - `FT_Stroker_Set.attributes_affect_geometry`: radius, line cap, line join,
    and miter-limit fields changing later stroke geometry exactly like pinned C.
  - `FT_Stroker_Set.miter_limit_clamped_to_one`: values below one clamped
    before miter fallback decisions.
  - `FT_Stroker_Set.clears_existing_path`: resetting attributes also clearing
    prior border/path state.
  - `FT_Stroker_Rewind.clears_previous_path`: rewind clearing previous
    border/path state.
  - `FT_Stroker_Rewind.attributes_preserved`: radius, cap, join, and
    miter-limit attributes preserved while path state is cleared.
  - `FT_Stroker_Rewind.set_calls_rewind`: `Set` performing the same implicit
    rewind/path clear sequence as pinned C.
  - `FT_Glyph_Stroke.outline_glyph_stroked_success`: an outline glyph stroked
    into the same output glyph format, outline points, tags, contours, and
    ownership state.
  - `FT_Glyph_Stroke.destroy_original_option`: `destroy=0` preserving the
    input glyph and `destroy=1` releasing or replacing it exactly like pinned C.
  - `FT_Glyph_StrokeBorder.outside_border_success`: outside-border stroking
    emitting the same outline geometry and ownership result.
  - `FT_Glyph_StrokeBorder.inside_border_success`: inside-border stroking
    emitting the same outline geometry and ownership result.
  - `FT_Glyph_StrokeBorder.destroy_original_option`: `destroy=0` preserving
    the input glyph and `destroy=1` releasing or replacing it exactly like
    pinned C.

2026-07-20 null-stroker no-op carve-out:

- `FT_Stroker_Set(NULL, ...)`, `FT_Stroker_Rewind(NULL)`, and
  `FT_Stroker_Done(NULL)` are exact pinned-C no-ops in FreeType 2.14.3
  `src/base/ftstroke.c`. They do not allocate, free, or touch border state.
- The maintained route for these three rows runs pinned C, Rust FFI, thin C
  ABI, and WASM ABI with a null stroker and compares
  `{"crash": false, "allocator_calls": "none"}`.
- This is not a full `FT_Stroker` implementation. Non-null stroker allocation,
  attribute mutation, rewind semantics, path building, export/count geometry,
  and glyph-stroking rows remain `pending-route`.

Required fix plan:

1. Add a maintained stroker route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement the pure-Rust stroker object/path state first. The C and WASM ABI
   layers may only own handle validation, record copying, and lifetime
   bookkeeping.
3. Compare exact output counts and exported outline geometry for line, conic,
   cubic, open-path, closed-path, cap, join, miter-limit, rewind, done, and
   append-to-existing-outline rows.
4. Keep the already-routed invalid-argument rows and outline-border rows real;
   do not demote them while building the success route.
5. Promote rows only after the focused `ftstroke` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: core stream/SVG/parameter runtime route blockers

Status: focused route probes classified on 2026-07-20.

Finding:

- `freetype.FT_FACE_FLAG_EXTERNAL_STREAM.open_face_stream_ownership` uses the
  maintained `input/fonts/DejaVuSans.ttf` font asset, but it is not real runtime
  parity yet.  It needs a maintained `FT_Open_Face` route using
  `FT_OPEN_STREAM` that observes caller-owned stream identity, close-callback
  behavior, and the `FT_FACE_FLAG_EXTERNAL_STREAM` bit through pinned C,
  Rust FFI, C ABI, and WASM.  The existing constant-value flag route proves only
  the macro value; reusing it as runtime ownership parity would be a green
  placeholder.
- `freetype.FT_LOAD_SVG_ONLY.svg_only_behavior` declares
  `fonts/svg/color-svg-glyph.ttf`, but that maintained OT-SVG fixture is absent.
  The future route must load the same SVG and non-SVG glyphs with
  `FT_LOAD_SVG_ONLY` and compare pinned C
  `freetype/include/freetype/freetype.h:3501-3636`,
  `freetype/src/base/ftobjs.c:943-1177`, and
  `freetype/src/truetype/ttgload.c:2485-2537` behavior against Rust FFI,
  C ABI, and WASM.
- `freetype.FT_Parameter.tag_data_parameters_match_c_behavior` declares
  `fonts/color/sbix-outline.ttf` for `FT_PARAM_TAG_IGNORE_SBIX`; that maintained
  sbix fixture is absent.  The row also needs a maintained `FT_Open_Face` with
  `FT_OPEN_PARAMS` route that compares known-tag, unknown-tag, null-data, and
  null-params behavior across pinned C, Rust FFI, C ABI, and WASM.

Rejected diagnostic path:

- Do not promote these rows through the existing constant/layout checks.  The
  runtime rows require observable ownership, glyph-load, or parameter-dispatch
  behavior for the same inputs.
- Do not replace missing SVG/sbix fixtures with generic fonts; that would test a
  different public input.

Required fix plan:

1. Add maintained core runtime routes for external-stream face opening,
   SVG-only glyph loading, and `FT_Parameter` dispatch.  Each route must run the
   same inputs through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Add or generate the missing OT-SVG and sbix fixtures before promoting the SVG
   and parameter rows.
3. Compare exact face flags, stream callback/ownership events, glyph slot format
   and public error behavior, parameter tag/data nullness, and observable
   parameter effects.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_FACE_FLAG_EXTERNAL_STREAM.open_face_stream_ownership
make -C pillow-rs-freetype test-case CASE=freetype.FT_LOAD_SVG_ONLY.svg_only_behavior
make -C pillow-rs-freetype test-case CASE=freetype.FT_Parameter.tag_data_parameters_match_c_behavior
```

### Issue Set Current: `TT_MaxProfile` malformed maxp fixture blocker

Status: classified as explicit pending-route on 2026-07-20.

Finding:

- `tttables.TT_MaxProfile.malformed_table_error_source` is intended to compare
  the pinned C face-load error or C-adjusted parsed `TT_MaxProfile` state for
  malformed `maxp` tables.
- The declared assets resolve, but they are not malformed maxp fixtures:
  `tests/fixtures/input/fonts/sfnt/truncated-maxp.ttf` and
  `tests/fixtures/input/fonts/sfnt/invalid-maxp.ttf` are symlinks to
  `../DejaVuSans.ttf`.
- Treating that row as runtime parity would compare a normal DejaVuSans face
  and would not prove the declared `ttload.c:785-835` malformed-table behavior.

Classification change:

- The row remains `pending-route`, but the route-audit reason now names the
  exact fixture blocker instead of the generic residual public-surface bucket.
- The stale residual mention of
  `ftcid.FT_Get_CID_Registry_Ordering_Supplement.public_header_signature` was
  removed from the residual list; current route audit already classifies that
  row as a compile contract.

Required fix plan:

1. Add or generate real malformed SFNT fixtures for truncated and invalid
   `maxp` tables under the maintained fixture workflow.
2. Add a maintained `face.load_then_get_sfnt_table.maxp` route that opens each
   malformed face through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
3. Compare exact face-load status, `FT_Get_Sfnt_Table(FT_SFNT_MAXP)` pointer
   nullness, and any C-adjusted `TT_MaxProfile` fields when FreeType keeps the
   face open.
4. Promote the row only after the focused case is runnable and proves exact
   same-input parity across all four lanes.

Verification for this audit-only clarification:

```bash
make -C pillow-rs-freetype test-case CASE=tttables.TT_MaxProfile.malformed_table_error_source
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_VALIDATE_BASE` absent-table expectation mismatch

Status: classified as explicit pending-route on 2026-07-20.

Finding:

- `ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output` declares an
  `ok` expectation for `FT_OpenType_Validate` on DejaVuSans with
  `FT_VALIDATE_BASE`, expecting `BASE_table` to become null when the table is
  absent.
- FreeType source has an absent-table success path in
  `src/otvalid/otvmod.c:41-56,105-149,209-213` when the OpenType validation
  service is present.
- The pinned oracle build used by this harness returns
  `FT_Err_Unimplemented_Feature` (`7`) before the absent-table path is
  observable. A focused attempted maintained route failed with:
  `oracle returned unexpected error 7`.
- Rust FFI’s current `FT_OpenType_Validate` also returns
  `FT_Err_Unimplemented_Feature` for non-null faces, which matches this pinned
  oracle build. Promoting the fixture’s declared `ok/null` expectation would be
  a green placeholder.

Classification change:

- The row remains `pending-route`, but the route-audit reason now names the
  exact expectation/oracle-build mismatch instead of the generic residual
  public-surface bucket.

Required fix plan:

1. Decide whether the maintained oracle contract should include the `otvalid`
   module for OpenType validation success/absence routes.
2. If yes, make the oracle build contract explicit and then implement the
   pure-Rust absent-table and present-table validation behavior needed for
   same-input Rust FFI, C ABI, and WASM ABI parity.
3. If no, update the fixture expectation to exact
   `FT_Err_Unimplemented_Feature` for this pinned build, without treating it as
   a success/null-output route.
4. Promote only after focused runtime compares the same input across pinned C,
   Rust FFI, C ABI, and WASM ABI with no oracle error mismatch.

Verification for this clarification:

```bash
make -C pillow-rs-freetype test-case CASE=ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: remove placeholder-style validation categories

Status: ftsynth null-slot rows promoted to real parity on 2026-07-20; one
wrapper-null row remains pending.

Finding:

- The route audit still reported `green_placeholder_style_rows=5` even though
  those rows were not counted as `real-parity`.
- The five rows were split into side categories:
  `raw-slot-null-validation=4` and `wrapper-null-validation=1`.
- Those categories validated useful partial behavior, but they did not prove
  full same-input C/Rust/C-ABI/WASM parity at the time:
  - `ftsynth.FT_GlyphSlot_AdjustWeight.null_slot_noop`
  - `ftsynth.FT_GlyphSlot_Embolden.null_slot_noop`
  - `ftsynth.FT_GlyphSlot_Oblique.null_slot_noop`
  - `ftsynth.FT_GlyphSlot_Slant.null_slot_noop`
  - `freetype.FT_Get_SubGlyph_Info.error_null_outputs`

Classification change:

- The five rows initially classified as `pending-route` with explicit reasons
  instead of side validation categories.
- The generated audit now reports `green_placeholder_style_rows=0`.
- Route audit counts changed from:
  `real-parity=4532`, `pending-route=424`,
  `raw-slot-null-validation=4`, `wrapper-null-validation=1`
  to `real-parity=4532`, `pending-route=429`.
- The four ftsynth null-slot rows were then promoted to `real-parity` after the
  WASM ABI gained an explicit null-handle no-op route and focused runtime
  comparison proved matching pinned C, Rust FFI, C ABI, and WASM output.
- `freetype.FT_Get_SubGlyph_Info.error_null_outputs` remains pending because
  native C dereferences valid-slot output pointers and still needs a same-input
  public route.

Required fix plan:

1. For `FT_Get_SubGlyph_Info.error_null_outputs`, add a same-input public C
   oracle route for valid composite slot setup plus null-output handling that
   is comparable across Rust FFI, C ABI, and WASM. Do not count wrapper-only
   null policy as full parity.
2. Keep the ftsynth null-slot rows real only while focused runtime continues to
   prove exact C/Rust/C-ABI/WASM output for handle/null-slot no-op behavior.

Verified command:

```bash
make -C pillow-rs-freetype test-case CASE=ftsynth.FT_GlyphSlot_AdjustWeight.null_slot_noop
make -C pillow-rs-freetype test-case CASE=ftsynth.FT_GlyphSlot_Embolden.null_slot_noop
make -C pillow-rs-freetype test-case CASE=ftsynth.FT_GlyphSlot_Oblique.null_slot_noop
make -C pillow-rs-freetype test-case CASE=ftsynth.FT_GlyphSlot_Slant.null_slot_noop
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_OpenType_Validate` unresolved table fixtures

Status: demoted from real-parity to explicit pending-route on 2026-07-20.

Finding:

- Several `ftotval.open_type_validate` table-selection and malformed/error rows
  were classified as `real-parity` even though focused runtime execution
  reported `runnable=0 pending=1` with an unresolved declared font asset.
- The missing fixtures are:
  - `fonts/opentype/valid-base.otf`
  - `fonts/opentype/valid-gdef.otf`
  - `fonts/opentype/valid-gpos.otf`
  - `fonts/opentype/valid-gsub.otf`
  - `fonts/opentype/valid-jstf.otf`
  - `fonts/opentype/valid-math.otf`
  - `fonts/opentype/valid-all-layout.otf`
  - `fonts/opentype/malformed-selected-layout.otf`
  - `fonts/opentype/malformed-gdef.otf`
  - `fonts/opentype/malformed-gpos.otf`
  - `fonts/opentype/malformed-gsub.otf`
  - `fonts/opentype/malformed-jstf.otf`
  - `fonts/opentype/malformed-math.otf`
  - `fonts/opentype/partial-malformed-layout.otf`
- Marking these rows real would be a green placeholder: there is no same input
  to feed through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.
- The null face/output and missing-service OpenType validation rows are
  separate: they remain real because they execute without the missing table
  fixtures.

Required fix plan:

1. Add maintained fixture generators or checked-in fixtures for each declared
   OpenType validation input. The generator must be reproducible from a clean
   checkout and version-locked to pinned FreeType 2.14.3 oracle behavior.
2. Keep `FT_OpenType_Validate` null face/output and service-missing rows real
   only where they already execute against existing inputs.
3. Promote table-selection or malformed/error rows only after focused runtime
   proves `runnable>0` and exact C oracle, Rust FFI, C ABI, and WASM ABI output
   match for the declared fixture.
4. Do not substitute DejaVuSans, generic `Unimplemented_Feature`, or any
   shared Rust fallback for missing OpenType validation fixtures.

Verification for this classification:

```bash
make -C pillow-rs-freetype test-case CASE=ftotval.FT_VALIDATE_GDEF.validate_selects_gdef_table
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FT_OpenType_Free` non-null validation buffer route

Status: classified as explicit pending-route on 2026-07-20.

Finding:

- `ftotval.FT_OpenType_Free.null_face_noop` and
  `ftotval.FT_OpenType_Free.null_table_noop` are already real null-validation
  rows across pinned C oracle, Rust FFI, C ABI, and WASM ABI.
- `ftotval.FT_OpenType_Free.frees_validated_table_with_face_memory` is a
  different public behavior. It requires `FT_OpenType_Validate` to return a
  non-null table buffer allocated from `FT_FACE_MEMORY(face)`, then
  `FT_OpenType_Free(face, table)` must release that exact buffer.
- The current pure-Rust `FT_OpenType_Validate` implementation still returns
  `FT_Err_Unimplemented_Feature` for non-null validation calls, and
  `FT_OpenType_Free` is a no-op. Promoting this row through the existing null
  no-op route would not prove ownership or freeing behavior and would be a
  green placeholder.

Required fix plan:

1. Implement non-null OpenType validation table output in core Rust first for
   the selected table fixture, preserving exact pinned-C errors and pointer
   nullness for BASE/GDEF/GPOS/GSUB/JSTF outputs.
2. Add owned validation-buffer state tied to the face memory model so
   `FT_OpenType_Free(face, table)` can release a real tracked allocation while
   retaining the already-real null face/table no-op behavior.
3. Expose only thin C ABI and WASM ABI wrappers for the core behavior; wrappers
   must not synthesize validation buffers or fake allocator events.
4. Promote `ftotval.FT_OpenType_Free.frees_validated_table_with_face_memory`
   only after pinned C oracle, Rust FFI, C ABI, and WASM ABI all compare the
   same validate error, non-null table pointer class, free event count, and
   allocator identity.

Verification for this classification:

```bash
make -C pillow-rs-freetype test-case CASE=ftotval.FT_OpenType_Free.frees_validated_table_with_face_memory
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: focused route probes rejected as non-runnable

Status: focused probe batch on 2026-07-20.

Baseline:

- Route audit before and after the probes stayed at `real-parity=4532` and
  `pending-route=424`.

Finding:

- `ftdriver.FT_Prop_IncreaseXHeight.property_set_get_round_trips_limit` is not
  a scalar TrueType `interpreter-version` property row.  The current maintained
  property route only handles scalar `FT_UInt` property values; this row needs
  the typed `FT_Prop_IncreaseXHeight { face, limit }` `void*` dispatch used by
  pinned FreeType `src/autofit/afmodule.c:172-187,326-336`, plus face-handle
  resolution through Rust FFI, thin C ABI, and WASM.  A strict diagnostic probe
  that added this case to the scalar allow-list still stopped before runtime
  execution, proving the blocker is the broader property route, not just a
  missing classification string.
- `ftstroke.FT_Stroker_LineTo.line_segment_success` is not covered by the
  existing stroker null/no-op route.  It requires maintained `FT_Stroker_New`,
  `FT_Stroker_BeginSubPath`, `FT_Stroker_LineTo`, `FT_Stroker_GetCounts`, and
  export/cbox state across Rust FFI, C ABI, and WASM.  The declared path asset
  `outlines/stroker/manual-paths.json` is still a required future asset.
- `ftgzip.FT_Gzip_Uncompress.uncompresses_valid_gzip_buffer` is not covered by
  the existing gzip exact-error route.  Its public input manifest declares
  `compressed/gzip/small-text-and-empty-payloads.json`, which is absent from
  the maintained fixture tree; related gzip rows also reference missing
  deterministic byte fixtures such as `compressed/gzip/small-valid.gz` and
  `compressed/gzip/invalid-and-truncated-payloads.json`.  The success row
  needs those maintained compressed payload fixtures plus exact byte-output
  comparison against pinned C `freetype/include/freetype/ftgzip.h:98-137` and
  `freetype/src/gzip/ftgzip.c:711-771`.  No pure-Rust gzip/zlib success route
  is currently present in `pillow-rs-freetype`, so routing it to a generic
  success or `Unimplemented_Feature` result would be a green placeholder.

Rejected diagnostic path:

- Do not promote `FT_Prop_IncreaseXHeight` by treating its `void*` value as an
  `FT_UInt*`; pinned C uses a face-specific record and mutates
  `AF_FaceGlobals.increase_x_height`.
- Do not promote stroker success rows through the current null/no-op stroker
  functions; they do not allocate or retain a real stroker object.
- Do not promote gzip success rows until deterministic compressed input
  fixtures and exact decompression output behavior exist.

Required fix plan:

1. For driver properties, add typed property dispatch in core first:
   `interpreter-version` stays scalar, while `increase-x-height` and
   `glyph-to-script-map` must consume/produce their public records and resolve
   live face handles.  Then add thin C/WASM wrappers and focused oracle routes.
2. For stroker, implement real pure-Rust `FT_Stroker` object/path state and
   export geometry.  Only then expose allocation/lifetime/copying through C
   and WASM.
3. For gzip, add deterministic fixture generation for compressed payloads and
   implement pure-Rust decompression/stream behavior, then compare exact output
   bytes and stream fields against pinned C.

Verified commands:

```bash
make -C pillow-rs-freetype test-case CASE=ftdriver.FT_Prop_IncreaseXHeight.property_set_get_round_trips_limit
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_LineTo.line_segment_success
make -C pillow-rs-freetype test-case CASE=ftgzip.FT_Gzip_Uncompress.uncompresses_valid_gzip_buffer
```

### Issue Set Current: `FT_Get_Track_Kerning` Type1/AFM success route

Status: blocked on required future assets on 2026-07-20.

Finding:

- `freetype.FT_Get_Track_Kerning.type1_afm_track_kerning_success` is the only
  remaining pending `FT_Get_Track_Kerning` row.  The null-face/null-output and
  SFNT/no-track-data error rows are already exact real parity.
- The success row declares `input/fonts/type1/track-kern-base.pfb` and
  `input/aux/type1/track-kern-base.afm` as `required_future_asset`.  The row
  needs a Type1 PFA/PFB face plus attached AFM track-kerning data for degree
  `-1`, `0`, and `1` across several 16.16 point sizes.
- Focused runtime parity therefore has no runnable same-input success case:
  it reports `runnable=0 pending=1` with the maintained route reason for core
  attach/open-face/track-kerning success behavior.  Promoting this row without
  those assets and an exact C oracle observation would be a green placeholder.

Required fix plan:

1. Add or normalize a maintained C-openable Type1 fixture plus matching AFM
   attachment that contains deterministic track-kerning data for negative,
   zero, and positive degrees.
2. Run pinned FreeType 2.14.3 first to record exact `FT_Get_Track_Kerning`
   return codes and `akerning` values for every declared point-size/degree
   pair after `FT_Attach_File`.
3. Implement any missing pure-Rust Type1/AFM track-kerning behavior in core.
   The C and WASM ABI layers must only expose the core result and must not
   embed fixture-specific values.
4. Promote the row only after focused runtime proves exact output through
   Rust FFI, thin C ABI, and WASM ABI for the same Type1/AFM input pair.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=freetype.FT_Get_Track_Kerning.type1_afm_track_kerning_success
```

### Issue Set Follow-up: `FT_FACE_DRIVER_NAME` TrueType driver-name route

Status: one C-openable row promoted to real parity on 2026-07-20.

Implemented:

- Added core `FT_FACE_DRIVER_NAME` macro-equivalent behavior for supported
  face drivers.  The current C-openable row validates that a TrueType face
  returns the driver module class name `truetype`, not the font-format service
  string `TrueType`.
- Added C ABI and WASM ABI test-support helpers only.  No public C export or
  public WASM export was added because `FT_FACE_DRIVER_NAME` is a C macro and
  the thin-wrapper export gate must remain strict.
- Added a pinned C oracle route and unified harness comparison for
  `ftmodapi.FT_FACE_DRIVER_NAME.returns_driver_module_name`.

Route audit impact:

- `real-parity=4522 -> 4523`
- `pending-route=434 -> 433`

Follow-up:

- `ftmodapi.FT_FACE_DRIVER_NAME.driver_name_not_font_format` now uses the
  maintained `fonts/cff/pure-cff-cubic.otf` fixture generated by
  `scripts/build_cff_fixtures.py` instead of the stale future asset reference.
- The row compares the driver module class name from `FT_FACE_DRIVER_NAME`
  against the independent `FT_Get_Font_Format` service string for the same CFF
  face through pinned C, Rust FFI, C ABI, and WASM ABI.  This proves the macro
  route is not inferred from the font-format string.

Follow-up route audit impact:

- `real-parity=4523 -> 4524`
- `pending-route=433 -> 432`

Verification for this follow-up:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_FACE_DRIVER_NAME.returns_driver_module_name
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_FACE_DRIVER_NAME.driver_name_not_font_format
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: route-audit snapshot after FTMM and size-record promotions

Status: current planning snapshot recorded on 2026-07-20 at `2eb49c8a8`.

Current route-audit ledger:

- `concrete_cases=7234`
- `real-parity=4519`
- `pending-route=437`
- `compile-contract=2265`
- `real-null-validation=8`
- `raw-slot-null-validation=4`
- `wrapper-null-validation=1`

Largest remaining pending-route surfaces:

- COLR/CPAL paint graph and palette success routes: 107 rows.
- FTC cache manager/image/cmap/sbit/node lifecycle routes: 90 rows.
- Stroker success, geometry, export, and lifecycle routes: 60 rows.
- Type1/MM table and private dictionary routes: 27 rows.
- GX validation and classic kern runtime routes: 16 rows.
- Image/raster callback and parameter routes: 16 rows.
- Incremental font callback routes: 14 rows.
- Module/library lifecycle and dynamic module routes: 12 rows.
- Stream wrapper routes: 11 rows.
- Glyph-object ownership and SVG glyph routes: 11 rows.
- Core FreeType face/open/slot macro routes: 10 rows.
- `FT_Parameter` and open-parameter routes: 9 rows.

Rejected candidates from this snapshot:

- `freetype.FT_Bitmap_Size.available_sizes_values_match_c` uses
  `fonts/bitmap/embedded-strikes.ttf`, but the worktree has
  `embedded-strike.ttf`.  Do not substitute the similarly named file; promote
  only after the declared asset is present or the input row is corrected with a
  pinned C oracle run for the exact replacement.
  Follow-up on 2026-07-20: probed the existing standard bitmap fixture
  candidates with pinned FreeType 2.14.3:
  `fonts/bitmap/bitmap-strikes.ttf`, `fonts/bitmap/embedded_strike.ttf`,
  `fonts/bitmap/embedded_strike_color_or_sbit.ttf`,
  `input/fonts/bitmap/embedded-strike.ttf`,
  `input/fonts/cache/bitmap-strike-small-sbits.ttf`, and
  `fonts/bitmap-strikes/public-bitmap-strike.ttf`.  All opened successfully
  but returned `face->num_fixed_sizes == 0` and
  `face->available_sizes == NULL`.  These are not valid replacements for the
  declared row because they cannot exercise the public
  `FT_FaceRec.available_sizes[]` contract.
  Follow-up on 2026-07-20: the row can use the maintained generated WinFNT
  fixture `fonts/winfnt/bitmap-header.fnt` because the manifest requirement
  allows Windows FNT and pinned FreeType exposes one fixed-size public record
  for it.  The route should compare exact `num_fixed_sizes` and
  `available_sizes[]` values across pinned C oracle, Rust FFI, C ABI, and WASM
  ABI.
- `freetype.FT_Attach_File.success_attach_auxiliary_file` and
  `freetype.FT_Attach_Stream.success_attach_auxiliary_stream` still require the
  declared Type1 PFB plus AFM/PFM auxiliary assets.
- Follow-up on 2026-07-20: the route audit no longer uses one broad core
  FreeType reason for the five remaining pending core rows.  Each row has a
  separate blocker:
  - `freetype.FT_Attach_File.success_attach_auxiliary_file` needs the declared
    C-openable Type1 PFA/PFB face plus matching AFM/PFM pathname asset and a
    maintained route that compares `FT_Attach_File` status plus post-attach
    kerning/track-kerning mutations across pinned C, Rust FFI, thin C ABI, and
    WASM.  Missing-file/null-path checks do not prove success attachment.
  - `freetype.FT_Attach_Stream.success_attach_auxiliary_stream` needs the same
    Type1/AFM payload through `FT_Open_Args` with `FT_OPEN_MEMORY`, including
    stream ownership and post-attach mutation checks across all ABI lanes.
  - `freetype.FT_FaceRec.populated_public_fields_match_c` must be split from
    its broad snapshot into concrete C-openable stages: initial face fields,
    size mutation, glyph load, charmap selection, auxiliary attachment, and
    variation mutation.  The current row still references missing bitmap and
    Type1 auxiliary assets.
  - `freetype.FT_Get_Track_Kerning.type1_afm_track_kerning_success` depends on
    maintained `input/fonts/type1/track-kern-base.pfb` and
    `input/aux/type1/track-kern-base.afm` assets, then must compare exact
    `akerning` values for negative, zero, and positive track degrees over the
    declared 16.16 point sizes.
  - `freetype.FT_Open_Args.open_face_consumes_args_like_c` must convert its
    abstract `arg_variants` description into explicit maintained `variants[]`
    rows consumed by the runner, then keep stream/pathname variants pending
    until real custom stream/path routes exist.
- Follow-up core-route audit on 2026-07-20 after promoting
  `FT_Bitmap_Size.available_sizes_values_match_c`:
  - `freetype.FT_FaceRec.populated_public_fields_match_c` is not just a route
    classification issue.  The row still names missing
    `fonts/bitmap/embedded-strikes.ttf` and missing
    `input/aux/type1/attach-afm-base.afm`, and its declared operation sequence
    spans initial face snapshots, size mutation, glyph loading, charmap
    selection, attachment, and variation mutation.  Required next fix: split or
    normalize this into concrete per-operation snapshots with C-openable
    assets before any real-parity promotion.
  - `freetype.FT_Open_Args.open_face_consumes_args_like_c` has existing
    memory/open-face variant helpers, but its fixture still describes abstract
    `arg_variants` instead of the concrete `variants[]` rows consumed by the
    maintained runner.  Required next fix: convert each variant into explicit
    row parameters and keep `FT_OPEN_STREAM`/`FT_OPEN_PATHNAME` rows pending
    until a real custom stream/path route exists.
  - `ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY.open_face_uses_legacy_family_name`
    and
    `ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY.open_face_uses_legacy_subfamily_name`
    were completed on 2026-07-20 by normalizing them to the existing option-row
    input shape and fixing Rust SFNT face-name selection to match pinned
    FreeType's WWS/typographic fallback order.
  - `freetype.FT_ENCODING_NONE.representative_runtime_observation` has a file
    at `fonts/no-encoding/bdf-or-pcf-encoding-none.bdf`, but pinned FreeType
    2.14.3 currently returns error 23 when opening it through the generated
    oracle.  Required next fix: replace it with a C-openable BDF/PCF fixture
    whose public charmap actually reports `FT_ENCODING_NONE`; do not clear
    `required_future_asset` based only on file presence.
- `freetype.FT_GlyphSlot.overwritten_by_subsequent_load` expects same
  face-owned slot identity.  The current Rust FFI returns a fresh slot snapshot
  from `FT_Load_Glyph`; the C ABI has a face-owned public slot.  Do not promote
  until Rust exposes/observes the same face-owned slot object rather than only
  matching copied slot values.
- `ftmodapi.FT_New_Library.creates_library_with_version_and_refcount`,
  `ftmodapi.FT_Reference_Library.increments_refcount`, and
  `ftmodapi.FT_Done_Library.decrements_reference_without_destroying` require a
  maintained public `FT_New_Library`/`FT_Done_Library`/`FT_Reference_Library`
  route with allocator identity, default-module state, and library refcount
  semantics.  Test-only `FT_New_Library_Without_Default_Modules` helpers are
  not enough because the manifest row observes public allocator-backed library
  construction.
- `ftmodapi.FT_FACE_DRIVER_NAME.returns_driver_module_name` requires public
  face driver/module representation.  The current C ABI `FT_FaceRec` exposes
  only `glyph`, `size`, and `internal`, so a helper-only module-name check would
  not prove public macro parity.
- `freetype.FT_Face_Properties.error_null_face` is intentionally pending:
  pinned FreeType 2.14.3 dereferences a null `face` when `num_properties > 0`
  and crashes.  Counting Rust `Invalid_Face_Handle` as parity would be a green
  placeholder.
- `freetype.FT_ENCODING_NONE.representative_runtime_observation` is
  intentionally pending: the tracked encoding-none font is not C-openable in
  the current fixture set; pinned C returns error `23`.
- `ftglyph.FT_SvgGlyph.feature_availability_recorded` and
  `ftincrem.FT_Incremental_FuncsRec.required_and_optional_callbacks` remain
  pending because they need maintained glyph-object/SVG and incremental-font
  callback routes, respectively.
- `ftstroke.FT_Stroker_Export.invalid_inputs_noop` remains pending with the
  broad stroker route; promoting just the invalid-input no-op would not prove
  stroker object/path ownership or export behavior.

Required fix plan for the next route batch:

1. Prefer one cohesive surface per change instead of one row when rows share
   state.  The next practical batches are module/library lifecycle,
   face-owned slot identity, or a narrow stroker null/export subset if it can be
   tied to a maintained stroker object route.
2. For module/library lifecycle, add public `FT_MemoryRec`-backed
   `FT_New_Library`, `FT_Reference_Library`, `FT_Done_Library`, and
   `FT_Get_Module` observations through Rust FFI first, then thin C ABI and
   WASM.  The implementation must preserve FreeType's public refcount behavior:
   first `FT_Done_Library` after `FT_Reference_Library` leaves the library
   usable; final done releases it.
3. For face-owned slot identity, make Rust expose the same persistent
   face-owned slot record that C exposes at `face->glyph`.  Compare slot
   pointer identity and overwritten slot fields after repeated loads through
   pinned C, Rust FFI, C ABI, and WASM.
4. For asset blockers, do not rename or substitute files silently.  Either add
   the exact required asset with provenance in `FONT_FIXTURE_INVENTORY.md` or
   update the input row only after a pinned C oracle proves the replacement
   exercises the same public behavior.
5. Keep crash rows and non-C-openable fixture rows pending unless the fixture is
   corrected.  Exact parity cannot mean replacing a C crash or C open error
   with a nicer Rust error.

Verification for this snapshot:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=freetype.FT_Face_Properties.error_null_face
make -C pillow-rs-freetype test-case CASE=freetype.FT_ENCODING_NONE.representative_runtime_observation
make -C pillow-rs-freetype test-case CASE=ftglyph.FT_SvgGlyph.feature_availability_recorded
make -C pillow-rs-freetype test-case CASE=ftincrem.FT_Incremental_FuncsRec.required_and_optional_callbacks
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Export.invalid_inputs_noop
```

FT module/library lifecycle follow-up on 2026-07-20:

- Implemented Rust core `FT_New_Library`, `FT_Reference_Library`, and
  `FT_Done_Library` state for the public module-lifecycle route.  The core now
  records the caller memory pointer, initializes library refcount to 1, leaves
  default modules absent for `FT_New_Library`, increments on
  `FT_Reference_Library`, and decrements without destruction while refcount is
  still non-zero.
- Updated the thin C ABI to keep the public `FT_LibraryRec` layout unchanged
  while storing internal library state behind `internal`.  Public
  `FT_New_Library`, `FT_Reference_Library`, and `FT_Done_Library` exports now
  route through the same core behavior; the C wrapper owns only raw
  `FT_MemoryRec` allocation/free bookkeeping and handle lifetime.
- Added WASM ABI test-support observations for the same lifecycle behavior.
  The WASM wrapper does not implement module logic; it delegates to core and
  copies observable values for the unified parity harness.
- Added a pinned C oracle route for:
  - `ftmodapi.FT_New_Library.creates_library_with_version_and_refcount`
  - `ftmodapi.FT_Reference_Library.increments_refcount`
  - `ftmodapi.FT_Done_Library.decrements_reference_without_destroying`
- The pre-existing `FT_New_Library` null-input and allocator-failure rows still
  pass through their existing exact-error route.  They were not broadened into
  the new success route because the success route is specifically about live
  library state after allocator-backed construction.
- Route audit impact: `real-parity=4519 -> 4522`,
  `pending-route=437 -> 434`, `compile-contract=2265` unchanged.

Verification for this follow-up:

```bash
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_New_Library.creates_library_with_version_and_refcount
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Reference_Library.increments_refcount
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_Done_Library.decrements_reference_without_destroying
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_New_Library.rejects_null_inputs_preserving_output
make -C pillow-rs-freetype test-case CASE=ftmodapi.FT_New_Library.allocation_failure_preserves_output
make -C pillow-rs-freetype route-audit
```

### Issue Set: `FT_Get_Multi_Master` generated Adobe MM descriptor route

Status: eight real descriptor/capacity rows and one related
default-named-instance service row implemented on 2026-07-20.

Baseline before this batch:

- Route audit at `760d707f9`: `real-parity=4466`,
  `pending-route=489`, `pending-core=1`.
- Follow-up baseline at `46e8d619c`: `real-parity=4474`,
  `pending-route=482`, `pending-core=0`.
- Follow-up baseline at `e7fa30b8e`: `real-parity=4478`,
  `pending-route=478`, `pending-core=0`.
- Follow-up baseline at `dd640d1bb`: `real-parity=4481`,
  `pending-route=475`, `pending-core=0`.

Finding:

- The generated Type 1 MM fixture opens in pinned C FreeType and exposes the
  Adobe Multiple Master service with `num_axis=2`, `num_designs=4`, axis names
  `Weight` and `Width`, and design-map extrema `400..900` and `100..200`.
- The legacy manifest path `fonts/mm/adobe-multiple-master.pfb` is now
  generated by the same maintained Type 1 fixture generator instead of being a
  missing `required_future_asset`.
- Rust parsed only basic Type 1 face metadata, so the Rust FFI, C ABI, and WASM
  ABI originally had no maintained `FT_Get_Multi_Master` descriptor route.
- FreeType `src/type1/t1load.c:T1_Get_Multi_Master` writes only
  `num_axis`, `num_designs`, and the populated `FT_MM_Axis` slots; unused
  caller slots retain their incoming sentinel values.
- FreeType `src/base/ftmm.c:694-716` returns OK from
  `FT_Get_Default_Named_Instance` when a Multiple Master service exists but
  the service has no `get_default_named_instance` callback; the caller's
  `instance_index` remains unchanged.
- FreeType `src/type1/t1load.c:290-378` `T1_Get_MM_Var` allocates an
  `FT_MM_Var`, writes `num_axis`, `num_designs`, `num_namedstyles=0`,
  provides a zero-filled axis-flags array directly after the descriptor,
  populates Adobe `FT_Var_Axis` records with 16.16 min/default/max values,
  inferred tags (`Weight -> wght`, `Width -> wdth`, etc.), `strid=~0U`, and
  a null namedstyle pointer.
- FreeType `src/base/ftmm.c:594-613` `FT_Get_Var_Axis_Flags` validates the
  `FT_MM_Var` pointer, output pointer, and axis index, then reads the
  `FT_UShort` axis-flags array stored immediately after the descriptor. For
  Type 1 Adobe MM descriptors produced by `T1_Get_MM_Var`, those flags are
  zero because hidden-axis flags are not meaningful for Adobe MM.

Implementation:

- Added pure-Rust parsing for the generated Type 1 MM descriptor keys:
  `BlendAxisTypes`, `BlendDesignPositions`, `BlendDesignMap`, and
  `WeightVector`.
- Added Rust FFI `FT_Get_Multi_Master`, thin C ABI `FT_Get_Multi_Master`, and
  thin WASM `fontdone_wasm_get_multi_master`.
- Added unified oracle/runtime comparison for the descriptor record, including
  axis name bytes, design minima/maxima, counts, and unused-slot sentinel
  preservation.
- Added a pinned C oracle/runtime route for generated Adobe MM `FT_Get_MM_Var`
  output, including descriptor pointer class, counts, axis pointer/nullness,
  namedstyle pointer nullness, axis name bytes, min/default/max 16.16 values,
  inferred tags, `strid`, zero axis flags, and `FT_Done_MM_Var` release status.
- Added Rust FFI `FT_Get_MM_Var` for Type 1 MM, thin C ABI
  `FT_Get_MM_Var` allocation/free bookkeeping, and thin WASM
  `fontdone_wasm_get_mm_var` record copying.
- Added Rust FFI `FT_Get_Var_Axis_Flags`, thin C ABI
  `FT_Get_Var_Axis_Flags`, thin WASM
  `fontdone_wasm_get_var_axis_flags`, and a pinned C oracle/runtime route for
  generated Adobe MM axis-flag reads. The route explicitly opens the generated
  `adobe_mm_font` asset rather than falling back to the unresolved variable-font
  half of the manifest row.
- Extended the Type 1 fixture generator to materialize the declared legacy
  Adobe MM asset path.
- Fixed Rust `FT_Get_Default_Named_Instance` for Type 1 MM faces to return OK
  and preserve the output sentinel when the C service callback is absent.
- Promoted only fixture-backed rows proven through pinned C oracle, Rust FFI,
  C ABI, and WASM ABI:
  - `ftmm.FT_MM_Axis.populated_by_get_multi_master`
  - `ftmm.FT_Multi_Master.populated_by_adobe_mm_service`
  - `ftmm.FT_Get_Multi_Master.adobe_mm_descriptor_success`
  - `ftmm.T1_MAX_MM_DESIGNS.record_design_capacity`
  - `ftmm.FT_Get_Default_Named_Instance.service_without_default_instance_success`
  - `ftmm.FT_Get_MM_Var.adobe_mm_descriptor_success`
  - `ftmm.FT_MM_Var.populated_for_adobe_mm`
  - `ftmm.FT_Var_Axis.adobe_mm_axis_values`
  - `ftmm.FT_Get_Var_Axis_Flags.valid_axis_flags`

Result:

- Route audit after this follow-up: `real-parity=4482`,
  `pending-route=474`, `pending-core=0`.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_MM_Axis.populated_by_get_multi_master
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Multi_Master.populated_by_adobe_mm_service
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Multi_Master.adobe_mm_descriptor_success
make -C pillow-rs-freetype test-case CASE=ftmm.T1_MAX_MM_DESIGNS.record_design_capacity
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Default_Named_Instance.service_without_default_instance_success
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_Var.adobe_mm_descriptor_success
make -C pillow-rs-freetype test-case CASE=ftmm.FT_MM_Var.populated_for_adobe_mm
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Var_Axis.adobe_mm_axis_values
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Var_Axis_Flags.valid_axis_flags
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Multi_Master.true_type_or_opentype_variation_error
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Multi_Master.invalid_or_non_variable_face_error
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-ffi-compat
make -C pillow-rs-freetype lint
make fontdone-ffi
git diff --check
```

### Issue Set: `FT_Get_MM_Var` maintained OpenType descriptor route

Status: four real OpenType `fvar` descriptor/namedstyle rows implemented on
2026-07-20.

Baseline before this batch:

- Route audit at `890d3a31b`: `real-parity=4482`,
  `pending-route=474`, `pending-core=0`.

Finding:

- The declared TrueType variable fixture
  `fonts/variable/multi-axis-named-instances.ttf` is present and opens in the
  pinned C oracle; the optional CFF2 fixture in the same manifest row remains
  unresolved and must not block the maintained TrueType half.
- FreeType `src/truetype/ttgxvar.c:2445-2906` constructs `FT_MM_Var` for
  OpenType variations from the `fvar` table: `num_designs=~0U`, axis
  min/default/max values are already 16.16 `FT_Fixed`, axis `strid` stores the
  fvar name ID, axis flags are stored in the adjacent `FT_UShort` array, and
  namedstyle records store one design coordinate per axis.
- FreeType `FT_Var_Named_Style::coords` points to exactly one 16.16 design
  coordinate per variation axis; rows must compare the coordinate arrays, not
  only the pointer class.
- The first runtime mismatch after routing was the missing PostScript-name ID
  sentinel: C `TT_Get_MM_Var` stores `0xFFFF`, while Rust initially used
  `~0U` (`4294967295`).

Implementation:

- Extended the pure-Rust `fvar` parser to preserve axis flags and name IDs.
- Added OpenType `FT_Get_MM_Var` descriptor filling in Rust FFI, including
  standard axis names, fvar axis flags, namedstyle records, and coordinate
  arrays.
- Kept C and WASM layers as thin storage owners: wrappers provide descriptor
  backing buffers and expose the core-filled public records without parsing
  font data.
- Extended the pinned C oracle and runtime comparison to include namedstyle
  records and axis flags.
- Tightened route audit unresolved-asset handling so optional future assets do
  not block a row when the required maintained fixture is present.
- Promoted only the fixture-backed row proven through pinned C oracle, Rust
  FFI, C ABI, and WASM ABI:
  - `ftmm.FT_Get_MM_Var.variable_font_descriptor_success`
  - `ftmm.FT_Var_Named_Style.coordinates_array_matches_axis_count`
  - `ftmm.FT_Var_Named_Style.psid_missing_sentinel_matches_c`
  - `ftmm.FT_Var_Axis.variable_font_axis_values`
- Added `fonts/variable/wght-wdth-opsz.ttf` through
  `scripts/build_fvar_fixtures.py`.  The fixture extends the compact variable
  font with an `opsz` axis (8/14/72 design values) while retaining `wght` and
  `wdth`, giving the descriptor route a maintained three-axis OpenType input.

Result:

- Route audit after this batch: `real-parity=4486`,
  `pending-route=470`, `pending-core=0`.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_Var.variable_font_descriptor_success
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Var_Named_Style.coordinates_array_matches_axis_count
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Var_Named_Style.psid_missing_sentinel_matches_c
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Var_Axis.variable_font_axis_values
make -C pillow-rs-freetype route-audit
```

### Issue Set: `FT_Get_Var_Axis_Flags` maintained OpenType hidden-axis route

Status: four real OpenType hidden/visible axis-flag rows implemented on
2026-07-20.

Baseline before this batch:

- Route audit at `69a52f6ab`: `real-parity=4486`,
  `pending-route=470`, `pending-core=0`.

Finding:

- FreeType `src/base/ftmm.c:604-613` reads OpenType axis flags from the
  `FT_UShort` array stored immediately after the aligned `FT_MM_Var`
  allocation, not from `FT_Var_Axis` itself.
- The test harness previously routed `ftmm.get_mm_var_then_axis_flags` only for
  Adobe MM, so OpenType hidden-axis rows stayed pending.  It also had no
  maintained fixtures for visible and hidden fvar axis flag combinations.
- The first runtime mismatch after routing was in the C ABI test harness: it
  tried to call `FT_Get_Var_Axis_Flags` on a copied `FT_MM_Var` after the live
  C ABI allocation had been released.  The C ABI helper already snapshots flags
  while the descriptor is live; token resolution and row output must use that
  snapshot.
- The exact-error row also exposed a harness shape issue: axis-flag row output
  used `status`, while the exact-error matrix checker recognizes row-level
  errors through an `error` field.

Implementation:

- Added generated compact fixtures through `scripts/build_fvar_fixtures.py`:
  - `fonts/variable/multi-axis-visible.ttf`
  - `fonts/variable/hidden-axis.ttf`
  - `fonts/variable/named-instances-hidden-axis.ttf`
- Extended the pinned C oracle and Rust runtime harness token resolution for
  `axis_with_fvar_flags_hidden`, `hidden_axis`, and `visible_axis`.
- Routed resolved OpenType `ftmm.get_mm_var_then_axis_flags` and
  `ftmm.get_var_axis_flags` rows through the existing Rust FFI, C ABI, and WASM
  comparator.
- Kept C/WASM wrappers thin: the C ABI helper snapshots axis flags from the
  live descriptor; no wrapper parses font data or infers fvar behavior.
- Added row-level `error` output for axis-flag matrices so exact-error rows are
  proven by output comparison instead of top-level placeholder status.
- Promoted only rows proven through pinned C oracle, Rust FFI, C ABI, and WASM
  ABI:
  - `ftmm.FT_Get_Var_Axis_Flags.hidden_axis_flag`
  - `ftmm.FT_Get_Var_Axis_Flags.out_of_range_axis_error`
  - `ftmm.FT_VAR_AXIS_FLAG_HIDDEN.returned_by_axis_flags`
  - `ftmm.FT_Var_Axis.hidden_axis_flag_adjacent_storage`

Result:

- Route audit after this batch: `real-parity=4490`,
  `pending-route=466`, `pending-core=0`.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Var_Axis_Flags.out_of_range_axis_error
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Var_Axis_Flags.hidden_axis_flag
make -C pillow-rs-freetype test-case CASE=ftmm.FT_VAR_AXIS_FLAG_HIDDEN.returned_by_axis_flags
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Var_Axis.hidden_axis_flag_adjacent_storage
make -C pillow-rs-freetype route-audit
```

### Issue Set: `FT_Set_MM_WeightVector` generated Adobe MM state route

Status: four real Type 1 MM weight-vector state/getter rows implemented on
2026-07-20.

Baseline before this batch:

- Route audit at `acc333be1`: `real-parity=4468`,
  `pending-route=487`, `pending-core=1`.
- Follow-up baseline at `665d71cc0`: `real-parity=4477`,
  `pending-route=479`, `pending-core=0`.

Finding:

- Pinned FreeType 2.14.3 public dispatch in `src/base/ftmm.c` validates
  `len != 0 && weightvector == NULL` before face service dispatch, calls the
  Type 1 MM service when available, sets `FT_FACE_FLAG_VARIATION` after a
  successful nonzero-length set, and clears it after a successful
  zero-length reset.
- The Type 1 service in `src/type1/t1load.c:T1_Set_MM_WeightVector` resets
  to the default `WeightVector` when called with `len == 0 && weightvector ==
  NULL`; otherwise it copies `min(len, num_designs)` entries, zero-fills the
  remaining design weights, ignores extra entries, and does not enforce the
  sum of weights.
- `src/type1/t1load.c:T1_Get_MM_WeightVector` reports the required design
  count through `*len`, returns `Invalid_Argument` when caller capacity is too
  small, writes current design weights, and zero-fills caller capacity beyond
  `num_designs`.
- Rust previously retained only the parsed descriptor axes/counts for the
  generated Type 1 MM fixture, so the first divergence was the generated
  fixture accepting weight-vector mutation in C while Rust FFI/C ABI/WASM had
  no mutable Type 1 MM weight-vector state.

Implementation:

- Added pure-Rust Type 1 MM default/current weight-vector state on `Font` and
  exact set/reset/copy/zero-fill behavior for the generated Adobe MM fixture.
- Added Rust FFI `FT_Set_MM_WeightVector` and
  `FT_Get_MM_WeightVector`; C ABI and WASM ABI wrappers remain thin pointer
  and handle adapters over the Rust FFI behavior.
- Added a pinned C oracle/runtime route for `ftmm.set_mm_weight_vector` that
  performs each set scenario and then observes `FT_Get_MM_WeightVector`,
  face flags, `FT_FACE_FLAG_VARIATION`, returned length, and output buffer.
- Added a getter-only capacity matrix route for the declared generated Adobe MM
  fixture path.  It compares exact per-row status, `len` before/after,
  written weight-vector values, and preserved/fill buffer slots for exact,
  larger, and smaller caller capacities.
- Promoted only the generated fixture-backed setter success rows:
  - `ftmm.FT_Set_MM_WeightVector.success_set_weight_vector`
  - `ftmm.FT_Set_MM_WeightVector.success_short_long_and_reset`
  - `ftmm.FT_Set_MM_WeightVector.success_unenforced_weight_sum`
  - `ftmm.FT_Get_MM_WeightVector.adobe_mm_weightvector_success`

Result:

- Route audit after this follow-up: `real-parity=4478`,
  `pending-route=478`, `pending-core=0`.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_WeightVector.success_set_weight_vector
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_WeightVector.success_short_long_and_reset
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_WeightVector.success_unenforced_weight_sum
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_WeightVector.error_null_weightvector_with_nonzero_len
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_WeightVector.error_unsupported_on_true_type_variations
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_WeightVector.adobe_mm_weightvector_success
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_WeightVector.len_without_buffer_error
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_MM_WeightVector.unsupported_face_error
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-ffi-compat
make -C pillow-rs-freetype lint
make fontdone-ffi
git diff --check
```

### Issue Set: Type 1 MM design-coordinate and named-instance reset state route

Status: three real Type 1 MM design/reset state rows implemented on
2026-07-20.

Baseline before this batch:

- Route audit at `859026680`: `real-parity=4471`,
  `pending-route=484`, `pending-core=1`.
- Follow-up baseline at `35ee3bc44`: `real-parity=4473`,
  `pending-route=483`, `pending-core=0`.

Finding:

- Pinned FreeType 2.14.3 `src/base/ftmm.c:169-210`
  `FT_Set_MM_Design_Coordinates` validates nonzero count with null coords,
  dispatches to the Type 1 MM `set_mm_design` service, sets
  `FT_FACE_FLAG_VARIATION` after a successful nonzero-count call, and clears
  it after a successful zero-count call.
- `src/type1/t1load.c:T1_Set_MM_Design` maps integer Adobe MM design
  coordinates through `BlendDesignMap`, computes blend weights for every
  design, ignores extra coordinates, and synthesizes C's missing-coordinate
  default before recomputing the weight vector.
- `src/type1/t1load.c:T1_Get_Var_Design` unmaps the current weight vector
  back to 16.16 design coordinates and zero-fills excess output entries.
  `T1_Get_MM_Blend` returns current normalized blend coordinates and fills
  excess output entries with `0x8000`.
- `src/base/ftmm.c:626-687` `FT_Set_Named_Instance` calls the MM service
  reset hook; for Type 1 MM, `src/type1/t1load.c:T1_Reset_MM_Blend` ignores
  the instance index and restores the default `WeightVector`.

Implementation:

- Extended the pure-Rust Type 1 MM descriptor with parsed design-map points.
- Added core Type 1 MM design-coordinate mutation, blend/weight recomputation,
  design-coordinate getter synthesis, and blend-coordinate getter synthesis.
- Added Rust FFI, thin C ABI, and thin WASM ABI
  `FT_Set_MM_Design_Coordinates` / `fontdone_wasm_set_mm_design_coordinates`
  wrappers.
- Added pinned C oracle/runtime routes for direct MM design-coordinate state
  and Adobe MM named-instance reset after a prior design-coordinate mutation.
- Added an explicit same-face multi-scenario route for partial coordinates,
  ignored extra coordinates, and `num_coords == 0 && coords == NULL` reset
  behavior.
- Promoted only state rows proven through pinned C oracle, Rust FFI, C ABI,
  and WASM ABI:
  - `ftmm.FT_Set_MM_Design_Coordinates.success_adobe_mm_design_coordinates`
  - `ftmm.FT_Set_MM_Design_Coordinates.success_partial_extra_and_reset`
  - `ftmm.FT_Set_Named_Instance.success_adobe_mm_resets_default`
- Left `ftmm.FT_Set_MM_Design_Coordinates.output_changes_for_mm_design`
  pending because glyph-output parity requires real Type 1 MM interpolation.
- Follow-up on 2026-07-20: the row's declared `glyph_index=42` is not a
  runnable C oracle input for the generated `adobe-mm-two-axis.pfb`; pinned
  FreeType 2.14.3 returns error code `6` after
  `FT_Set_MM_Design_Coordinates(face, [700,300])` and `FT_Load_Glyph(42)`.
  Probing the same fixture showed glyph `1` is the only rendered glyph among
  sampled indices, but its metrics and bitmap bytes were identical for
  `[400,500]`, `[700,300]`, `[100,1000]`, and `[1000,100]`.  Changing the
  row to glyph `1` would therefore validate only "load after set", not the
  manifest's declared output-changing Type 1 MM interpolation behavior.  Keep
  the row pending until the synthetic Type 1 MM fixture has a valid glyph whose
  rendered output changes under design-coordinate mutation, or until pinned C
  evidence proves a corrected output-changing fixture.

Result:

- Route audit after this batch: `real-parity=4474`, `pending-route=482`,
  `pending-core=0`.

Verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_Design_Coordinates.success_adobe_mm_design_coordinates
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_Design_Coordinates.success_partial_extra_and_reset
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_Named_Instance.success_adobe_mm_resets_default
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-ffi-compat
make -C pillow-rs-freetype lint
make fontdone-ffi
git diff --check
```

### Issue Set Current: Adobe Type 1 MM fixture and false-green route guard

Status: fixture added and false-green route promotion blocked on 2026-07-20.

Finding:

- The remaining Adobe MM rows previously referenced
  `fonts/type1-mm/adobe-mm-two-axis.pfb`, but no maintained fixture existed.
- A compact synthetic Type 1 Multiple Master fixture can be generated
  reproducibly with `scripts/build_type1_fixtures.py`.  The fixture declares
  FreeType's Type 1 MM parser keys from `src/type1/t1load.c`:
  `BlendAxisTypes`, `BlendDesignPositions`, `BlendDesignMap`, and
  `WeightVector`.
- Pinned C FreeType opens the generated fixture and returns:
  `FT_Get_Multi_Master -> Ok, num_axis=2, num_designs=4`, axis names
  `Weight` and `Width`, ranges `400..900` and `100..200`;
  `FT_Set_MM_Design_Coordinates(face, [700,150]) -> Ok`; and
  `FT_Set_Named_Instance(face, 0) -> Ok`.
- Adding the fixture exposed a route-audit bug: Adobe MM success rows were
  promoted to `real-parity` because the asset was present even though the
  unified harness has no explicit Rust FFI, C ABI, and WASM ABI route for those
  success operations.  Focused parity failed with `unexpected error 7` for
  `FT_Set_MM_Design_Coordinates` and `FT_Var_Axis.adobe_mm_axis_values`.

Classification change:

- Keep these rows as `pending-route` until exact same-input C/Rust/C-ABI/WASM
  routes exist and pass focused parity:
  - `ftmm.FT_Set_MM_Design_Coordinates.output_changes_for_mm_design`
  - `ftmm.FT_MM_Var.populated_for_adobe_mm`
  - `ftmm.FT_Var_Axis.adobe_mm_axis_values`

Required fix plan:

1. Parse the Type 1 MM top-level fields in pure Rust for Type 1 faces:
   axis names, design positions, design maps, and default/current weight vector.
2. Add safe core methods for Adobe MM descriptor, design-coordinate set/reset,
   weight-vector set/get, and named-instance reset-to-default semantics.  C
   reference: `src/type1/t1load.c:489-643` and public dispatch in
   `src/base/ftmm.c`.
3. Add explicit unified routes for `FT_Get_Multi_Master`,
   `FT_Set_MM_Design_Coordinates`, `FT_Set_MM_WeightVector`,
   `FT_Get_MM_WeightVector`, and `FT_Set_Named_Instance(0)` on the generated
   Adobe MM fixture.  The C and WASM ABI layers must stay thin record/pointer
   wrappers over core state.
4. Keep glyph-output rows pending until Type 1 MM charstring interpolation is
   implemented; descriptor and state APIs alone are not glyph parity.
5. Promote one row at a time only after focused `make -C pillow-rs-freetype
   test-case CASE=...` proves exact pinned C, Rust FFI, C ABI, and WASM ABI
   output.

Verification for this guard batch:

```bash
make -C pillow-rs-freetype route-audit
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_Design_Coordinates
make -C pillow-rs-freetype test-case CASE=ftmm.FT_Var_Axis.adobe_mm_axis_values
```

### Issue Set Current: `FTMM` future variable-font fixture substitutions rejected

Status: investigated and left pending on 2026-07-20.

Current baseline:

- Route audit at `ea229b4e7`: `real-parity=4457`,
  `pending-route=498`, `pending-core=1`.

Finding:

- Existing local variable fixtures are not valid drop-in replacements for the
  future FTMM success assets.  Two tempting candidates were checked:
  `tests/fixtures/fonts/variable/compact-variable.ttf` and
  `tests/fixtures/input/fonts/variation/mvar-vertical-metrics.ttf`.
- Aliasing the missing semantic IDs such as
  `fonts/variable/multi-axis-named-instances.ttf`,
  `fonts/variable/named-instances-wght-wdth.ttf`,
  `fonts/variable/named-instance-missing-psid.ttf`,
  `fonts/variable/inter-wght.ttf`, and related MVAR/HVAR/GVAR names to those
  existing files made the rows C-openable, but pinned FreeType returned
  `FT_Err_Invalid_Argument` (`7`) for the declared success APIs.
- Failed focused probes included:
  - `ftmm.FT_Get_MM_Var.variable_font_descriptor_success`
  - `ftmm.FT_Var_Named_Style.coordinates_array_matches_axis_count`
  - `ftmm.FT_Var_Named_Style.psid_missing_sentinel_matches_c`
  - `ftmm.FT_Get_Var_Design_Coordinates.success_default_design_coordinates`
  - `ftmm.FT_Get_Var_Design_Coordinates.success_named_instance_design_coordinates`
  - `ftmm.FT_Get_Var_Design_Coordinates.excess_output_coordinates_zero_filled`
  - `ftmm.FT_Get_Var_Blend_Coordinates.success_default_blend_coordinates`
  - `ftmm.FT_Get_Var_Blend_Coordinates.success_after_set_var_blend_coordinates`
  - `ftmm.FT_Get_Var_Blend_Coordinates.excess_output_coordinates_zero_filled`
  - `ftmm.FT_MM_Var.ownership_matches_c`
- Therefore those aliases would be green placeholders: they change route audit
  classification without same-input C/Rust/C-ABI/WASM success parity.

Required fix plan:

1. Do not satisfy the missing future FTMM rows by aliasing to the existing
   compact or MVAR fixtures unless a focused pinned-C probe first returns
   success for the exact public API row.
2. Add or generate source-backed variable fixtures whose FreeType services
   actually expose the required descriptor, named-style, design-coordinate,
   blend-coordinate, axis-flag, and ownership behavior.
3. Keep Adobe MM rows pending until a real Adobe MM fixture and pure-Rust Adobe
   MM support exist.  Do not emulate them with OpenType variable fonts.
4. Promote each row only after the focused operation passes through pinned C
   oracle, Rust FFI, thin C ABI, and WASM ABI for the same fixture and
   parameters.

Verification commands used for the failed probes:

```bash
make -C pillow-rs-freetype test-op OP=ftmm.get_mm_var
make -C pillow-rs-freetype test-op OP=ftmm.get_var_design_coordinates
make -C pillow-rs-freetype test-op OP=ftmm.get_var_blend_coordinates
make -C pillow-rs-freetype test-op OP=ftmm.get_and_done_mm_var
make -C pillow-rs-freetype route-audit
```

Follow-up finding:

- After adding a maintained `FT_Get_Var_Design_Coordinates` runner, pinned C
  validates the default single-axis and named-instance design-coordinate rows
  against Rust FFI, C ABI, and WASM ABI.
- The generated three-axis `fonts/variable/wght-wdth-opsz.ttf` fixture was
  corrected to remove `avar`, `gvar`, `HVAR`, and `STAT` data inherited from
  the two-axis compact base.  With those axis-count-specific tables present,
  pinned C returned an error for coordinate setters; with only the coherent
  three-axis `fvar` and name data, pinned C accepts
  `FT_Set_Var_Design_Coordinates`.
- Pinned C preserves caller-provided design coordinate values in
  `FT_Get_Var_Design_Coordinates` while filling omitted axes from `fvar`
  defaults.  Rust previously clamped the public stored design coordinates to
  each axis min/max.  The internal normalized-coordinate path still clamps for
  variation math; only the public design-coordinate state now preserves C's
  unclamped values.
- The row
  `ftmm.FT_Get_Var_Design_Coordinates.success_after_set_var_design_coordinates`
  now validates the after-set active design coordinate state through pinned C
  oracle, Rust FFI, C ABI, and WASM ABI.
- The row named
  `ftmm.FT_Get_Var_Design_Coordinates.excess_output_coordinates_zero_filled`
  remains pending.  Its manifest text says entries beyond the axis count are
  zero-filled, but pinned FreeType 2.14.3 on the maintained ABI returned
  non-zero, run-varying data in the second output slot for the single-axis
  fixture.  Matching that unstable value in Rust would be a green placeholder,
  so the row must stay pending until the public C behavior is pinned with a
  deterministic fixture or the manifest expectation is corrected from C
  evidence.
- Source audit on 2026-07-20: the public wrapper delegates to the service
  (`src/base/ftmm.c:362-388`) and the TrueType service documents zero-fill but
  advances `a` through the active axes before its excess loop
  (`src/truetype/ttgxvar.c:3438-3488`).  For the one-axis `inter-wght.ttf`
  fixture with `num_coords=4`, the oracle emitted a process-dependent
  pointer-like `/coords/1` value while Rust returned deterministic zero-fill.
  Do not change Rust to synthesize or preserve adjacent-memory values, and do
  not promote this row until pinned C, Rust FFI, C ABI, and WASM ABI have exact
  deterministic same-input output.
- Rechecked on 2026-07-20 from branch
  `ftmm-route-audit-placeholder-parity` after `76b6832b5`: temporarily routing
  this row as real parity produced a focused failure, not a pass.  Command:
  `make -C pillow-rs-freetype test-case CASE=ftmm.FT_Get_Var_Design_Coordinates.excess_output_coordinates_zero_filled`.
  Result: pinned C expected `/coords/1 = 105553139515584` for the sentinel-filled
  output buffer while Rust returned `0`; runtime parity failed in bucket
  `rust ffi:field:/coords/1`.  Keep the row `pending-route`; promoting it would
  reward the manifest label rather than exact C behavior.

Latest route audit after this follow-up: `real-parity=4491`,
`pending-route=465`, `pending-core=0`.

Follow-up finding for blend-coordinate state rows:

- A maintained `FT_Get_Var_Blend_Coordinates` runner now validates the default
  single-axis blend-coordinate row against pinned FreeType, Rust FFI, C ABI, and
  WASM ABI.  The same runner preserves the existing null-output and
  non-variable-face error rows; non-variable FTMM service absence must map to
  `FT_Err_Invalid_Argument`, not invalid font format.
- Follow-up on 2026-07-20: the single-axis
  `ftmm.FT_Set_Var_Blend_Coordinates.success_aliases_mm_blend_setter` and
  `success_variation_flag_matches_c` rows now have maintained C oracle routes
  and validate through Rust FFI, C ABI, and WASM ABI.  The alias row is not a
  boolean assumption: pinned C returns OK for `FT_Set_Var_Blend_Coordinates`
  but the control `FT_Set_MM_Blend_Coordinates` call exposes the TrueType
  service sentinel `-2`, so `matches_control_call=false` is the exact public
  behavior for this fixture.
- Pinned C also exposed the public-wrapper distinction in
  `freetype/src/base/ftmm.c:390-572` and
  `freetype/src/truetype/ttgxvar.c:3166-3184`:
  `FT_Set_MM_Blend_Coordinates` does not translate internal service return
  `-2` to OK, while `FT_Set_Var_Blend_Coordinates` does.  Rust FFI, C ABI, and
  WASM ABI now keep those wrapper semantics separate instead of aliasing the MM
  setter to the Var setter.
- Follow-up on 2026-07-20: the
  `ftmm.FT_Set_MM_Blend_Coordinates.success_set_normalized_coordinates` row now
  validates Adobe Type 1 MM instead of the OpenType `inter-wght.ttf` path that
  pinned C returns as sentinel `-2`.  Pinned FreeType 2.14.3 proves
  `FT_Set_MM_Blend_Coordinates(adobe-mm-two-axis.pfb, [32768,32768]) -> Ok`,
  followed by `FT_Get_MM_Blend_Coordinates -> [32768,32768]` with
  `FT_FACE_FLAG_VARIATION` set.  Rust now routes Type 1 MM blend mutation
  before the TrueType/OpenType sentinel path and recomputes the Type 1 MM
  weight vector using `src/type1/t1load.c:t1_set_mm_blend` semantics: clamp
  `num_coords` to `num_axis`, ignore extra coordinates, and use 0.5 for omitted
  axes.
- Follow-up on 2026-07-20: the
  `ftmm.FT_Set_MM_Blend_Coordinates.success_partial_and_extra_coordinates` row
  now validates the same Adobe Type 1 MM service with two same-face scenarios.
  Pinned FreeType proves `num_coords=1, coords=[16384]` reads back
  `[16384,32768]`, using the Type 1 MM default 0.5 for the omitted Width axis;
  and `num_coords=4, coords=[16384,-16384,32768,65536]` reads back
  `[16384,0]`, proving extra coordinates are ignored after the two real axes.
  The maintained scenario route compares those rows through pinned C oracle,
  Rust FFI, C ABI, and WASM ABI.
- Follow-up on 2026-07-20: the
  `ftmm.FT_Set_MM_Blend_Coordinates.success_reset_to_default` row now validates
  the Adobe Type 1 MM reset path with the same scenario route.  Pinned FreeType
  proves a same-face non-default set to `[16384,16384]` reads back
  `[16384,16384]` with `FT_FACE_FLAG_VARIATION` set; a subsequent
  `num_coords=0, coords=NULL` reset reads back `[32768,32768]` and clears
  `FT_FACE_FLAG_VARIATION`.
- Follow-up on 2026-07-20: keep
  `ftmm.FT_Set_MM_Blend_Coordinates.output_changes_for_active_blend` pending.
  The current row uses the OpenType fixture
  `fonts/variable/gvar-hvar-wght.ttf`, but pinned FreeType 2.14.3 returns the
  public-wrapper sentinel `FT_Err_Unimplemented_Feature` (`-2`) for the same
  call shape:
  `FT_Set_MM_Blend_Coordinates(face, 1, [65536])`, before any glyph load or
  render can prove output parity.  The state-only Adobe Type 1 MM route is not
  a substitute for this glyph-output row: `scripts/build_type1_fixtures.py`
  explicitly keeps `adobe-mm-two-axis.pfb` glyph programs minimal, so it is
  suitable for descriptor, coordinate, weight-vector, and reset state but not
  yet for proving blend-dependent glyph metrics, cbox, or bitmap bytes.  Moving
  this row to `real-parity` with either the OpenType error or the minimal Type 1
  glyph would be a green placeholder.  The required next fix is a maintained
  Type 1 MM fixture whose glyph outline changes under non-default blend/design
  coordinates, followed by a pinned-C/Rust FFI/C ABI/WASM ABI glyph-output
  runner for `FT_Set_MM_Blend_Coordinates`.
- Strict probe on 2026-07-20 from branch
  `ftmm-route-audit-placeholder-parity`: temporarily removing only
  `ftmm.FT_Set_MM_Blend_Coordinates.output_changes_for_active_blend` from the
  FTMM route guard moved the audit classification from
  `pending-route` to `real-parity`, but focused runtime parity still failed.
  Command:
  `make -C pillow-rs-freetype test-case CASE=ftmm.FT_Set_MM_Blend_Coordinates.output_changes_for_active_blend`.
  Result: `runtime_cases: runnable=1 pending=0`,
  `runtime_parity: passed=0 failed=1`, bucket `rust ffi:value:1`, with
  `rust ffi: ftmm.FT_Set_MM_Blend_Coordinates.output_changes_for_active_blend oracle returned unexpected error -2`.
  Keep the route guard in place.  The required next fix is an oracle-backed
  success fixture and glyph-output runner; classifier movement alone is not a
  valid parity gain.
- The promoted variation-flag matrix pins the C state transitions on
  `fonts/variable/inter-wght.ttf`: blend coords `[0]` keep `face_flags=2841`
  and `FT_IS_VARIATION=false`, blend coords `[32768]` set
  `face_flags=35609` and `FT_IS_VARIATION=true`, and zero/null reset returns
  to `face_flags=2841`.
- The remaining MM/Var blend setter rows stay pending until the same concrete
  fixture and parameters return success in pinned C and the maintained runner
  compares exact active coordinates, variation flag, and any declared glyph
  output.
- Follow-up on 2026-07-20: do not promote
  `ftmm.FT_Set_MM_Blend_Coordinates.success_set_normalized_coordinates` or
  `ftmm.FT_Set_MM_Blend_Coordinates.success_reset_to_default` from the current
  manifest text.  Temporarily lifting each route guard and running the focused
  parity row proved pinned FreeType 2.14.3 returns the public-wrapper sentinel
  `-2`, while the manifest declares `status: ok`.  The reset row inherits the
  same issue because its prior non-default `FT_Set_MM_Blend_Coordinates` call
  returns `-2` before the zero-count reset path.  Keep both rows pending until
  their expected behavior is corrected from C evidence or a different
  C-success fixture/parameter set is introduced.
- A maintained `FT_Get_MM_Blend_Coordinates` default-row route now validates the
  concrete OpenType variable-font row through pinned C, Rust FFI, C ABI, and
  WASM ABI.  The row's optional Adobe MM asset remains unresolved and is not
  counted by this route; only the C-openable `variable_font` row is promoted.
- Follow-up on 2026-07-20: the
  `ftmm.FT_Get_MM_Blend_Coordinates.after_set_blend_coordinates` row now uses
  the maintained single-axis `fonts/variable/inter-wght.ttf` fixture instead
  of the absent future `fonts/variable/avar-multi-axis.ttf` asset.  Pinned
  FreeType 2.14.3 proves the concrete behavior:
  `FT_Set_Var_Blend_Coordinates(face, [32768]) -> Ok`, then
  `FT_Get_MM_Blend_Coordinates(face, axis_count) -> [32768]`, with
  `face_flags=35609` and `FT_FACE_FLAG_VARIATION` set.  The unified route now
  validates that same active normalized blend state through pinned C oracle,
  Rust FFI, C ABI, and WASM ABI.
- The MM invalid argument-matrix row stays exact by comparing all three public
  C scenarios explicitly: variable face with null coords, null face with valid
  coords, and non-variable face with valid coords.
- The `FT_Get_MM_Blend_Coordinates.partial_or_excess_count` OpenType
  variable-font row now compares the maintained count matrix
  `0, 1, axis_count, axis_count + 2` through pinned C, Rust FFI, C ABI, and
  WASM ABI.  The optional Adobe MM half of the manifest row remains unresolved
  and is not counted by this route.
- The `FT_Get_Var_Blend_Coordinates.excess_output_coordinates_zero_filled`
  row is distinct from the design-coordinate excess row: pinned C returns a
  deterministic zero-filled blend vector for the single-axis fixture when four
  coordinates are requested, and the maintained runner compares that exact
  output through Rust FFI, C ABI, and WASM ABI.
- Follow-up on 2026-07-20: the missing
  `fonts/variable/avar-wght-wdth.ttf` asset is now a documented semantic alias
  of the maintained compact variable fixture (`c7ed80798946`) so the
  `ftmm.FT_Get_Var_Blend_Coordinates.success_after_set_var_blend_coordinates`
  row has a C-openable two-axis avar/gvar/HVAR fixture.  Pinned C preserves the
  caller's normalized 16.16 blend vector after
  `FT_Set_Var_Blend_Coordinates`, returning `[32768, -32768]` for the row.
  Rust previously converted blend coordinates to design coordinates, then
  recomputed public blend output from design state; on the degenerate `wdth`
  axis (`default == max`) this collapsed the first coordinate to `0`.  Rust now
  stores public blend-coordinate state separately from the internal F2Dot14
  variation coordinates used for glyph and metric deltas.

Latest route audit after this follow-up: `real-parity=4492`,
`pending-route=464`, `pending-core=0`.
- Follow-up on 2026-07-20: the
  `ftmm.FT_Set_Var_Blend_Coordinates.success_partial_extra_and_reset` row now
  has a maintained scenario route comparing partial, excess, and zero/null
  calls through pinned C, Rust FFI, C ABI, and WASM ABI.  Pinned C truncates
  excess public blend coordinates to the axis count, fills missing axes with
  normalized zero, and for a zero-count/null-pointer call preserves the prior
  internal blend/design coordinate arrays while clearing `FT_FACE_FLAG_VARIATION`.
  Rust previously rebuilt default coordinate state for the zero/null call,
  returning blend coordinate `0` where C still exposed the previous `16384`.
  Rust now preserves public blend/design state for this reset path while
  clearing the variation flag.

Latest route audit after this follow-up: `real-parity=4493`,
`pending-route=463`, `pending-core=0`.

- Follow-up on 2026-07-20: the
  `ftmm.FT_Set_Var_Design_Coordinates.success_partial_extra_and_reset` row now
  has the same maintained scenario route pattern.  Pinned C over
  `fonts/variable/wght-wdth-opsz.ttf` returns design/blend rows
  `[45875200, 26214400, 917504] / [65536, 0, 0]`,
  `[45875200, 4915200, 917504] / [65536, -65536, 0]`, then reset defaults
  `[6553600, 26214400, 917504] / [0, 0, 0]` with `face_flags=2841`.
  The first Rust divergence after adding the route was
  `/rows/0/blend_coords/0`: Rust returned `0` where C returned `65536`
  because Rust clamped the public design coordinate to the degenerate
  `default == max` axis before normalization.  FreeType instead normalizes the
  caller design coordinate in `ttgxvar.c:2152-2211`, where values above a
  degenerate default/max axis clamp to +1.0 before division.  A second
  divergence on `/rows/2/face_flags` proved the zero-count design reset must
  clear `FT_FACE_FLAG_VARIATION` through `ftmm.c:281-360` while recomputing
  default design/blend coordinates.

Latest route audit after this follow-up: `real-parity=4494`,
`pending-route=462`, `pending-core=0`.

Full parity follow-up on 2026-07-20:

- The broad `fontdone-parity` run exposed nine FTMM failures after the focused
  design-coordinate route: three were the `FT_Set_MM_WeightVector` scenario
  oracle using a shorter JSON shape than the Rust/C-ABI/WASM lanes, two were
  weight-vector error rows missing `sentinel_after` and `buffer_after`, one was
  `FT_Set_MM_Design_Coordinates.error_null_coords_with_nonzero_count` where the
  C oracle route failed to forward the declared null coordinate pointer, and two
  were null-output/null-argument rows still referencing future variable assets
  instead of maintained variable fixtures.
- `FT_Get_MM_Var.null_output_error` now uses
  `fonts/variable/wght-wdth-opsz.ttf` and preserves the pinned C oracle's public
  error status/output shape for `FT_Get_MM_Var(face, NULL)` across Rust FFI,
  C ABI, and WASM ABI.  `FT_Get_Var_Axis_Flags.null_master_or_flags_error` now
  uses `fonts/variable/hidden-axis.ttf` and compares both declared rows:
  null `master` and null `flags`.
- The row `ftmm.FT_Multi_Master.populated_by_adobe_mm_service` was demoted back
  to explicit `pending-route`.  Its current maintained route returns pinned C
  `FT_Err_Invalid_Argument`, so keeping it in `real-parity` would be a green
  placeholder until a C-success Adobe MM service fixture is available.
- Full parity after this cleanup: `runtime_parity: passed=6697 failed=0
  total=6697`, with `pending=537`.  Current route audit:
  `real-parity=4493`, `pending-route=463`, `pending-core=0`.

Descriptor lifecycle follow-up on 2026-07-20:

- `ftmm.FT_Done_MM_Var.frees_descriptor_success` and
  `ftmm.FT_MM_Var.ownership_matches_c` now use the maintained
  `fonts/variable/wght-wdth-opsz.ttf` fixture instead of future variable fixture
  names.  The route calls `FT_Get_MM_Var`, observes a non-null descriptor class,
  then calls `FT_Done_MM_Var` with the owning library.
- Pinned C behavior is `FT_Get_MM_Var -> FT_Err_Ok` followed by
  `FT_Done_MM_Var -> FT_Err_Ok`; C allocates and releases the descriptor with
  the library memory (`ftmm.c:123-163`).  Rust FFI uses caller-provided
  descriptor storage, C ABI validates removal from its owned descriptor table,
  and WASM validates removal from its face-owned descriptor side storage through
  feature-gated test support.  The comparison intentionally uses identity
  classes (`non_null`, `same_pointer`, `allocation_released`) rather than raw
  pointer values, so it checks the public contract without backend-specific
  addresses.
- Full parity after this lifecycle route: `runtime_parity: passed=6699 failed=0
  total=6699`, with `pending=535`.  Current route audit:
  `real-parity=4495`, `pending-route=461`, `pending-core=0`.

FTMM glyph-output route follow-up on 2026-07-20:

- The Var Blend and Var Design glyph-output rows were not C-runnable as
  authored: both used `glyph_index=36` with the maintained
  `fonts/variable/gvar-hvar-wght.ttf` fixture, whose `maxp.numGlyphs` is 20.
  Pinned FreeType 2.14.3 therefore returned `FT_Err_Invalid_Argument` (`6`)
  before any glyph-output parity comparison.
- Probing the same fixture with `glyph_index=10` returned `FT_Err_Ok` for both
  `FT_Set_Var_Blend_Coordinates -> FT_Load_Glyph -> FT_Render_Glyph` and
  `FT_Set_Var_Design_Coordinates -> FT_Load_Glyph -> FT_Render_Glyph`, with a
  rendered normal grayscale bitmap.  The manifest inputs now use glyph 10 so
  the rows validate actual same-input C oracle output instead of an
  out-of-range placeholder.

FTMM metrics-variation route follow-up on 2026-07-20:

- `ftmm.FT_Set_Var_Design_Coordinates.success_updates_metrics_variations`
  now uses the maintained `fonts/variable/mvar-hvar-vvar.ttf` fixture with an
  explicit valid `glyph_index=10`.  Pinned FreeType 2.14.3 accepts the
  two-axis design coordinates `[45875200, 4915200]`, sets variation state, and
  reports exact face metrics, active `FT_Size_Metrics`, and loaded glyph
  advance after `FT_Set_Pixel_Sizes`, `FT_Set_Var_Design_Coordinates`, and
  `FT_Load_Glyph`.
- The maintained route compares those metrics through pinned C oracle, Rust
  FFI, C ABI, and WASM ABI.  This route intentionally observes metrics and
  advance only; rendered bitmap parity for the same fixture is covered by the
  glyph-output rows above.

FTMM Adobe MM descriptor route follow-up on 2026-07-20:

- `ftmm.FT_Multi_Master.populated_by_adobe_mm_service` now has a maintained
  two-face route using `fonts/type1-mm/adobe-mm-two-axis.pfb` as the Adobe MM
  success face and `fonts/variable/inter-wght.ttf` as the OpenType variation
  error-control face.  Pinned FreeType 2.14.3 returns `FT_Err_Ok` for the
  Adobe MM descriptor and `FT_Err_Invalid_Argument` for the OpenType control.
- The route compares `num_axis`, `num_designs`, all four inline `FT_MM_Axis`
  slots, and preserved sentinel state for unused axis slots through pinned C
  oracle, Rust FFI, C ABI, and WASM ABI.  This avoids treating OpenType
  variable-font behavior as Adobe MM behavior.

### Issue Set Current: MVAR vertical-header SFNT table mutation

Status: promoted to real parity after the MVAR vertical-header implementation.

Current verified ledger:

- Focused `tttables.TT_VertHeader.sfnt_table_present_runtime.mvar_variation`
  now passes exact pinned C oracle vs Rust FFI comparison for the default and
  changed `TT_VertHeader` record fields.
- Route audit at promotion time: `real-parity=4471`, `pending-core=1`,
  `pending-route=514`.
- Later strict unresolved-asset classification kept this MVAR row real, but
  moved 54 unrelated future-batch success rows with missing assets from
  `real-parity` to `pending-route`; the current global route audit ledger is
  `real-parity=4417`, `pending-core=1`, `pending-route=568`.
- Full runtime parity at promotion time: `6613/6613` runnable passed,
  `pending=621`; after the later unresolved-asset correction, full runtime
  parity is `6601/6601`, `pending=633`.
- Follow-up wrapper verification adds thin C ABI and WASM ABI execution for the
  same MVAR `TT_VertHeader` sequence: C exports
  `FT_Set_Var_Design_Coordinates`, WASM exports
  `fontdone_wasm_set_var_design_coordinates` and
  `fontdone_wasm_get_sfnt_vhea`, and the unified harness no longer delegates
  this row's C/WASM backend checks to Rust FFI.

Remaining core row after this issue set:

- `ftmm.FT_Set_Named_Instance.success_adobe_mm_resets_default`

Finding:

- The maintained fixture
  `tests/fixtures/input/fonts/variation/mvar-vertical-metrics.ttf` is generated
  by `make -C pillow-rs-freetype font-fixture-mvar`.  Pinned C FreeType
  observes its vertical-header MVAR deltas: default `Ascender=880`,
  `Descender=-120`, `Line_Gap=20`, `caret_Slope_Rise=1`,
  `caret_Slope_Run=0`, `caret_Offset=0`; after setting design coordinates to
  the maximum `wght` instance, C reports `Ascender=912`, `Descender=-144`,
  `Line_Gap=32`, `caret_Slope_Rise=3`, `caret_Slope_Run=3`,
  `caret_Offset=4`.
- The first implementation divergence was that Rust parsed no `MVAR` table and
  returned a static `TT_VertHeader` parsed from `vhea`; after coordinate
  mutation, C reported `Ascender=912` while Rust stayed at `880`.
- A second oracle divergence was found in the new C route: the default
  `TT_VertHeader*` pointer had to be copied before calling
  `FT_Set_Var_Design_Coordinates`, because FreeType mutates the same face-owned
  record in place.
- Pinned FreeType applies this through `tt_apply_mvar`:
  - `freetype/src/truetype/ttgxvar.c:1406-1472` maps MVAR tags to public
    face/table fields.
  - `freetype/src/truetype/ttgxvar.c:1480-1612` parses the MVAR value records
    and shared item variation store.
  - `freetype/src/truetype/ttgxvar.c:1633-1755` applies item deltas after
    variation coordinates change.

Implemented fix:

1. Added a pure-Rust `src/tt/mvar.rs` parser that reuses
   `src/tt/varstore.rs` for item variation store evaluation.
2. Stored parsed `MVAR` on `FontData` and rebuilt the face with explicit
   design coordinates for `FT_Set_Var_Design_Coordinates`.
3. Applied `VASC`, `VDSC`, `VLGP`, `VCRS`, `VCRN`, and `VCOF` deltas to the
   cached `TT_VertHeader` record returned by `FT_Get_Sfnt_Table`.
4. Added an exact C oracle route for the default/changed vertical header
   sequence and narrowed the static route audit so this row counts as real
   parity.
5. Added thin C ABI and WASM ABI forwarding/record-copy paths for this row so
   the wrapper backends also compare the default/changed `TT_VertHeader`
   records exactly.

Focused verification before promotion:

```bash
make -C pillow-rs-freetype font-fixture-mvar
make -C pillow-rs-freetype test-case CASE=tttables.TT_VertHeader.sfnt_table_present_runtime.mvar_variation
make fontdone-test
make fontdone-ffi
make fontdone-ffi-compat
make -C pillow-rs-freetype fmt
make -C pillow-rs-freetype clippy
```

### Issue Set Current: `t1tables` Type1 runtime table route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `03a1d946d`: `real-parity=4465`,
  `generic-fallback=231`, `pending-route=283`, `pending-core=7`.

Finding:

- Type1 scalar constants and record layouts are already `compile-contract`.
- The Type1 runtime rows for `FT_Get_PS_Font_Info`,
  `FT_Get_PS_Font_Private`, `FT_Get_PS_Font_Value`,
  `FT_Has_PS_Glyph_Names`, Multiple Master blend dictionary fields, blend flag
  groups, and Type1 encoding runtime cases had stayed in `generic-fallback`
  with the reason `no explicit maintained route classification`, then under
  one broad Type1 tables pending reason. The current classifier names each
  blocked row explicitly.
- Those rows are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  Type1 tables route that opens Type1/CFF/MM fixtures, reads the font-info and
  private dictionaries, queries `FT_Get_PS_Font_Value` for encoding and blend
  keys, and compares exact public records, arrays, scalar values, lengths, and
  unsupported/sentinel behavior.

Classification change:

- 27 current `t1tables.*` runtime rows are explicit `pending-route` records
  with case-specific blockers instead of a subsystem-wide pending reason.
- 29 `t1tables.*` constants/layout rows remain `compile-contract`.
- The route audit count remains stable for this refinement; it changes blocker
  granularity, not the number of accepted parity rows.

Case-specific blocker groups:

- `FT_Get_PS_Font_Info`, `FT_Get_PS_Font_Private`,
  `FT_Get_PS_Font_Value`, and `FT_Has_PS_Glyph_Names` need C-openable
  Type1/CFF fixtures and exact record, selector, length, glyph-name, and public
  error comparison across pinned C, Rust FFI, C ABI, and WASM ABI.
- `T1_BLEND_BLUE_SCALE`, `T1_BLEND_BLUE_SHIFT`,
  `T1_BLEND_BLUE_VALUES`, `T1_BLEND_FAMILY_BLUES`,
  `T1_BLEND_FAMILY_OTHER_BLUES`, `T1_BLEND_FORCE_BOLD`,
  `T1_BLEND_ITALIC_ANGLE`, `T1_BLEND_OTHER_BLUES`,
  `T1_BLEND_STANDARD_HEIGHT`, `T1_BLEND_STANDARD_WIDTH`,
  `T1_BLEND_STEM_SNAP_HEIGHTS`, `T1_BLEND_STEM_SNAP_WIDTHS`,
  `T1_BLEND_UNDERLINE_POSITION`, and
  `T1_BLEND_UNDERLINE_THICKNESS` need Multiple Master Type1 fixtures and exact
  blend dictionary scalar/array value comparison.
- `T1_Blend_Flags.font_info_blend_group` and
  `T1_Blend_Flags.private_blend_group` need runtime proof that public blend
  flags select the same font-info and private dictionary fields as pinned C.
- `T1_ENCODING_TYPE_ARRAY`, `T1_ENCODING_TYPE_EXPERT`,
  `T1_ENCODING_TYPE_ISOLATIN1`, `T1_ENCODING_TYPE_NONE`,
  `T1_ENCODING_TYPE_STANDARD`, and the `T1_EncodingType` runtime cases need
  maintained Type1 encoding fixtures proving exact encoding classification,
  encoding array records, and glyph-name resolution.

Required fix plan:

1. Add a maintained Type1 table route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement pure-Rust Type1/CFF private dictionary, font-info, encoding, and MM
   blend extraction first. The C and WASM ABI layers may only own handle
   validation, record copying, and lifetime bookkeeping.
3. Compare exact `T1_FontInfo`, `T1_Private`, `FT_Get_PS_Font_Value` values,
   array lengths and contents, encoding type classifications, blend flag groups,
   and sentinel rejection/no-field behavior.
4. Keep the existing constants and record-layout compile contracts separate
   from runtime parity; do not count imports/layout checks as value parity.
5. Promote rows only after focused `t1tables` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `ftcolor` COLR/CPAL success route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `8ce9477e8`: `real-parity=4465`,
  `generic-fallback=339`, `pending-route=175`, `pending-core=7`.

Finding:

- The route audit already has exact real parity for selected `ftcolor` null,
  malformed, disabled-color-layers, invalid iterator, invalid root transform,
  and unsupported paint-format rejection rows.
- The remaining COLR/CPAL paint graph, composite mode, transform, clipbox,
  layer iterator, colorline, palette data/select/foreground-color, and paint
  record success rows had stayed in `generic-fallback` with the reason
  `no explicit maintained route classification`, then under one broad color
  subsystem pending reason.
- Those rows are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  color subsystem route that opens COLR/CPAL fixtures, walks
  `FT_Get_Color_Glyph_Paint`, `FT_Get_Paint`, `FT_Get_Paint_Layers`,
  `FT_Get_Colorline_Stops`, palette APIs, and clipbox APIs, then compares exact
  public structs, iterator state, palette entries, root transforms, composite
  modes, and output preservation.

Classification change:

- Current route audit has 107 `ftcolor.*` pending rows. They now use explicit
  case-set blockers grouped by behavior surface instead of a subsystem-wide
  color reason.
- The route audit count remains stable for this refinement; it changes blocker
  granularity, not the number of accepted parity rows.

Current blocker families:

- Palette and foreground-color rows are split by exact obligation instead of
  sharing a broad palette blocker:
  - `FT_Color.palette_entries_preserve_bgra_order`: BGRA byte order, alpha, and
    palette entry indexing.
  - `FT_PALETTE_FOR_DARK_BACKGROUND` and `FT_PALETTE_FOR_LIGHT_BACKGROUND`:
    CPAL palette flag bits.
  - `FT_Palette_Data.palette_data_get_values` and
    `FT_Palette_Data_Get.success_sfnt_cpal_palette_data`: palette counts, entry
    counts, name IDs, flag arrays, and SFNT CPAL metadata.
  - `FT_Palette_Data_Get.success_non_sfnt_null_palette_data`: non-SFNT success
    plus null-data/output preservation behavior.
  - `FT_Palette_Select.success_selects_palette_and_returns_entries`,
    `success_null_output_selects_without_return`,
    `success_reselect_resets_user_modifications`, and
    `success_non_sfnt_returns_null_palette`: selected palette pointers, entry
    values, active palette state, null-output selection, reselection reset, and
    non-SFNT null palette behavior.
  - `FT_Palette_Set_Foreground_Color.success_sets_sfnt_foreground_color`,
    `success_non_sfnt_noop`, and `default_foreground_color_policy`: foreground
    color mutation, non-SFNT no-op behavior, default policy, and later
    overrides.
- Layer iterator rows are split by exact obligation instead of sharing a broad
  layer-route blocker:
  - `FT_Get_Color_Glyph_Layer.layer_iteration_success`: successive COLR v0
    layer glyph indexes, color indexes, and iterator advancement.
  - `FT_Get_Color_Glyph_Layer.foreground_color_index`: foreground color index
    sentinel values emitted and preserved exactly like pinned C.
  - `FT_Get_Color_Glyph_Layer.terminal_false_preserves_last_outputs`: false
    return after the final layer preserving prior output fields and iterator
    state.
  - `FT_Get_Paint_Layers.success_iterates_colr_v1_layers`: COLR v1 layer paint
    handles, layer count, and iterator advancement.
  - `FT_Get_Paint_Layers.end_of_iteration`: false return after the final v1
    paint layer preserving output paint and iterator fields.
  - `FT_LayerIterator.initialized_and_advanced_by_layer_apis`: public iterator
    fields initialized and advanced by COLR v0 and v1 APIs with pinned-C
    counter and opaque-state semantics.
  - `FT_COLR_PAINTFORMAT_COLR_LAYERS.paint_colr_layers_payload`:
    `COLR_LAYERS` payload initialization of the same layer iterator fields and
    nested state.
  - `FT_PaintColrLayers.get_paint_initializes_layer_iterator`: layer count and
    initialized `FT_LayerIterator` output from the public union.
- Root paint and root transform rows are split by exact obligation instead of
  sharing a broad root-paint blocker:
  - `FT_Get_Color_Glyph_Paint.root_paint_success_no_root_transform`: initial
    opaque paint output and output preservation when transforms are omitted.
  - `FT_Get_Color_Glyph_Paint.root_paint_success_include_root_transform`: root
    transform insertion/exposure before downstream traversal.
  - `FT_Get_Color_Glyph_Paint.downstream_paint_graph_contract`: opaque paint
    handles produced by root lookup must be consumable by `FT_Get_Paint` and
    graph traversal.
  - `FT_COLOR_INCLUDE_ROOT_TRANSFORM.include_transform_runtime` and
    `FT_COLOR_NO_ROOT_TRANSFORM.omit_transform_runtime`: flag behavior for
    `s12` and `s48` size variants.
  - `FT_Color_Root_Transform.root_transform_controls_initial_paint`: enum
    control of initial paint and transform insertion for `s12` and `s48`.
  - `FT_COLR_PAINTFORMAT_TRANSFORM.included_root_transform_payload`: transform
    paint format, affine fields, and nested paint handle for `s16` and `s31`.
  - `FT_Get_Paint.success_inserts_root_transform` and
    `FT_Affine23.root_transform_values`: synthesized transform union output and
    exact `xx`, `xy`, `dx`, `yx`, `yy`, `dy` values.
- Paint resolution rows are split by exact obligation instead of sharing a
  broad `FT_Get_Paint` blocker:
  - `FT_Get_Paint.success_resolves_each_supported_paint_format`: supported
    COLR paint format dispatch to the same public union tag and payload as
    pinned C.
  - `FT_OpaquePaint.produced_and_consumed_by_paint_apis`: root and nested
    opaque paint handle production/consumption, lifetime, and identity
    semantics.
  - `FT_PaintColrGlyph.get_paint_colr_glyph_values`: nested glyph ID and nested
    paint handle fields in the public union.
  - `FT_PaintGlyph.get_paint_glyph_values`: glyph ID plus nested paint handle
    fields in the public union.
  - `FT_COLR_PAINTFORMAT_COLR_GLYPH.paint_colr_glyph_runtime`: format tag
    emission with the pinned-C `FT_PaintColrGlyph` payload shape.
  - `FT_COLR_PAINTFORMAT_GLYPH.paint_glyph_payload`: glyph format tag emission
    with glyph ID plus nested paint handle payload.
  - `FT_COLR_PAINTFORMAT_SOLID.paint_solid_color_index`: solid paint color
    index, alpha, and palette-index semantics.
  - `FT_ColorIndex.solid_and_color_stop_values`: solid paint and color-stop
    palette index, alpha, and foreground sentinel behavior.
  - `FT_PaintSolid.get_paint_solid_values`: color index and alpha fields in the
    public union.
  - `FT_PaintFormat.paint_union_shape_runtime`: public format tags selecting
    the same `FT_COLR_Paint` union arm and record layout for each supported
    paint node.
- Colorline and gradient rows are split by exact obligation instead of sharing
  a broad colorline blocker:
  - `FT_ColorLine.gradient_colorline_values`: extend mode, stop count, and
    iterator fields from `FT_Get_Paint`.
  - `FT_ColorStop.iterator_output_values`: stop offset, color index, and alpha
    emitted by `FT_Get_Colorline_Stops`.
  - `FT_ColorStopIterator.initialized_by_get_paint` and
    `advanced_by_get_colorline_stops`: iterator initialization, mutation, and
    output preservation across calls.
  - `FT_Get_Colorline_Stops.success_iterates_static_colorline_stops`,
    `success_iterates_variable_colorline_stops`, and `end_of_iteration`: static
    stops, variation-adjusted stops, false return, and terminal output
    preservation.
  - `FT_PaintLinearGradient`, `FT_PaintRadialGradient`, and
    `FT_PaintSweepGradient`: public union fields plus attached colorline
    iterator state.
  - `FT_COLR_PAINTFORMAT_LINEAR_GRADIENT`, `RADIAL_GRADIENT`, and
    `SWEEP_GRADIENT`: payload shape and nested colorline state.
  - `FT_COLR_PAINT_EXTEND_PAD`, `REFLECT`, `REPEAT`, and `FT_PaintExtend`: exact
    extend enum values and iterator preservation.
- Transform paint rows are split by exact obligation instead of sharing a broad
  transform-paint blocker:
  - `FT_PaintRotate.get_paint_rotate_values`: angle, center coordinates, and
    nested paint handle fields.
  - `FT_PaintScale.get_paint_scale_values`: x/y scale factors, center
    coordinates, and nested paint handle fields.
  - `FT_PaintSkew.get_paint_skew_values`: x/y skew angles, center coordinates,
    and nested paint handle fields.
  - `FT_PaintTransform.get_paint_transform_values`: explicit affine matrix
    fields and nested paint handle.
  - `FT_PaintTranslate.get_paint_translate_values`: dx/dy translation values
    and nested paint handle fields.
  - `FT_COLR_PAINTFORMAT_ROTATE.paint_rotate_normalized_payload`:
    FreeType-normalized rotate payload values and nested paint handle.
  - `FT_COLR_PAINTFORMAT_SCALE.paint_scale_normalized_payload`:
    FreeType-normalized scale payload values, center handling, and nested paint
    handle.
  - `FT_COLR_PAINTFORMAT_SKEW.paint_skew_normalized_payload`:
    FreeType-normalized skew payload values, center handling, and nested paint
    handle.
  - `FT_COLR_PAINTFORMAT_TRANSFORM.explicit_transform_payload`: affine
    `xx`/`xy`/`dx`/`yx`/`yy`/`dy` fields and nested paint handle.
  - `FT_COLR_PAINTFORMAT_TRANSLATE.paint_translate_payload`: translation
    dx/dy payload values and nested paint handle.
- Composite paint graph rows are split by exact obligation instead of sharing a
  broad graph blocker:
  - `FT_PaintComposite.get_paint_composite_values`: source paint, backdrop
    paint, and composite mode fields from the public union.
  - `FT_COLR_PAINTFORMAT_COMPOSITE.paint_composite_payload`: composite payload
    shape and nested opaque paint handles.
  - `FT_Composite_Mode.paint_composite_modes_runtime`: every public composite
    enum value emitted from a valid graph with exact pinned-C numeric values.
  - `FT_COLR_COMPOSITE_MAX.sentinel_not_emitted_by_valid_paint_graph`: prove the
    sentinel is not emitted for valid composite paints.
  - `FT_COLR_COMPOSITE_CLEAR`, `COLOR_BURN`, `COLOR_DODGE`, `DARKEN`, `DEST`,
    `DEST_ATOP`, `DEST_IN`, `DEST_OUT`, `DEST_OVER`, and `DIFFERENCE`: graph
    traversal must emit each mode at the same graph position without skipping
    nested source or backdrop paints.
  - `FT_COLR_COMPOSITE_EXCLUSION`, `HARD_LIGHT`, `HSL_COLOR`, `HSL_HUE`,
    `HSL_LUMINOSITY`, `HSL_SATURATION`, `LIGHTEN`, `MULTIPLY`, `OVERLAY`,
    `PLUS`, `SCREEN`, `SOFT_LIGHT`, `SRC`, `SRC_ATOP`, `SRC_IN`, `SRC_OUT`,
    `SRC_OVER`, and `XOR`: graph construction must expose each mode with exact
    enum value and nested paint handles.
- Clipbox: scaled/transformed box values for size variants and false-with-output
  preservation when no clipbox exists.

Required fix plan:

1. Add a maintained COLR/CPAL route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement the pure-Rust COLR/CPAL parsing, paint graph traversal, palette,
   clipbox, layer, and colorline iterator state first. The C and WASM ABI
   layers may only own handle validation, record copying, and lifetime
   bookkeeping.
3. Compare exact public scalar and record output for paint formats, affine
   transforms, composite modes, color stops, layer iterators, palette counts and
   BGRA entries, foreground-color policy, clipbox coordinates, null-output
   preservation, and end-of-iteration behavior.
4. Keep the already-routed `ftcolor` rejection rows real; do not demote them
   while building the broader color success route.
5. Promote rows only after focused `ftcolor` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
```

### Issue Set Current: `FTC_*` cache subsystem success/lifecycle route placeholders

Status: classified as explicit pending-route on 2026-07-20.

Baseline before this batch:

- Route audit at `cd06d9a3c`: `real-parity=4465`,
  `generic-fallback=429`, `pending-route=85`, `pending-core=7`.

Finding:

- The route audit already has exact real parity for selected FTC cache null/error
  rows and the existing maintained `FTC_Manager_Reset` and
  `FTC_SBitCache_Lookup` success routes.
- The remaining cache manager, image cache, cmap cache, sbit scaler, node,
  scaler descriptor, face-id, and type-contract rows had stayed in
  `generic-fallback` with the reason `no explicit maintained route
  classification`, then under one broad cache subsystem pending reason.
- Those rows are not same-input C/Rust/C-ABI/WASM parity. There is no maintained
  cache subsystem route that constructs a real `FTC_Manager`, installs image,
  cmap, and sbit caches, exercises requester callbacks and `FTC_FaceID`
  identity, performs repeated lookup/removal/reset/unref/done sequences, and
  compares exact public outputs and lifecycle side effects.

Classification change:

- Current route audit has 91 `ftcache.*` pending rows. 89 former broad-cache
  rows now use explicit case-set blockers grouped by behavior surface.
- The existing
  `ftcache.FTC_Node_Unref.null_or_invalid_inputs_noop` row keeps its specific
  foreign-node layout blocker.
- The existing
  `ftcache.FTC_SBitCache_Lookup.missing_bitmap_has_null_buffer` row keeps its
  specific non-C-openable bitmap fixture blocker.
- The route audit count remains stable for this refinement; it changes blocker
  granularity, not the number of accepted parity rows.

Current blocker families:

- `FTC_Manager_New` / `FTC_Manager` allocation and lifecycle rows are split by
  exact obligation instead of sharing a broad manager blocker:
  - `FTC_Manager_New.planned_cache_subsystem_not_out_of_scope`: maintained
    same-input cache-manager route, not an out-of-scope placeholder.
  - `FTC_Manager_New.success_defaults_for_zero_limits`: zero `max_faces`,
    `max_sizes`, and `max_bytes` inputs selecting the same pinned-C default
    cache limits.
  - `FTC_Manager_New.success_custom_limits_and_req_data`: explicit limits and
    requester data stored, forwarded, and reported through lookups.
  - `FTC_Manager_New.lifecycle_create_lookup_reset_done`: create, lookup,
    reset, and done sequencing with the same observable cache state and return
    codes.
  - `FTC_Manager.reset_and_done_lifecycle`: reset clearing cached faces, sizes,
    caches, and nodes while done tears down ownership.
  - `FTC_Manager.owns_faces_sizes_and_cache_nodes`: manager-owned face, size,
    cache, and node lifetimes.
- `FTC_Manager_Done` rows are split by exact obligation instead of sharing a
  broad teardown blocker:
  - `FTC_Manager_Done.planned_cache_subsystem_not_out_of_scope`: maintained
    same-input manager teardown route, not an out-of-scope placeholder.
  - `FTC_Manager_Done.success_destroy_empty_manager`: empty manager release or
    no-op behavior.
  - `FTC_Manager_Done.success_destroy_populated_manager`: cached faces, sizes,
    caches, and nodes destroyed in pinned-C observable order and ownership
    state.
  - `FTC_Manager_Done.success_null_or_invalid_library_noop`: null managers and
    foreign-library ownership cases returning or no-oping exactly like pinned C.
  - `FTC_Manager_Done.node_reference_lifecycle_on_done`: referenced nodes during
    manager teardown keeping or releasing observable cache ownership exactly
    like pinned C.
- `FTC_Manager_LookupFace` / `FTC_FaceID`: pointer identity, first requester
  callback, cached repeat lookup, and current-size behavior.
- `FTC_Manager_LookupSize` / `FTC_Scaler` rows are split by exact obligation
  instead of sharing a broad scaler blocker:
  - `FTC_Manager_LookupSize.planned_cache_subsystem_not_out_of_scope`:
    maintained same-input cache manager route for scaler-based size lookup, not
    an out-of-scope placeholder.
  - `FTC_Manager_LookupSize.success_pixel_size_scaler`: width/height pixel
    sizes selecting the same `FT_Size` metrics and cached size handle.
  - `FTC_Manager_LookupSize.success_point_size_resolution_scaler`: 26.6 point
    sizes plus x/y resolution selecting the same `FT_Size` metrics and cached
    size handle.
  - `FTC_Manager_LookupSize.success_repeat_lookup_cached_size`: repeated scaler
    lookup returning the same cached size identity and output fields.
  - `FTC_ScalerRec.pixel_scaler_uses_integer_pixels`: `pixel=1` interpreting
    width/height as integer pixel sizes.
  - `FTC_ScalerRec.point_scaler_uses_26_6_points_and_resolution`: `pixel=0`
    interpreting width/height as 26.6 point sizes with x/y resolution.
  - `FTC_Scaler.points_to_call_owned_scaler`: public `FTC_Scaler` argument as a
    call-owned descriptor whose pointed fields follow pinned-C copy/consume
    lifetime semantics.
- `FTC_Manager_RemoveFaceID` rows are split by exact obligation instead of
  sharing a broad face-id eviction blocker:
  - `FTC_Manager_RemoveFaceID.planned_cache_subsystem_not_out_of_scope`:
    maintained same-input face-id eviction route, not an out-of-scope
    placeholder.
  - `FTC_Manager_RemoveFaceID.success_removes_unreferenced_face_size_and_nodes`:
    unreferenced face, size, and node entries for the face ID removed.
  - `FTC_Manager_RemoveFaceID.success_referenced_nodes_hidden_until_unref`:
    referenced nodes hidden from future lookup until `FTC_Node_Unref`.
  - `FTC_Manager_RemoveFaceID.success_other_face_ids_unchanged`: eviction of
    one face ID preserving cached faces, sizes, and nodes for other face IDs.
  - `FTC_Manager_RemoveFaceID.success_null_manager_noop`: null manager returning
    or no-oping exactly like pinned C.
  - `FTC_Manager_RemoveFaceID.success_null_or_unknown_face_id`: null or unknown
    face IDs leaving cache state unchanged.
- CMap/Image/SBit cache creation rows are split by exact obligation instead of
  sharing a broad manager-owned-cache blocker:
  - `FTC_CMapCache.manager_owned_opaque_cache`: CMap cache handle ownership,
    stability across lookups, and distinction from caller-owned descriptors.
  - `FTC_ImageCache.manager_owned_opaque_cache`: Image cache handle ownership,
    stability across glyph lookups, and node ownership participation.
  - `FTC_SBitCache.manager_owned_sbit_cache`: SBit cache handle ownership,
    stability across sbit lookups, and node ownership participation.
  - `FTC_CMapCache_New.planned_cache_subsystem_not_out_of_scope`: maintained
    same-input CMap cache creation route, not an out-of-scope placeholder.
  - `FTC_CMapCache_New.success_create_and_destroy_with_manager`: manager-owned
    CMap cache destruction through `FTC_Manager_Done`.
  - `FTC_CMapCache_New.success_multiple_cache_registration_limit`: repeated
    cache registration limit and preservation of prior caches.
  - `FTC_CMapCache_New.lifecycle_after_manager_reset`: manager reset preserves
    the CMap cache handle while clearing cached CMap entries.
  - `FTC_ImageCache_New.planned_cache_subsystem_not_out_of_scope`: maintained
    same-input Image cache creation route, not an out-of-scope placeholder.
  - `FTC_ImageCache_New.success_create_lookup_destroy_lifecycle`: create, glyph
    lookup, node ownership, and manager-driven destroy behavior.
  - `FTC_ImageCache_New.success_manager_reset_preserves_handle`: manager reset
    preserves the Image cache handle while clearing cached glyph and node state.
  - `FTC_SBitCache_New.creates_manager_owned_cache`: SBit cache creation,
    lookup/node lifecycle participation, and manager teardown ownership.
- `FTC_CMapCache_Lookup` rows are split by exact scenario and codepoint variant
  instead of sharing a broad cmap-cache blocker.  Each scenario must be proven
  for `cp65`, `cp1114111`, and `cp57344`:
  - `planned_cache_subsystem_not_out_of_scope`: prove the CMap cache subsystem
    is implemented as a maintained same-input route, not excluded as out of
    scope.
  - `success_lookup_hit_and_repeat_hit`: prove first lookup, repeat lookup,
    glyph index output, requester use, and cache identity.
  - `success_lookup_miss_returns_zero`: prove a missing character lookup returns
    exactly zero without corrupting cache state.
  - `success_negative_cmap_index_uses_current_charmap`: prove `cmap_index=-1`
    uses the face's current charmap.
  - `lifecycle_remove_faceid_and_reset`: prove cache entries are evicted or
    rebuilt after `FTC_Manager_RemoveFaceID` and manager reset exactly like
    pinned C.
- `FTC_ImageCache_Lookup` / `FTC_ImageType` rows are split by exact obligation
  instead of sharing a broad image-type blocker:
  - `FTC_ImageType.points_to_call_owned_descriptor`: public `FTC_ImageType`
    argument as a call-owned descriptor whose fields follow pinned-C
    copy/consume lifetime semantics.
  - `FTC_ImageTypeRec.drives_image_and_sbit_lookup`: `face_id`, width, height,
    flags, and load flags driving image and sbit cache lookup.
  - `FTC_ImageCache_Lookup.planned_cache_subsystem_not_out_of_scope`:
    maintained same-input image-cache route, not an out-of-scope placeholder.
  - `FTC_ImageCache_Lookup.success_lookup_hit_and_repeat_hit`: first lookup,
    repeat lookup, glyph output, requester use, and cache identity.
  - `FTC_ImageCache_Lookup.success_node_acquire_and_unref`: `anode`
    acquisition, `FTC_Node_Unref` release, and post-unref cache state.
  - `FTC_ImageCache_Lookup.success_null_anode_ephemeral_glyph`: null `anode`
    returning an ephemeral glyph with pinned-C ownership and cache-node side
    effects.
- `FTC_ImageCache_LookupScaler` rows are split by exact scenario and fixture
  variant instead of sharing a broad scaler-image blocker.  Each scenario must
  be proven for `f1 DEFAULT`, `f2 NO_HINTING`, `f3 RENDER`, and
  `f4 HIGH_BITS_SET`:
  - `planned_cache_subsystem_not_out_of_scope`: prove the cache subsystem is
    implemented as a maintained same-input route, not excluded as out of scope.
  - `success_pixel_and_point_scalers`: prove integer pixel sizes and 26.6 point
    sizes with x/y resolution select the same `FT_Size` metrics and glyph
    output as pinned C.
  - `success_lookup_hit_miss_and_repeated`: prove first lookup, repeat lookup,
    and missing glyph behavior match pinned C cache node identity and output.
  - `success_node_acquire_and_unref`: prove `anode` acquisition,
    `FTC_Node_Unref` release, and post-unref cache state.
  - `load_flags_truncation_policy`: prove `FT_ULong` input is truncated to the
    pinned C signed `load_flags` path before lookup.
- `FTC_SBitCache_LookupScaler`: scaler size semantics and int32 load-flag
  truncation for all concrete font variants.
- `FTC_Node` / `FTC_Node_Unref`: cache handle identity, lookup references,
  unref release, and flushability after the final reference.

Required fix plan:

1. Add a maintained FTC cache route instead of per-row expected output
   shortcuts. It must run the same operation sequence through pinned C
   FreeType, Rust FFI, thin C ABI, and WASM ABI.
2. Implement the pure-Rust cache manager/cache/node/requester state first. The
   C and WASM ABI layers may only own handle validation, record copying, and
   lifetime bookkeeping.
3. Compare exact return codes, glyph/index outputs, cache-owned descriptor
   nullness and stability, node reference behavior, requester invocation counts,
   reset/remove-face-id/done effects, scaler size interpretation, and load flag
   truncation.
4. Keep the already-routed FTC null/error rows, `FTC_Manager_Reset`, and
   maintained `FTC_SBitCache_Lookup` rows real; do not demote them while
   building the broader cache route.
5. Promote rows only after focused `ftcache` runtime proves exact C oracle,
   Rust FFI, C ABI, and WASM ABI output for the same input.

Verification for the classification batch:

```bash
make -C pillow-rs-freetype route-audit
```
