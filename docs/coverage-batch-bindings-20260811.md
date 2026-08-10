# Binding coverage batch proposal

Status: audit complete; no new input retained.

## Scope and managed profile

- Worktree: `coverage-batch-bindings-20260811`
- Branch: `codex/coverage-batch-bindings-20260811`
- CPU scope: **23,429 / 26,611 lines (88.0425%)**
- Direct baseline snapshot: `f14bdccb-debc-4c65-826d-36277c72d319`
- Baseline revision: `bec899a5e38902642ca6347bd13e347ef28686bc`
- Managed worktree id: `24b16f92-7952-43c8-9d8e-f23cb5592e57`

The fixed manifest has explicit managed coverage components for the Python
facade and Rust core paths, but none for `pillow-rs-py/src/lib.rs` or
`pillow-rs-js/src/lib.rs`. The Python binding source is present in the LLVM
snapshot as an instrumented file, not as a declared component. On the direct
baseline it reports 3,113 / 3,647 lines, 281 / 350 branches, 400 / 502
functions, and 4,653 / 5,907 regions. Those figures are observations and must
not be presented as a managed binding-component denominator.

There is no managed JS/WASM profile. `pillow-rs-js/src/lib.rs` is absent from
the baseline snapshot, and `make coverage-wasm` explicitly reports that no
WASM coverage profile is declared. This audit makes no JS/WASM coverage claim.

## Public manifest paths

| Public path | Declared input family | Existing evidence | Binding conclusion |
| --- | --- | --- | --- |
| `PIL.Image.open` | bytes, path, stream | path/bytes cases exist | Path branch is covered; the stream branch at `pillow-rs-py/src/lib.rs:688-691` remains uncovered. |
| `PIL.Image.fromarray` | array-interface record | buffer-backed and dtype/shape cases exist | Existing public cases exercise the array-interface conversion; no direct binding probe is needed. |
| `PIL.Image.frombytes` | bytes and record | existing cases | `pillow-rs-py/src/lib.rs:1470-1479` is covered. |
| `PIL.Image.frombuffer` | bytes and array-interface record | existing cases | The manifest-supported public path is already represented. |
| `PIL.Image.Image.save` | bytes, path, stream | `...save.nuanced.in-memory-stream` exists | The public stream path is already covered, including the binding write branch. |
| `PIL.Image.Image.tobytes` | public image call | existing cases | The Python facade routes this call to `tobytes_encoded`; the separate Rust binding helpers at `pillow-rs-py/src/lib.rs:925-935` are not reached by a valid public manifest workflow. |

## Remaining binding gaps

The direct baseline identifies these relevant Rust binding gaps:

- `Image.open` stream read: lines 688-691. The existing generator asset
  `in-memory-byte-stream` is an empty `BytesIO`; a trial public case reached
  both implementations but failed exact parity because Pillow emitted
  `cannot identify image file <_io.BytesIO object ...>` while the target emitted
  `cannot identify image file '<_io.BytesIO object ...>'`. The manifest requires
  exact error messages, so the case was removed and is not coverage evidence.
- `Image.open` invalid-format `unreachable!`: line 675. Input validation makes
  this branch unreachable through the public call contract.
- `PyImage::tobytes` and `tobytes_formatted`: lines 925-935. The supported
  public `Image.tobytes` workflow reaches the encoded helper instead; a direct
  `_core` call would be an unsupported probe.
- `align_row_to_32`: lines 2000-2002. No manifest-supported public workflow was
  identified for this binding utility, so it is not assigned a synthetic case.

## Candidate inputs for a future reviewed batch

1. Add a seeded stream asset only if the existing input runner gains a reviewed
   builtin that produces a valid encoded `BytesIO`; then use the manifest
   `PIL.Image.open.parameter.fp` workflow and require exact oracle/target parity.
2. Keep the existing bytes, array-interface, and save-stream cases as the valid
   binding-path coverage set. They already use public manifest targets and do
   not require wrapper or runtime changes.
3. Do not add private `_core` calls, JS/WASM claims, direct utility probes, or
   Python-wrapper logic to compensate for the gaps above.

## Verification record

- `make migration-parity-inputs` — passed; after rejecting the stream case,
  generated inputs returned to the baseline with no fixture diff.
- `make build-dev` — passed.
- Managed parity command:
  `make migration-parity-case CASE_ID=PIL.Image.open.nuanced.in-memory-stream`
  — rejected: exact error-message mismatch described above; no case committed.
- `make migration-parity-fixtures-check` — inventory and input reproduction
  ran, but the repository baseline currently has fixed-manifest drift and the
  legacy v0 duplicate-accounting test sees no checked-in legacy fixture files.
- `make migration-parity-evidence-check` — the current generated denominator is
  3,090 while the stale test expectation is 3,059.
- `make repo-map-check` — the generated map is missing six pre-existing
  `run_migration_*_native_cases.py` entries; no unrelated map refresh was
  included in this proposal.
- `make coverage-wasm` — intentionally reports no declared WASM profile.

No new managed coverage snapshot was created because the only candidate input
was not exact-parity safe. Snapshot `f14bdccb-debc-4c65-826d-36277c72d319`
remains the authoritative CPU comparison baseline.
