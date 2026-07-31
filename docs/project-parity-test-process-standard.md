# Universal migration parity test process standard

Date: 2026-07-31

Status: design standard for any implementation migration.

This document defines a single reproducible parity process for migrating behavior from any source implementation to any target implementation.

It is intentionally language-agnostic. The source may be Python, C, C++, Java, JavaScript, Go, Rust, a CLI, a service, or a binary library. The target may be any language/runtime. The standard is about truth, repeatability, coverage, and public-behavior compatibility.

Project-specific examples for `pillow-rs` appear only in the final profile section.

## Core rule

Every migration parity test follows one process:

```text
input-only fixture
-> live source oracle execution
-> live target implementation execution
-> normalized Result comparison
-> coverage/evidence ledger
```

Input fixtures describe only inputs. They do not describe expected outputs.

The source implementation is the oracle. The target implementation is the system under test. The evidence ledger records what ran, where it ran, what version ran, what passed, what failed, and what coverage was proven.

## Terminology

- Source implementation: the existing implementation whose observable behavior must be matched.
- Target implementation: the new implementation being migrated.
- Oracle: a controlled runtime execution of the source implementation.
- Fixture: checked-in input data used to call both source and target.
- Manifest: the authoritative public-surface and coverage-intent index.
- Result envelope: a normalized success/error payload produced by both source and target.
- Comparator: generic logic that compares two Result envelopes.
- Coverage ledger: durable system of record for approved commands, retained logs, and coverage snapshots.
- Public surface: externally observable API, CLI command, file format behavior, protocol, ABI, wire behavior, or user-visible contract.
- Active case: a case that must run and pass.
- Pending case: a known gap that is documented, visible, and not counted as passed.
- Unsupported case: behavior the source exposes as unsupported/error and the target must match.
- Deprecated case: old test/fixture material retained only as migration reference.

## Goals

The standard must improve all important dimensions together:

- parity truth: compare against live source behavior, not stale checked-in expected output;
- API truth: manifest covers public surfaces explicitly;
- input truth: fixtures contain only independent inputs;
- error truth: success/error status comes from runtime execution;
- coverage truth: coverage claims require fresh evidence;
- reproducibility: versions, commands, inputs, assets, environment, and outputs are deterministic;
- migration safety: old tests are removed only after equivalent or better migrated evidence;
- anti-cheat safety: tests reject self-comparison, embedded expected output, and coverage shortcuts;
- maintainability: adding a public operation requires manifest, input, runner, oracle, comparator, and coverage updates.

## Non-goals

The standard must not:

- preserve old test layout merely for convenience;
- keep stored expected pixels/hashes/text/bytes as the primary oracle;
- compare target output against target output;
- use deprecated fixtures as active truth;
- hide pending implementation gaps;
- mark a route trusted because a narrow unit test touched it;
- claim full coverage without a current coverage artifact;
- move behavior into a wrapper/binding just to satisfy tests;
- special-case case IDs to force equality.

## Required repository layout

Every project should have one active migration fixture root:

```text
tests/fixtures/
  manifest.yaml
  assets/
  inputs/
```

Recommended optional roots:

```text
tests/deprecated/
tests/support/
tests/oracles/
docs/
```

Rules:

- active runners read from `tests/fixtures`;
- deprecated/reference-only tests read from `tests/deprecated` only when explicitly named;
- generated run outputs go under `target/`, `.coverage-*`, or another ignored build directory;
- checked-in fixtures are inputs, not generated expected outputs.

## Single manifest

There is exactly one active manifest for the migration suite:

```text
tests/fixtures/manifest.yaml
```

The manifest is the authoritative index of:

- source/oracle identity;
- target identity;
- public namespaces/surfaces;
- operations/endpoints;
- input files;
- asset roots;
- required parameter/value coverage;
- required branch/region coverage;
- out-of-scope features;
- pending/unsupported/deprecated routes;
- migration status.

The manifest is not an output store.

## Manifest schema

Minimum shape:

