# Canonical manifest, input, evidence, and documentation design

Date: 2026-07-31

This review compares five different artifacts that were previously discussed as
though they were the same kind of “manifest.” They are not. It also records the
required project-wide design.

## Decision

The canonical manifest and its indexed input JSON are specification.

Generated parity comparisons, coverage snapshots, benchmark measurements,
aggregate status, and generated status documentation are evidence.

The manifest must be the only public operation inventory. Parity, coverage,
benchmark, stub, and documentation consumers select work from it. They must not
keep independent operation lists.

The current `pillow-rs/tests/fixtures/manifest.yaml` is not a final project-wide
manifest. It is a bootstrap hybrid containing complete Font cases and
name-level placeholders for the other old project surfaces. Its “100%” value
means only that names from the old root manifest were classified. It does not
mean 100% scenario specification, parity, code coverage, benchmark readiness,
or documentation.

`font` is also not the correct canonical surface ID. The source namespace is
`PIL.ImageFont`, so the canonical ID is `ImageFont`. Lowercase storage slugs may
be mapped explicitly, but they are not public identities.

## The five artifacts are different

| Artifact | What it actually models | Main strength | Main limitation |
| --- | --- | --- | --- |
| `manifest.yaml` at repository root | Broad Pillow product/API capability catalog | Twelve modules, signatures, modes, formats, variants, targets, edge cases; already feeds coverage and benchmark tooling | Does not directly enumerate executable live-oracle cases; support status is declaration-heavy and sometimes absent/ambiguous |
| Deprecated Font manifest | One high-level `PIL.ImageFont` parity suite | 41 required operations and 445 input-only cases executed against live Pillow and public Rust behavior | Font-only; mixes some public Pillow APIs with native/helper consumer paths; no project-wide coverage or benchmark contract |
| Current canonical manifest | Hybrid migration index plus the migrated Font parity suite | Preserves the 445 live Font cases and accounts for all old root-manifest names | Only Font is concretely scoped; other surfaces are name placeholders; one status model is overloaded; it does not yet drive project coverage, benchmarks, or docs |
| `fontdone/tests/manifest.yaml` | Low-level FreeType C API/ABI subject and case ledger | Exhaustive subjects/cases, strong manifest-case mapping, route accounting, multi-backend parity, stale-cache protection | FreeType-specific; input files include expectation/error metadata; not the high-level Pillow product manifest and not a benchmark/documentation source |
| `image-slash-star/manifest.yaml` plus generated matrix | Codec/format plan and generated reference matrix | Deep format taxonomy, assets, edge cases, decode/encode lifecycle coverage | Runner reads a generated second inventory with stored oracle outputs/errors; no live Pillow source on every run; benchmark/docs are not integrated |

### Repository-root project manifest

The old root `manifest.yaml` is 2,477 lines and contains twelve modules:

```text
Image ImageModule ImageDraw ImageFilter ImageEnhance ImageOps
ImageChops ImageColor ImagePalette ImageFont ImageStat ImageSequence
```

It has 173 top-level operation rows: 162 declared `implemented`, five
`ignored`, and six without a status. Rows can contain:

```text
name signature supported_modes supported_formats param_variants edge_cases
pillow_since supported_targets methods properties type reason
```

This is the closest existing artifact to a product specification. It already
feeds:

- operation/mode registries;
- fixture and coverage validation;
- multi-backend coverage;
- benchmark specification generation.

Its weakness is traceability. Concrete parity cases are inferred through other
files and conventions rather than linked through stable requirement IDs. It
also conflates a declaration such as `implemented` with evidence that the
behavior has been compared and covered.

### Deprecated Font manifest

The archived Font v0 manifest is:

```text
pillow-rs/tests/deprecated/font_public_api_v0/font_manifest.yaml
```

It contains:

- 41 required operations;
- one negative operation;
- 23 operation-level parameter coverage declarations;
- 35 grouped input files;
- 445 input-only cases.

It is an executable parity suite:

```text
input case -> live Pillow 12.2.0 oracle
           -> public Rust target execution
           -> exact normalized Result comparison
```

It is excellent evidence for `ImageFont`, but it is not a full product
specification. It has no general code-coverage target/dimension schema, no
benchmark workloads or budgets, and no generated-document interface.

### Current canonical manifest

The newly created file is:

```text
pillow-rs/tests/fixtures/manifest.yaml
```

