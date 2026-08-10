# Binding and dynamic raster coverage audit (v1)

Status: read-first audit complete. No source, manifest, fixture, fontdone, or
input files were changed. No parity batch was selected or submitted.

## Scope and evidence

- Worktree: `coverage-audit-bindings-v1-20260811`
- Branch: `codex/coverage-audit-bindings-v1-20260811`
- Checkout: `f4a5b9230931b3f394940a88db72c4d10359dc29`
- Audited files only: `pillow-rs-py/src/lib.rs` and
  `pillow-rs/src/raster/dynamic.rs`
- Current Coverage MCP snapshot: `3a3fb5d9-ba8b-417e-9870-aaec2d27962e`
  (`migration-parity-rust`, LLVM, branch `main`, commit `f4a5b9230`)
- Compare baseline: `f14bdccb-debc-4c65-826d-36277c72d319` at
  `bec899a5e38902642ca6347bd13e347ef28686bc`

The manifest declares `image-core` for the Python facade and core image/ops
paths, but not either audited file. `pillow-rs/src/lib.rs` is listed under a
separate `image-font` component; that does not manage `raster/dynamic.rs`.
The file rates below are therefore LLVM observations, not managed component
denominators or threshold claims.

| File | Current snapshot | Baseline -> current |
| --- | --- | --- |
| `pillow-rs-py/src/lib.rs` | 3,113/3,647 lines; 281/350 branches; 400/502 functions; 4,653/5,907 regions | unchanged |
| `pillow-rs/src/raster/dynamic.rs` | 388/865 lines; 2/4 branches; 60/119 functions; 741/1,636 regions | 355 -> 388 lines; 54 -> 60 functions; 684 -> 741 regions; branches unchanged |

## Binding gaps and reachability

Coverage MCP line queries report these exact gaps in `pillow-rs-py/src/lib.rs`:

| Public/low-level area | Exact observed gap | Reachability conclusion |
| --- | --- | --- |
| `Image.open` | `675`; `681` partial (1/2 branches); `688-691` | The manifest declares `fp` as bytes, path, or stream. The path branch is covered; the stream read is not. `675` is an `unreachable!` after input validation and is not valid public input. |
| `Image.filter` | `862` partial (1/2 branches); `863` | The manifest accepts a filter instance or class. Existing generated workflows use instances, so the callable-class branch is a real public-shape gap. Line `886` is a zero-hit struct-literal source line inside an otherwise covered path (`881-883`, `887-889`), not an independent route. |
| `Image.Image.tobytes` helpers | `925-927`, `933-935` | The public facade calls `tobytes_encoded` (`936-945`), not these private `_core` helpers. A direct `_core` call would be an unsupported probe. |
| Palette/transparency helpers | `948-950`, `952-954`, `956-958`, `960-962`, `976-978`, `984-987`, `989`, `991-994`, `996`, `998-1000` | These are private binding helpers. Public `getpalette` uses `getpalette_with_input` (`964-974`); no manifest endpoint calls the listed helpers directly. |
| Unmapped utilities | `backend_enabled`: `1984-1987`, `1992`; `align_row_to_32`: `2000-2002` | No maintained parity-manifest endpoint or exact public workflow was found. No synthetic utility calls were added. |

For `Image.open`, `scripts/build_migration_parity_inputs.py` special-cases
`fp` to an encoded path. Its generic stream fallback is the builtin empty
`io.BytesIO()` from `scripts/run_migration_parity.py`. A valid seeded encoded
stream would require a reviewed input-runner asset and exact error/result
parity; the existing empty stream is not such a route. The maintained save
stream case is separate and already covers the write path.

## `DynamicImage` gaps and reachability

The following are the exact uncovered line ranges returned by bounded
Coverage MCP queries for `pillow-rs/src/raster/dynamic.rs`:

- Clone and constructors: `138-142`, `146-158`, `160`, `166-171`, `174-183`,
  `185`, `189-191`, `195-197`, `201-203`, `213-215`, `219-221`, `225-227`,
  `231-233`, `237-239`, `243-245`.
- Typed conversions: `291-296`, `307-311`, `322-326`, `328`, `334-337`,
  `339`, `343-346`, `348`, `355`, `361-364`, `366`, `370-373`, `375`,
  `379-382`, `384`, `406-411`.
- Typed accessors and metadata: `511-514`, `516`, `522`, `528-531`, `533`,
  `539`, `545-548`, `550`, `553-556`, `558`, `562-565`, `567`, `573`,
  `579-582`, `584`, `587-590`, `592`, `596-599`, `601`, `604-607`, `609`,
  `613-616`, `618`, `624`, `630-633`, `635`, `638-641`, `643`, `647-650`,
  `652`, `655-658`, `660`, `664-667`, `669`, `672-675`, `677`, `704-708`,
  `721-725`, `743-745`.
- Typed geometry: `776-780`, `813-817`, `843-847`, `850-854`, `899-903`,
  `929-933`, `936-937`, `939-943`, `1002-1007`, `1010`, `1012-1017`, `1020`,
  `1022-1027`, `1030`, `1032-1037`, `1040`, `1042-1047`, `1050`.
- `GenericImage` trait arms: `1243-1244`, `1249-1251`, `1253-1255`,
  `1257-1259`, `1264-1267`, `1269-1273`, `1275`, `1277-1282`, `1284`,
  `1286-1291`, `1293`, `1295-1299`, `1302`, `1304-1308`, `1311`, `1317`,
  `1319-1333`, `1335-1336`.

The maintained typed-PNG corpus has a valid `L16` asset and public crop,
resize, rotate, convert, transpose, load, and pixel cases. Those cases reach
the native `ImageLuma16` arms. The pinned decoder route does not produce
`ImageLumaA16`, `ImageRgb16`, `ImageRgba16`, `ImageRgb32F`, or
`ImageRgba32F` from the allowed public PNG inputs; constructing those enum
variants directly would be a synthetic core probe. The public
`PIL.Image.Image.putpixel` route calls `Image::putpixel_value` in
`pillow-rs/src/image.rs`, not `DynamicImage::put_pixel`, so the trait arms are
not a valid public-parity route either.

Constructors, typed accessors, conversions, and the remaining typed geometry
arms therefore have no managed public input route in this snapshot. They need
a separately declared component or a reviewed direct-core test lane; adding
fabricated buffers, excluded TIFF/GPU/crash inputs, or fontdone work would not
be exact public parity.

## Batch decision and commands

Candidate batch: **none**. There is no managed component for either target
file, and the only plausible new binding case (a valid `Image.open` stream)
has no exact input-runner asset. No synthetic probe was retained.

Read-only evidence used:

- Coverage MCP `project_context`, `coverage_query` (`summary` and bounded
  `file`/line-range views), and `coverage_compare` (`files`) only.
- `git rev-parse HEAD`, `git status --short --branch`, `rg`, `nl`, and `sed`
  reads of the manifest, input generator, runner, Python facade, and audited
  Rust files.
- No `coverage show`, test submission, fixture generation, GPU, crash,
  pending-TIFF, or fontdone lane was run.
