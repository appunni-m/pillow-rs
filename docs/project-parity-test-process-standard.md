# Project parity test process standard

Date: 2026-07-31

Status: design standard for the next migration.

This document defines the single reproducible test process for `pillow-rs` project parity. It is intentionally strict. The purpose is to make parity evidence trustworthy, repeatable, coverage-aware, and hard to fake.

## Core rule

The project has one parity process:

```text
input-only fixture -> live Pillow oracle -> live Rust execution -> normalized Result comparison -> Coverage MCP evidence
```

No checked-in input file may contain expected output, expected errors, hashes, byte counts, status, oracle results, or reference output paths.

Pillow is the oracle. Rust is the implementation under test. Coverage MCP is the evidence ledger.

## Goals

The standard must improve all directions at once:

- parity truth: compare against live Pillow 12.2.0 behavior, not stale stored output;
- API truth: manifest covers public surfaces explicitly;
- input truth: fixtures describe only independent inputs;
- error truth: success/error status comes from oracle/runtime execution;
- coverage truth: coverage claims require fresh Coverage MCP snapshots;
- reproducibility: all commands, oracle versions, inputs, assets, and outputs are deterministic;
- migration safety: old tests are removed only after equivalent or better migrated coverage;
- anti-cheat safety: tests reject self-comparison, embedded expected output, and coverage shortcuts.

## Non-goals

The new standard must not:

- preserve old fixture layout for convenience;
- keep stored expected pixels/hashes as the primary oracle;
- compare Rust against Rust;
- use deprecated fixtures as active truth;
- hide pending implementation gaps;
- mark a route covered because a unit test touched it;
- claim 100% coverage without an ingested Coverage MCP artifact;
- place behavior logic in Python/JS bindings.

## Repository scope

The active project-level corpus lives under the `pillow-rs` crate:

```text
pillow-rs/tests/fixtures/
  manifest.yaml
  assets/
  inputs/
```

The repository-root legacy fixture corpora are deprecated:

```text
tests/deprecated/fixtures
tests/deprecated/fixtures_2
```

They may be used as migration source material only. They are not active truth.

Sibling repositories are upstream dependencies:

```text
../image-slash-star
../fontdone
```

For this project-level process, sibling repositories may be read to understand behavior, but the active project parity corpus and runner live in `pillow-rs`.

## Directory layout

Target layout:

```text
pillow-rs/tests/fixtures/
  manifest.yaml

  assets/
    fonts/
    images/
    encoded/
    raw/

  inputs/
    ImageFont/
      getbbox.json
      getlength.json
      getmask.json
      load.json
      truetype.json

    Image/
      open.json
      new.json
      frombytes.json
      convert.json
      crop.json
      paste.json
      verify.json

    ImageDraw/
      text.json
      line.json
      rectangle.json

    ImageOps/
    ImageChops/
    ImageFilter/
    ImagePalette/
```

Deprecated/reference-only material may live under:

```text
pillow-rs/tests/deprecated/
```

or the already deprecated repository-root paths, but active test runners must not read from deprecated fixture roots.

## Single manifest

There is exactly one active project manifest:

```text
pillow-rs/tests/fixtures/manifest.yaml
```

The manifest is the authoritative index of:

- oracle identity;
- namespace inventory;
- public operations;
- input files;
- required parameter coverage;
- out-of-scope features;
- explicitly pending routes;
- migration status;
- coverage requirements.

The manifest is not an output store.

### Manifest required top-level fields

```yaml
version: 1

oracle:
  provider: pillow
  version: "12.2.0"
  python: ".oracle-venv/bin/python"
  import_contract: "PIL public API"

rust:
  crate: pillow-rs
  public_api: "pillow_rs root API only"
  binding_rule: "Python and JS bindings are thin wrappers only"

policy:
  input_only: true
  runtime_oracle: true
  result_comparison: true
  coverage_mcp_required: true

namespaces: []
```

### Namespace entry

Each namespace maps one Pillow public module/class family to Rust public API.

```yaml
namespaces:
  - id: ImageFont
    pillow_path: PIL.ImageFont
    rust_surface: pillow_rs::imagefont
    input_dir: inputs/ImageFont
    assets_dir: assets
    status: active
    out_of_scope:
      - libraqm successful shaping
    operations:
      - id: getbbox
        kind: method
        public: true
        input_file: getbbox.json
        coverage:
          parameters:
            text: required
            mode: required
            direction: no-libraqm-error
            features: no-libraqm-error
            language: no-libraqm-error
            stroke_width: required
            anchor: required
```

### Operation fields

Every operation must declare:

