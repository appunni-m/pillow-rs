# Project manifest test design review

Date: 2026-07-30

Scope:

- `pillow-rs` Font/ImageFont parity in this repository
- `image-slash-star`
- `fontdone`

Constraint for this review: sibling repositories were read only. This document is written only in `pillow-rs`.

## Executive summary

The strongest model is the current `pillow-rs` Font/ImageFont parity test:

1. fixture JSON contains inputs only;
2. the oracle is executed at test runtime;
3. Rust output is executed at test runtime;
4. the comparison is exact `Result` payload parity.

`fontdone` is also manifest-driven and very rigorous, but it is lower-level and FreeType-specific. It uses a large public C API manifest, input case files, route-audit categories, and a pinned C oracle cache. It allows `expect_error` and `expectation` fields because its public surface must classify C ABI failure contracts and pending/safety routes explicitly.

`image-slash-star` is the least aligned with the target project-wide design. It has a useful high-level `manifest.yaml`, but the active runner is driven by generated `coverage_matrix.json` and generated reference artifacts. Those rows embed oracle status/error/ref paths/byte counts. This is practical for codecs today, but it is not the desired “input-only + live Pillow oracle” model.

The project-wide redesign should therefore use Font/ImageFont as the base runner contract, borrow `fontdone`'s manifest coverage discipline and route accounting, and migrate image-slash-star away from stored output references for project-level parity.

## Active fixture locations observed

Root-level legacy corpora were moved to:

- `tests/deprecated/fixtures`
- `tests/deprecated/fixtures_2`

Active crate-local corpora still exist under:

- `pillow-rs/tests/fixtures/font`
- `pillow-rs/tests/fixtures/image_backend`

Sibling corpora:

- `../image-slash-star/tests/fixtures`
- `../fontdone/tests/fixtures`
- `../fontdone/tests/manifest.yaml`

This means the next migration should not assume “all old fixtures are gone.” The root-level legacy corpora are deprecated; crate-local active test fixtures still need migration.

## `pillow-rs` Font/ImageFont model

Primary files:

- `pillow-rs/tests/fixtures/font/font_manifest.yaml`
- `pillow-rs/tests/font_public_api.rs`
- `pillow-rs/tests/support/font_runner.rs`
- `pillow-rs/scripts/font_oracle.py`

### Manifest shape

The Font manifest declares:

- version/suite/input directory;
- oracle scope (`PIL.ImageFont`, Pillow runtime, Rust contract);
- out-of-scope features such as successful libraqm shaping;
- required public operations;
- negative operations;
- public method parameter coverage;
- exact input files.

It is still Font-specific:

- file name is `font_manifest.yaml`;
- input root is `inputs/public-api`;
- required operations are all Font/ImageFont-specific;
- test constants still hard-code Font public operation lists.

### Input JSON contract

The test enforces input-only fixtures.

Allowed document envelope:

```text
version
operation
cases
```

Allowed case envelope:

```text
case_id
operation
inputs
```

Forbidden keys include output/oracle/error material:

```text
error
expect_error
expected
hash
oracle
output
outputs
pixels_hex
status
```

This is the correct project-wide rule.

### Oracle model

`font_public_api.rs` starts the pinned Python interpreter from `.oracle-venv/bin/python`, runs `scripts/font_oracle.py`, sends the input-only cases over stdin, and receives a case-id result map.

Important properties:

- the oracle is generated at runtime;
- fixture JSON does not contain expected output;
- Pillow public surface is queried live;
- Pillow layout enum members are queried live;
- public method signatures are queried live;
- Rust root API coverage is checked against `src/lib.rs`;
- input files must exactly match manifest `input_files`.

### Rust execution model

`font_runner::run(case, fixture_root)` returns a structured JSON payload. The test compares:

```text
live Pillow oracle payload == Rust payload
```

The comparison checks status first and then exact payload equality.

This gives the desired `Result<Output, Error>` behavior even though the envelope is represented as JSON.

### Strengths

- Best match for target trust model.
- Input-only fixtures are actively enforced.
- Public Pillow surface is queried at runtime, reducing stale manifest risk.
- Rust implementation is checked through the public/root API path.
- It contains guardrails against fake coverage/oracle shortcuts in `imagingft.rs`.
- It handles no-libraqm errors as a real core error contract rather than a test hack.

### Weaknesses to fix during migration

- Manifest and runner are Font-specific.
- A large amount of public surface knowledge is hard-coded in `font_public_api.rs`.
- Some repo-helper operations are mixed with true Pillow public operations.
- Some guardrails read unrelated binding files directly from a Font test.
- Current implementation is one large test file rather than a reusable shared parity harness.

### Migration implication

Keep the semantics, not the Font-specific structure.

The shared project runner should extract:

- manifest parser;
- input-only validation;
- case-id uniqueness validation;
- asset resolution validation;
- runtime oracle execution contract;
- `Result` envelope comparison;
- public surface coverage checks.

Then Font/ImageFont becomes just one namespace in the project manifest.

## `image-slash-star` model

Primary files:

- `../image-slash-star/manifest.yaml`
- `../image-slash-star/tests/coverage_matrix_tests.rs`
- `../image-slash-star/tests/support/json.rs`
- `../image-slash-star/tests/fixtures/coverage_matrix.json`
- `../image-slash-star/tests/fixtures/input/jsons/*.json`
- `../image-slash-star/tests/fixtures/outputs/**`

### Manifest shape

The top-level `manifest.yaml` is broad and format-oriented. It records:

- Pillow 12.2.0 oracle identity;
- codec-specific oracle versions;
- quality goals;
- format coverage plans;
- format-specific edge cases and assets;
- decode/encode status.

This is useful as a planning and coverage map, but it is not the active runner input by itself.

### Active runner shape

The active Rust runner is `coverage_matrix_tests.rs`, driven by `tests/fixtures/coverage_matrix.json`.

The matrix rows contain fields such as:

- `expect_error`;
- `oracle_status`;
- `oracle_error_type`;
- `oracle_error_message`;
- `oracle_error_kind`;
- `inspect_status`;
- `verify_status`;
- `ref_mode`;
- `ref_size`;
- `ref_path`;
- `ref_bytes`;
- `encoded_ref_path`;
- `encoded_ref_bytes`.

Decode cases read encoded assets, run `detect_format`, `inspect`, `verify`, `decode`, then compare decoded pixels to stored reference bytes.

Encode cases decode a source asset, encode with params, compare encoded bytes to stored reference bytes, and optionally decode the encoded result back to compare pixels.

### Strengths

- Very broad codec coverage.
- Good lifecycle checks: detect/inspect/verify/decode/cache behavior.
- Structured error kind matching exists.
- Encoded byte parity is exact where reference bytes exist.
- Format-specific contract checks catch structural regressions that pure pixel comparison may miss.
- Custom JSON parser keeps tests independent of serde for this crate.

### Weaknesses relative to target model

- The active test is not input-only.
- Oracle status/errors/reference paths/byte counts are embedded in generated matrix rows.
- Expected encoded output is stored in fixture outputs.
- Generated reference artifacts can drift from the current Pillow runtime unless regenerated and verified.
- The runner is coupled to codec formats, not a generic Pillow public API namespace/operation model.
- Some row mutation in the test creates malformed encode inputs internally instead of expressing all inputs in JSON.

### Migration implication

For the project-wide manifest, do not copy `coverage_matrix.json` as-is.

Instead:

- migrate each decode/encode row into input-only case JSON;
- run Pillow 12.2.0 oracle live for decode, inspect, verify, encode, and error behavior;
- use project runner `Result` comparison;
- keep `image-slash-star` structural contract checks as optional secondary assertions only after live oracle equality;
- keep generated reference files only as deprecated/debug evidence, not as the source of truth.

## `fontdone` model

Primary files:

- `../fontdone/tests/manifest.yaml`
- `../fontdone/tests/fixtures/inputs/public-api/*.json`
- `../fontdone/tests/unified_fixture_parity.rs`
- `../fontdone/scripts/run_runtime_parity.py`
- generated route audit under `../fontdone/target/api-abi-audit/route_audit.json`
- generated pinned oracle under `../fontdone/target/unified-fixtures/gen_unified_oracle`

### Manifest shape

`tests/manifest.yaml` is a public API subject manifest. It records:

- subject id;
- kind;
- C symbol;
- public header;
- description;
- case ids.

This is a true public-surface manifest, not just a fixture index.

Examples of subject kinds include enums, enum variants, structs, functions, macros, and public constants.

### Input JSON shape

Input files live under `tests/fixtures/inputs/public-api`.

Cases contain:

- `case_id`;
- `subject`;
- `case`;
- optional `covers_manifest_cases`;
- `operation`;
- `schema`;
- `expect_error`;
- `expectation`;
- `inputs`;
- optional variants.

`inputs` may contain assets, params, and variants. Variants are expanded into concrete cases with `case_id@variant`.

Unlike the desired project-wide Pillow model, `fontdone` intentionally allows expected-error metadata because C API parity needs exact error contract routing and strict error ledgers.

### Oracle model

`unified_fixture_parity.rs` builds/runs a pinned C FreeType oracle, caches oracle output by:

- oracle identity;
- argv batch;
- asset identity;
- input cases.

It compares three backends to the same oracle where applicable:

- Rust FFI path;
- C ABI path;
- WASM ABI path.

The runtime summary reports:

- runnable cases;
- pending cases;
- passed/failed totals;
- covered manifest cases;
- route evidence categories.

### Route accounting

The route audit categorizes concrete cases:

- `real-parity`;
- `real-null-validation`;
- `pending-route`;
- `safety-extension`;
- other categories.

Pending/safety routes are not hidden. They are counted and sampled.

### Strengths

- Strong public surface coverage discipline.
- Explicit mapping from manifest subjects/cases to concrete runtime inputs.
- Excellent pending-route visibility.
- Oracle cache key tracks asset identity, preventing stale cache from silently passing.
- Multi-backend comparison is systematic.
- Large-scale batching and deduplication are built in.

### Weaknesses relative to target Pillow project model