```yaml
version: 1

source:
  name: source-system
  version: "exact-version"
  runtime: "command-or-runtime-used-for-oracle"
  contract: "public observable behavior"

target:
  name: target-system
  version: "current checkout/build"
  runtime: "command-or-library-entrypoint"
  contract: "public target surface used by tests"

policy:
  input_only: true
  live_oracle: true
  result_comparison: true
  coverage_required_for_claims: true

surfaces: []
```

Surface entry:

```yaml
surfaces:
  - id: ImageFont
    source_path: PIL.ImageFont
    target_path: pillow_rs::imagefont
    input_dir: inputs/ImageFont
    asset_dir: assets
    status: active
    out_of_scope:
      - successful complex text shaping
    operations:
      - id: getbbox
        kind: method
        public: true
        input_file: getbbox.json
        status: active
        coverage:
          parameters:
            text:
              required_values:
                - "Hello"
                - ""
                - "ज"
            mode:
              required_values:
                - "<default>"
                - "1"
                - "bad"
          branches:
            - empty_text
            - invalid_mode
          regions:
            - file: src/font/imagingft.rs
              target: 100
```

## Operation status rules

Every operation must be one of:

- `active`: must have input cases and must pass runtime parity;
- `pending`: known target gap; must have blocker/reason and appear in reports;
- `unsupported`: source behavior is unsupported/error and target must match it;
- `deprecated`: retained only as migration reference; active runner must ignore it.

Pending operation entry:

```yaml
operations:
  - id: complex_feature
    status: pending
    reason: "Target implementation lacks source-compatible X path"
    blocker: "docs/gap-analysis.md#complex-feature"
```

Pending is not passing. Pending is visible debt.

## Input JSON standard

Input files contain only runnable inputs.

Required document shape:

