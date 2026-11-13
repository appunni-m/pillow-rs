# Reusable parity framework

Status: reusable internal framework for projects that need repeatable behavior
parity or fault-contract verification. The PostgreSQL-backed implementation in
this repository is the first reference implementation. The model is designed
for cross-project use, while its schema and adapter API remain under active
development until validated with an unrelated project.

This guide defines the project-neutral model and adapter boundary. The current
Python/PostgreSQL reference implementation, its database lifecycle, and the
first project integration are documented in the
[PostgreSQL parity store guide](PARITY_STORE.md).

The framework's data model is reusable across languages and project types.
The checked-in implementation is currently a repository-local Python package
backed by PostgreSQL; it is not yet a separately versioned package for other
repositories to install. Python adapters can use the typed interfaces here;
other languages can keep thin project adapters and use this relational
contract through a project-owned bridge. Validate the design with a second,
unrelated project before extracting and versioning a shared package. This guide
does not claim a published cross-language wire protocol or replace any
project's existing parity result format.

## Purpose

Parity verification answers a project-specific question:

> For this frozen input, does this implementation produce the same public
> outcome as the chosen reference, or satisfy the recorded output or fault
> contract?

The framework makes the inputs, implementation identities, executions,
comparisons, and evidence reusable. It does not define what a project means by
equality, what an error means, which reference is authoritative, or which
operations a product promises to support. Those rules remain explicit and
versioned in the project adapter and its authored specification.

Use the same framework for libraries, command-line tools, services, compilers,
protocol implementations, data transformations, and runtime backends. A project
may compare a live reference against one or more candidates, verify a target
against recorded outputs, or exercise a fault and check the public failure
contract.

## Design rules

1. **The project owns behavior.** Project adapters execute the public calls,
   select inputs and profiles, and implement the declared comparator. Shared
   storage does not import or call project code.
2. **Authored inputs remain authoritative.** A checked-in manifest, corpus,
   generated-input recipe, or other versioned specification is reviewed and
   pinned by its project. The database is a rebuildable index and evidence
   ledger, not a second editing surface.
3. **Persist meaning as typed relations.** Store cases, dimensions, steps,
   implementation identities, observations, outcomes, and references in
   normalized tables. Do not use JSON or JSONB columns, JSON operators, or
   serialized documents as a substitute for the relational model.
4. **Store binary data once.** Large byte inputs and outputs live in
   content-addressed compressed blobs. Case and observation rows refer to the
   blob digest. The digest detects reuse or corruption; comparators still read
   and compare the bytes when the contract requires byte equality.
   Large rectangular numeric arrays use a canonical typed binary encoding and
   the same blob store; their element type and shape are relational metadata,
   and hydration reconstructs the original nested sequence.
5. **Stream bounded work.** Import and execute incrementally. Bound batches by
   case count and estimated payload size, and fetch only the selected batch and
   its referenced blobs.
6. **Record identity before reuse.** Reuse an observation only when all
   behavior-relevant source, target, runtime, configuration, specification,
   input, and adapter identities match.
7. **Keep infrastructure separate from product behavior.** A runner crash,
   database error, timeout in the harness, or invalid receipt is an
   infrastructure failure. It is not a passing result or a product parity
   mismatch.
8. **Never make the comparator weaker to make a campaign green.** Changes to
   normalization or equality are project contract changes and need explicit
   review and a new comparator identity.

## Shared concepts

Names below are conceptual; a project can use different public names in its
adapter. The storage implementation uses stable IDs and project-scoped
specification snapshots rather than project-specific database enums.

| Concept | Meaning |
| --- | --- |
| Project | The repository or product whose behavior is being verified. |
| Specification snapshot | An immutable digest of the reviewed case definitions, input indexes, and relevant generated-input recipe. |
| Operation | A public action, API method, protocol exchange, command, or transformation under verification. |
| Requirement | A project-owned behavior claim that one or more cases exercise. |
| Profile | A named candidate configuration, such as runtime, platform, backend, compiler flags, device, or architecture. |
| Case | A stable, input-driven scenario with dimensions, ordered actions, assets, and the observations to collect. |
| Implementation identity | A digest-backed description of a reference, candidate, or comparator build and its relevant runtime configuration. |
| Execution | One implementation's observed public outcome for one case under one identity. |
| Observation | A typed result or public error captured at a named point in a case. |
| Comparison | A comparator's decision and any typed differences for the selected case and profile. |
| Fault contract | An intentional fault input plus the expected public failure boundary and observable fields. |
| Artifact | A referenced input or output byte sequence, normally deduplicated by raw SHA-256. |
| Batch | A bounded, resumable set of cases executed as one persistence unit. |

