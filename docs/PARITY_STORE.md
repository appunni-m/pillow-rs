# PostgreSQL parity store reference implementation

Status: implementation notes for the reusable parity framework's first
`pillow-rs` adapter. The store is being built and has not yet completed the CPU
campaign. The project-neutral concepts and cross-project adoption guidance are
in the [reusable parity framework guide](PARITY_FRAMEWORK.md). This document
records the PostgreSQL reference implementation and Pillow-specific integration;
it does not establish the final shared package API or freeze a cross-project
schema standard.

## Purpose and boundary

The parity store provides one reusable way to index executable cases, run
identities, source and target observations, comparison outcomes, and fault
contracts across projects. PostgreSQL stores typed relational data and
content-addressed binary blobs. It does not execute project code, interpret
operation semantics, select normalization rules, or decide whether two
project-specific values are equivalent.

Each project keeps its pinned manifest and human-authored inputs as the
specification authority. A project adapter validates those inputs and imports
them into the store. The database is a rebuildable execution index and evidence
ledger, not a second place to edit the specification. No manifest or workflow
JSON is stored in PostgreSQL; there are no JSON or JSONB columns or SQL JSON
operators.

The generic package owns schema migration, typed values, blob storage,
incremental case batches, run identity, resumability, and result persistence.
Project adapters own parsing their input format, resolving project-specific
operation names, calling each oracle and target, and applying the project's
declared comparator. The store must not import Pillow, image-slash-star, or any
other project implementation.

```text
versioned project specification and input files
    -> validated project importer
        -> generic relational catalog + content-addressed blobs
            -> bounded case batches
                -> project oracle and target adapters
                    -> project comparator
                        -> generic execution, observation, and outcome rows
```

The existing Pillow adapter transport may serialize one bounded batch between
isolated subprocesses. That transport is outside PostgreSQL; the store itself
does not persist or query serialized JSON.

## Generic data model

Every project-owned key is scoped by `project_id` and an immutable
`specification_digest`. IDs retain their public spelling. Profiles and case
dimensions are data, not database enums: a project may use `backend`,
`architecture`, `mode`, `format`, `platform`, `device`, `width`, `height`, or
other dimensions without changing the shared schema.

The versioned relational schema contains these entity groups:

| Group | Records and relationships |
| --- | --- |
| Specification | Projects, immutable specification digests, oracles, targets, target profiles, operations, requirements, and declared operation/profile support |
| Cases | Cases, typed dimensions, requirement links, ordered workflow steps, named arguments, observations, and asset references |
| Values | Typed scalar nodes and ordered array/object children; large rectangular numeric arrays keep dtype and shape in a typed relation and payload in a compressed blob |
| Fault contracts | Fault injection point and input, contract identity, and expected public outcome, linked to a case and target profile |
| Binary data | Content-addressed blobs with SHA-256, raw length, compression codec, and compressed `BYTEA` payload; assets and binary observations reference blobs |
| Runs | Campaign identity, immutable oracle/target/comparator identities, selected case IDs and digests, and per-batch state plus exact membership |
| Evidence | Source/target executions, ordered observations, comparator-bound outcomes, and typed difference records referencing their observed values |

Typed value kinds are `null`, `boolean`, `integer`, `float64`, `text`,
`bytes`, `array`, `object`, `asset-reference`, and `step-binding`. A float is
stored by its IEEE-754 bits so an import does not round through decimal text.
Arrays use child ordinals; objects use child keys. Large rectangular numeric
arrays use migration `0004`'s typed storage relation with a canonical binary
payload. The database round-trip reconstructs the same nested sequence,
including scalar type and order. This optimization is generic and does not
interpret image modes or operation semantics. Scalar columns are constrained
so a value kind cannot silently carry a value of another type.

Binary assets and observed byte/image payloads are stored once by their raw
SHA-256 digest. The client compresses the bytes before insertion and verifies
the digest and decompressed length on read. The digest is for identity,
deduplication, and corruption detection only. The project comparator compares
the actual bytes whenever the public contract requires byte equality; it must
not replace byte comparison with hash-only comparison.

Foreign keys, unique keys, and checks reject dangling references, duplicate
case IDs within a specification, duplicate step order, invalid value shapes,
unknown execution sides, and impossible outcome states. Case and profile
dimensions are indexed by project/specification and dimension name/value.
Blob digests, case IDs, operation IDs, requirement links, run identity, and
batch status have indexes for incremental selection and resumability.

## Adapter contract

