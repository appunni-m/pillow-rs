# Migration Parity Defect Ledger

This is the single defect ledger for the project-wide migration-parity rebuild.
It records defects observed while comparing the current repository with the
`opensource:build-migration-parity-tests` contract loaded from local
`codegen-marketplace` commit `2eff258`.

The ledger is evidence, not the manifest. The manifest remains a specification
of the public contract; run status, completion percentages, failures, and
measurements belong in generated result documents.

## Scope and closure rule

- Repository scope: `pillow-rs` only.
- Oracle: Pillow `12.2.0`.
- Intended target: the public `pillow_rs` facade backed by the Rust
  implementation.
- Inventory authority to migrate: the deprecated project-wide
  `manifest.yaml` version `0.2.0`.
- A defect is closed only by a repository change plus a maintained verification
  command. A prose decision alone does not close it.
- Counts below are a diagnostic snapshot from 2026-07-31. They must not be
  copied into the active manifest as completion claims.

## Checkpoint after the fixed-spec rebuild

The following is the current implementation/evidence checkpoint. It is kept
here so the defect list remains auditable without turning measurements into
manifest truth.

| Area | Current state | Defect-ledger interpretation |
| --- | --- | --- |
| Active specification | `migration-parity/manifest@2`; 22 surfaces, 204 operations, 1,780 requirements | MAN-001..MAN-018 are structurally addressed by the fixed generator. The strict auditor now reports 0 errors and 0 review items; the historical denominator decisions DEC-001..DEC-007 remain explicit. |
| Active inputs | 1,181 unique input-only parity workflows; indexed parity, coverage, and benchmark documents; no expected outputs in active inputs | INP-001..INP-014 are addressed for the rebuilt corpus. `scripts/validate_migration_parity_contract.py` now rejects unknown fields, duplicate IDs, broken references, missing required arguments, invalid descriptors, and expected-output fields. Every legacy fixture root and oracle harness now lives under `deprecated/migration-parity-v0/` and is not an active input. |
| Live parity | The current committed run executes all 1,244 indexed cases: 1,244 pass / 0 fail / 0 infrastructure errors / 0 not-run. Coverage-gap review found and fixed real divergences: `ImageChops.invert` for LA/RGBA, `ImageColor` alpha handling (explicit alpha dropped or forced to 255, mode-`1` thresholded instead of ITU-R 601 graylevel), `ImageOps.colorize` (mid/blackpoint/whitepoint/midpoint ignored; float interpolation instead of Pillow's floor-division LUT), and `TransposedFont` orientation (Pillow int enum vs string-only) | RUN-001, RUN-004, RUN-006, RUN-007, and RUN-009 have maintained producers/boundaries. The result is live source-vs-target evidence, not a fixture replay. RUN-008 remains a classification/anti-cheat follow-up for separate regression tests that intentionally retain oracle fixtures. |
| Coverage | `make migration-parity-coverage-rust` is the maintained merged Python+Rust lane: it temporarily instruments the extension, executes the 22 indexed plans plus the coverage-only font-native command and the Rust exercise test (1,782 tests: 1,325 parity workflows + 457 native font cases; 0 failures), and emits a strict `coverage-result@1` with per-file function/line/branch/region dimensions. Declared-path measurement: 1,551/2,180 functions (71.2%), 19,016/25,434 lines (74.8%), 2,665/4,416 branches (60.4%), 25,393/35,239 regions (72.1%); therefore code coverage is not 100%. The font-native command + Rust exercise test are the maintained port of the deprecated `font_public_api_v0` corpus: imagingft.rs regions 1,002/1,444 (69.4%) -> 1,393/1,444 (96.5%) and pilfont.rs 146/554 (26.4%) -> 509/554 (91.9%). Filter, transform, module, and paste buckets are closed or near-closed (filter.rs 72/76, transform.rs 56/58, module_fns.rs 346/411, paste.rs 280/362). Known algorithm gap: `Image.quantize` ignores method/kmeans/palette/dither and its median-cut output differs from Pillow on diverse images (documented at the implementation site). The image.rs bucket has fixed the ImageStat I/F extrema quirk, the Stat constructor list-type check, putpixel string messages, and three save/encode divergences; image.rs regions are 2,121/3,359 (63.1%) with ~1,240 remaining (stat general path, load/verify, backend locking, palette expansion). Draw bucket in progress (draw/mod.rs 62.2%). The region-coverage report (`make migration-parity-region-coverage`) lists the 165/204 operations below 90% ascending; worst is image-ops at 46.3%, image-draw 62.2%, image-sequence 57.2%, image-core 67.0% (`PIL.Image.Image.getbbox -> 4,134/6,940`), image-color/palette 66.1%. Known unreachable surface: `Image.getexif` is a core stub (empty bytes), so `exif_get_orientation`/`exif_remove_orientation` and EXIF-carrying JPEG parity are unverified until EXIF extraction is implemented. No managed snapshot has been ingested, so aggregate coverage outcomes remain `not_proven` | COV-001 and COV-002 now have strict producers and regression checks; nuanced cases are selected by coverage plans, the font-native command covers internal paths the PIL surface cannot reach, and the Rust exercise test measures the legacy core variants. Managed proof still requires explicit Coverage MCP approval and a fresh snapshot of the new artifact. Closing the remaining measured gap to the declared 100% thresholds is the long-pole code work, bucket by bucket from the ascending report. |
| Benchmark | 203 deterministic workloads are selected; 156 are measured and 47 are explicitly not-run by the correctness gate. No workload has an inferred performance budget | BEN-001, BEN-002, and BEN-004 have producers. No performance budget is declared in the current manifest, so budget outcomes remain `not_proven`/empty rather than being inferred from timings. |
| Aggregate/docs | Strict `status-report@1` join and generated specification/evidence pages are maintained | RUN-010 and DOC-003/DOC-004 have a generated path; freshness and incompatible evidence are still visible and must be rerun after each manifest or target revision change. |
| Skill/auditor | Local skill commit is byte-identical to the installed cache; the strict auditor reports 0 errors and 0 review items after the manifest declares the callable builtin as both `handle` and `any_json` | SKL-001..SKL-006 remain external skill/interface risks for a future marketplace fix; the project-side compatibility declaration is covered by a regression test and does not change the manifest/result separation. |

The active result interfaces are generated under `build/migration-parity/` and
are intentionally ignored by Git. The checked-in documentation pages are
specification/evidence views only; they are regenerated from the manifest,
inputs, and compatible results.

The obsolete Rust migration harness, Python oracle tests, WASM oracle tests,
backend fixture consumers, old manifest, expected-output corpora, and legacy
coverage/benchmark tooling are archived under
`deprecated/migration-parity-v0/`. Only the manifest-driven migration tests
and strict result-interface unit tests remain active.

## Verified inventory facts

| Fact | Observed value |
| --- | ---: |
| Deprecated project surfaces | 12 |
| Expanded deprecated inventory rows, including nested class members | 199 |
| Earlier top-level-only accounting used by project documentation | 173 |
| Operations in the active fixed manifest | 204 |
| Legacy project input documents in `deprecated/migration-parity-v0/fixtures/python/suite0` | 186 |
| Unique operation keys in those documents | 179 |
| Cases in those documents | 823 |
| Additional legacy input documents in `deprecated/migration-parity-v0/fixtures/python/suite1` | 172 |
| Active indexed parity input documents | 22 |
| Active indexed coverage input documents | 22 |
| Active indexed benchmark input documents | 22 |
| Source inventory paths resolved against Pillow 12.2.0 | 198 of 199 |
| Target paths resolved against the current `pillow_rs` facade | 190 of 199 |

The one source lookup that is not a class attribute is
`PIL.Image.Image.info`; it is a public instance attribute. The nine target
lookups that are not class attributes are the computed
`pillow_rs.ImageStat.Stat` instance properties. These are representation
details to model explicitly, not reasons to omit the endpoints.

## A. Source of truth and manifest defects

| ID | Severity | Defect | Required correction |
| --- | --- | --- | --- |
| MAN-001 | blocker | There are multiple apparent sources of truth: root `manifest.yaml`, its deprecated snapshot, and `pillow-rs/tests/fixtures/manifest.yaml`. | Leave exactly one active fixed manifest and make every active consumer use it. Keep old versions only under `deprecated/`. |
| MAN-002 | blocker | The current active draft is not `migration-parity/manifest@2`; it uses numeric `version: 1` and the top-level keys `accounting`, `evidence`, `migration`, `policy`, singular `source`, singular `target`, and `surfaces`. | Replace it with the exact fixed `manifest@2` top-level interface and reject unknown fields. |
| MAN-003 | blocker | The draft mixes specification with observed state: counts, percentages, active/pending status, case counts, evidence commands, and migration progress are embedded in the manifest. | Keep only contract, applicability, support policy, requirements, commands, and interface declarations in the manifest. Generate status separately. |
| MAN-004 | blocker | The draft calls the Font surface `font`, while other surfaces use legacy title case. Neither form is a canonical public identity. | Use canonical public paths such as `PIL.ImageFont`, `PIL.ImageFont.FreeTypeFont`, and `PIL.Image.Image`; use separate storage slugs for filenames. |
| MAN-005 | blocker | The draft contains name-only pending placeholders for 11 surfaces. A full-scope manifest cannot count placeholders as represented operations. | Give every endpoint a complete source binding, target binding, result contract, requirements, and all three lane policies. |
| MAN-006 | high | The draft reports 206 operations, the expanded deprecated authority has 199 rows, and the older project accounting used 173 top-level rows. The denominator is therefore unstable and “100%” is undefined. | Publish one deterministic inventory expansion rule and a bijection check. Report top-level groups separately from independently observable endpoints. |
| MAN-007 | high | Thirty Font runner/helper operations are counted as public operations in the draft. Names such as native render and fixture helpers are implementation/test mechanics, not Pillow endpoints. | Map helper execution to requirements for real public endpoints or maintained coverage commands; do not inflate the public endpoint denominator. |
| MAN-008 | high | Nested class members are expanded for Font helper work but are not consistently expanded for `ImageStat.Stat` and other legacy class rows. | Apply one expansion model to every class/type in the authority. |
| MAN-009 | high | The deprecated manifest models `Image.open` and `Image.new` as “class methods” while also listing module-level `ImageModule.open` and `ImageModule.new`. This creates duplicate or misleading identities. | Resolve both legacy rows to explicit canonical source paths and document whether they are aliases, duplicates, or distinct target entry points. |
| MAN-010 | high | The deprecated manifest has 188 `implemented`, five `ignored`, and six unclassified rows. These are historical outcomes, not complete support contracts. | Convert them to explicit target support variants with required reasons, blockers, authorities, and missing requirement IDs. Do not infer parity from `implemented`. |
| MAN-011 | high | Many deprecated Font methods, classes, and properties have no signatures; `ImageStat` properties are bare strings rather than operation records. | Record complete typed source parameter tables, omissions, result observations, and error policy for every endpoint. |
| MAN-012 | high | The deprecated surface model is structurally inconsistent: functions, methods, properties, and classes use different row shapes and nested members use another shape. | Normalize every endpoint to the one fixed operation interface. |
| MAN-013 | high | The draft has no oracle registry, target registry, target profiles, structured command registry, interface registry, input index, coverage components, or generated-document destinations. | Add all mandatory `manifest@2` registries with exact keys and reference checks. |
| MAN-014 | high | The draft uses `current-checkout` as target version/state. A manifest must not contain the revision of a particular run. | Put immutable source/target revisions and runtime identities only in generated results. |
| MAN-015 | high | Target support is effectively surface-wide and CPU-only in the old data. It does not distinguish public target bindings from backend profiles. | Declare target bindings per operation and applicability per target profile. |
| MAN-016 | high | Required mode, format, edge, parameter-combination, backend, error, and historical-divergence semantics are not represented as stable requirement IDs. | Convert every declared contract dimension to requirements that join manifest, inputs, results, documentation, and aggregation. |
| MAN-017 | medium | Constructors and factories needed to create receivers are absent or inconsistently represented, for example `PIL.ImageDraw.Draw`. | Inventory every public setup operation needed by a workflow or define an explicit, fixed test-value facility without pretending it is a Pillow endpoint. |
| MAN-018 | medium | The active file has a `.yaml` name but is emitted as sorted JSON. JSON is valid YAML, but the result is hard to review as the project’s human-facing specification. | Emit stable, readable YAML with intentional operation and requirement ordering. |

## B. Archive and migration defects

| ID | Severity | Defect | Required correction |
| --- | --- | --- | --- |
| MIG-001 | blocker | `scripts/migrate_font_parity.py` migrates only the Font v0 corpus while claiming project-wide accounting. | Replace it with a project-wide deterministic migrator driven by the deprecated authority and all retained input corpora. |
| MIG-002 | blocker | The migrator emits the obsolete draft schema rather than the fixed manifest, parity, coverage, and benchmark interfaces. | Generate and validate only the accepted schema versions. |
| MIG-003 | high | The migrator copies 35 legacy Font documents into 42 helper-oriented active documents but does not migrate the 823 project cases or the 172 suite-two documents. | Map every retained legacy input to a new case, an explicit duplicate, or a documented retirement record. |
| MIG-004 | high | There is no checked-in old-to-new migration map for project fixtures, Font fixtures, operation identities, case IDs, assets, and intentionally retired outputs. | Add a deterministic mapping report and make the drift check verify it. |
| MIG-005 | high | The migrator writes directly into active roots and deletes existing generated subtrees with `shutil.rmtree` before the replacement has validated. | Generate into a temporary staging directory, validate fully, and replace only the exact generated roots after success. |
| MIG-006 | high | Legacy parity material was previously split across several `tests/deprecated/` trees and active roots, making the archive boundary ambiguous. | Consolidate every retired project parity/oracle/input/tooling tree under `deprecated/migration-parity-v0/` with a checked-in mapping and no active imports. |
| MIG-007 | medium | Input assets are copied recursively without an active manifest index proving that each copied asset is referenced and each referenced asset exists. | Add asset reachability, digest, duplicate, and orphan checks. |
| MIG-008 | medium | The old generated outputs remain useful only as migration evidence, but their relationship to new live-oracle cases is undocumented. | Record provenance and mapping under `deprecated/`; never use those outputs as active oracle evidence. |
| MIG-009 | medium | The reproducibility target diffs only the draft manifest, Font inputs, and Font assets. It does not validate project-wide inputs, coverage plans, benchmark workloads, schemas, or docs. | Expand the drift command to cover every generated specification artifact. |

## C. Parity input and workflow defects

| ID | Severity | Defect | Required correction |
| --- | --- | --- | --- |
| INP-001 | blocker | Active Font files use `{version, surface, operation, cases}` rather than `migration-parity/parity-input@1`. | Regenerate them with the exact `schema` and `cases` document interface. |
| INP-002 | blocker | Cases contain `{case_id, operation, inputs}` rather than the fixed case fields `surface`, `operation`, `covers`, `target_profiles`, `assets`, `steps`, and `observations`. | Convert every case to an explicit public workflow. |
| INP-003 | blocker | Cases select runner helper operations instead of calling manifest operations step by step. | Make every step identify a manifest surface and operation; setup, receiver binding, calls, and observations must be explicit. |
| INP-004 | high | Assets are an arbitrary mapping without per-asset IDs, fixed variants, media types, or SHA-256 stimulus digests. | Use the fixed asset array variants and verify referenced bytes. |
| INP-005 | high | Cases have no requirement coverage IDs, so no static proof connects a case to a mode, edge, error, backend, or benchmark requirement. | Populate `covers` and require a complete requirement-to-input mapping. |
| INP-006 | high | Cases have no target profiles, so CPU/GPU/WASM applicability cannot be verified. | Select declared profiles explicitly and reject missing or incompatible profiles. |
| INP-007 | high | Cases have no observation list. The current runner decides what to serialize based on hidden operation-specific logic. | Declare observed public step results explicitly. |
| INP-008 | high | Arbitrary `params` and `environment` objects are not checked against typed source parameters. | Validate argument names, required omissions, descriptor variants, and value types against the called operation. |
| INP-009 | high | Mutating operations currently rely on hidden runner behavior that returns both a method result and image state. | Express the mutation call and later public state observations as separate workflow steps. |
| INP-010 | high | The old generic engine uses fixture-only pseudo-parameters such as `_args`, `_fixture_mode`, `_table_pattern`, `data_pattern`, `function`, `mask_img`, `destination`, and `io_kind`. They are not public Pillow arguments. | Translate them into public setup steps, assets, bindings, or a reviewed fixed fixture-value interface; never declare them as Pillow parameters. |
| INP-011 | high | Legacy operation metadata loses class ownership for many Font cases. For example several class-method files record only `getbbox`, `getmask`, or `getlength`. | Recover ownership from the authority and filenames, then lock it with a migration bijection test. |
| INP-012 | high | One `ImageStat.Stat` legacy case bundles nine property observations, while the authority counts the properties as separate endpoints. | Represent construction and each property observation explicitly, with coverage IDs for all nine endpoints. |
| INP-013 | medium | The active corpus has no separate `inputs/parity`, `inputs/coverage`, and `inputs/benchmark` roots or complete input index. | Adopt the canonical lane layout and reject unindexed/discovered files. |
| INP-014 | medium | Case IDs use lowercase `font.*` runner identity rather than canonical public surface prefixes. | Use stable public prefixes and storage slugs independently. |

## D. Runner, comparator, and result defects

| ID | Severity | Defect | Required correction |
| --- | --- | --- | --- |
| RUN-001 | blocker | `make migration-parity-test` runs only the Rust `font_public_api` integration test. It is not a project-wide parity command. | Add a manifest-driven project runner and make the command fail on every unmapped, unexecuted, mismatched, or hidden case. |
| RUN-002 | blocker | `migration_parity.rs` deserializes the obsolete `{operation, inputs}` case model. | Replace it with strict fixed manifest/workflow loaders and unknown-field rejection for every accepted schema. |
| RUN-003 | blocker | The only success output shape is `Object`. Public results include none, scalar, sequence, mapping, record, bytes, image, mask, encoded file, metrics, handle, iterator, stream, and filesystem observations. | Implement all declared fixed result shapes and comparison policies. |
| RUN-004 | blocker | There is no persisted strict `parity-result@1` containing suite/run identity, source and target identities, case outcomes, comparisons, and artifact references. | Implement and validate the complete parity result interface. |
| RUN-005 | high | Current errors have `class`, `kind`, `message`, and `stage` but no optional/declared code and no manifest-controlled exact/normalized/ignored message policy. | Serialize the fixed error fields and apply only declared normalization. |
| RUN-006 | high | Comparison is whole-JSON equality for objects; it does not use per-observation comparison policies from the operation contract. | Compare each declared observation generically with exact typed policies. |
| RUN-007 | high | Source and target identity protection only checks that two caller-provided strings differ. It does not execute public identity handshakes or record immutable revisions. | Add identity commands, handshake assertions, and result provenance. |
| RUN-008 | high | The retired Python parity harness discovered fixture roots and read stored expected outputs. | Archived under `deprecated/migration-parity-v0/python/`; the active runner executes Pillow and `pillow_rs` independently from the same input workflow. Add a no-archive-import gate to prevent regression. |
| RUN-009 | high | Active Rust Font tests still contain paths to sibling `fontdone` fixtures. That prevents the Pillow parity specification from being self-contained. | Copy/migrate required stimuli into the active indexed asset root and verify digests. |
| RUN-010 | high | There is no aggregate `status-report@1`, compatibility join, stale-evidence check, or rule preventing missing cases from disappearing from summaries. | Implement strict aggregation over compatible lane results with complete manifest accounting. |
| RUN-011 | medium | The comparator has no structured artifact contract for large image, mask, bytes, trace, or encoded-file evidence. | Implement the fixed artifact references and integrity checks from the evidence contract. |
| RUN-012 | medium | The original rebuilt runner had no maintained invalid-document tests or strict input loaders. | Resolved for the manifest and active inputs with `scripts/validate_migration_parity_contract.py`, negative tests in `tests/test_migration_parity_contract.py`, and the `migration-parity-inputs-check` Make lane. Result identity/count validation remains in `scripts/validate_migration_parity_result.py`; mixed-version evidence joins remain an aggregate concern. |

## E. Coverage defects

| ID | Severity | Defect | Required correction |
| --- | --- | --- | --- |
| COV-001 | blocker | There is no `coverage-input@1` plan set joining requirement IDs, target profiles, selected parity cases/commands, and code components. | Generate complete coverage plans and validate every reference. |
| COV-002 | blocker | There is no strict `coverage-result@1` with snapshot identity and per-dimension covered/total counts. | Run coverage in a managed environment and persist the fixed result interface. |
| COV-003 | high | `scripts/coverage/ops_registry.py` still derives behavior from hard-coded call styles, defaults, and value-operation sets in addition to the old root manifest. | Make the fixed manifest and indexed inputs the only operation contract source. |
| COV-004 | high | `scripts/coverage/validate_coverage.py` has hard-coded filters/modes and can treat CPU coverage as satisfying GPU/WASM claims. | Attribute coverage to exact target profiles and components; never cross-credit profiles without instrumented evidence. |
| COV-005 | high | Existing “trusted” coverage is based on at least one passing parity test, which is not the same as 100% function, line, branch, or region coverage. | Report contract mapping and code coverage as separate dimensions with explicit thresholds. |
| COV-006 | high | There is no many-to-many operation-to-component mapping for the full inventory. | Declare reusable components and verify that required paths exist and are instrumented. |
| COV-007 | medium | No anti-exclusion gate proves that files, branches, or profiles were not omitted to improve coverage. | Audit include/exclude configuration and record it in the coverage result. |
| COV-008 | high | `Image.quantize` method 0 (median cut) diverged from Pillow 12.2.0 on diverse RGB images: the splitlists walk direction and crossing-group placement differed (C keeps the midpoint-crossing value group on the high side and the walk direction matters at exact-half counts), the left/right box orientation was inverted (C's left box is the high-value side), the leaf palette order visited right-before-left while `annotate_hash_table` visits left first, and the box heap tie order used BinaryHeap insertion-index tie-breaking instead of QuantHeap.c's exact sift rules. RESOLVED: `try_split` now walks high-to-low with the C crossing rule and high-side left orientation, `collect_tree_leaves` visits left first, and a 1-indexed QuantHeap.c port replaces the BinaryHeap. Verified byte-identical across seeds 1-12, colors 2..=32, kmeans 0..=4 (672/672) and via a standalone C harness extracted from the Pillow 12.2.0 sdist. Diverse-image method-0 parity cases are now added to the corpus. | Keep the C harness (`/tmp` diagnostic; a maintained generator would live in `scripts/`) as reference, and keep the added median-cut parity cases green. |
| COV-009 | medium | `Image.convert` drops palette-transparency metadata: Pillow's convert carries `info["transparency"]` through P-to-RGB/L by converting the transparency index through the palette (e.g. index 1 with palette (255,0,0) yields `{"transparency": (255, 0, 0)}` for RGB and the gray level for L), and drops it for RGBA/LA after applying it to the palette alpha. The Rust convert result carries no info, so opened transparency PNGs diverge in `info` while pixel bytes match. Found by asset-based convert cases. | Propagate the transformed transparency metadata on convert results (or move the convert-on-opened-transparency cases behind a metadata-parity gate). |
| COV-010 | medium | Derived images retained the opened container's `format` (e.g. `format == "PNG"` on `getchannel`/`resize`/`convert` results), while Pillow sets `format` to `None` on every derived image. Pixel bytes matched; the parity image comparator failed on the `format` field, blocking asset-based cases for image-valued operations. RESOLVED: `push_op`, `push_mode_changing_op`, and `copy` now produce `format: None` results while the originally opened image keeps its container format; save still infers the format from the destination path/extension. getchannel/resize opened-asset cases re-enabled. | Keep the re-enabled image-valued asset cases green. |

## F. Benchmark defects

| ID | Severity | Defect | Required correction |
| --- | --- | --- | --- |
| BEN-001 | blocker | There are no `benchmark-input@1` workloads and suites for the public inventory. | Define deterministic workloads, subjects, inputs, measurement boundaries, iteration policy, and correctness gates. |
| BEN-002 | blocker | There is no strict `benchmark-result@1` with environment, sample statistics, budget evaluation, and correctness-gate status. | Implement the fixed result producer and validator. |
| BEN-003 | high | Existing benchmark selection uses hard-coded GPU-applicable operations and priorities outside the manifest. | Express applicability and performance requirements in the manifest and workloads. |
| BEN-004 | high | Existing benchmark paths are not uniformly gated by live source-target correctness for the exact workload. | Require `parity_pass` or `source_target_match` before accepting measurements. |
| BEN-005 | high | There is no stable separation between benchmark policy and observed measurements. | Keep budgets in manifest requirements and all timings/samples in generated results. |
| BEN-006 | medium | There are no weighted real-world suites spanning image, drawing, filters, fonts, I/O, statistics, and sequence workloads. | Define reviewed suites after endpoint workloads are complete. |

## G. Documentation and command defects

| ID | Severity | Defect | Required correction |
| --- | --- | --- | --- |
| DOC-001 | high | `docs/project-parity-test-process-standard.md` describes an older active/pending manifest model and conflicts with `manifest@2`. | Rewrite it around fixed spec/input/result boundaries and link to generated pages. |
| DOC-002 | high | `docs/project-manifest-test-design-review.md` still contains superseded numeric-version and singular source/target examples. | Update it to the accepted interface or mark historical sections explicitly. |
| DOC-003 | high | Manual status pages mix intended scope with current outcomes, so they can drift from both spec and evidence. | Generate specification pages from manifest+inputs and status pages from compatible results. |
| DOC-004 | high | The Make interface lacks complete project-wide parity, coverage, benchmark, aggregate, docs, drift, and all-lanes commands. | Add repository-native commands declared in the manifest and verify `make help`. |
| DOC-005 | medium | Important moved/added parity files are not yet represented by a completed repository-map update and check. | Run the maintained repo-map update after layout stabilizes and verify it. |

## H. Anti-cheat and evidence-integrity defects

| ID | Severity | Defect | Required correction |
| --- | --- | --- | --- |
| INT-001 | blocker | No gate proves the target cannot read oracle results or deprecated expected outputs. | Sandbox/inspect target execution and fail on forbidden fixture/output access. |
| INT-002 | blocker | No gate proves the oracle and target are independent implementations rather than aliases or the same loaded module. | Add public identity handshakes and circular-oracle detection. |
| INT-003 | high | No gate detects case-specific comparator branches, hidden mismatches, dropped failures, or selective execution. | Audit comparator dispatch, require generic policies, and reconcile all selected cases in results. |
| INT-004 | high | No freshness gate joins results to exact manifest/input digests and implementation revisions. | Reject stale or incompatible evidence during aggregation and docs generation. |
| INT-005 | high | No gate proves deterministic generators reproduce assets, inputs, manifests, and documentation without reading active outputs. | Add hermetic regeneration and full-tree drift checks. |

## I. Reloaded skill and auditor defects

These defects are recorded here because the current task forbids editing outside
this repository. They belong in a later `codegen-marketplace` fix.

| ID | Severity | Defect | Evidence and required correction |
| --- | --- | --- | --- |
| SKL-001 | high | The static auditor crashes on the current malformed manifest instead of returning schema findings. | `audit_parity_fixtures.py` calls `set(metrics)` when `metrics` is absent and raises `TypeError: 'NoneType' object is not iterable`. Validate the discriminant and required field before constructing derived sets. |
| SKL-002 | high | The fixed workflow value descriptors cannot clearly represent deterministic non-JSON public arguments such as callables, array-interface objects, deformers, and drawing outline objects. | Define one fixed, source-neutral fixture-value/builtin registry interface or a new reviewed schema version. Do not force projects to abuse literal values, asset semantics, or public endpoint inventory. |
| SKL-003 | high | The plugin contains prose contracts and one auditor but no separately versioned machine-readable schemas for manifest, all three inputs, all four results, or aggregate joins. | Add machine-readable schemas or equivalent reusable validators plus conformance fixtures for every interface. |
| SKL-004 | high | The skill has no executable regression test suite for valid, invalid, unknown-field, unsupported-version, and malformed-discriminant documents. | Add tests that prove the auditor always reports findings and never crashes on untrusted input. |
| SKL-005 | medium | Local marketplace commit `2eff258` is installed and byte-identical to the cache, but the marketplace checkout is one commit ahead of `origin/main`. | Push the reviewed commit before relying on it outside this machine. |
| SKL-006 | high | The auditor rejects a valid fixed `builtin` asset used for a deterministic non-JSON public handle: `Image.point`'s callable variant is declared as a `handle`, but `validate_value_descriptor` only treats assets as `any_json`, `bytes`, `path`, `image`, `font`, or `stream`. | Extend the fixed builtin/value registry (with conformance tests) so a declared callable/handle builtin is type-checkable; do not replace the callable with an expected-output literal or silently drop the requirement. |

## J. Open contract decisions that block a truthful final spec

These are not completion excuses. Each must be resolved in the manifest design
and locked by tests.

| ID | Decision required |
| --- | --- |
| DEC-001 | Whether the authority denominator is the 199 expanded legacy rows or a corrected public inventory that also includes missing workflow constructors/factories. |
| DEC-002 | How duplicate legacy identities such as `Image.open/new` versus `ImageModule.open/new` map to canonical public paths without double-counting behavior. |
| DEC-003 | Whether the parity target is the public Python-compatible `pillow_rs` facade, the lower-level Rust crate API, or two separate targets. Current evidence mixes both. |
| DEC-004 | Which target profiles are contractually required: Rust CPU, GPU, WASM, Python facade, and/or JavaScript facade. |
| DEC-005 | The fixed representation of deterministic non-JSON test values required by callback/object-taking Pillow endpoints. |
| DEC-006 | Which legacy suite-two cases are distinct requirements versus duplicates of suite zero. |
| DEC-007 | Exact public comparison policy for encoded files, lazy images, handles/capsules, Qt objects, iterators, and filesystem side effects. |

## Immediate order of repair

1. Freeze the corrected authority and canonical endpoint identities.
2. Replace the active manifest with strict `manifest@2` operation contracts.
3. Replace helper-oriented Font cases and migrate all retained legacy inputs to
   explicit parity workflows.
4. Add coverage plans and deterministic benchmark workloads.
5. Implement strict loaders, live oracle/target execution, result interfaces,
   aggregation, generated documentation, and anti-cheat gates.
6. Run the complete definition, mapping, code-coverage, parity, and benchmark
   gates; publish each dimension separately.