Keep a case's normalized fields in their own relations. Use its typed `payload`
for compact adapter-specific metadata or original spellings needed to rebuild
the public case input. Do not copy large workflow arguments, expected values,
numeric arrays, or asset bytes into both the payload and their normalized
records.

Use arbitrary typed dimensions when selecting coverage, for example:
`operation`, `mode`, `format`, `size`, `platform`, `backend`, `architecture`,
`runtime`, `protocol-version`, `optimization-level`, or `device`. The database
must not require a schema migration just because one project adds a new
dimension.

## Verification contracts

The framework supports three common contract forms without requiring every
project to implement all three:

| Contract | Execution | How success is decided |
| --- | --- | --- |
| Reference parity | Run the same case through a pinned reference and candidate. | The project's comparator evaluates the paired public observations. |
| Recorded-output contract | Run the candidate and compare it to reviewed, typed expected observations. | The project's expected-output comparator evaluates the result. No reference execution is invented. |
| Fault contract | Inject a declared failure into the candidate and observe the public boundary. | The project's fault comparator checks the expected failure and stable metadata. |

The stored outcome vocabulary should distinguish at least `pass`, `fail`,
`not_run`, `not_applicable`, and `infrastructure_error`. Projects can refine
the vocabulary through a versioned result contract, but must not classify
unimplemented work as a behavior mismatch unless the project's declared
contract says that absence itself is a failure. A rejection is a mismatch only
when the public contract says the operation is possible and required for that
input; unsupported or not-yet-implemented behavior must remain identifiable.

For fault cases, keep the injected fault, observed public error, and comparison
decision as separate records. Do not turn an internal crash or collector
failure into evidence that the public error contract passed.

## Responsibilities and adapter boundary

The reusable storage package has no project execution hooks. Each project
provides the following components:

1. **Specification importer** validates the pinned source documents and emits
   typed project, operation, requirement, profile, case, asset, and expected
   observation records. It computes immutable source and case digests. It reads
   and writes incrementally.
2. **Case hydrator** loads a bounded batch from storage and reconstructs the
   project's in-memory input representation, resolving only referenced
   artifacts.
3. **Reference executor** runs the live reference when the case uses one. It
   records the executable identity and public observations.
4. **Candidate executor** runs one target profile using the project's actual
   public entry point. It does not substitute an internal helper for the
   public behavior being tested.
5. **Comparator** receives typed observations and the declared contract, then
   returns an outcome and typed difference records. It is versioned and its
   identity is included in the run.
6. **Reporter** aggregates persisted rows for requested dimensions. A report
   is derived evidence; it does not replace the case-level records.

An adapter may combine these responsibilities in one program, but their
boundaries must remain explicit. Shared code may provide typed data models,
blob encoding, migrations, batching, resumption, and SQL persistence. Shared
code must not contain project operation tables, image-mode rules, compiler
semantics, API-specific normalization, or project error mappings.

### Minimal integration API

The shared store deliberately stops at typed case and evidence records. A
project integration supplies a project-owned importer and two injected runtime
strategies:

```python
class ProjectExecutor:
    identity: ImplementationIdentity

    def execute_batch(self, cases: Sequence[CaseRecord]) -> Mapping[str, ExecutionRecord]:
        # Call this project's public oracle or target interface.
        ...


class ProjectComparator:
    identity: ImplementationIdentity

    def compare(
        self,
        case: CaseRecord,
        source: ExecutionRecord | None,
        target: ExecutionRecord,
    ) -> ComparisonDecision:
        # Apply this project's current result and fault-contract rules.
        ...
```

The executor and comparator own policy. The shared coordinator owns only
profile selection, bounded batches, identity-compatible execution reuse, and
typed persistence:

```python
store = PostgresStore.connect(dsn)
try:
    store.migrate()
    import_project_specification(store)  # project-owned importer
    summary = run_target_profile(
        store,
        project_id="example-project",
        specification_digest=specification_digest,
        profile_id="release-x86_64",
        source=reference_executor,  # omit for target-only contracts
        target=target_executor,
        comparator=project_comparator,
        host_description=host_identity,
        maximum_cases=128,
        maximum_estimated_bytes=32 * 1024 * 1024,
    )
finally:
    store.close()
```

