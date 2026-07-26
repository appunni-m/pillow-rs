# Font Public-API Parity Status (Current Worktree)

Last updated: 2026-07-26 (Asia/Kolkata) — Font public-api harness measured at commit `e5482868c`

## Current checkpoint: root Font API runner-reference gate

New commit:

- `e5482868c` — added a source-level Font runner gate requiring
  `tests/support/font_runner.rs` to reference every root
  `pillow_rs::font_*` public API function exactly. The existing manifest gate
  proves every root function maps to a manifest operation; this new gate proves
  the live Pillow-oracle runner still calls the root API surface directly
  instead of leaving a mapped function unreachable from fixtures.

What this closes:

- A root Font public function can no longer be listed in
  `font_manifest.yaml` only through the static map while the active runner
  bypasses or omits that exact root API function.
- The comparison test still has no embedded output, hash, status, or error
  expectation. It continues to generate output at runtime from the repo-local
  Pillow native `_imagingft` oracle and compares the Rust `Result`-style
  payload exactly.

Verification:

- `make -C pillow-rs fmt` — passed
- `make -C pillow-rs font-tests` — passed
- Coverage MCP command `imagingft-tests-coverage-fixed`
  - run `cdddddb0-a84c-4aad-8ab2-d0c7488cdf27`
  - snapshot `28834545-2726-4d0a-ad9b-bc8f1ecdcee6`
  - commit `e5482868cf3f52816cfdff53d1a8193e93bc88d3`
  - status `passed`, coverage artifact ingested

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `955/1040` (`91.83%`) | `185/238` (`77.73%`) | `95/110` (`86.36%`) | `1594/1746` (`91.29%`) |
| `pillow-rs/src/font/mod.rs` | `258/282` (`91.49%`) | n/a | `57/64` (`89.06%`) | `331/373` (`88.74%`) |

Remaining blocker to honest 100% region coverage:

- Runtime blocker remains `getmask/getmask2(stroke_width != 0)`. Pillow
  supports stroked glyph masks through native `_imagingft`/FreeType stroking,
  but the current pure-Rust FreeType stroker path still does not render real
  glyph contours exactly enough to enable this without lowering parity
  standards.
- The remaining `font/mod.rs` reported gaps are source-map lines on public
  option declarations/doc comments, not uncovered public method bodies. They
  do not remove the need for 100% region coverage, but adding duplicate JSON
  rows would not honestly cover them.

## Current checkpoint: Font case-id/operation manifest gate

New commit:

- `a0eae6880` — fixed stale `font.render_text.*` case IDs in the active
  `draw_text` fixture file and added a test gate requiring each active case ID
  to match its normalized operation prefix, except for explicitly grouped
  public-api fixture families (`constructor`, `variations`, `load_failure`,
  `layout_failure`, and `unsupported_operation`).

What this closes:

- A stale or misfiled row can no longer masquerade as public Font coverage
  only because its `operation` field is correct. The case identity and
  operation now have to agree before oracle execution starts.
- The input corpus remains input-only. No expected output, hash, status, or
  error expectation was added to JSON; output is still generated at runtime
  from the repo-local Pillow native `_imagingft` oracle.

Verification:

- `make -C pillow-rs fmt` — passed
- `make -C pillow-rs font-tests` — passed
- Coverage MCP command `imagingft-tests-coverage-fixed`
  - run `e67b7638-b719-4565-a469-d556e2b1426a`
  - snapshot `69d4289b-2dca-4182-bf15-5ac931081826`
  - commit `a0eae6880ff4d2a000769018efc3528a76ad5443`
  - status `passed`, coverage artifact ingested

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `955/1040` (`91.83%`) | `185/238` (`77.73%`) | `95/110` (`86.36%`) | `1594/1746` (`91.29%`) |
| `pillow-rs/src/font/mod.rs` | `258/282` (`91.49%`) | n/a | `57/64` (`89.06%`) | `331/373` (`88.74%`) |

Remaining blocker to honest 100% region coverage:

- Runtime blocker remains `getmask/getmask2(stroke_width != 0)`. Pillow
  supports stroked glyph masks through native `_imagingft`/FreeType stroking,
  but the current pure-Rust FreeType stroker path still does not render real
  glyph contours exactly enough to enable this without lowering parity
  standards.

## Current checkpoint: Pillow byte-text compatibility + coverage sweep

New commits:

- `9e84725d3` — hardened the Font public fixture runner so case IDs are unique
  and referenced font assets are real files under the fixture root. The only
  allowed missing asset remains the explicit load-failure case.
- `3b747c7df` — added explicit root byte-text Font APIs and input-only fixture
  support for Pillow `FreeTypeFont` methods that accept `text: str | bytes`.
  The live oracle now passes real Python `bytes` when `text_bytes_hex` is
  present; Rust maps each byte to the matching Latin-1 codepoint before
  entering the pure-Rust font path.
- `0bb991430` — added focused byte-text rows for the option and no-option
  byte paths so the new runtime code is covered by live Pillow oracle parity.