It currently contains:

- twelve surfaces;
- 206 operation rows;
- 41 active Font operations;
- one Font negative/unsupported row;
- 164 pending placeholder rows;
- 42 indexed Font input files;
- 445 concrete cases, all belonging to the migrated Font suite.

The old root catalog has 173 rows while the current manifest has 206 because the
Font migration expands detailed public/helper operation accounting beyond the
root catalog’s top-level rows.

What the current file proves:

- the twelve old surface names were not silently lost;
- all migrated Font input files and 445 cases are indexed;
- Font cases are input-only and runnable through the live oracle.

What it does not prove:

- complete signatures, modes, formats, variants, edges, and exclusions for the
  eleven non-Font surfaces;
- concrete parity cases outside Font;
- project-wide line/branch/function/region coverage;
- project-wide benchmark workload readiness or performance;
- current documentation generated from compatible evidence.

Therefore the current `accounting: 100%` is too broad. At most it is:

```text
inventory classification = represented old names / old manifest names
```

It must not be presented as “manifest completeness.”

### `fontdone`

`fontdone` is the sibling pure-Rust FreeType-compatible implementation. It is
not another spelling of `ImageFont`.

Its manifest is a low-level C API/ABI denominator:

- 1,543 public subjects;
- 4,184 declared manifest cases;
- functions, records, enums, enum variants, flags, macros, tags, types,
  constants, and errors;
- concrete input files mapping runtime cases to manifest cases;
- route categories such as real parity, null validation, pending route, and
  safety extension;
- Rust, C ABI, and WASM paths where applicable.

The project should borrow:

- exhaustive denominator discovery;
- explicit manifest requirement-to-input mapping;
- bidirectional route audits;
- visible pending/incomplete lanes;
- cache/evidence keys containing executable, input, and asset identity.

The project must not copy:

- expected output/error fields into active Pillow input JSON;
- C symbol/header/ABI structure as the high-level Pillow schema;
- operation-specific comparison shortcuts;
- C/WASM requirements for high-level APIs that do not expose those interfaces.

### `image-slash-star`

`image-slash-star` is a codec-focused sibling project. Its top-level manifest
describes eight codecs and roughly 380 edge cases:

```text
jpeg png gif bmp webp tiff ico avif
```

It has rich format details:

- oracle and codec versions;
- extensions and magic bytes;
- decoder/encoder ownership;
- assets and format-specific edge cases;
- planned gaps and quality goals.

The active test, however, reads generated `coverage_matrix.json` and stored
reference artifacts. Matrix rows include oracle status/error and expected
reference paths/bytes. This creates two practical sources of truth:

```text
high-level manifest -> generated matrix + stored output -> active runner
```

The project should borrow the format taxonomy, deterministic asset generation,
edge cases, and lifecycle stages. It must convert them to input-only live-oracle
parity cases. Generated oracle output remains evidence, not input truth.

## Source-of-truth architecture

There are three specification layers and three generated evidence lanes:

```text
                         CANONICAL SPECIFICATION

 authoritative inventory
          |
          v
 manifest.yaml ---- requirements/status/policy/interfaces/docs
      |                         |                         |
      v                         v                         v
 parity input JSON       coverage plan JSON       benchmark input JSON
 public invocations      test/case selection       workloads/execution knobs
      |                         |                         |
      v                         v                         v
 live source+target      managed instrumentation   benchmark runner
      |                         |                         |
      v                         v                         v
 parity-result.json      coverage-result.json      benchmark-result.json
      \_________________________|_________________________/
                                |
                  compatible identity/hash join
                                |
                                v
                       status-report.json
                                |
                                v
                 generated specification/status docs
```

Nothing below the “canonical specification” line writes results back into the
manifest or input JSON.

## Canonical manifest contract

The manifest is versioned specification. Its required top-level shape is:

```yaml
schema: migration-parity/manifest@2
version: 2
scope:
  id: pillow-public-api
  phase: final
  inventory:
    authority: manifest.yaml plus verified Pillow public discovery
    revision: Pillow-12.2.0
source:
  name: Pillow
  version: "12.2.0"
  runtime: .oracle-venv/bin/python
  contract: selected PIL public observable behavior
target:
  name: pillow-rs
  version: current-checkout
  runtime: public Rust API
  contract: selected public Rust observable behavior
policy:
  input_only: true
  live_oracle: true
  result_comparison: true
  coverage_required_for_claims: true
interfaces:
  parity:
    input_schema: migration-parity/parity-input@1
    result_schema: migration-parity/parity-result@1
    command: maintained repository command
  coverage:
    input_schema: migration-parity/coverage-input@1
    result_schema: migration-parity/coverage-result@1
    command: maintained managed-coverage command
  benchmark:
    input_schema: migration-parity/benchmark-input@1
    result_schema: migration-parity/benchmark-result@1
    command: maintained benchmark command
  aggregation:
    input_schemas:
      - migration-parity/parity-result@1
      - migration-parity/coverage-result@1
      - migration-parity/benchmark-result@1
    result_schema: migration-parity/status-report@1
    command: maintained aggregation command
surfaces: []
documentation:
  command: maintained documentation command
  outputs: []
```

The manifest owns:

- canonical surface and operation identity;
- source and target signatures;
- endpoint classification;
- declared target support;
- exhaustive behavior/performance requirements;
- parity, coverage, and benchmark lane readiness;
- indexed input file paths;
- output/comparison policy;
- coverage targets, dimensions, and thresholds;
- benchmark metrics and product budgets;
- maintained runner and documentation interfaces;
- explicit exclusions, blockers, and deprecation mappings.

It does not own:

- pass/fail;
- covered/total counts or percentages;
- snapshot/run IDs;
- timings or sample distributions;
- regression outcomes;
- last successful results;
- generated documentation text.

## Strict, well-defined, extensible interfaces

“Extensible” means the fixed schema has enough defined structure for diverse
interfaces and can improve deliberately. It does not mean runtime extension
registries or dynamic payload schemas.

- Every manifest, input, result, and aggregate object has exact allowed fields.
- Unknown fields and unsupported schema identifiers fail before execution.
- Diversity is expressed as data inside defined fields: interface `kind`,
  requirements, public params, typed assets, environment, output shape,
  coverage dimensions, benchmark metrics, thresholds, and budgets.
- If a real case cannot be represented, propose the missing fixed field or enum
  value with exact semantics.
- An accepted shape/meaning change gets a new schema major, coordinated
  producer/consumer changes, and a deterministic maintained migrator.
- Old and migrated examples plus intentionally invalid examples must be tested.
- Evidence from different schema versions cannot be joined.
- Documentation is regenerated from the migrated specification.

This keeps the interface closed and predictable today while leaving a clear,
reviewable path to improve it tomorrow.

## Operation contract

Each public name appears once. An endpoint operation has:

```yaml
- id: getbbox
  kind: method
  source_signature: "getbbox(text, mode='', direction=None, ...)"
  target_signature: "getbbox(text, ...)"
  classification: endpoint
  support:
    status: supported
  requirements:
    - id: text.basic_latin
      dimension: input_family
      description: Basic Latin text
      lanes: [parity, coverage]
    - id: performance.standard_latency
      dimension: performance
      description: Standard bbox latency
      lanes: [benchmark]
      budget:
        metric: latency
        statistic: median
        operator: less_than_or_equal
        value: 10
        unit: millisecond
  parity:
    status: active
    input_files: [inputs/parity/ImageFont/getbbox.json]
    output_shape: sequence
    comparison:
      normalization: exact
  coverage:
    status: active
    input_files: [inputs/coverage/ImageFont/getbbox.json]
    targets: [pillow-rs/src/font.rs]
    dimensions: [function, line, branch, region]
    thresholds:
      line: 100
      branch: 100
  benchmark:
    status: active
    input_files: [inputs/benchmark/ImageFont/getbbox.json]
    metrics: [latency]
```

Requirements are the semantic join. Every input item names requirement IDs in
`covers`. No mapping is inferred from filenames or prose.

## Status definitions

Three different facts must never share one status field.

### Classification

- `endpoint`: independently observable public behavior.
- `non_endpoint`: inventoried public metadata, namespace, type marker,
  re-export, or other non-invocable name.

### Target support declaration

- `supported`: target claims the complete declared behavior contract.
- `partial`: exact missing requirement IDs and reason are present.
- `unimplemented`: required behavior is absent; reason and blocker are present.
- `intentionally_unsupported`: policy exclusion with reason and authority.
- `out_of_scope`: outside the declared denominator with reason and authority.
- `deprecated`: retained for migration traceability with replacement/authority.
- `not_applicable`: only for a `non_endpoint`.

