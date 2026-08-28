# Node/browser WASM parity gap manifest

This report distinguishes selected cases, completed comparisons, actual parity failures, and pending cases.

- Common scope: **10952 selected**
- Node: **10952 executed**, 10881 passed, 71 failed, 0 pending, 0 infrastructure errors
- Browser: **10952 executed**, 10881 passed, 71 failed, 0 pending, 0 infrastructure errors
- Node/browser semantic target result streams identical (opaque WASM pointers ignored): **True**

## What is actually pending

**0 cases**. Pending means a selected case has no completed target comparison (`summary.not_run`). A facade `NotImplementedError` is not pending: the case ran and failed parity.

## Failure categories

| Category | Cases | Meaning / next owner |
| --- | ---: | --- |
| `result_mismatch` | 10 | Completed target result differs; fix core behavior or metadata, keeping bindings thin. |
| `target_error` | 47 | Target reached a public error path whose class/message/result differs; fix boundary validation or core error parity. |
| `target_not_implemented` | 11 | JS/WASM facade or an underlying public operation is missing; add a thin export only when core already owns the behavior. |
| `workflow_dependency_not_run` | 3 | A later observation was blocked by an earlier target failure; fix the first target error, not the dependent observation. |

## Ordered operation failures

The complete case IDs are in the JSON manifest; each row below is ordered by failing-case count.

| Rank | Public operation | Cases |
| ---: | --- | ---: |
| 1 | `PIL.Image.open` | 13 |
| 2 | `PIL.Image.Image.paste` | 12 |
| 3 | `PIL.Image.Image.info` | 6 |
| 4 | `PIL.ImageSequence.Iterator` | 4 |
| 5 | `PIL.Image.merge` | 3 |
| 6 | `PIL.Image.Image.filter` | 2 |
| 7 | `PIL.Image.Image.frombytes` | 2 |
| 8 | `PIL.Image.Image.getxmp` | 2 |
| 9 | `PIL.Image.Image.transpose` | 2 |
| 10 | `PIL.ImageDraw.Draw` | 2 |
| 11 | `PIL.ImageFilter.Kernel` | 2 |
| 12 | `PIL.ImageOps.deform` | 2 |
| 13 | `PIL.ImageOps.fit` | 2 |
| 14 | `PIL.ImageSequence.Iterator.__next__` | 2 |
| 15 | `PIL.Image.effect_mandelbrot` | 1 |
| 16 | `PIL.Image.frombuffer` | 1 |
| 17 | `PIL.Image.frombytes` | 1 |
| 18 | `PIL.Image.Image.convert` | 1 |
| 19 | `PIL.Image.Image.getim` | 1 |
| 20 | `PIL.Image.Image.tobytes` | 1 |
| 21 | `PIL.Image.Image.toqimage` | 1 |
| 22 | `PIL.Image.Image.toqpixmap` | 1 |
| 23 | `PIL.ImageDraw.Outline` | 1 |
| 24 | `PIL.ImageDraw.ImageDraw.getfont` | 1 |
| 25 | `PIL.ImageEnhance.Sharpness` | 1 |
| 26 | `PIL.ImageEnhance.Sharpness.enhance` | 1 |
| 27 | `PIL.ImageFilter.BoxBlur` | 1 |
| 28 | `PIL.ImageOps.pad` | 1 |
| 29 | `PIL.ImageSequence.Iterator.__iter__` | 1 |

## Interpretation

Node and browser use the same manifest, workflow payload, and JS adapter. They therefore should have the same selected and executed denominators. Opaque WASM handle pointers are host-process addresses and are intentionally ignored when checking semantic target equivalence. A capability difference belongs in the separate WebGPU/WGSL lane; it must not silently remove public parity cases.

The JSON `groups` array is the actionable backlog. Use `case_ids` for incremental runs and preserve the Python oracle as the behavioral authority. `target_errors` lists the most frequent target error signatures.
