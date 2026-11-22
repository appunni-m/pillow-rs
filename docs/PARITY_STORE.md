# PostgreSQL parity input store

Status: first project adapter for the reusable input-store contract in
[the parity framework guide](PARITY_FRAMEWORK.md). PostgreSQL stores immutable
test inputs and referenced assets. The normal parity runner executes cases and
writes its result artifacts locally; it does not record run results in
PostgreSQL.

## Boundary

The pinned manifest, version-locked generated corpus, and binary fixtures are
the source of truth. PostgreSQL provides a durable, indexed copy of typed
inputs so a large corpus can be reused and selected incrementally. It is not a
campaign ledger or an editing surface.

The active importer writes only project/specification metadata, operation and
profile declarations, typed case inputs, expected-output/fault-contract inputs,
and referenced binary assets. It does not execute Pillow or pillow-rs. It does
not write run identities, batch state, observations, comparisons, errors,
benchmark measurements, or coverage results. The existing parity commands
continue to produce their local result files and CI artifacts.

The current migration history contains result-table schema from an earlier
campaign-store design, and an existing database may contain historical rows.
Those tables are not used by the input import command. Initialization applies
forward-only migrations and preserves existing logical row values and the
Docker volume; it does not drop historical result tables or delete run rows.
Pending migration `0006` converts typed text columns to lossless UTF-8 byte
storage, changing their physical representation while preserving their
contents.

## Versioned generated inputs

The generated parity, coverage, benchmark, and binary-asset corpus is too large
to commit as source files. Git tracks
`pillow-rs/tests/fixtures/input-corpus.lock` instead. The lock binds the corpus
to its generator, helper modules, manifest, source assets, and pinned Python
requirements, and records the corpus tree digest and path/size inventory.
Changing the generator or authored inputs requires regenerating the corpus and
deliberately updating this lock.

GitHub Actions keys its shared cache from that lock. A preparation job restores
the exact input snapshot and generates it on a cache miss. Other parity and
benchmark jobs restore and verify the same snapshot. There are no fallback
cache keys, so stale inputs cannot satisfy a changed lock.

Generated JSON documents are deterministic `.json.gz` files to bound the CI
cache and local disk footprint. Manifest paths remain logical `.json` names;
fixture readers stream compressed or raw files, and the lock hashes logical
uncompressed content. PostgreSQL does not store these documents as JSON/JSONB.
The importer maps them into typed relations. Binary assets are stored once as
compressed content-addressed blobs.

Use `make migration-parity-inputs-check` for the full corpus digest check,
`make migration-parity-inputs-cache-check` after a cache restore, and
`make migration-parity-inputs-regenerate-check` when explicitly auditing
generator determinism. A generated corpus change requires
`make migration-parity-inputs-lock-update` and review of the lock diff.

## Current runner wiring

`make migration-parity-test` can load an explicit case-ID selection from a
verified PostgreSQL snapshot instead of scanning the generated JSON documents.
Set `MIGRATION_PARITY_INPUT_STORE=postgres`, provide the immutable
`MIGRATION_PARITY_INPUT_SPECIFICATION_DIGEST`, and pass IDs with
`MIGRATION_PARITY_CASE_ID_FILE` or `MIGRATION_PARITY_CASE_IDS`. Each invocation
is limited to 10,000 cases. The loader verifies the manifest digest and full
import marker, then hydrates and executes selected cases in bounded batches of
at most 500 cases and 16 MiB of estimated input data. An individual case above
that byte budget runs alone. The runner streams comparisons into one normal
parity result and records the selected source-document digests. Pillow and
pillow-rs still run through the existing isolated public adapters; parity
results remain local artifacts and are not written to PostgreSQL.

PostgreSQL may contain multiple immutable snapshots for the same manifest, so
the runner requires an explicit specification digest. This is also how a retry
can use the exact input snapshot from an earlier failure even after the
generated corpus has advanced. A missing case, stale manifest, incomplete
snapshot, or unknown digest fails before either adapter starts.

TODO (`pillow-rs#bhwe`): stream a complete profile through bounded store batches
for campaigns larger than 10,000 cases. The current runner materializes only
the requested selection; full campaigns continue to use the versioned fixture
reader. A `parity_blob` row represents one unique SHA-256 payload; case asset
rows reference that digest, so identical bytes across cases/imports reuse the
stored payload instead of creating copies. A count such as 94,874 blob rows is
a count of distinct payloads, not the number of case references.

