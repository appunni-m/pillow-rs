# CID parity batch plan

Objective: exact same-input parity for the public CID endpoints through pinned
C FreeType, the safe Rust FFI surface, the thin C ABI, and the WASM ABI.

This plan exists because the current route audit still has CID rows that are
declared as semantic success cases but are not backed by real CID inputs or
implemented public endpoints. Counting them now would be a green placeholder.

## Current evidence

Update on 2026-07-22:

- Added the smaller Adobe `FDArrayTest257.otf` as
  `tests/fixtures/input/fonts/cid/ot-cff-cid-keyed.otf`.
- Recorded OFL-1.1 license and provenance next to the fixture.
- Promoted only the SFNT-wrapped CID rows to real runtime candidates:
  `FT_Get_CID_From_Glyph_Index.opentype_cid_face_supported` and
  `FT_Get_CID_Is_Internally_CID_Keyed.sfnt_wrapped_cid_supported`.
- First divergence: after promotion, Rust returned the generic fallback error
  for CID success rows because core CFF parsing did not preserve ROS metadata or
  charset CID mappings and the Rust/C/WASM FFI surfaces did not expose the CID
  endpoints.
- C behavior: pinned FreeType reports `FDArrayTest257.otf` as internally
  CID-keyed through the CID service and maps GID 0/1/last to exact CIDs, while
  `FT_IS_CID_KEYED(face)` remains false for this SFNT-wrapped path.
- Rust behavior before the fix: no CFF CID metadata, no glyph-index-to-CID map,
  no public CID wrapper exports, and generic error output for the promoted rows.
- Current fix: safe Rust parses CFF ROS and charset CID data, Rust FFI owns
  face-lifetime ROS strings and CID outputs, C/WASM wrappers remain thin, and
  the pinned C oracle records exact CID service output.
- Verified result: route audit moved `pending-route` from 233 to 229 and
  `real-parity` from 4754 to 4758 at the current branch baseline. Full
  `make fontdone-test` passes with `runtime_parity: passed=7028 failed=0
  total=7028` and `pending=234`.
- Non-SFNT Type1 CID rows remain pending until a separate Type1 CID fixture is
  available and opens in pinned C.
- `FT_Get_CID_Registry_Ordering_Supplement.success_cid_keyed_face` remains a
  follow-up route promotion even though the thin Rust/C/WASM endpoint surface is
  now present; it still needs unified harness output wiring before it can count
  as real parity.

Baseline from `make -C pillow-rs-freetype route-buckets` on `main` at
`9d07011f1`:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 293, 'real-null-validation': 9, 'real-parity': 4670}
```

Focused probe:

```bash
make -C pillow-rs-freetype test-op OP=ftcid.get_cid_from_glyph_index
```

Result:

```text
runtime_parity_progress: compared=1 total=1 passed=1 failed=0
runtime_cases: runnable=1 pending=7
pending_reasons=ftcid.get_cid_from_glyph_index:declared semantic row has no maintained runtime-resolved input; exact runtime parity requires the declared same input to execute against pinned C, Rust FFI, thin C ABI, and WASM ABI; counting the selection-skipped row as real parity would be a green placeholder:7
```

The current repo-local files under `tests/fixtures/fonts/cid/` are symlinks to
`../../input/fonts/DejaVuSans.ttf`. They are not CID-keyed fixtures and must not
be used to satisfy CID success rows.

The public endpoint implementation is also absent from the Rust FFI export set:

- `FT_Get_CID_From_Glyph_Index`
- `FT_Get_CID_Is_Internally_CID_Keyed`
- `FT_Get_CID_Registry_Ordering_Supplement`

`src/tt/cff.rs` currently documents and implements only a minimal non-CID
OpenType CFF/Type2 shape. It does not parse CFF CID Top DICT fields, charset CID
mapping, FDSelect/FDArray, or Registry/Ordering/Supplement metadata.

## External fixture candidates

Internet search found `adobe-fonts/fdarray-test`, whose README describes two
special-purpose CID-keyed OpenType/CFF fonts based on Adobe-Identity-0 ROS and
states the license is OFL-1.1. The smaller `FDArrayTest257.otf` is the preferred
candidate for SFNT-wrapped CID success rows because its README gives
deterministic GID assignment: GID+0 is `.notdef`, and GID+1 through GID+256 map
to FDArray elements 0 through 255.

Use this only with explicit provenance recorded next to the fixture:

- upstream: `https://github.com/adobe-fonts/fdarray-test`
- fixture candidate: `FDArrayTest257.otf`
- license: OFL-1.1 from the upstream repository
- required local note: source URL, upstream commit or immutable release/tag,
  SHA-256 of the stored fixture, and why this font is CID-keyed