- `id`: operation name used in input JSON;
- `kind`: function, method, property, constructor, constant, codec, or helper;
- `public`: whether this is a Pillow public surface;
- `input_file`: relative file under namespace `input_dir`;
- `coverage`: parameters, branches, and route intent;
- `status`: active, pending, unsupported, or deprecated;
- optional `reason` for pending/unsupported.

Operation status rules:

- `active`: must have input cases and runtime parity must pass;
- `pending`: must have a documented blocker and must be visible in reports;
- `unsupported`: must match Pillow unsupported/error behavior when called;
- `deprecated`: must not be included in active parity execution.

### Coverage fields

Coverage requirements belong in the manifest, not in input cases:

```yaml
coverage:
  parameters:
    mode:
      required_values:
        - "<default>"
        - "1"
        - "L"
        - "RGBA"
        - "bad"
    anchor:
      required_values:
        - "<default>"
        - "la"
        - "mm"
        - "xy"
  branches:
    - empty_text
    - non_ascii_text
    - invalid_mode
    - missing_asset
  regions:
    - file: src/font/imagingft.rs
      target: 100
```

Inputs prove coverage by exercising these values. Inputs do not say what output should happen.

## Input JSON standard

Input files contain only runnable input cases.

Required document shape:

```json
{
  "version": 1,
  "namespace": "ImageFont",
  "operation": "getbbox",
  "cases": []
}
```

Required case shape:

```json
{
  "case_id": "ImageFont.getbbox.basic_latin_default",
  "operation": "getbbox",
  "inputs": {
    "assets": {},
    "params": {}
  }
}
```

Allowed case keys:

```text
case_id
operation
inputs
```

Allowed `inputs` keys:

```text
assets
params
environment
```

`environment` is allowed only for declarative input conditions such as locale, feature flags, or Pillow plugin availability. It must not contain expected output.

### Forbidden keys anywhere in input JSON

The runner must recursively reject these keys:

```text
error
expect_error
expectation
expected
hash
oracle
output
outputs
pixels
pixels_hex
raw_path
ref_path
ref_bytes
encoded_ref_path
encoded_ref_bytes
sha256
status
actual
baseline
golden
```

Reason: these keys encode expected behavior. Expected behavior must come from live Pillow.

### Case ID standard

Case IDs must be globally unique and deterministic:

```text
<Namespace>.<operation>.<short_independent_path>
```

Examples:

```text
ImageFont.getbbox.basic_latin_default
Image.open.png_bad_idat_crc_verify
Image.paste.rgba_mask_partial_alpha
ImageDraw.text.no_libraqm_direction_rtl
```

Case IDs must not include random values, timestamps, host paths, machine names, or generated counters that can change between runs.

### Independent input rule

Every input case must cover an independent behavior path.

Do not add duplicates just to increase case count. If two cases cover the same parameters, same branch, same mode, same error category, and same asset family, keep one.

Acceptable reasons for multiple cases:

- different public operation;
- different parameter branch;
- different image mode;
- different font table/path;
- different codec path;
- success vs error;
- boundary value;
- regression for a known divergence;
- platform-independent Pillow behavior difference.

## Asset standard

Assets are inputs. They are allowed.

Asset references must be relative to `pillow-rs/tests/fixtures/assets` unless a namespace explicitly sets a narrower asset directory.

Allowed asset descriptor:

```json
{
  "kind": "ref",
  "path": "fonts/DejaVuSans.ttf"
}
```

Allowed asset kinds:

- `ref`: tracked fixture asset;
- `inline_bytes`: small byte payload for malformed/minimal inputs;
- `generated_input`: deterministic generated asset declared by a maintained generator;
- `pillow_builtin`: Pillow built-in/default resource, if the oracle uses the same resource deterministically;
- `missing_ref`: intentional missing-file input for error parity only.

Rules:

- assets must be read-only during tests;
- asset paths must canonicalize under the fixture asset root;
- no absolute paths in committed input JSON;
- no network access for assets during tests;
- generated assets must have a maintained generator and deterministic seed;
- generated assets must be committed only if needed for reproducibility;
- output artifacts from a test run are never assets unless promoted intentionally as input fixtures.

## Oracle runtime standard

The oracle is live Pillow 12.2.0 running in the repository oracle environment.

Default oracle executable:

```text
.oracle-venv/bin/python
```

The runner must:

- verify the interpreter path exists;
- verify the interpreter is under the repository oracle environment;
- isolate user site packages;
- verify Pillow version is exactly 12.2.0;
- verify relevant Pillow plugin versions when format-specific behavior matters;
- pass input cases over stdin or a deterministic batch file;
- receive normalized JSON results over stdout;
- treat non-zero oracle exit as test failure;
- include oracle stderr in bounded failure output.