This is a product declaration, not measured proof.

### Lane readiness

Parity, coverage, and benchmark each use:

- `active`: indexed inputs exist and the runner is executable; the result may
  pass or fail.
- `pending`: runner is not ready; reason and blocker are required.
- `blocked`: inputs/routing exist but an external prerequisite blocks execution;
  reason, blocker, and unblock condition are required.
- `not_applicable`: the lane intentionally does not apply; an endpoint needs a
  reason.

Passing, failing, coverage percentage, and benchmark regression are generated
evidence, never lane state.

## Three input interfaces

All active input files are specification and recursively reject expected or
observed results.

### Parity input

```json
{
  "schema": "migration-parity/parity-input@1",
  "surface": "ImageFont",
  "operation": "getbbox",
  "cases": [
    {
      "case_id": "ImageFont.getbbox.basic_latin",
      "operation": "getbbox",
      "covers": ["text.basic_latin"],
      "inputs": {
        "assets": {},
        "params": {"text": "hello"},
        "environment": {}
      }
    }
  ]
}
```

The same case independently enters the live Pillow oracle and public Rust
target. Neither side receives the other side’s result.

### Coverage plan input

```json
{
  "schema": "migration-parity/coverage-input@1",
  "surface": "ImageFont",
  "operation": "getbbox",
  "plans": [
    {
      "plan_id": "ImageFont.getbbox.public_paths",
      "operation": "getbbox",
      "covers": ["text.basic_latin"],
      "selectors": {
        "parity_case_ids": ["ImageFont.getbbox.basic_latin"],
        "repository_test_ids": []
      },
      "execution": {
        "contexts": ["parity"],
        "features": [],
        "backends": ["cpu"]
      }
    }
  ]
}
```

This selects execution. It does not contain line counts, percentages, snapshots,
or exclusions. Repository test IDs may cover target internals but cannot prove
public parity.

### Benchmark workload input

```json
{
  "schema": "migration-parity/benchmark-input@1",
  "surface": "ImageFont",
  "operation": "getbbox",
  "workloads": [
    {
      "workload_id": "ImageFont.getbbox.standard_latin",
      "operation": "getbbox",
      "covers": ["performance.standard_latency"],
      "input": {
        "parity_case_id": "ImageFont.getbbox.basic_latin"
      },
      "execution": {
        "profile": "standard",
        "warmup_iterations": 20,
        "measurement_iterations": 100,
        "samples": 30,
        "concurrency": 1,
        "cache_state": "warm",
        "backends": ["cpu"]
      }
    }
  ]
}
```

Execution controls are specification. Timings, machine data, statistics, and
budget outcomes are results.

### Forbidden input content

Every input kind rejects:

```text
actual baseline error expect_error expectation expected golden hash oracle
output outputs pass passed pixels raw_path ref_bytes ref_path regression
result results sha256 snapshot status threshold_met timing timings
```

An invalid call or missing asset may be an input. The input must not say which
error or result should occur.

## Three result interfaces

Every result includes:

- schema and artifact kind;
- durable run identity and timestamps;
- manifest path/version/hash;
- exact indexed input path/hash list;
- immutable target revision/build/runtime identity;
- command/cwd identity;
- lane-specific environment identity;
- selected/executed/not-run counts;
- infrastructure failures separated from behavior/results.

### Parity result

`migration-parity/parity-result@1` contains:

- source identity;
- normalized source `PublicResult`;
- normalized target `PublicResult`;
- generic comparison outcome and structured diffs per case;
- selected, executed, passed, failed, and not-run counts;
- infrastructure failures separately.

Public result is exactly:

```json
{"case_id": "id", "status": "ok", "value": {}}
```

or:

```json
{
  "case_id": "id",
  "status": "error",
  "error": {
    "class": "ValueError",
    "kind": "invalid_argument",
    "message": "stable public message",
    "stage": "layout",
    "code": null
  }
}
```

A public error is behavior. Oracle startup, timeout, crash, malformed output, or
missing IDs is infrastructure failure.

### Coverage result

`migration-parity/coverage-result@1` contains:

- managed run and snapshot identity;
- collector name/version;
- plans and exact selected tests/cases;
- test execution status;
- instrumented target paths/modules;
- integer covered/total counts per function/line/branch/region dimension;
- uncovered item locations;
- threshold calculations and outcomes;
- artifact ingestion state and infrastructure failures.

