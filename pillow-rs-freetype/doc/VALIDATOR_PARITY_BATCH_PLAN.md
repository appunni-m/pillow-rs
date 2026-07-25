# GX/OpenType validator parity batch plan

Objective: exact same-input parity for FreeType validator public endpoints
through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI.

This surface must not be counted using service-missing fallbacks or table-shaped
placeholder fonts. Validator success rows require the pinned C oracle to execute
the same input and expose the same selected-table output/free behavior.

## Current evidence

Baseline from `make -C pillow-rs-freetype route-buckets` on `main` at
`9d07011f1`:

```text
route audit concrete_cases=7238 category_counts={'compile-contract': 2266, 'pending-route': 293, 'real-null-validation': 9, 'real-parity': 4670}
```

Largest validator pending buckets:

```text
16 operation=FT_TrueTypeGX_Validate input_hash=f00401408ec2
 9 operation=ftgxval.truetype_gx_validate input_hash=3a80cf04a98b
 9 operation=ftotval.open_type_validate input_hash=515dd75e7840
 7 operation=ftotval.open_type_validate input_hash=549568b44082
```

Focused OpenType validator probe:

```bash
make -C pillow-rs-freetype test-op OP=ftotval.open_type_validate
```

Result:

```text
runtime_parity_progress: compared=3 total=3 passed=3 failed=0
runtime_cases: runnable=3 pending=17
```

The three runnable rows are null-face/null-output/service-missing behavior. The
seventeen pending rows are blocked by missing OpenType validator fixtures or by
an oracle-build mismatch. One declared BASE absent-table row expects
success/null-output behavior, but the pinned FreeType oracle currently returns
`FT_Err_Unimplemented_Feature (7)` before table absence can be observed.

Pinned FreeType module evidence:

```text
freetype/modules.cfg: AUX_MODULES += gxvalid is commented out
freetype/modules.cfg: AUX_MODULES += otvalid is commented out
freetype/include/freetype/config/ftmodule.h has no gxvalid or otvalid module
```

Therefore success/malformed validator rows cannot be promoted under the current
oracle build contract.

## Required decision before implementation

Choose and document one oracle contract:

1. Keep the current pinned FreeType build without `gxvalid`/`otvalid`.
   - Then success/malformed validator rows must remain pending or be
     reclassified as build-dependent unavailable rows.
   - Do not implement Rust table validation to make rows green when pinned C
     reports `FT_Err_Unimplemented_Feature`.
2. Enable the same validator modules in the pinned C oracle build and in the
   public contract.
   - Then regenerate/verify oracle behavior for success, malformed, selected
     table output, partial cleanup, and free semantics.
   - Rust must implement matching behavior in safe Rust before rows count.

This choice is not a test weakening step; it defines which FreeType build is the
version-matched oracle for these public APIs.

## Missing fixture set

Current declared OpenType validator fixtures are absent:

- `fonts/opentype/valid-all-layout.otf`
- `fonts/opentype/valid-base.otf`
- `fonts/opentype/valid-gdef.otf`
- `fonts/opentype/valid-gpos.otf`
- `fonts/opentype/valid-gsub.otf`
- `fonts/opentype/valid-jstf.otf`
- `fonts/opentype/valid-math.otf`
- `fonts/opentype/malformed-gdef.otf`
- `fonts/opentype/malformed-gpos.otf`
- `fonts/opentype/malformed-gsub.otf`
- `fonts/opentype/malformed-selected-layout.otf`
- `fonts/opentype/partial-malformed-layout.otf`

Internet search found two viable fixture directions:

- `simoncozens/test-fonts` for OpenType layout examples.
- FontTools `otlLib` for generated GSUB/GPOS-style tables.

Any imported fixture must have license/provenance recorded. Any generated
fixture must have a maintained script under `pillow-rs-freetype/scripts/` and a
Make target. Do not hand-edit binary fixtures without a reproducible generator.

## Implementation requirements if validators are enabled

1. Update `scripts/build_ft.sh` only if the chosen contract is to enable
   validator modules for the pinned oracle. Record the reason in this file.
2. Add maintained fixtures.
   - Success fixtures for all selected table flags.
   - Malformed fixtures for each malformed-table public error row.
   - Partial failure fixture proving output cleanup/free behavior.
3. Implement safe Rust validator behavior.
   - `FT_OpenType_Validate` must return exact selected table buffers or exact
     errors.
   - `FT_OpenType_Free` must release only buffers owned by the validation call.
   - `FT_TrueTypeGX_Validate`, `FT_TrueTypeGX_Free`,
     `FT_ClassicKern_Validate`, and `FT_ClassicKern_Free` must match the chosen
     C module behavior.
4. Keep wrappers thin.
   - C ABI and WASM ABI may validate pointers and marshal records only.
   - They must not parse OpenType/GX tables or encode parity-specific behavior.
5. Extend the unified oracle and runner.
   - Compare error code, selected output slot nullness, table byte hashes/lengths,
     write bitmap, and free lifecycle.
   - Do not compare only `FT_Error` for success rows.

## Verification gates

Focused:

```bash
make -C pillow-rs-freetype test-op OP=ftotval.open_type_validate
make -C pillow-rs-freetype test-op OP=ftgxval.truetype_gx_validate
make -C pillow-rs-freetype test-op OP=ftgxval.classic_kern_validate
```

Route and wrapper gates:

```bash
make -C pillow-rs-freetype route-buckets
make fontdone-ffi
make fontdone-ffi-compat
```

Only commit validator implementation when the same inputs execute through pinned
C, Rust FFI, thin C ABI, and WASM ABI with exact selected-table/free outputs.