```json
{
  "version": 1,
  "surface": "ImageFont",
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

`environment` may describe declarative input conditions such as locale, platform feature flag, protocol version, runtime option, or plugin availability. It must not contain expected output.

## Forbidden input keys

The runner must recursively reject these keys in active input JSON:

```text
actual
baseline
encoded_ref_bytes
encoded_ref_path
error
expect_error
expectation
expected
golden
hash
oracle
output
outputs
pixels
pixels_hex
raw_path
ref_bytes
ref_path
sha256
status
```

Reason: these keys encode expected behavior. Expected behavior must come from live source execution.

## Case ID standard

Case IDs must be globally unique and deterministic:

```text
<Surface>.<operation>.<short_independent_path>
```

Examples:

```text
ImageFont.getbbox.basic_latin_default
Image.open.png_bad_idat_crc_verify
Codec.decode.gif_lzw_no_eoi
Cli.convert.missing_input_file
HttpClient.request.invalid_header_name
Parser.parse.empty_input
```

Case IDs must not contain:

- random values;
- timestamps;
- host paths;
- machine names;
- generated counters that can change between runs.

## Independent input rule

Every case must cover an independent behavior path.

Do not add duplicates just to increase case count.

Acceptable reasons for multiple cases:

- different public operation;
- different parameter branch;
- different data mode/type;
- different file format/protocol variant;
- different asset family;
- success vs error;
- boundary value;
- known historical divergence;
- source implementation documented edge behavior;
- platform-independent observable behavior difference.

Duplicate-filter rule:

```text
same surface + same operation + same parameter class + same branch + same mode + same asset family + same expected source status = duplicate
```

Duplicates should be removed or marked deprecated.

## Asset standard

Assets are inputs. They are allowed.

Asset references must be relative to the fixture asset root.

Allowed asset descriptor:

```json
{
  "kind": "ref",
  "path": "fonts/DejaVuSans.ttf"
}
```

Allowed asset kinds:

- `ref`: tracked fixture asset;
- `inline_bytes`: small byte payload;
- `generated_input`: deterministic generated asset from maintained generator;
- `builtin`: source runtime built-in/default resource;
- `missing_ref`: intentional missing-file input for error parity;
- `remote_mock`: deterministic local fixture representing a remote response, not a live network call.

Rules:

- assets are read-only during tests;
- asset paths canonicalize under fixture asset root;
- no absolute paths in committed input JSON;
- no network access during parity tests unless the public surface is explicitly network behavior and the network is replaced by a deterministic local server fixture;
- generated assets need maintained generator, deterministic seed, and documented command;
- generated assets are committed only when required for reproducibility;
- output artifacts from a run are never active input assets unless intentionally promoted by a reviewed fixture-generation change.

## Oracle runtime standard

The oracle is a controlled runtime execution of the source implementation.

The oracle runner must:

- verify the source runtime path/version;
- isolate user/global environment where possible;
- pin dependencies/plugins/feature flags;
- pass input cases through stdin, deterministic file, local IPC, or stable API call;
- receive normalized Result envelopes;
- treat oracle startup failure as test failure;
- treat oracle non-zero exit as test failure;
- record bounded oracle stderr/stdout on failure;
- ensure oracle result count equals input case count;
- ensure oracle never reads target output.

The oracle may be:

- a Python script calling a source package;
- a compiled C/C++ helper;
- a CLI wrapper;
- an HTTP/gRPC local fixture server;
- a WASM/JS runtime script;
- a JVM/.NET process;
- a direct library binding.

The oracle must not read checked-in expected output.

## Target runtime standard

The target runner executes the target implementation through the public target surface.

Rules:

- use the target public API, CLI, ABI, protocol, or wire surface;
- do not call deep private implementation paths unless the migrated public surface is itself low-level/internal by design;
- do not use test-only behavior to change production results;
- do not read oracle files from target implementation code;
- do not launch the source oracle from target implementation code;
- do not add coverage exclusions to fake completeness;
- do not implement behavior in a wrapper/binding if the core target must own it.

If wrappers/bindings exist:

- wrappers may convert types;
- wrappers may map errors;
- wrappers may manage handles/lifetimes;
- wrappers must not implement algorithms, fixture interpretation, or source-specific parity hacks.

## Result envelope

Both source oracle and target output normalize to the same envelope.

Success:

```json
{
  "case_id": "Surface.operation.case",
  "status": "ok",
  "value": {}
}
```

Error:

```json
{
  "case_id": "Surface.operation.case",
  "status": "error",
  "error": {
    "class": "ValueError",
    "kind": "invalid_argument",
    "message": "stable source-visible message"
  }
}
```

The target may internally use any idiomatic error mechanism:

- `Result<T, E>`;
- exceptions;
- error codes;
- status objects;
- process exit codes;
- rejected promises;
- protocol error frames.

The parity harness converts that native mechanism into the standard envelope.

## Comparator standard

Comparison is generic:

```text
case_id must match
status must match
if status == ok: value must match according to declared output shape
if status == error: error must match according to declared error policy
```

The comparator must not contain case-id-specific pass logic.

The comparator may contain reusable type-specific normalization rules, but every normalization rule must be documented.

Valid normalization examples:

- tuple/list shape normalization;
- map/object deterministic key ordering;
- path normalization to basename when the source exposes host-specific absolute paths;
- stable float representation when the source documents/uses it;
- byte output encoded as hex/base64 in runtime JSON;
- platform-neutral newline normalization when the public contract specifies text, not raw bytes.

Invalid normalization examples:

- ignore mismatching bytes;
- ignore mismatching error class;
- accept any error for an error case;
- round numbers just to make a failing case pass;
- compare only hash when raw bytes are available;
- special-case a case ID;
- suppress fields that differ without declaring them out of scope.

## Output shape standard

Every operation must have a declared output shape.

Common shapes:

- scalar: bool, integer, float, string, null;
- sequence: tuple/list/array;
- object: deterministic key-value structure;
- bytes: raw byte array represented deterministically;
- image: mode, size, bands, palette, frames, raw bytes;
- mask: mode, size, offset, raw bytes;
- encoded file: format, bytes, decoded verification;
- metrics: exact numeric values;
- protocol response: status, headers, body, trailers;
- CLI result: exit code, stdout, stderr, generated files;
- error: class/category/kind/message/stage.

### Byte-like output

Byte-like output must compare raw bytes exactly unless the source contract is explicitly nondeterministic.

Diagnostic hashes may be included in runtime output, but raw bytes are authoritative when practical.

### Structured output

Structured output must compare:

- field presence;
- field values;
- ordering when order is public behavior;
- absence when absence is public behavior;
- numeric type/precision when observable.

### Encoded/nondeterministic output

If source output is nondeterministic:

- declare nondeterminism in manifest;
- compare stable public observations instead;
- include a deterministic secondary validation if possible;
- do not store one generated output as truth.

Example:

```yaml
determinism:
  encoded_bytes: false
  reason: "source encoder embeds timestamp"
  compare_instead:
    - decoded_pixels
    - public_metadata_without_timestamp