`execute_batch` receives only a selected bounded group and returns exactly one
typed execution per case ID. The executor handles project-specific hydration
and can split that work into an internal hydrator and oracle/target clients.
The coordinator verifies the case set and every execution identity before
writing results. `ComparisonDecision` is the project's typed comparator
result; the coordinator binds it to persisted run and execution IDs. It does
not rewrite a project's external report. The current reference implementation
exposes these records from `scripts/parity_store`.

The project importer is equally narrow: it validates project-authored inputs,
registers project/operation/requirement/profile records, and calls
`insert_cases` in bounded transactions. It can read JSON, YAML, binary files,
database fixtures, or generated inputs. Source format is not stored as a
serialized case document in PostgreSQL; the importer maps each case to typed
rows and stores binary content by digest.

## Relational storage model

A relational implementation normally needs these record groups:

| Record group | Typical fields and links |
| --- | --- |
| Projects and specification snapshots | Project key; immutable spec digest; authoring schema/version; source document names and digests. |
| Operations and requirements | Stable operation IDs; project-owned requirement IDs; many-to-many links. |
| Profiles and support declarations | Target/profile IDs; typed profile attributes; optional operation support rows keyed by typed dimensions. |
| Cases | Stable case ID; operation; verification kind; source document and ordinal; case digest; estimated materialization size. |
| Case dimensions | Name plus a typed scalar value; indexed for incremental filtered selection. |
| Case actions and observations | Ordered action steps, named arguments, receiver/dependency links, observation selectors, optional expected values. |
| Artifacts | Content digest, raw byte count, compression identifier, compressed binary payload; case or observation references. |
| Packed numeric values | Numeric element type and shape, with the exact little-endian payload stored as a content-addressed blob. This is a storage encoding of a typed array, not a project-specific value convention. |
| Execution identities | Reference/candidate/comparator identity, revision or build digest, runtime, dirty state, typed configuration attributes. |
| Runs and batches | Frozen case-set digest and membership, selected reference/candidate/comparator identities, host/environment, batch boundaries and membership, state, timestamps, retry/error receipt. |
| Executions and observations | Identity, case and input digests, execution state, duration, typed values or public errors. |
| Comparisons and differences | Comparator identity, source/target execution references, outcome, typed differences, timestamps. |

Typed scalar values should preserve booleans separately from integers and
floating-point bit patterns separately from decimal formatting. Arrays preserve
order; objects use typed keys. Large rectangular numeric arrays are packed
without rounding or type coercion, while arbitrary arrays remain ordered child
values. Binary values are referenced by blob digest. Every reference is
checked by constraints or by an integrity query. Store project-authored
freeform documents outside the relational result model; import the fields
needed for execution into explicit typed rows.

Database rows should not embed executable code, shell fragments, SQL fragments,
or implicit project behavior. All queries use parameters. Migrations are
ordered, checksummed, and applied before import or execution. A project can
rebuild its database from pinned specification inputs and artifact sources.

## Execution, batching, and resumption

Before a run starts, freeze and record:

- the specification digest and selected case-set digest;
- the reference, candidate profile, comparator, runtime, and relevant config
  identities;
- the exact dimensions and case IDs selected;
- batch count and byte bounds;
- any allowed output normalization or tolerance, identified by comparator
  version.

An importer may reuse a previous validation or import only when the immutable
specification digest, importer/schema identity, and all referenced source
digests match. A path, file size, timestamp, or case ID alone is not enough to
prove an unchanged input snapshot. For a focused selection, an adapter may
load only the selected missing cases after that snapshot check; it must still
prove every requested ID is present and binds to the frozen snapshot.

For each batch:

1. Claim or create the batch record with the exact case IDs and identity set.
2. Load only those cases, expected values, and referenced artifact blobs.
3. Reuse a completed execution only if its identity digest and case digest
   match the frozen run exactly.
4. Execute the missing reference and candidate cases through the project
   adapters.
5. Compare complete observations in memory with the pinned comparator. Do not
   compare only hashes when the contract requires value or byte equality.
6. Commit each execution and comparison as a typed record. Mark the batch
   complete only after every required record is committed. Do not hold a
   database transaction open while an oracle or target process runs.
7. Release the batch payload before fetching the next one.

