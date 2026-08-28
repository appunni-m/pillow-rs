# Node/browser WASM parity gap manifest

This report distinguishes selected cases, completed comparisons, actual parity failures, and pending cases.

- Common scope: **10952 selected**
- Node: **10952 executed**, 10657 passed, 295 failed, 0 pending, 0 infrastructure errors
- Browser: **10952 executed**, 10657 passed, 295 failed, 0 pending, 0 infrastructure errors
- Node/browser semantic target result streams identical (opaque WASM pointers ignored): **True**

## What is actually pending

**0 cases**. Pending means a selected case has no completed target comparison (`summary.not_run`). A facade `NotImplementedError` is not pending: the case ran and failed parity.

## Failure categories

| Category | Cases | Meaning / next owner |
| --- | ---: | --- |
| `result_mismatch` | 133 | Completed target result differs; fix core behavior or metadata, keeping bindings thin. |
| `target_error` | 113 | Target reached a public error path whose class/message/result differs; fix boundary validation or core error parity. |
| `target_not_implemented` | 21 | JS/WASM facade or an underlying public operation is missing; add a thin export only when core already owns the behavior. |
| `workflow_dependency_not_run` | 28 | A later observation was blocked by an earlier target failure; fix the first target error, not the dependent observation. |

## Ordered operation failures

The complete case IDs are in the JSON manifest; each row below is ordered by failing-case count.

| Rank | Public operation | Cases |
| ---: | --- | ---: |
| 1 | `PIL.Image.effect_noise` | 37 |
| 2 | `PIL.Image.Image.convert` | 22 |
| 3 | `PIL.ImageOps.colorize` | 16 |
| 4 | `PIL.Image.Image.getpalette` | 14 |
| 5 | `PIL.ImagePalette.ImagePalette.getcolor` | 14 |
| 6 | `PIL.Image.open` | 13 |
| 7 | `PIL.Image.Image.point` | 13 |
| 8 | `PIL.ImageOps.scale` | 13 |
| 9 | `PIL.Image.Image.paste` | 12 |
| 10 | `PIL.ImageOps.expand` | 11 |
| 11 | `PIL.Image.eval` | 9 |
| 12 | `PIL.ImageFilter.Color3DLUT` | 9 |
| 13 | `PIL.Image.Image.filter` | 8 |
| 14 | `PIL.Image.fromarray` | 7 |
| 15 | `PIL.Image.Image.draft` | 7 |
| 16 | `PIL.Image.Image.info` | 6 |
| 17 | `PIL.ImageFilter.Color3DLUT.generate` | 6 |
| 18 | `PIL.ImageOps.equalize` | 5 |
| 19 | `PIL.Image.Image.entropy` | 4 |
| 20 | `PIL.Image.Image.has_transparency_data` | 4 |
| 21 | `PIL.ImageFilter.MaxFilter` | 4 |
| 22 | `PIL.ImageFilter.MedianFilter` | 4 |
| 23 | `PIL.ImageFilter.MinFilter` | 4 |
| 24 | `PIL.ImageSequence.Iterator` | 4 |
| 25 | `PIL.Image.merge` | 3 |
| 26 | `PIL.Image.Image.getchannel` | 3 |
| 27 | `PIL.ImagePalette.ImagePalette` | 3 |
| 28 | `PIL.Image.frombytes` | 2 |
| 29 | `PIL.Image.Image.frombytes` | 2 |
| 30 | `PIL.Image.Image.get_flattened_data` | 2 |
| 31 | `PIL.Image.Image.getxmp` | 2 |
| 32 | `PIL.Image.Image.transpose` | 2 |
| 33 | `PIL.ImageDraw.Draw` | 2 |
| 34 | `PIL.ImageFilter.Kernel` | 2 |
| 35 | `PIL.ImageOps.deform` | 2 |
| 36 | `PIL.ImageOps.fit` | 2 |
| 37 | `PIL.ImageSequence.Iterator.__next__` | 2 |
| 38 | `PIL.Image.effect_mandelbrot` | 1 |
| 39 | `PIL.Image.frombuffer` | 1 |
| 40 | `PIL.Image.Image.apply_transparency` | 1 |
| 41 | `PIL.Image.Image.getim` | 1 |
| 42 | `PIL.Image.Image.tobytes` | 1 |
| 43 | `PIL.Image.Image.toqimage` | 1 |
| 44 | `PIL.Image.Image.toqpixmap` | 1 |
| 45 | `PIL.ImageDraw.Outline` | 1 |
| 46 | `PIL.ImageDraw.ImageDraw.getfont` | 1 |
| 47 | `PIL.ImageEnhance.Sharpness` | 1 |
| 48 | `PIL.ImageEnhance.Sharpness.enhance` | 1 |
| 49 | `PIL.ImageFilter.BoxBlur` | 1 |
| 50 | `PIL.ImageFilter.Color3DLUT.__repr__` | 1 |
| 51 | `PIL.ImageOps.pad` | 1 |
| 52 | `PIL.ImagePalette.ImagePalette.copy` | 1 |
| 53 | `PIL.ImagePalette.ImagePalette.getdata` | 1 |
| 54 | `PIL.ImagePalette.ImagePalette.save` | 1 |
| 55 | `PIL.ImagePalette.ImagePalette.tobytes` | 1 |
| 56 | `PIL.ImageSequence.Iterator.__iter__` | 1 |
| 57 | `PIL.ImageStat.Stat` | 1 |

## Interpretation

Node and browser use the same manifest, workflow payload, and JS adapter. They therefore should have the same selected and executed denominators. Opaque WASM handle pointers are host-process addresses and are intentionally ignored when checking semantic target equivalence. A capability difference belongs in the separate WebGPU/WGSL lane; it must not silently remove public parity cases.

The JSON `groups` array is the actionable backlog. Use `case_ids` for incremental runs and preserve the Python oracle as the behavioral authority. `target_errors` lists the most frequent target error signatures.