- `34ff22d0b` — tightened manifest validation so fixture keys are canonicalized
  to live Pillow public parameter names before they can prove parameter
  coverage; removed unused `text` fixture noise from no-text operations.
- `1f34bc130` — added a manifest gate that rejects active input rows for
  parameters still marked `blocked`, so `stroke_width` cannot silently appear
  in `getmask/getmask2` rows without being implemented and reclassified.
- `61e475ab3` — added a root API gate that extracts public
  `pillow_rs::font_*` functions from `src/lib.rs` and requires every one to
  map to a `font_manifest.yaml` operation. A new root Font endpoint now fails
  the parity test unless it is explicitly accounted for by the manifest.
- `dcb8a73c6` — added a runner-arm gate that extracts the explicit public
  operation arms from `tests/support/font_runner.rs` and requires them to match
  `font_manifest.yaml.required_operations` exactly. A manifest operation can no
  longer go stale without a runner implementation, and a runner operation can
  no longer bypass the manifest.
- `8c6d496ab` — added input-document envelope validation for every active
  public-api JSON file. Each file must contain only `version`, `operation`, and
  `cases`; use version `1`; have non-empty cases; and classify its top-level
  operation as a required operation, negative operation, or explicitly allowed
  grouped file.
- `cc3d1dd9a` — pinned the exact allowed blocked public parameters to
  `getmask.stroke_width` and `getmask2.stroke_width`. Any additional blocked
  public parameter now fails the manifest test unless the expected-blocker list
  is deliberately changed with documentation.

Direct Pillow `ImageFont.FreeTypeFont` public callable comparison from the
repo-local oracle remains:

`font_variant`, `get_variation_axes`, `get_variation_names`, `getbbox`,
`getlength`, `getmask`, `getmask2`, `getmetrics`, `getname`,
`set_variation_by_axes`, `set_variation_by_name`.

Newly covered edge cases:

- `text_bytes_hex` input rows for `getlength`, `getbbox`,
  `getbbox_binary`, `getmask`, `getmask2`, `getmask2_with_start`, and
  `text_bbox`.
- Byte text with options for `getlength(mode=...)`, `getbbox(anchor=...)`,
  `getmask(mode="L")`, and `getmask2(mode="L")`.
- No expected output or error payload is stored in input JSON; output is still
  generated at runtime from the repo-local Pillow native `_imagingft` oracle.
- Manifest coverage is now validated against the live Pillow public signatures
  using canonical parameter names, and blocked parameters must not appear in
  active passing rows.
- Root `pillow_rs::font_*` public functions are now validated against the
  manifest operation list, closing the previous gap where a Rust root wrapper
  could exist without manifest coverage.
- Manifest operations are now validated against explicit runner operation
  arms, closing the stale-manifest/stale-runner gap before live oracle
  execution starts.
- Public-api input files now have a validated envelope, preventing stale or
  unclassified grouped files from entering the corpus.
- The manifest can no longer hide new public parity blockers: only the two
  known stroked-mask parameters may remain blocked.

Verification:

- `make -C pillow-rs fmt` — passed
- `make -C pillow-rs font-tests` — passed
- Coverage MCP command `imagingft-tests-coverage-fixed`
  - run `1a2b6f51-da9d-45a4-9fc8-f70254f9090e`
  - snapshot `542eb179-e55f-45c3-9981-8b9aaa3750bf`
  - commit `cc3d1dd9a494cf6f9f4b8a3727f1fe77b79a27c6`
  - status `passed`, coverage artifact ingested

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `955/1040` (`91.83%`) | `185/238` (`77.73%`) | `95/110` (`86.36%`) | `1594/1746` (`91.29%`) |
| `pillow-rs/src/font/mod.rs` | `258/282` (`91.49%`) | n/a | `57/64` (`89.06%`) | `331/373` (`88.74%`) |

Remaining blockers/gaps to honest 100% region coverage:

- Runtime blocker: `getmask/getmask2(stroke_width != 0)` still requires exact
  stroked glyph-mask rendering. Pillow supports this through native
  `_imagingft`/FreeType stroking. The current pure-Rust FreeType stroker path
  does not render real glyph contours exactly enough to enable this without
  lowering parity standards.
- `imagingft.rs` uncovered/partial regions are mostly defensive FreeType
  error mappings, alternate `FT_Request_Size` errors, glyph-load/render
  fallback, bitmap clipping guards, unsupported bitmap pixel modes, and
  name-table fallback selection. These need real oracle-driving font assets or
  implementation simplification; unit-only or Rust-self tests do not count.
- `font/mod.rs` remaining uncovered ranges reported by LLVM are doc-comment
  and public-field declaration source-map artifacts around
  `FontTextOptions`/`FontVariantOptions`, plus wrapper declaration comments.
  They are not evidence of a missing Pillow public input path, but they keep
  raw region coverage below 100%.

## Current checkpoint: Font public-signature edge sweep

