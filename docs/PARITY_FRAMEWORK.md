# Cross-project parity input store

Status: reusable working contract for projects that need a durable, indexed
copy of parity inputs. It is intended for use across projects, languages,
runtimes, and domains. It is not yet a separately released package or a frozen
cross-language wire protocol. The `pillow-rs` integration is the first
reference implementation; validate the model against another project before
freezing a reusable package API.

This framework standardizes **test input storage**, not campaign execution or
result reporting. Each project keeps execution and comparison in its existing
parity tooling and stores run outputs as local files or CI artifacts.

## Purpose and boundary

Parity asks whether two public implementations produce the same outcome for a
fixed input, or whether a candidate meets a recorded output or fault contract.
Large or generated input corpora are expensive to rebuild and useful across
local runs and CI jobs. This store gives those inputs stable identities,
typed indexing, and reusable binary assets.

The database contains immutable specification snapshots, case inputs, declared
case dimensions, expected-output and fault-contract inputs, and referenced
assets. It does **not** contain campaign runs, implementation execution
results, comparisons, failures, benchmark measurements, or coverage results.
Those remain in the project's existing local result artifacts.

## Portable architecture

| Layer | Owns | Does not own |
| --- | --- | --- |
| Input producer | Reviewed authored inputs, deterministic generators, and version locks | Database access or run results |
| Project importer | Source validation, project-specific parsing, typed case construction, and asset resolution | Shared SQL or comparison rules |
| Input store | Immutable snapshots, typed case rows, content-addressed assets, indexed selection, and bounded import/hydration | Executing project code or storing run outcomes |
| Parity runner | Source and target calls, runtime identity, comparison, retries, and local reports | Mutating the input snapshot |

The data flow is:

```text
versioned specification and generated inputs
    -> project importer and asset resolver
    -> typed immutable case records and content-addressed blobs
    -> PostgreSQL input corpus
    -> bounded case selection and hydration
    -> existing parity runner and comparator
    -> local result file or CI artifact
```

The importer and runner are separate commands. Importing data never launches
an oracle or candidate. Running parity never writes observations or outcomes
back into the input store.

## Data model and invariants

1. **Authored inputs remain authoritative.** A checked-in manifest, corpus,
   generated-input recipe, or lock file defines the source of truth. The
   database is a durable index and reusable copy, not an editing surface.
2. **Snapshots are immutable.** A snapshot digest binds the schema, source
   documents, importer version, and generated-input recipe. Re-importing the
   same digest verifies existing case identities and reuses them. Changed input
   produces a new snapshot; it does not overwrite an older snapshot.
3. **Persist meaning as typed relations.** Store dimensions, ordered steps,
   named arguments, expected values, and asset references as typed relational
   data. Do not use JSON or JSONB columns, SQL JSON operators, or serialized
   workflow envelopes as the database model.
4. **Store binary inputs once.** Assets and large byte inputs use
   content-addressed blobs with SHA-256, raw length, compression codec, and
   compressed payload. References point to the digest. Verify the digest and
   decoded length when reading. A digest identifies and deduplicates bytes; it
   does not replace byte comparisons in the parity runner.
5. **Preserve value types.** Represent null, booleans, integers, floating-point
   bit patterns, text, bytes, arrays, objects, asset references, and step
   bindings explicitly. Array order and object keys must survive round trips.
   Large rectangular numeric arrays should use a typed binary encoding plus
   relational dtype and shape metadata.
6. **Bound memory while importing and reading.** Stream source documents and
   fetch only the selected case batch and referenced assets. Use batches of up
   to 10,000 cases, with a documented byte ceiling where payload size requires
   an earlier flush. The batch limit is a memory bound, not a campaign window.
7. **Keep results outside the store.** Do not insert execution identities,
   host descriptions, run membership, observations, comparisons, errors,
   timing, or coverage into PostgreSQL. Write those outputs using the project's
   current result format and preserve them as local or CI artifacts.
8. **Reject conflicting immutable data.** If a stored case ID or asset digest
   is presented with different content under the same snapshot, fail the
   import. Never silently replace inputs.

Project semantics stay in project-owned adapters. The shared schema does not
define equality, error meaning, image modes, compiler behavior, or which
operations a project promises to support.

## Shared concepts

| Concept | Meaning |
| --- | --- |
| Project | Repository or product that owns a parity input corpus. |
| Specification snapshot | Immutable digest of case definitions, source documents, and relevant generation rules. |
| Operation | Public action, API method, protocol exchange, command, or transformation named by the project. |
| Profile | Declared candidate configuration such as backend, architecture, platform, runtime, or device; it is input metadata, not a run identity. |
| Case | Stable input scenario containing dimensions, ordered actions, input values, and referenced assets. |
| Expected-output contract | Reviewed expected values stored as test input for a target-only case. |
| Fault contract | Declared fault injection and expected public failure, stored as test input. |
| Asset | Immutable binary input addressed by its content digest. |

Dimensions are project data, not database enums. Examples include `operation`,
`mode`, `format`, `size`, `platform`, `backend`, `architecture`,
`protocol-version`, `optimization-level`, and `device`. A new dimension must
not require a schema migration.

## Project integration

Each project supplies:

1. **Input producer:** checked-in source data or deterministic generators, plus
   a version lock that records recipe and output digests.
2. **Importer:** validates the exact source snapshot and streams typed case
   records and assets into the store. It does not run implementations.
3. **Hydrator:** reads selected cases in bounded batches and reconstructs the
   project's public-call input representation.
4. **Existing parity runner:** invokes the live reference and candidate, or
   checks recorded outputs and fault contracts. It owns process isolation and
   captures public results.
5. **Comparator and reporter:** apply the project's declared equality and
   error contracts, then write its existing report format outside PostgreSQL.

The storage boundary should be a narrow `InputCorpusStore` port containing
snapshot registration, typed case insertion, immutable identity checks,
selection, and bounded reads. Campaign coordinators must not receive this port
as a result sink. Database drivers and migrations remain behind the concrete
store adapter.

## Local, central, and CI use

For local development, Docker Compose runs PostgreSQL with a named persistent
volume bound to loopback. A database initialization target creates local
credentials, starts the service, and applies schema migrations. Stopping the
service retains the volume; removing the volume is an explicit destructive
operation and is not part of normal cleanup.

A central PostgreSQL endpoint can hold the same immutable corpus for shared
reuse when a project configures one. Keep credentials in environment or an
ignored local file. CI should restore the version-locked generated input cache
before importing; it should not regenerate an unchanged corpus or persist
campaign results into the database.

The database engine is replaceable. PostgreSQL is the reference implementation
because its relational constraints, binary payloads, and indexed selection fit
large reusable corpora. MySQL or another engine can implement the same input
store contract after it preserves the identity, immutability, type, blob, and
bounded-read rules above.
