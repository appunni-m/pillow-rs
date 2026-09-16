# Pipeline performance roadmap

Status: Historical implementation statuses; current release acceptance remains separate.

This public index retains all 64 reviewed performance work items from the
[pre-consolidation roadmap](https://github.com/appunni-m/pillow-rs/blob/a8c79d735c0a4188b6881c63a17be99a7e5142d9/docs/image-pipeline-performance-roadmap.md).
The statuses below were preserved during documentation consolidation; a new
benchmark does not close an item or establish support on every backend.
`closed` applies to the original scoped work item. `in progress` and `proposed`
remain open. Current API boundaries are in [maturity](COMPATIBILITY.md), and
current measurements and unproven timing budgets are in [benchmarking](BENCHMARKING.md).

The report command joins this complete index with benchmark evidence and
rejects missing, duplicate, or unexpected IDs. Changes to a status require
reviewed implementation and measurement evidence.

| ID | Work item | Priority | Status |
| --- | --- | --- | --- |
| FIL-01 | Expand the benchmark matrix | P0 | closed |
| FIL-02 | Add execution-phase and actual-backend telemetry | P0 | closed |
| FIL-03 | Measure allocations, peak bytes, and transfer volume | P0 | in progress |
| FIL-04 | Establish repeatable profiling workflows | P0 | closed |
| FIL-05 | Separate kernel, core-pipeline, and binding benchmarks | P0 | in progress |
| FIL-06 | Define operation-class performance acceptance gates | P0 | closed |
| FIL-07 | Remove the owned clone from read-only materialization | P0 | in progress |
| FIL-08 | Make `Image` clones cheap and independent of graph length | P0 | closed |
| FIL-09 | Replace quadratic `Vec<PipelineOp>` append behavior | P0 | in progress |
| FIL-10 | Share large operation payloads | P1 | closed |
| FIL-11 | Share palette and metadata state | P1 | proposed |
| FIL-12 | Propagate dimensions and mode without materialization | P0 | in progress |
| FIL-13 | Represent mode changes inside one plan | P0 | proposed |
| FIL-14 | Introduce ownership-aware in-place execution | P0 | proposed |
| FIL-15 | Add a bounded scratch-buffer arena | P0 | proposed |
| FIL-16 | Deduplicate secondary-image materialization | P0 | proposed |
| FIL-17 | Reuse materialized branch nodes | P1 | in progress |
| FIL-18 | Consolidate operation metadata into one typed descriptor | P0 | proposed |
| FIL-19 | Plan, validate, and route once | P1 | in progress |
| FIL-20 | Add cost-based segmentation and fusion planning | P0 | in progress |
| FIL-21 | Replace the parallel helper API with safe writable chunks | P0 | in progress |
| FIL-22 | Introduce native typed pixel-kernel views | P0 | proposed |
| FIL-23 | Compose point and LUT operations into one traversal | P0 | in progress |
| FIL-24 | Replace sort-based autocontrast and multi-pass equalize | P0 | closed |
| FIL-25 | Implement exact rolling-window box and Gaussian blur | P0 | closed |
| FIL-26 | Optimize median, rank, minimum, and maximum filters | P1 | in progress |
| FIL-27 | Specialize and tile convolution | P1 | in progress |
| FIL-28 | Flatten and cache resize coefficient tables | P0 | closed |
| FIL-29 | Borrow the resize source and parallelize both passes | P0 | in progress |
| FIL-30 | Fuse alpha premultiplication with resampling | P1 | in progress |
| FIL-31 | Optimize transpose, crop, reduce, and simple geometry | P1 | in progress |
| FIL-32 | Optimize Chops and multi-image kernels | P0 | closed |
| FIL-33 | Batch drawing onto one mutable canvas | P1 | closed |
| FIL-34 | Consolidate effects, enhancement, and color kernels | P1 | in progress |
| FIL-35 | Profile and optimize quantization by algorithm phase | P2 | in progress |
| FIL-36 | Avoid full owned materialization for terminal reads and reductions | P1 | in progress |
| FIL-37 | Make SIMD capability truthful | P0 | proposed |
| FIL-38 | Retain a native SIMD working frame across the batch | P0 | proposed |
| FIL-39 | Add portable runtime architecture dispatch | P0 | proposed |
| FIL-40 | Vectorize point and LUT kernels first | P0 | closed |
| FIL-41 | Vectorize Chops and alpha compositing | P1 | in progress |
| FIL-42 | Vectorize copy-like geometry | P1 | closed |
| FIL-43 | Vectorize filters after scalar algorithms are fixed | P1 | in progress |
| FIL-44 | Coordinate thread-level and lane-level parallelism | P1 | proposed |
| FIL-45 | Reduce cold GPU initialization and shader compilation | P1 | proposed |
| FIL-46 | Eliminate redundant input packing and the second upload | P0 | in progress |
| FIL-47 | Remove duplicate readback and unpack copies | P0 | in progress |
| FIL-48 | Persist and bound GPU resource pools | P0 | in progress |
| FIL-49 | Deduplicate auxiliary images and static GPU resources | P0 | in progress |
| FIL-50 | Reuse parameter arenas and bind groups | P1 | proposed |
| FIL-51 | Fuse compatible GPU point shaders | P0 | in progress |
| FIL-52 | Replace loop-per-sample GPU neighborhood kernels | P0 | in progress |
| FIL-53 | Tune workgroup shape by operation and device | P1 | proposed |
| FIL-54 | Carry per-operation mode, dimensions, and native layout on GPU | P0 | proposed |
| FIL-55 | Keep images device-resident across lazy graph nodes | P0 | in progress |
| FIL-56 | Make submission, waiting, fallback, and device loss explicit | P0 | in progress |
| FIL-57 | Use cheap shared handles at Python and JS boundaries | P0 | proposed |
| FIL-58 | Release the Python GIL for every heavy terminal execution | P1 | in progress |
| FIL-59 | Reduce Python byte and array copies | P1 | proposed |
| FIL-60 | Reduce JavaScript/WASM linear-memory copies | P1 | proposed |
| FIL-61 | Add asynchronous GPU observation and direct terminal sinks | P1 | proposed |
| FIL-62 | Add performance regression reporting and guarded budgets | P1 | in progress |
| FIL-63 | Roll out architecture changes behind observable internal stages | P1 | proposed |
| FIL-64 | Maintain one roadmap and one generated status report | P2 | closed |