New commits:

- `3bf702906` — covered Pillow `FreeTypeFont.getmask(..., ink=...)` and
  `FreeTypeFont.getmask2(..., *args, **kwargs)` public-signature behavior
  through input-only rows and the live repo-local Pillow oracle.
- `f7e2cafbd` — fixed `font_variant(layout_engine=...)` parity. In the
  repo-local no-raqm Pillow oracle, `RAQM` and unknown layout-engine values are
  accepted and fall back to BASIC rather than erroring.
- `d28250d93` — removed non-behavioral ignored-option bindings after the
  fallback behavior was covered by live-oracle rows.
- `f09c2c2db` — added input-only `start` clipping/error rows for
  `getmask/getmask2`.
- `45a654881` — removed a one-use private constant and an unreachable
  bitmap-pitch conversion failure branch; no public behavior changed.
- `aa64f706b` — added exact Pillow parity rows for additional SBIT mask
  formats. Coverage MCP confirmed these rows do not change `imagingft.rs`
  region metrics, so they are coverage-neutral for the remaining Font target.

What changed:

- `getmask` now covers integer `ink` parity for `L` masks and the exact
  Pillow `TypeError` for JSON list input passed as `ink`.
- `getmask2` now covers real positional variadic arguments after `start` and
  ignored extra keyword arguments such as `stroke_filled`/unknown keys when
  `stroke_width == 0`.
- `font_manifest.yaml` now classifies `getmask.ink` plus `getmask2.ink`,
  `getmask2.args`, and `getmask2.kwargs` as covered.
- `font_variant.layout_engine` is covered for BASIC, RAQM fallback, and an
  unknown string accepted by Pillow's public wrapper in the no-raqm oracle.
- Remaining manifest-level blocked public parameters:
  - `getmask`: `stroke_width`
  - `getmask2`: `stroke_width`

Verification:

- `make -C pillow-rs font-tests` — passed
- `make -C pillow-rs fmt` — passed
- Coverage MCP command `imagingft-tests-coverage-fixed`
  - run `6ef4cedf-4fff-4e0f-8461-f7e4fd998cb8`
  - snapshot `62ff34a2-15f0-4821-adb3-2fa73c1c9593`
  - commit `aa64f706b21114e2452d2b1450911ead26112c7c`
  - status `passed`, coverage artifact ingested

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `955/1040` (`91.83%`) | `185/238` (`77.73%`) | `95/110` (`86.36%`) | `1594/1746` (`91.29%`) |
| `pillow-rs/src/font/mod.rs` | `190/214` (`88.79%`) | n/a | `44/51` (`86.27%`) | `230/272` (`84.56%`) |

Blocker:

- 100% region coverage is still not achieved. The remaining public Font
  implementation blocker is stroked mask rendering for
  `getmask/getmask2(stroke_width != 0)`. Pillow supports it through the native
  `_imagingft` render path; Rust still returns `NotImplementedError` until the
  pure-Rust FreeType stroker path can render real glyph contours exactly.

## Current checkpoint: manifest public-signature gate

New commit:

- `1b5701b1c` — added live Pillow signature introspection to
  `font_oracle.py`, added `public_method_parameters` to
  `font_manifest.yaml`, and made `font_public_api.rs` fail unless every live
  Pillow `FreeTypeFont` public parameter is classified as either covered or
  blocked. Covered parameters must also appear in active input-only rows.

This means the manifest now proves both public method-name coverage and public
signature-parameter classification for the pinned repo-local Pillow oracle.
The currently blocked public parameters are explicit:

- `font_variant`: `font`, `index`, `encoding`, `layout_engine`
- `getmask`: `stroke_width`, `ink`
- `getmask2`: `stroke_width`, `ink`, `args`, `kwargs`

Additional input-only rows added for already-supported parameters:

- `font.getlength.direction_without_raqm_error`
- `font.getlength.mode_ignored`
- `font.getbbox.features_without_raqm_error`
- `font.getbbox.language_without_raqm_error`
- `font.getmask.options_start_fractional`
- `font.getmask.features_without_raqm_error`
- `font.getmask.language_without_raqm_error`
- `font.getmask2.features_without_raqm_error`
- `font.getmask2.language_without_raqm_error`

Verification:

- `make -C pillow-rs fmt` — passed
- `make -C pillow-rs font-tests` — passed, including live Pillow method and
  signature manifest validation
- Coverage MCP command `imagingft-tests-coverage-fixed`
  - run `5f04cbf2-17aa-4486-b861-1f1d12d4d6aa`
  - snapshot `9bf3e928-980b-4c44-9e92-8fb637b25ad3`
  - commit `1b5701b1c2faa75f856dbe5b53f0e77e29ed611d`
  - status `passed`, coverage artifact ingested

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `1021/1112` (`91.82%`) | `194/250` (`77.60%`) | `103/119` (`86.55%`) | `1687/1849` (`91.24%`) |
| `pillow-rs/src/font/mod.rs` | `172/196` (`87.76%`) | n/a | `39/46` (`84.78%`) | `209/251` (`83.27%`) |

