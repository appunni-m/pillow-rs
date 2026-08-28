# Public-input audit for the requested Pillow surfaces

Audited on 2026-08-27 against the active manifest-driven parity corpus.

This is an input-coverage audit, not a claim that the whole Pillow Python
package or the whole Rust workspace is 100% covered. The canonical campaign
selected and executed all 10,952 active public parity cases. Individual GUI
operations such as `toqimage` remain explicitly marked `not_applicable` in the
manifest, but they do not create incomplete workflows or infrastructure
failures. The run reported zero incomplete workflows and zero infrastructure
errors.

## Requested behavior dimensions

| Surface | Public-input evidence | Pillow Python source result |
|---|---|---|
| `Image.point` | `i-affine-callable`, `i16-affine-callable`, `f-affine-callable`, `l-float-callable`, `l-float-output-mode`, and expanded-LUT cases | `Image.point`: 14/15 lines, 9/10 branches |
| `ImageStat` masks | `parameter.mask` and `nuanced.nonzero-mask`, plus mode and histogram cases | `ImageStat.py`: 74/74 lines, 26/26 branches |
| `ImageDraw.line` joint | `parameter.joint`, `wide-joint-curve`, `mid-joint-curve`, and additional wide/straight/flipped joint cases | `ImageDraw.line`: 24/24 lines, 15/16 branches |
| filter validation | valid filters plus invalid even sizes for `RankFilter`, `MaxFilter`, `MinFilter`, `MedianFilter`, and zero-size `ModeFilter` | each requested constructor/filter path is executed; native validation is below coverage.py’s Python layer |
| `BoxBlur` / `GaussianBlur` | scalar, zero, fractional, and `(x,y)` radius cases | `BoxBlur` and `GaussianBlur` paths: 100% of their reported lines/branches |
| `ImageChops` mismatch errors | `add` mode mismatch and `blend` size mismatch, with logical-mode mismatch cases | `ImageChops.py`: 78/78 lines, 2/2 branches |
| `scale`, `thumbnail`, `rotate`, `putdata` edges | negative/zero/NaN/overflow and invalid-parameter cases, including empty sources and malformed data | `scale`: 7/7 lines, 4/4 branches; `thumbnail`: 17/17, 7/8; `rotate`: 32/32, 16/16; `putdata`: 2/2, no branches |

Therefore the answer is **yes for the requested input dimensions**: each has
public inputs in the corpus, and the Pillow source-side audit reaches the
corresponding ordinary implementation paths. This does not mean every branch
in those files is reachable by an input-only JSON workflow.

## Deliberately unclaimed branches

Three remaining branches are representation or invariant cases, not missing
ordinary inputs:

1. `Image.point` dispatch to a custom `ImagePointHandler` remains uncovered
   (Pillow `Image.py` line 2025). It requires a user-defined Python object with
   a `point()` method; the input-only manifest does not serialize arbitrary
   Python objects, and the JS facade cannot represent that callback contract.
2. `Image.thumbnail` retains the `self.size == final_size` postcondition branch
   (branch `2908 -> 2915`). Exhaustive small valid source/target-size probes did
   not reach it. It depends on an internal decoder `draft()` resize decision,
   not on the normal target-size arithmetic; the installed Pillow decoder used
   for the probe did not provide such a resize for the available asset.
3. `ImageDraw.line` retains the `ink is None` fallback branch (branch
   `243 -> -234`). A normal `ImageDraw.Draw(image)` has a default ink, and the
   input-only workflow cannot mutate the draw object's internal ink state to
   manufacture this condition.

These branches should remain visible in coverage reports. They should not be
filled with fabricated invalid inputs: the first is a binding representation
gap, while the latter two are internal-state/invariant paths. If the public
API is later expanded to accept custom Python objects or custom draw state,
they should receive dedicated binding-level parity workflows.

The `Image.point` campaign also exposed and fixed one real setup mismatch:
Pillow saturates accepted byte components to `0..255`, while the Rust path was
wrapping with `as u8`. The corrected clamp now passes the 100 repeated-LUT
cases on CPU, SIMD, GPU, Node WASM, and browser WASM. The final Node/browser
campaign executes the full 10,952-case corpus with zero parity failures, zero
pending cases, and identical semantic target streams. GPU/WGSL dispatch
coverage remains a separate runtime coverage measure and must not be
conflated with public parity completion.
