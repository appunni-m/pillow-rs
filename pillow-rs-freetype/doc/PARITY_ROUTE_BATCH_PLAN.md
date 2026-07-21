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
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 350, 'real-null-validation': 9, 'real-parity': 4610}
runtime_parity: passed=6880 failed=0 total=6880 covered_manifest_cases=3787
runtime_cases: runnable=6880 pending=355
```

This baseline means the maintained same-input routes are green; it does not mean
full public API parity is complete. The 350 route-pending rows remain outside
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
- `input/fonts/cff/fontinfo-populated.otf` is not a valid
  `FT_Get_PS_Font_Info` success row for the current pinned build; pinned C
  returned error `7` while Rust returned success. This remains implementation
  work, not a green fixture promotion.

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

After the malformed `TT_MaxProfile` route:

```text
route audit concrete_cases=7235 category_counts={'compile-contract': 2266, 'pending-route': 349, 'real-null-validation': 9, 'real-parity': 4611}
runtime_parity: passed=1 failed=0 total=1 covered_manifest_cases=1
runtime_cases: runnable=1 pending=0
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