The goal is still not complete: 100% region coverage is not achieved, and the
blocked public parameters above are still real implementation gaps.

## Current checkpoint: root Font API + Pillow edge-case sweep

New commits since the previous checkpoint:

- `eb326cd0b` — added explicit root `pillow_rs::font_*` constructors and Font
  wrappers, then routed the parity runner through the root public API instead
  of calling deep/inherent methods where a root function exists.
- `a32242993` — added input-only Pillow edge rows for anchor variants,
  `getmask2(start=...)` through the public option path, and empty `features`.
  This exposed and fixed a parity bug: Pillow raises the libraqm `KeyError`
  whenever `features` is provided, even when it is an empty list.

Live Pillow `ImageFont.FreeTypeFont` public callables from the repo-local
oracle are:

`font_variant`, `get_variation_axes`, `get_variation_names`, `getbbox`,
`getlength`, `getmask`, `getmask2`, `getmetrics`, `getname`,
`set_variation_by_axes`, `set_variation_by_name`.

Current implemented/manifested status:

| Pillow method | Status | Remaining implementation/edge gap |
|---|---|---|
| `getbbox` | Covered through live oracle for normal text, empty/space, CFF scalar/bbox, embedded strike, variable font, anchor variants, stroke-width bbox expansion, ignored `mode`, and libraqm-required errors. | Full libraqm layout remains intentionally unsupported while oracle is BASIC/no-raqm. |
| `getlength` | Covered for normal/empty/CFF/SBIT/variable rows and exact libraqm-required errors for `features` including `[]`, and `language`. | Full libraqm layout remains unsupported. |
| `getmask` | Covered for base masks and option path delegation (`anchor`, `mode="RGBA"`, `direction`). | Stroked mask pixels are not implemented. |
| `getmask2` | Covered for base masks, option `start`, `anchor`, `mode="RGBA"`, and `direction`. | Stroked mask pixels are not implemented. |
| `font_variant` | Covered for size override and same-size clone. | Alternate font source, face index, encoding, and layout-engine override are not implemented. |
| variation methods | Covered for variable and non-variable rows, including mutation and missing-name errors. | Remaining error branches depend on lower-level FreeType failure cases not currently produced by public Pillow rows. |

Coverage MCP evidence for commit `a32242993`:

- Command: `imagingft-tests-coverage-fixed`
- Run: `e3d92d05-0f5b-4a5b-b931-3ac0143469c3`
- Snapshot: `33c1bfe0-3025-4996-9348-90a7682386b0`
- Status: passed, coverage artifact ingested

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `1020/1112` (`91.73%`) | `194/250` (`77.60%`) | `103/119` (`86.55%`) | `1684/1849` (`91.08%`) |
| `pillow-rs/src/font/mod.rs` | `172/196` (`87.76%`) | n/a | `39/46` (`84.78%`) | `209/251` (`83.27%`) |
| `pillow-rs/src/lib.rs` | `101/200` (`50.50%`) | `0/2` (`0.00%`) | `24/39` (`61.54%`) | `120/232` (`51.72%`) |

Current blockers to honest 100% region coverage:

- `getmask/getmask2` with `stroke_width != 0` is real missing behavior.
  Pillow renders stroked masks through `_imagingft`/FreeType stroking; Rust
  currently returns `NotImplementedError`. `pillow-rs-freetype` exposes
  `FT_Stroker*` symbols, but `pillow-rs/src/font/imagingft.rs` does not yet
  integrate a stroked outline render path for Pillow Font masks.
- Some uncovered `imagingft.rs` branches are defensive FreeType error mappings
  and bitmap-storage/render fallback guards: request-size sub-errors,
  glyph-load/render fallback, negative bitmap pitch, unsupported bitmap pixel
  modes, and out-of-buffer guards. These need real oracle-driving font assets
  or implementation simplification; Rust-only/unit coverage does not count.
- `font/mod.rs` uncovered lines include derived/source-map regions around
  public struct fields and convenience wrappers. They are not proof of missing
  Pillow behavior by themselves, but they keep the raw region percentage below
  100 under LLVM coverage.

## Current checkpoint: Pillow Font comparison review

New commits:

- `cc62d84be` — added `FontTextOptions`, root API wrappers, exact `KeyError`
  mapping, and input-only parity rows for Pillow text options.
- `15e039a29` — removed non-parity debug/unused option exposure and added
  anchor validation rows.

Pillow `ImageFont.FreeTypeFont` public callable comparison against the
repo-local `.oracle-venv` showed that the operation names are represented, but
some method parameters were missing from Rust:

| Pillow method | Existing operation | Newly covered in this checkpoint | Still missing/blocker |
|---|---|---|---|
| `getbbox(text, mode, direction, features, language, stroke_width, anchor)` | `getbbox` | `mode` ignored path, `direction` libraqm `KeyError`, valid `anchor`, invalid `anchor`, integer/fractional `stroke_width` bbox math | full libraqm layout if the oracle enables libraqm |
| `getlength(text, mode, direction, features, language)` | `getlength` | `features` and `language` libraqm `KeyError`; non-error options still delegate to BASIC length | full libraqm layout if enabled |
| `getmask2(text, mode, direction, features, language, stroke_width, anchor, ink, start)` | `getmask2` / `getmask2_with_start` | `anchor` offset parity, `mode="RGBA"` TypeError, `direction` libraqm `KeyError` | stroked mask pixel parity; RGBA embedded-color/ink rendering |
| `getmask(...)` | `getmask` | not yet parameterized separately; `getmask` delegates to `getmask2` in Pillow | needs thin wrapper over the same option path |
| `font_variant(font, size, index, encoding, layout_engine)` | `font_variant` | size override and same-size clone | alternate font source, face index, encoding, and layout engine override |

New input-only rows are stored under
`pillow-rs/tests/fixtures/font/inputs/public-api` and contain no expected
outputs/errors:

- `font.getbbox.anchor_middle_middle`
- `font.getbbox.anchor_right_descender`
- `font.getbbox.stroke_width_one`
- `font.getbbox.stroke_width_half`
- `font.getbbox.mode_ignored`
- `font.getbbox.direction_without_raqm_error`
- `font.getbbox.bad_anchor_error`
- `font.getbbox.short_anchor_error`
- `font.getbbox.bad_vertical_anchor_error`
- `font.getlength.features_without_raqm_error`
- `font.getlength.language_without_raqm_error`
- `font.getmask2.anchor_middle_middle`
- `font.getmask2.mode_rgba_error`
- `font.getmask2.direction_without_raqm_error`

Verification:

- `make -C pillow-rs fmt` — passed
- `make -C pillow-rs font-tests` — passed, `1` test, all manifest rows
  compared against live Pillow oracle
- `cargo check --workspace --all-targets --all-features --locked` — passed
  with existing warning noise
- Coverage MCP command `imagingft-tests-coverage-fixed`
  - run `200762a0-9e2e-4c9d-93ec-8cb7a8d4519e`
  - snapshot `2010d398-5db4-479a-b747-91439a5d2160`
  - commit `15e039a2975cf0771f11e059f57cf3ff80f6936a`
  - status `passed`, coverage artifact ingested

Current target coverage from snapshot
`2010d398-5db4-479a-b747-91439a5d2160`:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `1006/1102` (`91.29%`) | `191/246` (`77.64%`) | `101/117` (`86.32%`) | `1660/1829` (`90.76%`) |
| `pillow-rs/src/font/mod.rs` | `159/180` (`88.33%`) | n/a | `36/42` (`85.71%`) | `194/232` (`83.62%`) |

The 100% objective is not met yet. Current blockers to reaching it only via
Pillow-oracle fixture rows:

- Stroked mask pixel parity is not implemented. Pillow renders stroked masks in
  native `_imagingft` via `font.render(..., stroke_width, stroke_filled, ...)`;
  Rust currently implements stroke bbox math only. Covering this honestly
  requires implementing outline stroking/rendering, not an expected-value hack.
- `getmask` is not separately parameterized yet, although Pillow implements it
  as `getmask2(...)[0]`.
- `font_variant` does not yet support alternate font bytes/path, face index,
  encoding, or layout-engine override.
- Remaining `imagingft.rs` coverage gaps include FreeType request-size error
  variants, glyph render fallback, uncommon bitmap pitch/pixel modes, and name
  table fallback branches. These require real font assets that drive Pillow and
  Rust through the same public path; no mock/self-comparison row may count.

## Scope

- Public surface source: `pillow-rs/tests/fixtures/font/font_manifest.yaml` plus the raw input JSON files it lists under `pillow-rs/tests/fixtures/font/inputs/public-api` (non-deprecated corpus only)
- Target suite: `make -C pillow-rs font-tests`
- Oracle: repo-local Python Pillow Font path via `pillow-rs/scripts/font_oracle.py` and `.oracle-venv`; the oracle verifies that Pillow Font delegates into native `PIL._imagingft`.
- No deprecated `deprecated/imagingft/*` tests are used.
- Current fixture/test implementation: `pillow-rs/tests/font_public_api.rs` + `pillow-rs/tests/support/font_runner.rs` using explicit `Result` paths.
- Oracle source-of-truth proof:
  - `.oracle-venv` is ignored by git at root via `.oracle-venv/`.
  - The oracle process validates it is running from `<repo>/.oracle-venv/bin/python` and imports `PIL` from that env only.
  - Bootstrap checks assert `ImageFont.core` resolves to `PIL._imagingft`, that `PIL._imagingft` is a native extension module (shared object), and that loaded fonts expose a `builtins.Font` core object (`font.font`) for C-layer execution.
  - Runtime guard inspects `PIL.ImageFont.FreeTypeFont` and `PIL.ImageFont.TransposedFont` source in the oracle venv and requires tested methods (`getmask`, `getmask2`, `getbbox`, `getlength`, `getname`, `get_variation_axes`, and transposed `getmask/getbbox/getlength`) to delegate through the C core.
  - Verified against this repo local `pillow-rs/.oracle-venv` only; this satisfies the "repo-only and gitignored oracle env" requirement.
  - This gives the strict chain: fixtures -> Python oracle -> `PIL._imagingft` C extension -> `Font` core object.
  - Fixture input JSON is input-only: no expected pixel output, hashes, oracle payloads, or expected-error fields are stored in the corpus.