A project integration supplies these boundaries. Hydration can be a separate
internal component of an executor when a project needs that separation:

1. **Importer:** validates the project's pinned specification and input
   documents, emits generic specification/case/value/asset records, and
   declares stable case dimensions. It reads one input document at a time and
   streams rows and blobs into a transaction. It records each source document
   digest and case ordinal for provenance.
2. **Executor:** hydrates one bounded batch and runs the selected workflows
   independently through the live source and each public target profile. A
   project may keep hydration as a separate internal component. The database
   never calls executables or constructs shell commands from case data.
3. **Comparator:** applies the project's existing declared result and error
   policy and returns a typed outcome plus typed differences. The shared store
   persists that decision without altering it.
4. **Reporter:** queries persisted case-level records and presents them using
   the project's existing report contract. The current coordinator returns
   per-run outcome totals; richer dimension reports remain project-owned
   queries over persisted records.

Projects may use different authoring formats and public APIs. Those details
belong in their adapters; the relational model and transaction lifecycle remain
the same. A project adapter version and all behavior-relevant source/target
identity fields are recorded with each run.

Parity cases use a live oracle result. Target-only expected-output cases store
their expected typed values as input contracts and have no oracle execution.
Fault-contract cases store the injected fault and expected public failure
separately; the target execution and contract outcome remain distinguishable
from ordinary source/target parity. Infrastructure failures are run failures,
not public target outcomes.

## Batching, reuse, and recovery

The importer verifies the complete specification digest and selected case-set
digest before execution. The runner uses keyset pagination over stable case IDs
and loads each bounded batch's typed rows and referenced blobs only when
needed. Batch limits apply to both a maximum case count and an estimated byte
budget, so one large workflow cannot inflate a whole-corpus in-memory envelope.

Each batch follows this lifecycle:

1. record the exact batch identity and selected case IDs;
2. load missing executions, then release the database transaction before
   calling project code;
3. execute missing source and target cases as bounded groups;
4. compare complete observations in memory using the project comparator;
5. commit each typed execution and comparison independently;
6. mark the batch complete after every required record is committed.

A crash can leave some case results committed and the batch incomplete. A new
run can reuse a source execution across target profiles only when the project,
specification and case digests, oracle identity, runtime, and source-adapter
identity all match. A target execution is reusable only when the immutable
target build, runtime, backend, architecture, features, profile attributes,
and case input digest match. A mismatch in any identity field makes the old
result stale; it is not silently reused.

Unique keys make result persistence idempotent. Database constraints and end-of-run
queries verify that each selected case has exactly one applicable outcome per
profile, with no missing, duplicate, or unselected IDs. The current campaign
summary reports per-run outcome totals. Dimension reports are project-owned
queries over persisted records; percentages are not stored as truth.

The first `pillow-rs` campaign is serial at the batch level. The schema permits
independent workers to claim batches in a later integration, but this campaign
does not add parallel scheduling. Database reads, writes, and compression
remain outside measured operation latency.

The current campaign coordinator handles reference parity, recorded-output,
and fault-contract outcomes. It is not a benchmark runner. Do not treat
`duration_ns` on an execution as benchmark evidence; workload selection,
warmups, samples, timing boundaries, and benchmark summaries remain with the
project's existing benchmark contract and runner.

## PostgreSQL and local lifecycle

Use the official PostgreSQL 18.7 Alpine image, pinned to
`postgres:18.7-alpine3.24`. Bind the published port to loopback, use a local
generated password file under ignored `build/`, and keep data in a named Docker
volume. Do not commit credentials, case payloads, blobs, or run results.

Use the repository commands for the local lifecycle:

```bash
make setup-parity-store
make parity-store-up
make migration-parity-test-postgres MIGRATION_PARITY_STORE_LIMIT=1000
make migration-parity-test-postgres
make parity-store-down
```

