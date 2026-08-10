# Quantize CPU coverage audit

Baseline: `4f0fe721c` on the isolated branch
`codex/coverage-quantize-v1-20260811`.

## Evidence

The managed full CPU snapshot `77df4e0e-41a2-49a5-ad76-fc6e70d5930c`
reports for `pillow-rs/src/ops/quantize.rs`:

- lines: `1,624/1,671` (`97.187%`)
- branches: `288/332` (`86.747%`)
- functions: `116/118` (`98.305%`)

The operation-scoped managed run `1cffc407-15c0-4804-ac3e-fb4ee84dd7bc`
completed successfully with snapshot
`44e05129-51bb-4a53-9911-f6cfe7b373cf` and exercised the public
`PIL.Image.Image.quantize` cases. It reports `1,464/1,671` lines,
`265/332` branches, and `106/118` functions. The existing
`fast-octree-rgba-transparent` parity case also passed independently.

## Classification

The uncovered and partial records fall into these groups:

1. **Impossible internal states**: empty histogram/bounds and empty palette
   returns; a tree node with only one child; a palette box with no pixels; and
   an invalid k-means palette index. Valid public inputs cannot produce these
   states because the preceding constructors establish the corresponding
   invariants.
2. **Defensive validation**: zero/one-pixel split rejection, empty axis data,
   invalid split bounds, zero-count split results, and the out-of-range color
   validation path. The public corpus already covers the externally observable
   validation errors; the remaining lines are internal guards after validated
   data has been constructed.
3. **Algorithmic alternatives already represented by valid inputs**: the
   median-cut R/G/B selection and octree ordering code is reached by the
   existing diverse RGB/RGBA and k-means cases. The remaining partial branches
   are tie/order/fallback outcomes, not a missing public operation.
4. **Non-quantize helper paths in this module**: web-palette conversion and
   helper cache branches are exercised by conversion lanes, not by
   `Image.quantize`. They must not be counted as missing quantize inputs.

The duplicate-chain update in `MaxCoverageHash::chain_insert` is not a valid
new public case: normal remapping performs a lookup before insertion, so an
existing key is returned by the lookup and never reaches the duplicate insert
arm. The adaptive hash rebuild cases are already in the corpus; adding another
large synthetic image would only duplicate that coverage.

## Decision

No parity input or runtime change is justified by this audit. Adding a fixture
would either duplicate an existing case or target an invariant-protected
branch, so the exact baseline remains unchanged.