- It is FreeType C API-specific and much heavier than needed for high-level Pillow APIs.
- It allows `expect_error` and `expectation` in inputs.
- Route selection has many operation-specific branches.
- The comparison backend model includes C ABI and WASM ABI, which is outside the current target if we only want Rust public API vs Pillow.

### Migration implication

Borrow these ideas:

- one public-surface manifest;
- exact subject/case coverage accounting;
- explicit pending route classification;
- route audit generated by maintained tooling;
- oracle cache keyed by executable/input/assets.

Do not copy these parts directly into the new project-wide Pillow runner:

- C ABI/WASM comparison requirement;
- C-symbol/header subject model;
- expected output/error fields in input JSON;
- route-specific special casing inside the runner.

## Desired project-wide test architecture

Target files:

```text
pillow-rs/tests/fixtures/manifest.yaml
pillow-rs/tests/fixtures/inputs/<namespace>/<operation>.json
pillow-rs/tests/support/project_parity/
pillow-rs/tests/project_public_api.rs
```

The root-level `tests/fixtures` name was just deprecated. The new active corpus should be crate-local under `pillow-rs/tests/fixtures`, because the active crate tests already use that root. If a true repository-root manifest is still desired, update Make targets and Coverage MCP commands together; do not split active fixture roots.

### Manifest should define

```yaml
version: 1
oracle:
  provider: pillow
  version: "12.2.0"
  runtime: ".oracle-venv/bin/python"

namespaces:
  - id: ImageFont
    pillow_path: PIL.ImageFont
    rust_surface: pillow_rs::imagefont_*
    input_dir: inputs/ImageFont
    operations:
      - id: getbbox
        public: true
        input_file: getbbox.json
      - id: getlength
        public: true
        input_file: getlength.json
```

The final schema should support all PIL namespaces, but the first migration should keep the schema small and strict.

### Input JSON should define only

```json
{
  "version": 1,
  "namespace": "ImageFont",
  "operation": "getbbox",
  "cases": [
    {
      "case_id": "ImageFont.getbbox.basic_latin_default",
      "operation": "getbbox",
      "inputs": {
        "assets": {},
        "params": {}
      }
    }
  ]
}
```

Forbidden in input JSON:

```text
expect_error
expectation
expected
oracle
output
outputs
hash
sha256
raw_path
ref_path
ref_bytes
encoded_ref_path
encoded_ref_bytes
status
error
```

Error expectations must come from the live oracle result, not the input fixture.

### Runtime comparison envelope

Both Pillow and Rust should normalize into:

```text
Result<ParityOutput, ParityError>
```

Serializable JSON shape:

```json
{
  "status": "ok",
  "value": {}
}
```

or:

```json
{
  "status": "error",
  "error": {
    "class": "ValueError",
    "kind": "bad_mode",
    "message": "..."
  }
}
```

Comparison rule:

- `ok` vs `ok`: exact value equality after deterministic normalization;
- `error` vs `error`: exact class/kind and stable message/category comparison;
- `ok` vs `error`: fail;
- `error` vs `ok`: fail.

The test file must not decide which case “should error.” Pillow decides at runtime.

## Migration sequence

1. Create a shared project manifest parser while keeping current Font test behavior unchanged.
2. Move `pillow-rs/tests/fixtures/font/font_manifest.yaml` content into `pillow-rs/tests/fixtures/manifest.yaml` under namespace `ImageFont`.
3. Update `font_public_api.rs` to read the project manifest but filter `namespace == ImageFont`.
4. Extract generic input-only validation into shared support code.
5. Extract generic runtime-oracle invocation into shared support code.
6. Extract generic `Result` payload comparison into shared support code.
7. Keep Font-specific operation execution in `font_runner.rs` temporarily.
8. Migrate crate-local `pillow-rs/tests/fixtures/image_backend` rows into the same manifest namespace model.
9. Replace stored image backend outputs with live Pillow oracle generation.
10. Migrate selected image-slash-star codec rows into project input-only cases.
11. Add route/pending accounting borrowed from fontdone, but only as manifest metadata; do not put expected runtime result in input JSON.
12. Once an old runner's cases are fully represented and Coverage MCP proves equivalent/better coverage, remove that old runner and its deprecated fixture outputs.

## Immediate action items

1. Decide active manifest location:
   - recommended: `pillow-rs/tests/fixtures/manifest.yaml`;
   - avoid repository-root `tests/fixtures` because that root was just deprecated and is not crate-local.
2. Add project manifest schema with only `ImageFont` migrated first.
3. Update Font runner to consume project manifest without changing case execution.
4. Run Font parity and Coverage MCP to prove no regression.
5. Migrate `pillow-rs/tests/fixtures/image_backend/manifest.json` into the same manifest schema.
6. Write a live Pillow oracle for image backend decode/verify/load parity.
7. Do not migrate image-slash-star generated output references directly; convert them to input-only cases and runtime oracle rows.
8. Keep fontdone read-only unless the user explicitly authorizes upstream changes.

## Design decision

Use Font/ImageFont as the canonical project-wide parity test design.

Use fontdone as the canonical manifest coverage/accounting design.

Treat image-slash-star's current coverage matrix as migration source material, not as the final project-level truth model.