`parity-store-init` creates `build/parity-store/local.env` with a generated
password and mode `0600`; it preserves an existing file. The Compose service
binds to `127.0.0.1:55432`, waits for PostgreSQL health, and stores data in a
named volume. `parity-store-down` stops the service and retains that volume.
The parity command validates the pinned fixture inputs, imports cases in
bounded transactions, then runs the selected target profile in bounded
case/byte batches. The Pillow importer reads one input document at a time and
streams its cases into batches; it does not build the whole corpus in memory.
It incrementally decodes each case with Python's standard-library JSON
decoder. This preserves the fixture corpus's `NaN` and `Infinity` values while
keeping only the current case and bounded insert batch in memory. This parsing
choice belongs to the Pillow importer; the shared store accepts typed records
and does not mandate a project input format or parser. On first import, the
adapter validates the complete fixture snapshot before registering it. Future
runs hash the indexed source documents to prove the snapshot is unchanged; if
that exact validated snapshot is already registered, they reuse its validation
and do not decode the full corpus again. An explicit case selection searches
for missing IDs and parses only candidate documents, while a full import still
streams every case in bounded batches. This keeps validation and snapshot
identity explicit without making every profile replay re-parse all cases.
Use
`MIGRATION_PARITY_STORE_PROFILE=python-cpu` to select the serial CPU profile,
or pass a comma-separated list through `MIGRATION_PARITY_STORE_CASE_IDS` to
run selected cases. First import validates the complete indexed specification;
for a previously validated, byte-identical snapshot, selected IDs restrict
which missing case rows are read and persisted. `MIGRATION_PARITY_STORE_LIMIT`
imports the full indexed case set, then selects the first N stable case IDs
for a pilot.

Apply versioned SQL migrations transactionally. The migration ledger stores
the applied schema version and migration digest. On import or schema mismatch,
fail before running cases. Back up the store with PostgreSQL's native dump
tools. Stop or remove the container without removing the named volume when
evidence must be retained. Deleting the volume is destructive and is not part
of the normal stop or cleanup command.

Migration `0004` stores large rectangular numeric arrays as typed references to
compressed binary blobs. Migration `0005` binds the source implementation
identity to each run before execution starts.

The implementation uses parameterized SQL. Case IDs, paths, operation names,
and input values are data, never SQL fragments or shell syntax. The database
is local-only for this workflow; it does not expose a service or accept
untrusted remote cases.

## First integration: `pillow-rs`

The first adapter imports the pinned `pillow-rs/tests/fixtures/manifest.yaml`
and its indexed inputs. It maps `surface`, `operation`, `verification`,
requirements, profile selections, workflow steps, typed argument descriptors,
asset references, expected outputs, and fault contracts to the generic
relations. It stores decoded binary asset and image bytes in the shared blob
table rather than copying base64 text into database rows. Its payload relation
keeps only compact case and step metadata; workflow arguments, expected values,
and assets are stored once in their typed relations. Case digests bind the
source document digest, case ordinal, and referenced asset digests, so an edit
to an indexed document conservatively invalidates its cases. The specification
digest also includes the adapter schema version, so a change to the import
mapping cannot collide with rows produced by an older adapter.

The frozen CPU selection for this integration contains 177,881 case IDs with
SHA-256
`eee56654a8f3a0a77d6fe07da75b9a9152d1cebf4f6c6aaef3f3a9bbb505e2e3`, under
manifest digest
`aea5f40d617175d88bc18ab2515bac729e44b6c097024fc9b41724bb18ae6445`. The
single GPU-only fault case is excluded from the CPU selection and remains in
the generic specification catalog.

The requested CPU selector matrix is Default, NEON, SSE2, SSE3, SSSE3, AVX,
AVX2, AVX+FMA, and AVX2+FMA. On the current Apple Silicon host, x86 selectors
must be recorded as Default fallback where they are unavailable; reports must
not claim native execution. Actual backend and architecture receipts remain
separate from the requested profile.

Before the full matrix, the integration must resolve two observed pilot issues:

- The parity runner emits `architecture` in target identity, but the current
  result validator rejects that field.
- One recorded LAB direct-rotate expected-output case differs from the target.

These are separate: fix the identity contract validation without weakening
comparison, and triage the image mismatch against the declared output contract.
The pilot selected 1,000 cases: 971 live Pillow comparisons passed, 28 of 29
expected-output cases passed, and one expected-output case failed. This pilot
is diagnostic, not completion evidence.

After all frozen CPU parity rows pass, run the planned CPU benchmark workloads
and compare each operation/profile against its compatible baseline. The
database campaign must not change benchmark workloads, timing boundaries,
sample counts, or the documented 1–5% allowance. Only then proceed to Parallel
CPU.

## Evolution

The relational schema is versioned independently from each project's manifest
and public result contracts. A schema change requires a new ordered migration,
a deterministic upgrade path, an explicit compatibility window, and
validation against at least one project adapter. Do not add a JSON/JSONB escape
column or project-specific fields to avoid modeling a real typed relation.

This store makes storage, batching, identity, and recovery reusable. It does
not force every project to share a public API, oracle, comparator, fault
injection mechanism, or result semantics. Those remain explicit adapter
contracts.