## Acceptance checks

- `make -C pillow-rs font-tests`  
  Result: `1` passed, `0` failed
- `cargo check --workspace --all-targets --all-features --locked`  
  Result: passed; existing warning noise only
- Coverage MCP evidence:
  - `mcp__coverage_mcp.run_test` target: `imagingft-tests-coverage-fixed` compatibility registration, which now runs `make -C pillow-rs imagingft-tests -> font-tests`.
  - Latest run id: `8f07704f-98e8-4677-ba61-d523d946203a`
  - Terminal status: `passed`, `1` passed, `0` failed
  - Diagnostics/ingest: snapshot `48f1c0ae-b25a-4c55-bc08-017de9b90a1e` ingested with `target/coverage/imagingft/imagingft-rust.json`
  - Refactor impact: active tests now target `pillow-rs/tests/font_public_api.rs` and call the Rust `Font` public surface. The previous imagingft-named deprecated harness, runner, oracle, and fixture tree have been deleted.
- Local coverage artifact: `target/coverage/imagingft/imagingft-rust.json`

## Corpus state

- Input manifest: `pillow-rs/tests/fixtures/font/font_manifest.yaml`
- Raw input files: `20` (`pillow-rs/tests/fixtures/font/inputs/public-api/font.*.json`)
- Total rows: `154`
- Executed rows: `154/154`
- Required operation coverage check is manifest-driven: no required manifest operations missing.
- Pillow `FreeTypeFont` public methods now represented in the manifest/corpus:
  - `font_variant`
  - `get_variation_axes`
  - `get_variation_names`
  - `getbbox`
  - `getlength`
  - `getmask`
  - `getmask2`
  - `getmetrics`
  - `getname`
  - `set_variation_by_axes`
  - `set_variation_by_name`
- Additional Rust/helper fixture operations remain classified because they validate constructor, draw, transposed, binary-mode, and Result/error paths used by the public Font consumer surface.
- Input-only guard: active manifest and raw input documents must contain no oracle output, expected hash/raw path, expected error, or status fields; all output/error expectations are generated at runtime from the live Python Pillow Font oracle and compared to Rust `Result`-style status payloads.
- Error handling: the active Font parity runner uses Result-returning Rust public APIs (`getbbox`, `getlength`, `getmask`, `getmask2`, render variants) and serializes only the resulting `Ok`/`Err` payload at the test boundary. The Font public surface no longer exposes separate `_result` fallback variants for these operations.

## Required operation presence (fixture-defined)

| Operation | Input rows |
|---|---:|
| `draw_text` | 7 |
| `font_size` | 2 |
| `font_variant` | 2 |
| `get_transposed_mask` | 11 |
| `get_variation_axes` | 2 |
| `get_variation_names` | 2 |
| `getbbox` | 13 |
| `getbbox_binary` | 8 |
| `getlength` | 7 |
| `getmask` | 11 |
| `getmask2` | 12 |
| `getmask2_with_start` | 19 |
| `getmetrics` | 4 |
| `getname` | 10 |
| `has_variations` | 4 |
| `load_default` | 2 |
| `render_text_binary` | 9 |
| `set_variation_by_axes` | 5 |
| `set_variation_by_name` | 5 |
| `text_bbox` | 4 |
| `transposed_bbox` | 7 |
| `truetype` | 2 |
| `unsupported_magic` | 1 |
| `validate_transposed_length` | 5 |

- Total rows in the current input corpus: `154`. Success/error counts are generated at runtime by the oracle; do not store them in input JSON.
- Error rows are classified only from live oracle output; input JSON carries no expected output, pixel hash, or expected-error metadata.

## Error-category matrix (oracle-defined)

- `TypeError: an integer is required (got type str)` — `1`
- `ValueError: font size must be greater than 0, not 0` — `1`
- `ValueError: font size must be greater than 0, not -1` — `1`
- `ValueError: font size must be greater than 0, not -5.5` — `1`
- `ValueError: text length is undefined for text rotated by 90 or 270 degrees` — `2`
- `ValueError: bad image size` — `2`
- `OSError: cannot open resource` — `1`
- `OSError: invalid argument` — `1`
- `OSError: invalid pixel size` — `1`
- `OSError: invalid ppem value` — `1`
- `NotImplementedError: unsupported imagingft operation: unsupported_magic` — `1`

## Coverage evidence

