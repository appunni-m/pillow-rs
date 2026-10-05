// Working core API example. The implementation lives in
// pillow-rs/src/compute/pool_gpu/stream.rs; Python's bounded run/join driver
// lives in pillow-rs-py/python/pillow_rs/gpu_batch.py. This example is outside
// Cargo and does not define another PipelineOp or scheduler.
use pillow_rs::{BatchOutput, GpuBatchConfig, GpuBatchExecutor, Image};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut executor = GpuBatchExecutor::new(GpuBatchConfig::default())?;
    let source = Image::new(256, 256, "L", (42, 42, 42, 255))?;
    let lazy = pillow_rs::imageops_invert(&source)?; // canonical Image::Pipeline / PipelineOp::Invert
    let id = executor.submit(&lazy)?;
    while let Some(result) = executor.next_result()? {
        assert_eq!(result.job_id, id);
        match result.image {
            BatchOutput::Cpu(image) => {
                let _materialized = image;
            }
            BatchOutput::Gpu(handle) => {
                let _materialized = handle.download()?;
            }
        }
    }
    executor.close();
    Ok(())
}

// Runtime flow:
// existing lazy Image -> inspect pending canonical graph (including auxiliaries)
// -> native mode / shape / device / cumulative-work proof
// -> reserve host + device + metadata credits before decode/allocation
// -> decode leaf / upload native bytes
// -> encode source and auxiliary stages in dependency order, stable uniforms
// -> pending Job { unique ID, optional key, command, output, credit }
// -> bounded chunk: queue.submit(graph commands + terminal copies/fence)
// -> Flight owns jobs and mapped-arena completion fence
// -> Ready owns shared staging ranges or resident terminal buffer
// -> identity-tagged BatchResult transfers materialized output ownership
// -> release credits when the final physical arena/handle owner drops.
//
// Python submit(lazy_image) / join(): bounded manual admission window.
// Python run((key, lazy_image) for ...): lazy higher-order stream, no collect.
// Result order is unspecified. Match by job_id/input_key, never position.
// output="gpu" returns opaque owning GpuImage; download leaves its lease alive.
// Native byte caps exclude one producer-owned lookahead and caller collections.
// close/drop stops pulling from the producer iterator and fences transfers/submissions.
// Codec direct-destination integration and external GPU interop remain future work.