If a batch fails, its already committed executions remain available for a new
run when their implementation identity and case digest match. A retry either
reuses those records or reports a determinism conflict if the same immutable
identity produces different observations. Resume only compatible completed
work; never silently overwrite a completed observation with output from a
different build, runtime, profile, input, oracle, adapter, or comparator.

Batching must not change the execution contract. It must preserve each
case's ordered workflow, isolation requirements, materialization boundary,
timing boundary, and public error behavior. Database reads, compression, and
result writes stay outside operation timing unless the benchmark specifically
measures storage or orchestration.

## Reporting and auditability

Reports are queries over persisted case-level evidence. At minimum, a report
should include:

- run and specification IDs;
- comparator and implementation identities;
- selected, executed, passing, failing, pending, and infrastructure-failed
  counts;
- counts grouped by operation, requirement, verification contract, and
  project-declared dimensions;
- every failure's case ID, relevant observations, typed differences, and
  associated artifact references;
- every skipped, unsupported, or not-run case and its reason;
- a check that the selected case set has exactly one applicable result per
  target profile, with no missing, duplicate, or extra IDs.

Aggregates must never hide an individual operation/profile regression. A
passing percentage is useful for navigation, but it cannot stand in for
case-level outcomes.

For benchmarks, use a separate workload contract and result type. Record the
same input/build/profile identities, timing boundaries, warmup policy, sample
count, and environment. Keep timing out of ordinary parity comparisons unless
performance itself is a contract. Compare per-operation results and preserve
the reference and target measurements independently.

## Extending the model across projects

A new project should first map its concepts onto the shared model without
changing the shared schema. Add project-specific values as typed dimensions,
operation IDs, requirement IDs, profile attributes, action arguments,
expected observations, or artifact metadata.

Create a schema migration only when a relationship or integrity rule is
genuinely absent from the shared model. The migration must remain project
neutral and include a deterministic upgrade plus an integrity check. If the
need is project-specific, keep it in the project's source specification or
adapter instead of adding a project-named SQL column.

Adoption sequence:

1. Keep the existing public parity/fault result contract and authored case
   files as the project's authority.
2. Write a small importer that maps one real case from that project into the
   typed case model, including its input assets and observation selectors.
3. Run the same case through the existing oracle and public target, then map
   the outputs into `ExecutionRecord` values.
4. Implement the project's comparator as an injected strategy and return a
   `ComparisonDecision`; do not move normalization or error policy into SQL.
5. Validate one bounded batch and its persisted/reloaded records before
   importing the full corpus.
6. Add project-specific fields only to the authored specification or adapter.
   Propose a generic schema migration only when the missing concept applies to
   more than one project.

This keeps each project free to preserve its existing public report and test
commands while sharing the storage, asset, batching, identity, and recovery
mechanisms. A project can adopt storage without adopting another project's
case syntax, API names, error taxonomy, or comparison rules.

Examples of profiles and observations that use the same core model:

| Project kind | Example profile dimensions | Example public observation |
| --- | --- | --- |
| HTTP client or service | protocol version, platform, TLS mode, feature set | status, headers, body bytes, or public error |
| Compiler or interpreter | language version, optimization level, target ISA | output bytes, diagnostics, exit status, or exception |
| Image or media library | format, mode, backend, architecture, dimensions | pixels, metadata, encoded bytes, or public error |
| Database driver | server version, protocol, transaction mode | rows, metadata, transaction result, or public error |

The examples illustrate dimension flexibility; they do not require the shared
framework to understand those project semantics.

Before extracting this reference implementation for reuse, onboard an
unrelated project and review:

1. whether it can import and execute without changing project behavior;
2. whether its comparator and public fault contract remain fully project-owned;
3. whether its expected-output cases fit the same typed observation model;
4. whether bounded import, blob reuse, retry, and identity rules work without
   project-specific SQL;
5. which schema fields are truly universal and which should move to the
   adapter;
6. the cost of maintaining a shared package, schema migration policy, and
   compatible PostgreSQL versions.

Only after that review should the project team freeze a reusable package API,
version the framework contract, and define compatibility guarantees. Until
then, treat the schema and interfaces as a reference design and preserve each
project's existing public parity result contract.

## Current reference implementation

The first implementation is in `scripts/parity_store/`, backed by PostgreSQL
in Docker. The Pillow adapter imports the pinned fixture manifest, runs the
existing source and target executors, and persists bounded CPU batches. Details
about its current schema, local container, import mapping, known pilot issues,
and verification gates are in [PARITY_STORE.md](PARITY_STORE.md).