Do not import large Source Han or Noto CJK assets unless a small test font cannot
prove the rows. The parity harness needs deterministic service outputs, not a
large production font.

## Batch scope

First batch should target the SFNT-wrapped CID path only:

- `ftcid.FT_Get_CID_Is_Internally_CID_Keyed.sfnt_wrapped_cid_supported`
- `ftcid.FT_Get_CID_From_Glyph_Index.opentype_cid_face_supported`
- `ftcid.FT_Get_CID_Registry_Ordering_Supplement.success_cid_keyed_face`

Leave non-SFNT Type1 CID rows pending until a separate small Type1/CID fixture is
available and opened by pinned C FreeType:

- `ftcid.FT_Get_CID_From_Glyph_Index.cid_face_returns_cid`
- `ftcid.FT_Get_CID_From_Glyph_Index.null_cid_output_matches_c`
- `ftcid.FT_Get_CID_Is_Internally_CID_Keyed.cid_face_reports_true`
- `ftcid.FT_Get_CID_Is_Internally_CID_Keyed.null_output_matches_c`

## Implementation requirements

1. Add a maintained fixture acquisition or generation workflow.
   - Do not leave a manual download step in chat only.
   - Prefer a script under `pillow-rs-freetype/scripts/` plus a Make target.
   - Record fixture provenance and SHA-256.
2. Extend core CFF metadata parsing in safe Rust.
   - Parse enough CFF Top DICT / ROS / charset data to identify internally
     CID-keyed OpenType/CFF fonts.
   - Preserve existing non-CID CFF behavior.
   - Do not call native FreeType.
3. Add safe Rust FFI public functions.
   - Match FreeType null-pointer and non-CID error behavior.
   - Return face-owned string pointers for registry/ordering. The ownership
     must be tied to the face handle, not temporary stack or test-only storage.
   - Return exact glyph-index-to-CID mapping for the selected fixture.
4. Add thin C ABI exports.
   - C ABI must only validate raw pointers, copy ABI records/strings, and call
     core Rust FFI.
   - No CID parsing or fixture-specific behavior in `ffi-c`.
5. Add thin WASM ABI exports.
   - WASM must expose the same observable values as the Rust FFI/C ABI route.
   - No CID parsing or fixture-specific behavior in `ffi-wasm`.
6. Extend the pinned C oracle route and unified parity runner.
   - Compare error code, output write bitmap, boolean values, registry/ordering
     strings, supplement, glyph index, and CID value.
   - Compare all four surfaces: pinned C, Rust FFI, thin C ABI, WASM ABI.

## Verification gates

Focused:

```bash
make -C pillow-rs-freetype test-op OP=ftcid.get_cid_is_internally_cid_keyed
make -C pillow-rs-freetype test-op OP=ftcid.get_cid_from_glyph_index
make -C pillow-rs-freetype test-op OP=ftcid.get_cid_registry_ordering_supplement
```

Route and wrapper gates:

```bash
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
```

Only promote CID implementation rows as real parity when the route audit shows
real parity increased and the focused rows compare exact outputs through all
required surfaces.