Percentages are derived from integer counts. Coverage does not upgrade or
replace parity evidence.

### Benchmark result

`migration-parity/benchmark-result@1` contains:

- workload IDs and exact execution controls;
- OS, architecture, CPU, memory, power mode, toolchain, and stable non-secret
  machine identity;
- sample count and units;
- min/median/mean/p95/max/standard deviation where relevant;
- raw sample artifact reference;
- declared budget calculation and outcome;
- optional compatible baseline result ID/comparison;
- infrastructure failures.

Baseline comparisons require compatible manifest/input, workload, target
configuration, and machine/environment identity.

## Aggregate status and documentation

The aggregate report is a derived join, not another source of truth. It accepts
an artifact only when manifest hash, indexed input hashes, target revision, lane
schema, and relevant runtime/backend identity are compatible.

Missing, stale, incompatible, partial, cancelled, or failed evidence is
`not_proven`. It is never silently treated as current.

Generated documentation has two layers:

1. Specification reference from the manifest and indexed inputs:
   surfaces, signatures, support declarations, exclusions, requirements,
   cases/plans/workloads, coverage policy, benchmark budgets, and commands.
2. Current status from compatible aggregate evidence:
   parity failures, uncovered code, benchmark measurements/budgets, blockers,
   stale evidence, and run/snapshot IDs.

Every generated page states:

- generator/schema version;
- manifest path/version/hash;
- target revision where evidence appears;
- parity run, coverage run/snapshot, and benchmark run IDs;
- whether each statement is `declared`, `measured`, `not_proven`, or
  `stale/incompatible`.

Recommended outputs:

```text
docs/generated/api-support.md
docs/generated/parity-status.md
docs/generated/coverage-status.md
docs/generated/benchmark-status.md
docs/generated/status-report.json
```

CI either regenerates and diffs checked-in documentation or publishes it from
immutable artifacts. Documentation never writes results into specification.

## Completeness language

Bare “100%” is forbidden. Report the following independently:

1. inventory classification = represented/classified discovered public names /
   authoritative discovered names;
2. requirement specification = requirements with complete lane policy /
   declared requirements;
3. parity input mapping = parity requirements mapped to cases / parity
   requirements;
4. coverage input mapping = coverage requirements mapped to plans / coverage
   requirements;
5. benchmark input mapping = benchmark requirements mapped to workloads /
   benchmark requirements;
6. parity/coverage/benchmark lane readiness = active runnable operations /
   applicable in-scope endpoint operations for that lane;
7. parity outcome = passing comparisons / executed comparisons plus run ID;
8. code coverage = covered / total for each named dimension plus snapshot ID;
9. benchmark outcome = workloads meeting budgets / measured workloads plus run
   and machine identity;
10. documentation freshness = outputs generated from the current manifest and
    compatible evidence / declared outputs.

Name placeholders satisfy only the first dimension.

## Required rebuild

The current canonical manifest must be rebuilt rather than relabeled:

1. preserve the old root manifest as migration input until every field is
   mapped;
2. use the exact twelve public surface names, including `ImageFont`;
3. flatten every function, method, property, type, and intentional non-endpoint
   into a uniquely classified operation;
4. preserve signatures, modes, formats, parameter variants, edge cases,
   supported targets, Pillow version, exclusions, and reasons;
5. split classification, support, parity, coverage, and benchmark states;
6. define stable requirements for every applicable lane;
7. migrate the 445 Font cases under canonical `ImageFont` IDs and map them with
   `covers`;
8. create coverage plans and benchmark workloads instead of claiming those
   lanes from parity cases alone;
9. convert image backend and `image-slash-star` cases to input-only live Pillow
   calls;
10. generate three result artifacts, aggregate only compatible evidence, and
    generate documentation;
11. add CI gates for authoritative inventory drift, per-lane input bijection,
    runner registry drift, evidence compatibility, and docs drift;
12. remove old/deprecated material only after equivalent or better mapped
    specification and compatible evidence exist.

Until that rebuild is complete, the honest current statement is:

```text
Old-name inventory: accounted for.
Font parity input migration: concrete.
Project-wide scenario specification: incomplete.
Project-wide parity: not proven.
Project-wide code coverage: not proven.
Project-wide benchmark readiness/performance: not proven.
Generated documentation freshness: not implemented.
```