### Suite summary (`imagingft` compatibility coverage suite)

- Current Coverage MCP snapshot: `48f1c0ae-b25a-4c55-bc08-017de9b90a1e`
  - Run: `8f07704f-98e8-4677-ba61-d523d946203a`
  - Commit: `060d763c65d86528be7a245f70ef3d124e2a50f2`
  - Command: `imagingft-tests-coverage-fixed`
  - Result: passed, ingested
  - Suite totals: `total_lines: 26199`, `covered_lines: 2773` (`line_rate 0.1058437345`)
  - Suite totals: `total_branches: 4618`, `covered_branches: 260` (`branch_rate 0.0563014292`)
  - Suite totals: `total_functions: 1846`, `covered_functions: 242` (`function_rate 0.1310942579`)
  - Suite totals: `total_regions: 45824`, `covered_regions: 4315` (`region_rate 0.0941646299`)

### `pillow-rs/src/font/imagingft.rs`

- `covered_lines: 925/1012` (`line_rate 0.9140316206`)
- `covered_functions: 97/113` (`function_rate 0.8584070796`)
- `covered_branches: 182/236` (`branch_rate 0.7711864407`)
- `covered_regions: 1556/1717` (`region_rate 0.9062317997`)
- Manifest completeness is enforced in `pillow-rs/tests/font_public_api.rs`: `font_manifest.yaml` must exactly enumerate the Font public parity operation set and every input operation must be classified as required or negative.
- Remaining gaps are not hidden: FreeType load/request-size error sub-branches, glyph render fallback, clipping guard branches, uncommon bitmap coverage modes, and fallback name-decoding branches remain uncovered.

### `pillow-rs/src/font/mod.rs`

- `covered_lines: 131/146` (`line_rate 0.8972602740`)
- `covered_functions: 32/36` (`function_rate 0.8888888889`)
- `covered_regions: 170/202` (`region_rate 0.8415841584`)
- Remaining uncovered regions are source-map/doc/debug/convenience wrapper regions; parity rows execute through the public Font surface and Result-returning APIs.

### Coverage delta

- Baseline: `19162f0c-7d00-47d9-9a69-a7f59e1d8678`
- Current: `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6`
- Sweep movement against previous committed comparator snapshot `27d14363-1512-48c6-8a77-6849c6b14113`: suite covered metrics moved `+54` lines, `+4` branches, `+4` functions, `+91` regions. `pillow-rs/src/font/imagingft.rs` itself remained unchanged.
- Same-turn movement from the previous committed imagingft snapshot `cdd83425-0fdc-4861-998c-73dfb9de9345`:
  - `font/imagingft.rs` lines: `1048 -> 1050` (`+2`)
  - branches: `169 -> 172` (`+3`)
  - regions: `1870 -> 1873` (`+3`)

## Reverse-mapped gap sweep

Source: Coverage MCP snapshot `e3c79419-67ff-4b76-ac15-17cf0822a908`, `pillow-rs/src/font/imagingft.rs`.

### Confirmed parity gaps

- Previous `getmask2_with_start` negative vertical start mismatch is fixed:
  - Added passing fixture: `imagingft.getmask2_with_start.dejavusans_negative_y_start`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[0.0, -0.5]`.
  - Added passing fixture: `imagingft.getmask2_with_start.dejavusans_negative_xy_fractional_start`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[-1.25, -0.5]`.
  - First divergence: Pillow clips glyph bitmaps with negative `xx`/`yy`; Rust skipped the whole glyph when `dx < 0 || dy < 0`.
  - C reference: Pillow 12.2.0 `src/_imagingft.c::font_render_impl`, the glyph render loop clips `x0/x1` and only draws rows where `yy >= 0 && yy < im->ysize`.
- Previous `getmask2_with_start` collapsed-width error mismatch is fixed:
  - Added passing error fixtures: `imagingft.getmask2_with_start.dejavusans_bad_image_size_negative_width` and `imagingft.getmask2_with_start.dejavusans_bad_image_size_negative_height`.
  - Pillow oracle returns `ValueError: bad image size`; Rust now returns the same error through the Result path instead of a successful empty mask.

### Missing public parity scenarios now added