## Stored input model

Every project key is scoped by `project_id` and an immutable
`specification_digest`. The digest binds the manifest, input-document digests,
and importer schema. A changed source creates a new snapshot; an existing
snapshot is not edited in place. Re-importing an identical snapshot verifies
and reuses stored cases.

| Input data | Representation |
| --- | --- |
| Specification | Project and immutable snapshot identities, source-document paths/schema IDs/digests, operations, requirements, declared profiles, and support dimensions |
| Cases | Stable case ID and digest, source document and ordinal, typed dimensions, requirement links, ordered steps, named arguments, and selected expected observations/contracts |
| Values | Typed scalar nodes and ordered array/object children; reversible UTF-8 `surrogatepass` for text; IEEE-754 bits for floats |
| Numeric arrays | Dtype and shape in typed relations, with canonical binary data in the blob store |
| Binary inputs/assets | SHA-256 content identity, raw byte length, compression codec, and compressed `BYTEA` payload; cases reference the digest |

No authored case documents, manifests, or workflow envelopes are stored in
JSON/JSONB columns. Case dimensions remain typed values rather than database
enums, so adding a project-specific dimension such as `mode`, `architecture`,
`operation`, `format`, or `size` does not require a schema migration.

Blob digests provide identity, deduplication, and corruption detection. They do
not replace comparing actual bytes when the parity contract requires exact byte
equality. Importing a case ID or asset digest with different content under the
same immutable snapshot fails rather than overwriting data.

## Incremental import

The Pillow importer validates the pinned manifest and generated inputs, then
streams one source document at a time. It decodes each JSON case incrementally,
preserving `NaN` and `Infinity`, and holds only the current case plus the
bounded insert batch. Each batch contains at most 10,000 cases and is also
bounded by an estimated payload-size ceiling. This is import batching only;
there are no database campaign windows.

The importer records source-document digests, case ordinals, and a completion
marker for a full snapshot import. On repeat import, it verifies that the
current source digests and stored case coverage still match. An unchanged full
snapshot can therefore be reused without decoding every case again. A partial
`--case-id` import stores only the selected inputs and does not claim that the
full corpus has been imported.

The import command never calls a parity adapter. To compare behavior, use the
existing local parity command with the frozen case IDs and preserve its output
artifact locally or as a CI artifact. Retrying failed cases is a selection on
that local artifact/case-ID list; PostgreSQL does not store which cases failed.

## Local Docker lifecycle

Use PostgreSQL 18.7 Alpine pinned to `postgres:18.7-alpine3.24`. Compose binds
port `127.0.0.1:55432`, creates a named volume, and waits for the database health
check. Local credentials are generated under ignored
`build/parity-store/local.env` with file mode `0600` and are not overwritten.

```bash
make migration-parity-inputs-postgres
make migration-parity-test
make parity-store-down
```

`make parity-db-init` installs the optional PostgreSQL driver and applies
pending schema migrations. With no `PARITY_STORE_DSN`, it also creates local
credentials if needed and starts Docker Compose; with a DSN, it initializes
that server without starting a local container. The local path preserves
existing tables, rows, and the named volume. The input importer then inserts or
verifies the immutable corpus; it does not run parity. The normal
`migration-parity-test` command writes results to its configured local
artifact. `make parity-store-down` stops the local service without removing its
volume. Removing the named volume is destructive and is not part of routine
cleanup.

The `migration-parity-inputs-postgres` Make target depends on
`parity-db-init`, so it can be used directly. Run `make parity-db-init` by
itself when you want to initialize or start the database without importing the
corpus yet.

For local development, omit a central DSN and use the Docker defaults. A
configured `PARITY_STORE_DSN` can point the same importer at a shared PostgreSQL
service. Keep credentials in environment or an ignored local file; never put
them or test-result artifacts in the input database.

The DB volume persists through container recreation. Back up long-lived input
corpora with PostgreSQL's native dump tools and keep the version lock alongside
the project. The checked-in lock and generated-input cache remain the authority
for reproducing exact test data.
