# Node/browser WASM parity gap manifest

This report distinguishes selected cases, completed comparisons, actual parity failures, and pending cases.

- Common scope: **10952 selected**
- Node: **10952 executed**, 10771 passed, 181 failed, 0 pending, 0 infrastructure errors
- Browser: **10952 executed**, 10774 passed, 178 failed, 0 pending, 0 infrastructure errors
- Node/browser semantic target result streams identical (opaque WASM pointers ignored): **False**

## What is actually pending

**0 cases**. Pending means a selected case has no completed target comparison (`summary.not_run`). A facade `NotImplementedError` is not pending: the case ran and failed parity.

## Failure categories

| Category | Cases | Meaning / next owner |
| --- | ---: | --- |
| `result_mismatch` | 64 | Completed target result differs; fix core behavior or metadata, keeping bindings thin. |
| `target_error` | 75 | Target reached a public error path whose class/message/result differs; fix boundary validation or core error parity. |
| `target_not_implemented` | 21 | JS/WASM facade or an underlying public operation is missing; add a thin export only when core already owns the behavior. |
| `workflow_dependency_not_run` | 21 | A later observation was blocked by an earlier target failure; fix the first target error, not the dependent observation. |

## Ordered operation failures

The complete case IDs are in the JSON manifest; each row below is ordered by failing-case count.

| Rank | Public operation | Cases |
| ---: | --- | ---: |
| 1 | `PIL.Image.Image.getpalette` | 14 |
| 2 | `PIL.ImagePalette.ImagePalette.getcolor` | 14 |
| 3 | `PIL.Image.open` | 13 |
| 4 | `PIL.Image.Image.paste` | 12 |
| 5 | `PIL.ImageOps.expand` | 11 |
| 6 | `PIL.Image.eval` | 9 |
| 7 | `PIL.ImageFilter.Color3DLUT` | 9 |
| 8 | `PIL.Image.fromarray` | 7 |
| 9 | `PIL.Image.Image.draft` | 7 |
| 10 | `PIL.Image.Image.info` | 6 |
| 11 | `PIL.ImageFilter.Color3DLUT.generate` | 6 |
| 12 | `PIL.Image.Image.filter` | 5 |
| 13 | `PIL.ImageOps.equalize` | 5 |
| 14 | `PIL.Image.Image.entropy` | 4 |
| 15 | `PIL.Image.Image.has_transparency_data` | 4 |
| 16 | `PIL.ImageSequence.Iterator` | 4 |
| 17 | `PIL.Image.merge` | 3 |
| 18 | `PIL.Image.Image.convert` | 3 |
| 19 | `PIL.Image.Image.getchannel` | 3 |
| 20 | `PIL.ImagePalette.ImagePalette` | 3 |
| 21 | `PIL.Image.Image.frombytes` | 2 |
| 22 | `PIL.Image.Image.get_flattened_data` | 2 |
| 23 | `PIL.Image.Image.getxmp` | 2 |
| 24 | `PIL.Image.Image.transpose` | 2 |
| 25 | `PIL.ImageDraw.Draw` | 2 |
| 26 | `PIL.ImageFilter.Kernel` | 2 |
| 27 | `PIL.ImageOps.deform` | 2 |
| 28 | `PIL.ImageOps.fit` | 2 |
| 29 | `PIL.ImageSequence.Iterator.__next__` | 2 |
| 30 | `PIL.Image.effect_mandelbrot` | 1 |
| 31 | `PIL.Image.frombuffer` | 1 |
| 32 | `PIL.Image.frombytes` | 1 |
| 33 | `PIL.Image.Image.apply_transparency` | 1 |
| 34 | `PIL.Image.Image.getim` | 1 |
| 35 | `PIL.Image.Image.tobytes` | 1 |
| 36 | `PIL.Image.Image.toqimage` | 1 |
| 37 | `PIL.Image.Image.toqpixmap` | 1 |
| 38 | `PIL.ImageDraw.Outline` | 1 |
| 39 | `PIL.ImageDraw.ImageDraw.getfont` | 1 |
| 40 | `PIL.ImageEnhance.Sharpness` | 1 |
| 41 | `PIL.ImageEnhance.Sharpness.enhance` | 1 |
| 42 | `PIL.ImageFilter.BoxBlur` | 1 |
| 43 | `PIL.ImageFilter.Color3DLUT.__repr__` | 1 |
| 44 | `PIL.ImageOps.pad` | 1 |
| 45 | `PIL.ImagePalette.ImagePalette.copy` | 1 |
| 46 | `PIL.ImagePalette.ImagePalette.getdata` | 1 |
| 47 | `PIL.ImagePalette.ImagePalette.save` | 1 |
| 48 | `PIL.ImagePalette.ImagePalette.tobytes` | 1 |
| 49 | `PIL.ImageSequence.Iterator.__iter__` | 1 |
| 50 | `PIL.ImageStat.Stat` | 1 |

## Interpretation

Node and browser use the same manifest, workflow payload, and JS adapter. They therefore should have the same selected and executed denominators. Opaque WASM handle pointers are host-process addresses and are intentionally ignored when checking semantic target equivalence. A capability difference belongs in the separate WebGPU/WGSL lane; it must not silently remove public parity cases.

The JSON `groups` array is the actionable backlog. Use `case_ids` for incremental runs and preserve the Python oracle as the behavioral authority. `target_errors` lists the most frequent target error signatures.
