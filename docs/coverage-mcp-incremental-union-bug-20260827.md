# Coverage MCP incremental-union reporting bug

Observed on 2026-08-27 with Coverage MCP 0.15.0, schema revision 15, for
`/Users/lazytrot/work/pillow-rs`.

## Expected behavior

For an incremental run, the dashboard's primary coverage number must be the
deduplicated union of the explicit baseline snapshot and the selected
increment. Therefore its covered-line count cannot be lower than the
baseline's covered-line count merely because the increment is a filtered
subset. Replacement-style subset differences may be reported separately, but
they must not be presented as the primary aggregate.

## Reproduction

- command: `eeb555bf-77f9-4cc9-ba81-c6261cedddd3`
- run: `377f757f-92f6-466d-a17c-13806dccbf90`
- baseline snapshot: `0a58d14e-db49-46f7-88b6-e4eb51c44748`
- selected current snapshot: `5f60624f-305d-4df3-b790-b159b2e901f5`
- execution mode: `incremental`
- selected case: `PIL.Image.Image.point.nuanced.i16-affine-callable`

The standalone incremental review reported:

| value | baseline | selected increment | primary `baseline_union_incremental` |
|---|---:|---:|---:|
| covered lines | 3664 | 620 | 3007 |
| total lines | 38807 | 31022 | 31022 |
| covered branches | 334 | 28 | 335 |
| newly covered lines | — | — | 30 |
| reported covered-line delta | — | — | -657 |

`3007 < 3664` makes the primary result impossible under union semantics.
The primary result also uses the selected subset's denominator, which makes
the dashboard percentage misleading even when the separate diff says the
subset has newly covered lines.

The newer focused `Image.point(..., mode="F")` run reproduces the same shape:
run `2155a4c5-2bce-4b1b-bec5-5f1b0fc87670`, baseline snapshot
`9c13e4fe-293c-450d-bae6-c49996e77500`, current snapshot
`92466125-0274-48df-b3f7-6aa658918b4b`; baseline covered 2012 lines while
the primary union projection reported 1890 and `covered_lines_delta=-122`.

A fresh managed incremental run using the accepted explicit `snapshot_id`
schema reproduces the defect after the runner fix as well:
run `9737767a-3103-4cd7-84e1-99ae8094bc54`, baseline snapshot
`1a70ff5f-d367-4394-b8f2-fbc17f592b7b`, current snapshot
`a5d38d26-b21c-4918-aea3-1770dc0df2a4`. The baseline has 3378 covered lines;
the selected current snapshot has 654; the primary result labelled
`baseline_union_incremental` reports 2830 covered lines and
`covered_lines_delta=-548`. Its `measurement_scope` is correctly marked
`selected_subset`, and the separate diff is `limited`, but the primary
aggregate is still lower than its baseline and therefore is not a union.

The automatic `run_review(view="status")` projection also returned
`not_measured` because it required about 855 words despite a large requested
word budget. The standalone `coverage_review(task="incremental")` did measure
the result when given `representation="compact"`.

## Control experiment

Multiple ordinary coverage artifacts attached to one run do aggregate
correctly:

- run: `f3660da6-157b-479e-a5a6-2511067b5ff4`
- snapshots: `561ffc12-b955-45ec-949f-86c9738d699c`,
  `e58ad7c2-ff85-4da3-a862-10c6071dfb15`
- the primary aggregate union reported 508 covered lines versus 496 in the
  first artifact and 4 covered branches versus 3.

This isolates the defect to incremental baseline-union projection/dashboard
reporting, not ordinary multi-file artifact ingestion.

The configured Coverage MCP tools expose no issue-submission endpoint, so this
file is the precise bug report and reproducible evidence to file with the
Coverage MCP maintainers.