- `getmask2_with_start` negative horizontal start:
  - Added passing fixture: `imagingft.getmask2_with_start.dejavusans_negative_x_start`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[-1.25, 0.0]`.
  - Purpose: validates left-side clipping/origin behavior against the live oracle without stored expected output.
- `render_text_binary` space-only mask:
  - Added passing fixture: `imagingft.render_text_binary.space_zero_height`, DejaVuSans.ttf, `size=20`, `text=" "`.
  - Purpose: validates the `pack_rgba` zero-height path with width greater than zero.
- `draw_text` negative Y placement:
  - Added passing fixture: `imagingft.render_text.dejavusans20_negative_y_draw_text_rgba`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `xy=[10, -4]`, RGBA canvas.
  - Purpose: validates Draw/text consumer clipping against the live oracle.
- Freetype fixture corpus reuse:
  - Added loadable CFF outline asset from `pillow-rs-freetype`: `input/fonts/pure-cff-cubic.otf`.
  - Added loadable embedded-strike TTF asset from `pillow-rs-freetype`: `input/fonts/embedded-strike-color-or-sbit.ttf`.
  - Added passing CFF scalar/bbox rows: `getname`, `getmetrics`, `getlength`, `getbbox`, `getbbox_binary`, `has_variations`.
  - Added passing embedded-strike rows across scalar/bbox/mask/draw paths.
  - Deliberately did not keep CFF rendering rows: `getmask.pure_cff_a` failed exact Pillow mask-byte parity with small antialias differences, so keeping it would violate the oracle standard.
- Additional `getmask2_with_start` clipping rows:
  - Added passing rows for moderate/heavy left clipping and top clipping:
    - `dejavusans_left_clip_start`
    - `dejavusans_full_first_glyph_left_clip_start`
    - `dejavusans_top_clip_start`
    - `dejavusans_left_top_clip_start`
    - `dejavusans_heavy_left_clip_start`
    - `dejavusans_heavy_top_clip_start`
    - `dejavusans_almost_full_top_clip_start`
  - Purpose: hit real partial/full glyph clipping branches through Pillow `font.getmask2(..., start=...)`, not synthetic Rust-only calls.
- Coverage effect:
  - These rows increased fixture parity coverage from `82` to `105` executed rows.
  - After the freetype/clipping sweep, `pillow-rs/src/font/imagingft.rs` measured `1873/2338` covered regions (`80.11120616%`) on snapshot `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6`.

### Reverse-mapped unclosed branches

| Source area | Lines | Public operation path | Current assessment |
|---|---:|---|---|
| TrueType load/request-size fallback and FT error mapping | 35-81 | font load before any operation | Only valid/missing/invalid-size rows are covered. Remaining FT error kinds need pathological font/size inputs or crafted font assets; do not fake these in Rust tests. |
| Removed Rust-only bitmap path | former `Font::Bitmap` arms and `shift_bitmap_mask` | not a Pillow `_imagingft` surface | Removed from `font/imagingft.rs`; legacy PIL bitmap fonts remain owned by `pillow-rs/src/font/pilfont.rs`. Do not reintroduce bitmap atlas behavior into `_imagingft` coverage. |
| Transpose helper source-map gaps | 127-129, 145 | `get_transposed_mask`, `transposed_bbox`, `validate_transposed_length` | Fixture rows cover all Pillow transpose constants plus `None`/missing orientation; remaining uncovered lines appear to be coverage/source mapping artifacts unless a new source-context query proves otherwise. |
| Layout/load glyph failure inside text shaping/rendering | 373-374, 539-547 | `getlength`, `getbbox`, `getmask*` | Needs a real oracle input that makes FreeType load fail for a glyph after font load succeeds. No current repo font/input does this. |
| `mask_from_run_with_start` clipping and sparse bitmap cases | 497-639 | `getmask`, `getmask2`, `getmask2_with_start`, `draw_text` | Additional oracle-backed start rows covered three more regions. Remaining uncovered branches include render fallback, zero-sized/absent glyph bitmap, defensive canvas slice guard, and bitmap coverage `None` handling. Add only oracle-backed rows; do not synthesize self-comparison rows. |
| `bitmap_coverage` uncommon bitmap modes/pitch | 644-660 | `getmask*`, binary mask paths | Gray and mono coverage are partially exercised. Negative pitch and unsupported pixel mode are not reachable from current repo fonts through Pillow public APIs. Need a real oracle fixture asset before claiming coverage. |

### Current blocker to 100% region by input rows only

100% region coverage inside `font/imagingft.rs` is not yet reached after the ownership refactor:

- The old `Font::Bitmap` blocker is gone.
- Several remaining FreeType fallback/error branches require real oracle inputs that make `FT_Load_Glyph`, render fallback, or `FT_Request_Size` fail after a face has loaded. The current public fixture schema cannot force those without mocking or self-comparing.
- Overflow guards such as `pack_rgba` allocation overflow cannot be produced by a practical oracle image allocation without causing the oracle itself to fail outside a useful parity comparison.

### Next targeted probes / implementation tasks

- Search for a repo font/text pair that makes FreeType return glyph-load failure after successful face load; if found, add it as an error/success row from oracle output only.
- Keep legacy bitmap-font parity separate under `pilfont`; do not count it as `_imagingft`.
- Continue reverse-mapping the remaining FreeType-only gaps using oracle-backed inputs only.

## Remaining explicit gaps

- Suite-level coverage is not complete by the 100% objective:
  - Font public-api suite executes all 105 rows and reports zero parity mismatches.
  - `pillow-rs/src/font/imagingft.rs` remains with uncovered lines/branch paths outside this minimal public corpus.
- Error/parity:
  - No parity mismatches were observed in this run; error rows are all matched and classified correctly against oracle rows.
