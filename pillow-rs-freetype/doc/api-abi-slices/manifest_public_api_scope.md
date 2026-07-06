# Manifest Public API Scope

`tests/manifest.yaml` is the coverage catalog for the C public FreeType
interface. It is not a Rust implementation catalog and must not contain local
helper subjects such as `fontdone.ffi.*`.

The manifest subject id is the stable public identity for a C component. It
must be unique and should preserve the C header namespace:

```text
freetype.FT_Load_Glyph
ftmm.FT_Set_MM_WeightVector
ftcache.FTC_Manager_New
ttnameid.TT_PLATFORM_MICROSOFT
```

The `freetype` namespace is reserved for declarations from
`freetype/freetype.h`. Other public headers use their file stem.

## Included Surface

The manifest covers these C public API components:

1. Public C functions.
2. Public typedefs.
3. Public structs and records.
4. Public enums.
5. Public enum variants, constants, flags, error codes, and tags exposed as
   macros.
6. Selected public function-like macros that users call as API.

The current generated API/ABI audit reports these raw C buckets:

```text
c_functions = 218
c_macros    = 1394
c_typedefs  = 92
c_structs   = 110
c_enums     = 29
```

The function bucket is already concrete: all 218 public `FT_EXPORT(...)`
functions are manifest subjects.

The type bucket must be represented by public typedef, struct, and enum names.
Record layout and enum variant checks are cases under those subjects; fields
are not separate subjects.

The macro bucket must be classified before expansion. FreeType exposes many
user-facing constants as macros, but the raw macro list also contains header
aliases, build options, compiler helpers, declaration helpers, and internal
plumbing. Those are not parity subjects.

## Excluded Surface

Do not add manifest subjects for:

- `freetype/internal/**` declarations.
- local Rust helpers or conversion functions.
- include/header alias macros such as `FT_FREETYPE_H`.
- compiler/export/plumbing macros such as `FT_EXPORT`.
- config/build-option macros such as `FT_CONFIG_OPTION_*`.
- implementation declaration helpers such as `FT_DEFINE_*` and `FT_DECLARE_*`.
- struct fields, function parameters, or record members as standalone subjects.

If one of these affects public behavior, it should appear as a case or shape
metadata under the owning public subject instead.

## Cases

Every subject must have at least one case. A case defines the reason that the
subject is covered, not the runtime output shape.

Function subjects that are not implemented yet still need a manifest case:

```yaml
- id: ftmm.FT_Set_MM_WeightVector
  kind: function
  symbol: FT_Set_MM_WeightVector
  header: freetype/ftmm.h
  cases:
    - id: migration_surface
      description: Enumerates the pinned C function for migration coverage.
```

When runtime parity inputs exist, they reference the same `subject` and one of
its manifest case ids. Inputs may carry comparison/output shape details because
those details vary by test row.

## Duplicate Rules

The manifest parser must reject:

- duplicate subject ids.
- duplicate `symbol:` values.
- duplicate case ids within the same subject.
- fixture input references to unknown subject/case pairs.

This prevents silent collapse from map/set parsing and keeps the manifest as a
literal coverage catalog.

## Current Manifest State

After the public-C interface expansion:

```text
manifest subjects    = 1544
function subjects    = 218
type subjects        = 62
record subjects      = 78
enum subjects        = 20
enum variant subjects = 158
constant subjects    = 365
flag subjects        = 122
error subjects       = 120
tag subjects         = 370
macro subjects       = 31
fontdone subjects    = 0
```

Every manifest subject has at least one case. The manifest is now the
source-of-truth catalog for the classified public C interface described above.

## Expansion Plan

1. Freeze the classifier rules.
   - Keep `FT_EXPORT(...)` functions as `kind: function`.
   - Split the type audit into `kind: type`, `kind: record`, and `kind: enum`.
   - Split macros into `kind: constant`, `kind: flag`, `kind: error`,
     `kind: tag`, `kind: enum_variant`, `kind: macro`, and `excluded`.
   - Record each excluded macro with a reason outside the manifest, not as a
     manifest subject.

2. Generate a candidate catalog from pinned headers.
   - Use header namespace ids.
   - Keep `symbol` as the C spelling.
   - Keep `header` as the direct public header path.
   - Do not generate verbose declaration blocks in the manifest.

3. Review macro classification by header family.
   - `fterrors.h` and `fterrdef.h`: error code subjects.
   - `ftmoderr.h`: module error subjects.
   - `ttnameid.h`: TrueType/OpenType platform, encoding, language, name, and
     Unicode-range constants.
   - `tttags.h`: table tag constants.
   - `freetype.h`: face flags, load flags, render modes, kerning modes, style
     flags, open flags, subglyph flags, FSType flags, encoding tags.
   - `ftimage.h`: pixel modes, glyph formats, outline flags, curve tags.
   - `ftmm.h`, `ftcolor.h`, `ftgxval.h`, `ftotval.h`, `ftparams.h`, and
     related public headers: feature flags and public parameter tags.

4. Add missing manifest subjects in batches.
   - Types/records/enums first because function signatures depend on them.
   - Error and module-error constants next.
   - Flags/tags/enumeration macros next.
   - Selected function-like public macros last.

5. Preserve cases during expansion.
   - Existing runtime parity cases stay attached to their public C subjects.
   - New subjects receive at least `migration_surface`, `value_matches_header`,
     `layout_matches_c`, or `enum_variants_match_header` as appropriate.
   - Font variability remains opt-in and only appears on cases where font
     choice affects output.

6. Verify after each batch.
   - Run the maintained unified coverage target.
   - Check subject/symbol/case uniqueness.
   - Check every manifest subject has at least one case.
   - Check every fixture input references a known subject/case.
   - Report remaining missing public-C buckets by category.