```

## Error comparison standard

Error comparison must include:

- `status: error`;
- public error class/category;
- stable kind;
- stable message or documented message pattern;
- operation stage when source distinguishes stages;
- exit/status code when applicable.

The case becomes an error case only because live source oracle returned an error.

Outcomes:

- source ok + target ok + equal value = pass;
- source ok + target ok + different value = fail;
- source ok + target error = fail;
- source error + target ok = fail;
- source error + target error + equal error = pass;
- source error + target error + different error = fail.

## Public surface standard

The manifest covers public behavior, not arbitrary implementation files.

For each surface:

- discover or document source public names/signatures;
- classify every public name as active, pending, unsupported, deprecated, or non-endpoint;
- verify every active operation has an input file;
- verify every input file maps to a manifest operation;
- verify every case operation maps to a runner arm;
- verify every target public endpoint is covered or documented as non-endpoint;
- report all unclassified source/target public names.

If a test needs deep implementation access, either:

- the public API is missing the necessary behavior, or
- the test belongs in a lower-level component suite, not the migration parity suite.

## Coverage standard

Coverage claims require a durable coverage/evidence ledger.

Required flow:

1. discover approved commands and latest results;
2. run only approved immutable commands;
3. poll until terminal;
4. inspect artifact ingestion;
5. capture snapshot IDs;
6. query summary/files/file/insights as needed;
7. report missing lines/branches/regions explicitly.

No coverage claim is valid without a current ingested artifact.

The process distinguishes:

- parity pass: all active fixture rows match live source;
- line coverage: source lines executed;
- branch coverage: branch outcomes executed;
- region coverage: compiler/source regions executed;
- function coverage: functions entered;
- public-surface coverage: manifest operations represented by input cases.

Passing parity does not imply coverage.

Coverage does not imply parity.

Both are required.

## 100% coverage claim rule

For any target file/module/surface, “100% coverage” can be claimed only when:

- the coverage snapshot is fresh for the current commit;
- the suite includes the active parity test;
- the target file/module appears in the coverage report;
- relevant totals and covered counts are present;
- uncovered lines/branches/regions/functions are zero for the claimed dimension;
- all active manifest rows ran successfully.

If any condition is absent, report:

```text
not proven
```

## Evidence ledger command standard

Every maintained parity/coverage command must be registered with:

- exact command;
- exact cwd;
- exact shell/runtime;
- suite name;
- declared artifacts;
- coverage format when applicable;
- human approval or review record;
- immutable command identity.

Do not run ad-hoc coverage commands and later claim coverage.

Use maintained build/test targets for normal workflows. If a repeated workflow has no target, add one.

## Anti-cheat guardrails

The parity suite must fail if:

- input JSON contains forbidden expected-output keys;
- active runner reads deprecated fixture roots;
- target implementation reads oracle or fixture paths;
- target implementation launches source oracle;
- target implementation contains test-only parity branches;
- comparator contains case-id-specific success logic;
- target output is used as oracle output;
- oracle result count does not equal input count;
- manifest input file list does not match discovered input files;
- active manifest operation has no runner arm;
- runner arm is not represented in manifest;
- source public operation is unclassified;
- target public endpoint is unclassified;
- stale/missing coverage is reported as coverage proof.

## Migration process

Migration must happen one public surface at a time.

For each surface:

1. inventory old tests/fixtures;
2. identify source public operations;
3. identify target public endpoints;
4. create manifest surface/operation entries;
5. create input-only JSON cases;
6. implement source oracle runner;
7. implement target runner through public target surface;
8. normalize both sides into Result envelopes;
9. compare exactly;
10. run narrow parity test;
11. run coverage/evidence suite;
12. add missing independent cases until intended coverage is proven;
13. mark old tests deprecated;
14. delete old tests only after equivalent or better evidence exists;
15. commit.

Do not delete first. Migrate, prove, then delete.

## Reproducibility checklist

A parity run is reproducible only when the report records:

- source repository/version/build identity;
- target repository/version/build identity;
- git commit or immutable source revision;
- dirty/clean status;
- manifest path and hash;
- input file list and hash;
- asset file list and hash;
- oracle runtime path/version;
- target runtime/toolchain version;
- dependency/plugin versions relevant to behavior;
- command id/target;
- run id;
- coverage snapshot id when coverage is claimed;
- command result and counters;
- exact failing case IDs if failed.

## Final report standard

Every parity task report must include:

- commit hash;
- changed files;
- tests run;
- pass/fail counts;
- evidence run id;
- coverage snapshot id when available;
- coverage percentage only if proven;
- pending manifest operations;
- deprecated tests removed or retained;
- final worktree status.

## First implementation milestone for any project

Start small:

1. create `tests/fixtures/manifest.yaml`;
2. choose one public surface;
3. migrate only that surface into the manifest;
4. keep existing input cases initially if they are input-only;
5. reject embedded outputs/errors;
6. implement source oracle execution;
7. implement target public-surface execution;
8. compare Result envelopes exactly;
9. run parity;
10. run coverage/evidence command;
11. commit.

Only after this is stable should broader surfaces be migrated.

## `pillow-rs` project profile

For this repository, instantiate the universal terms as:

```text
source implementation = Pillow 12.2.0
source oracle = .oracle-venv/bin/python executing PIL public APIs
target implementation = pillow-rs
target runner = Rust tests calling pillow_rs root public API
coverage ledger = Coverage MCP
active fixture root = pillow-rs/tests/fixtures
single manifest = pillow-rs/tests/fixtures/manifest.yaml
```

Current migration priority:

1. **Project inventory accounted:** the single active manifest derives its
   denominator from
   `pillow-rs/tests/deprecated/project_manifest_v0/manifest.yaml` and
   classifies all 12 legacy Pillow surfaces and all 173 catalogued public
   names. This is 100% public-surface accounting, not 100% runnable parity.
2. **Active:** Font/ImageFont is represented by 42 input documents, 445
   input-only cases, a live Pillow identity handshake, the Rust public target
   runner, a shared strict Result comparator, anti-cheat/schema tests, and
   managed coverage evidence. It contributes 41 active operations and one
   tested unsupported operation. Its bespoke v0 corpus is retained under
   `pillow-rs/tests/deprecated/font_public_api_v0`.
3. **Pending:** the remaining 11 project surfaces and 164 legacy public names
   are explicit pending rows with migration reasons and blockers. They do not
   claim input files, output shapes, runner arms, branch coverage, or parity.
4. **Pending:** migrate image backend parity; its current stored-output suite
   remains active until equivalent live-oracle evidence exists.
5. **Pending:** migrate codec rows from `image-slash-star` as input-only cases
   with a live Pillow oracle.
6. **Pending:** migrate the deprecated root Python fixture corpora operation by
   operation; deprecated material is never active truth.
7. Use `fontdone` as the model for public-surface accounting and pending-route
   visibility, not as the direct schema.

Project-specific hard rules:

- Python and JS bindings remain thin;
- target behavior belongs in Rust core;
- active input JSON must not contain output/error expectations;
- Coverage MCP is required for coverage claims;
- deprecated root fixture corpora are migration references only.
