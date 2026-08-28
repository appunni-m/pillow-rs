# Node/browser WASM parity gap manifest

This report distinguishes selected cases, completed comparisons, actual parity failures, and pending cases.

- Common scope: **10952 selected**
- Node: **10952 executed**, 10850 passed, 102 failed, 0 pending, 0 infrastructure errors
- Browser: **10952 executed**, 10850 passed, 102 failed, 0 pending, 0 infrastructure errors
- Node/browser semantic target result streams identical (opaque WASM pointers ignored): **True**

## What is actually pending

**0 cases**. Pending means a selected case has no completed target comparison (`summary.not_run`). A facade `NotImplementedError` is not pending: the case ran and failed parity.

## Failure categories

| Category | Cases | Meaning / next owner |
| --- | ---: | --- |
| `result_mismatch` | 0 | Completed target result differs; fix core behavior or metadata, keeping bindings thin. |
| `target_error` | 1 | Target reached a public error path whose class/message/result differs; fix boundary validation or core error parity. |
| `target_not_implemented` | 0 | JS/WASM facade or an underlying public operation is missing; add a thin export only when core already owns the behavior. |
| `workflow_dependency_not_run` | 101 | A later observation was blocked by an earlier target failure; fix the first target error, not the dependent observation. |

## Ordered operation failures

The complete case IDs are in the JSON manifest; each row below is ordered by failing-case count.

| Rank | Public operation | Cases |
| ---: | --- | ---: |
| 1 | `PIL.Image.Image.filter` | 100 |
| 2 | `PIL.Image.open` | 1 |
| 3 | `PIL.ImageFilter.Kernel` | 1 |

## Interpretation

Node and browser use the same manifest, workflow payload, and JS adapter. They therefore should have the same selected and executed denominators. Opaque WASM handle pointers are host-process addresses and are intentionally ignored when checking semantic target equivalence. A capability difference belongs in the separate WebGPU/WGSL lane; it must not silently remove public parity cases.

The JSON `groups` array is the actionable backlog. Use `case_ids` for incremental runs and preserve the Python oracle as the behavioral authority. `target_errors` lists the most frequent target error signatures.