The oracle script must not read Rust output. It only sees input cases and assets.

The oracle script must not use checked-in expected output.

## Rust runtime standard

Rust execution must use the public root API exposed by `pillow-rs/src/lib.rs`.

Rules:

- no deep imports from implementation modules in project parity tests;
- no direct calls into private/internal modules;
- no test-only implementation shortcuts;
- no subprocess or Python calls from Rust implementation code;
- no fixture/oracle path reads from Rust implementation code;
- no `cfg(test)` behavior that changes production results;
- no coverage exclusions in implementation code to fake 100% coverage.

Bindings must stay thin:

- Python ABI forwards inputs to Rust;
- JS ABI forwards inputs to Rust;
- binding code may convert types and map errors;
- binding code must not implement algorithms or parity logic.

## Result envelope

Both oracle and Rust output must normalize to one envelope.

Success:

```json
{
  "case_id": "ImageFont.getbbox.basic_latin_default",
  "status": "ok",
  "value": {}
}
```

Error:

```json
{
  "case_id": "Image.open.bad_png",
  "status": "error",
  "error": {
    "class": "SyntaxError",
    "kind": "malformed_png",
    "message": "broken PNG file"
  }
}
```

The Rust side may internally use idiomatic `Result<T, PilError>`. The serialized test payload still uses the same envelope as Pillow.

### Result comparison

Comparison is generic:

```text
status must match
if status == ok: value must match exactly
if status == error: error must match exactly according to the error policy
```

The comparator must not contain per-case expected output.

The comparator may contain type-specific normalization rules, but those rules must be reusable and documented.

Examples of valid normalization:

- convert tuples/lists into the same JSON array shape;
- serialize image bytes as hex only in runtime output, never input;
- normalize path display to basename if Pillow exposes host-specific absolute paths;
- normalize floating point only when Pillow itself uses a documented representation.

Examples of invalid normalization:

- ignore mismatching pixels;
- ignore mismatching error class;
- accept any error for an error case;
- round numbers just to make a failing case pass;
- compare only hash when raw bytes are available;
- special-case a case ID to force equality.

## Output type standard

Each operation declares an output shape in runner code or manifest schema. The shape controls normalization and exact comparison.

Common output types:

- scalar: bool, integer, float, string, null;
- tuple/list;
- dictionary/object with deterministic key ordering;
- image: mode, size, bands, palette, raw bytes;
- mask: mode, size, offset, raw bytes;
- encoded bytes: format, bytes, optional decoded verification;
- font metrics: exact numeric values;
- error: class, kind, message/category.

### Image output comparison

Image-like results must compare:

- mode;
- size;
- bands where applicable;
- palette bytes where applicable;
- transparency info where applicable;
- frame count for sequences;
- per-frame duration/loop metadata where applicable;
- raw pixel bytes.

Hash may be included in runtime output for diagnostics, but raw bytes remain the authoritative comparison when practical.

### Encoded output comparison

Encoded bytes are exact only when Pillow produces deterministic bytes for the operation/platform.

For deterministic encoders:

- compare encoded bytes exactly;
- then decode output and compare decoded pixels/metadata as a secondary check.

For nondeterministic encoders:

- the manifest must mark deterministic-byte parity as out of scope with reason;
- compare Pillow-observable decoded result and stable metadata;
- document the nondeterminism source.

Do not put encoded reference bytes in input JSON.

### Error output comparison

Error comparison must include:

- status: `error`;
- public error class/category;
- stable kind;
- message or stable message pattern when Pillow wording is stable;
- operation stage if Pillow distinguishes open/load/verify/save.

An expected-error input is forbidden. The case becomes an error case only because live Pillow returned an error.

If Pillow returns success and Rust returns error, fail.

If Pillow returns error and Rust returns success, fail.

If both error but class/kind mismatches, fail.

## Public surface standard

The manifest must cover public surfaces, not implementation files.

For each namespace:

- query live Pillow public names/signatures where possible;
- classify every public name as active, pending, unsupported, or non-endpoint;
- verify every active operation has an input file;
- verify every input file maps to a manifest operation;
- verify every case operation maps to a runner arm;
- verify every Rust public root API endpoint has manifest coverage or documented non-endpoint status.

Do not expose implementation functions just because tests need them.

If a test needs deep implementation access, the public API is probably wrong or the test belongs at a lower-level crate.

## Coverage standard

Coverage claims require Coverage MCP.

Required flow:

1. call `project_context`;
2. run an approved immutable command;
3. poll with `get_run_data` until terminal;
4. inspect coverage ingestion status;
5. record snapshot IDs;
6. query summary/files/file/insights as needed;
7. report missing lines/regions explicitly.

No coverage claim is valid without a current ingested snapshot.

### Coverage levels

The process distinguishes:

- parity pass: all active fixture rows match live Pillow;
- line coverage: source lines executed;
- branch coverage: branch outcomes executed;
- region coverage: compiler/source regions executed;
- function coverage: functions entered;
- public-surface coverage: manifest operations represented by input cases.

Passing parity does not imply coverage.

Coverage does not imply parity.

Both are required.

### 100% region coverage rule

For a target file or module, “100% region coverage” can be claimed only when:

- the Coverage MCP snapshot is fresh for the current commit;
- the suite includes the active project parity test;
- the target file appears in the coverage report;
- region total and covered counts are present;
- uncovered regions are zero;
- all active manifest rows ran successfully.

If any condition is absent, report “not proven.”

## Coverage MCP command standard

Every maintained parity/coverage command must be registered in Coverage MCP with:

- exact command;
- exact cwd;
- exact shell;
- suite name;
- declared coverage artifact path;
- coverage format;
- human approval note.

Do not run ad-hoc coverage commands and later claim coverage.

Use Make targets for normal workflows. If a repeated workflow has no Make target, add one.

## Anti-cheat guardrails

The project parity test must fail if:

- input JSON contains forbidden expected-output keys;
- active runner reads deprecated fixture roots;
- Rust implementation reads oracle or fixture paths;
- Rust implementation launches subprocesses;
- Rust implementation contains test-only parity branches;
- comparator contains case-id-specific success logic;
- Rust output is used as oracle output;
- oracle output count does not equal input case count;
- manifest input file list does not match discovered input files;
- an active manifest operation has no runner arm;
- a runner arm is not represented in the manifest;
- a public Pillow operation is unclassified;
- a public Rust root API endpoint is unclassified;
- Coverage MCP run is stale/missing but a coverage claim is attempted.

## Migration rules

Migration must be one surface at a time.

For each surface:

1. list current old fixture files and tests;
2. identify public Pillow operations;
3. create manifest namespace/operation entries;
4. create input-only JSON cases;
5. implement oracle runtime support;
6. implement Rust runner support through public root API;
7. compare exact `Result` envelopes;
8. run narrow parity test;
9. run Coverage MCP coverage suite;
10. add missing independent cases until intended region/branch coverage is proven;
11. mark old tests deprecated;
12. delete old tests only after new suite has equivalent or better evidence;
13. commit.

Do not delete first. Migrate, prove, then delete.

## Handling pending and unsupported behavior

Pending behavior belongs in the manifest, not in input expected output.

Allowed pending entry:

```yaml
operations:
  - id: complex_feature
    status: pending
    reason: "Rust implementation lacks Pillow-compatible X path"
    blocker: "link or doc section"
```

Pending rows must be visible in reports.

Unsupported behavior means Pillow public behavior is unsupported and Rust should match the same public error. Unsupported is still testable through live oracle.

## Reproducibility checklist

A parity run is reproducible only when the report records:

- git commit;
- dirty/clean status;
- manifest path and hash;
- input file list and hash;
- asset file list and hash;
- oracle Python path;
- Pillow version;
- Pillow plugin versions when relevant;
- Rust toolchain version;
- Make target or Coverage MCP command id;
- Coverage MCP run id;
- coverage snapshot id if coverage is claimed;
- command result and counters;
- exact failing case IDs if failed.

## Report format

Every final report for parity work must include:

- commit hash;
- changed files;
- tests run;
- pass/fail counts;
- Coverage MCP run id;
- Coverage MCP snapshot id when available;
- coverage percentage only if proven;
- pending manifest operations;
- deprecated tests removed or still retained;
- final git status.

## First implementation milestone

The first migration must be intentionally small:

1. create `pillow-rs/tests/fixtures/manifest.yaml`;
2. migrate existing Font/ImageFont manifest content into namespace `ImageFont`;
3. keep existing Font input JSON initially;
4. update Font test to read the project manifest;
5. extract input-only validation into shared test support;
6. keep exact current Font parity behavior;
7. run Font parity;
8. run Coverage MCP;
9. commit.

Only after this is stable should image backend and codec surfaces be migrated.

## Design decision

The canonical standard is:

```text
Font/ImageFont runtime-oracle parity semantics
+ fontdone manifest coverage/accounting discipline
- image-slash-star stored-output matrix as active truth
```

This is the project standard until superseded by a newer design document and migration commit.
