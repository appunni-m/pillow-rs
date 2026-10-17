# Execution identity capture

Parity and benchmark runners accept `--attestation-output PATH` to retain
execution identity in a separate sidecar. Use a new path for each run. Existing
artifacts are preserved, and the public `@1` result schemas and child envelopes
retain their current fields.

Keep execution scoped: select affected parity cases with repeated `--case-id`
arguments and benchmark workloads with repeated `--workload-id` arguments.
Freeze the profiles and necessary guards before running them. See
[benchmarking](BENCHMARKING.md) for the workload and timing contracts.

## Producer structure

| Module | Responsibility |
| --- | --- |
| `migration_execution_identity.py` | Capture already loaded packages, interpreter, host, selected extension and backend configuration; validate the fixed fingerprint schema. |
| `migration_execution_attestation.py` | Allocate unique child receipt paths, collect receipts, compare identities and membership, and bind the sidecar to the closed report bytes. |
| `migration_producer_identity.py` | Freeze producer source files, entrypoints and loaded `sitecustomize`; identify already loaded producer modules against that inventory. |
| `migration_producer_attestation.py` | Retain separate parent and child producer companions bound to the report and execution receipt bytes. |
| `migration_build_witness.py` | Capture a release Cargo command, source snapshots, resolved features and the emitted extension hash. |
| `run_migration_parity.py` | Request each child receipt and capture after public calls and existing execution receipts finish. |
| `run_migration_benchmark.py` | Retain receipts for every timed child and separate target execution-proof child, including fresh cold processes. |

Capture runs outside image timers. The helpers import no image APIs and do not
select backends, perform pixel work, or materialize images. Optional capture
failures preserve raw public results and latency samples; a missing or invalid
receipt makes the sidecar `not_proven`.

The stable fingerprint covers the interpreter executable, runtime, host,
installation files, profile, features and selected core. Invocation-local
loaded-module lists remain separate because different cases can import different
modules. File-backed modules must belong to the captured implementation.
Target `PIL.*` aliases can resolve to either facade files or `pillow_rs` files;
the top-level `PIL` package remains bound to its facade installation.

On Linux, capture checks that the selected extension's device and inode match
its process mapping. Other platforms report that mapping check as unavailable.
These are selected on-disk file identities, not hashes of loaded memory. Package
enumeration is not an atomic snapshot of Python code already executing.

The report's canonical SHA-256 covers its complete original JSON bytes. For
gzip reports, those are the decompressed original bytes; the gzip container hash
and length are retained separately. The report writer closes before hashing.

## Admission remains separate

`complete` describes collected identity receipts. It does not establish exact
parity, actual vector/GPU execution, or an accepted performance target. The
producer validator compares a projection supplied by the report writer. The
separate `pillow-perfdb` consumer derives that projection from the retained raw
report and validates exact role contexts and process multiplicity independently.

The consumer matches exact workflow assets, steps and observations to a
passing parity case and checks requirement ownership against the original
manifest. Its `workflow-parity-link@2` route additionally validates producer
companions, the build witness and actual backend receipts before admitting a
workflow row to ranking or comparison. The raw benchmark retains its original
`successful_execution` gate. A legacy `@1` link remains diagnostic.

A parity-proof reference's shape is checked here; linked artifacts are
dereferenced by the separate consumer. Conditional image-module requirements
at observed error boundaries still need their own scoped capture evidence.
Historical reports cannot acquire missing child identity from today's files.

## Producer and build companions

Add `--producer-attestation-output PATH` alongside `--attestation-output PATH`
for a new parity or benchmark run. The producer collector writes a frozen
bundle, one receipt for each planned child, and the report-bound companion.
Parent entrypoints can differ between benchmark and parity; their shared
source inventory, child entrypoint and extras must match. Imports can grow
between parent captures, but existing producer module identities cannot change
or disappear. These are on-disk identities, with the same loaded-memory limits
as the runtime receipts.

Capture the native build with `migration_build_witness.py --root ROOT --output
NEW_PATH -- COMMAND`. The command must build the binding manifest with
`--release`, `--locked` and JSON Cargo messages. Use `--set-env KEY=VALUE` and
`--unset-env KEY` for explicit build environment changes. Completion requires
unchanged before/after source snapshots, successful Cargo completion, actual
feature lists and fresh compilation of both core and binding. Cached
dependencies are allowed. Every linked target child must load extension bytes
matching the witness artifact; packaging may change the path.

Workspace Cargo events are identified by their exact manifest path. Current
path package IDs can contain only a version after `#`, so a package-name parser
alone can incorrectly reject a successful build. For that specific capture
defect, `repair_build_witness(original, repaired, diagnostic)` can reinterpret
the unchanged retained Cargo log without rebuilding. It checks the original
log hash and source snapshot, writes new exclusive outputs, and preserves the
failed receipt. The diagnostic records the repair producer and states that the
extension hash was read at repair time from Cargo's emitted artifact path.

The witness records observed files, commands and emitted artifacts. It does
not sign them or establish compiler memory identity. Capture failure preserves
the raw image result and leaves provenance unproven. Admission establishes
evidence eligibility; each operation's latency and throughput targets remain
separate requirements.

## Focused implementation evidence

The 2026-10-14 foundation checkpoint passed 48 distinct synthetic contracts in
49 executions: 45 initially, two review fixes, then one alias contract and its
affected external-module rejection guard. No full test campaign was repeated.

One unchanged RGB convolution guard ran once against Pillow and CPU on the
native runner. Pillow capture passed; target capture exposed the legitimate
facade-alias defect. Only the failed target capture was repeated after its
repair. The selected core matched the existing extension and Linux mapping;
public observations matched the retained source result. Different helper
revisions prevent admitting that diagnostic as a current strict parity proof.
There were no timing samples, image builds, coverage runs or operation
completion credits.

Durable evidence is retained in the separate `pillow-perfdb` project under
`evidence/execution-attestation-foundation-20261014/`. Its notes include exact
plans, producer hashes, contract logs, child receipts and runner preservation
limits. Where a runner prohibits deletion, enable retained temporary namespaces
before launching children: normal `TemporaryDirectory` cleanup removes newly
created directories too.
