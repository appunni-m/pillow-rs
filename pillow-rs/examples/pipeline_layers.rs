//! Release-only pure-Rust pipeline boundary benchmark.
//!
//! This example measures the same four representative graph shapes used by
//! the public adapter benchmark, but constructs them directly through the
//! `pillow-rs` API.  It intentionally reports graph construction separately
//! from terminal materialization so Python binding overhead is not attributed
//! to the core executor.

use pillow_rs::{
    Backend, Image, PilError, chops_multiply, chops_screen, image_eval_validated, imageops_invert,
    imageops_mirror,
};
use std::env;
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
struct AllocationSample {
    calls: u64,
    bytes: u64,
}

fn reset_allocations() {
    Backend::reset_pipeline_allocation_telemetry();
}

fn allocation_sample() -> AllocationSample {
    let sample = Backend::take_pipeline_allocation_telemetry();
    AllocationSample {
        calls: sample.allocation_count,
        bytes: sample.allocated_bytes,
    }
}

fn execution_telemetry() -> (usize, u64, AllocationSample) {
    Backend::take_pipeline_telemetry()
        .map(|(_, _, operation_count, _, _, _, _, _, resource, _, _)| {
            let allocation = resource
                .as_ref()
                .map(|resource| AllocationSample {
                    calls: resource.host_allocation_count,
                    bytes: resource.host_allocated_bytes,
                })
                .unwrap_or_default();
            (
                operation_count,
                resource
                    .map(|resource| resource.fused_operation_count)
                    .unwrap_or_default(),
                allocation,
            )
        })
        .unwrap_or_default()
}

const WORKLOADS: [&str; 4] = [
    "transpose-twice",
    "gaussianblur-invert",
    "multiply-screen",
    "invert-mirror",
];
const GRAPH_SCALING_WORKLOAD: &str = "graph-scaling";
const GRAPH_LENGTHS: [usize; 6] = [0, 1, 8, 64, 1_024, 10_000];
const PAYLOAD_SCALING_WORKLOAD: &str = "payload-scaling";
const PAYLOAD_LENGTHS: [usize; 6] = [0, 1, 8, 64, 1_024, 10_000];
const BRANCH_CACHE_WORKLOAD: &str = "branch-cache";

#[derive(Debug)]
struct Sample {
    graph_ns: u128,
    execute_ns: u128,
    total_ns: u128,
    graph_allocations: AllocationSample,
    execute_allocations: AllocationSample,
    digest: u64,
}

#[derive(Debug)]
struct GraphSample {
    graph_ns: u128,
    clone_ns: u128,
    mode_ns: u128,
    graph_allocations: AllocationSample,
    signature: u64,
}

fn source(mode: &str, color: (u8, u8, u8, u8)) -> Result<Image, PilError> {
    Image::new(1024, 1024, mode, color)
}

fn build_workload(name: &str, backend: Backend) -> Result<Image, PilError> {
    let image = match name {
        "transpose-twice" => {
            let first = source("RGB", (47, 131, 223, 255))?.transpose("ROTATE_90")?;
            first.transpose("ROTATE_90")?
        }
        "gaussianblur-invert" => {
            let blurred = source("RGB", (23, 97, 181, 255))?.gaussian_blur(2.0)?;
            imageops_invert(&blurred)?
        }
        "multiply-screen" => {
            let primary = source("RGB", (17, 83, 149, 255))?;
            let secondary = source("RGB", (211, 127, 43, 255))?;
            let multiplied = chops_multiply(&primary, &secondary)?;
            chops_screen(&multiplied, &secondary)?
        }
        "invert-mirror" => {
            let inverted = imageops_invert(&source("RGB", (47, 131, 223, 255))?)?;
            imageops_mirror(&inverted)?
        }
        other => return Err(PilError::ValueError(format!("unknown workload: {other}"))),
    };
    Ok(image.use_backend(backend))
}

fn build_graph(length: usize) -> Result<Image, PilError> {
    let mut image = source("L", (127, 127, 127, 255))?;
    for _ in 0..length {
        image = imageops_invert(&image)?;
    }
    Ok(image)
}

fn build_lut_graph(length: usize) -> Result<Image, PilError> {
    let mut image = Image::new(1, 1, "L", (127, 127, 127, 255))?;
    let lut: Vec<u8> = (0..=u8::MAX).map(|value| u8::MAX - value).collect();
    for _ in 0..length {
        image = image_eval_validated(&image, &lut)?;
    }
    Ok(image)
}

fn build_branch_workload(backend: Backend) -> Result<(Image, Image), PilError> {
    let prefix = source("RGB", (23, 97, 181, 255))?
        .gaussian_blur(2.0)?
        .use_backend(backend);
    let invert_branch = imageops_invert(&prefix)?;
    let mirror_branch = imageops_mirror(&prefix)?;
    Ok((invert_branch, mirror_branch))
}

fn digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |state, byte| {
        state
            .wrapping_mul(0x100000001b3)
            .wrapping_add(u64::from(*byte))
    })
}

fn median<T>(samples: &[T], select: impl Fn(&T) -> u128) -> u128 {
    let mut values: Vec<u128> = samples.iter().map(select).collect();
    values.sort_unstable();
    values[values.len() / 2]
}

fn median_allocation(
    samples: &[AllocationSample],
    select: impl Fn(&AllocationSample) -> u64,
) -> u64 {
    median(samples, |sample| u128::from(select(sample))) as u64
}

fn run_graph_scaling(samples: usize) -> Result<String, PilError> {
    let mut records = Vec::with_capacity(GRAPH_LENGTHS.len());
    for length in GRAPH_LENGTHS {
        let mut measurements = Vec::with_capacity(samples);
        for _ in 0..samples {
            reset_allocations();
            let graph_started = Instant::now();
            let image = build_graph(length)?;
            let graph_ns = graph_started.elapsed().as_nanos();
            let graph_allocations = allocation_sample();

            let clone_started = Instant::now();
            let cloned = std::hint::black_box(image.clone());
            let clone_ns = clone_started.elapsed().as_nanos();

            let mode_started = Instant::now();
            let mode = cloned.mode()?;
            let mode_ns = mode_started.elapsed().as_nanos();
            let signature = digest(mode.as_bytes()) ^ length as u64;
            std::hint::black_box(mode);
            std::hint::black_box(cloned);
            measurements.push(GraphSample {
                graph_ns,
                clone_ns,
                mode_ns,
                graph_allocations,
                signature,
            });
        }
        let first = measurements.first().ok_or_else(|| {
            PilError::InternalError("graph benchmark produced no samples".to_owned())
        })?;
        records.push(format!(
            "{{\"workload_id\":\"pipeline.core.graph-scaling.{length}\",\"chain_length\":{length},\"samples\":{samples},\"graph_median_ns\":{},\"clone_median_ns\":{},\"mode_median_ns\":{},\"graph_alloc_calls_median\":{},\"graph_allocated_bytes_median\":{},\"signature\":{}}}",
            median(&measurements, |item| item.graph_ns),
            median(&measurements, |item| item.clone_ns),
            median(&measurements, |item| item.mode_ns),
            median_allocation(&measurements.iter().map(|item| item.graph_allocations).collect::<Vec<_>>(), |item| item.calls),
            median_allocation(&measurements.iter().map(|item| item.graph_allocations).collect::<Vec<_>>(), |item| item.bytes),
            first.signature,
        ));
    }
    Ok(format!("[{}]", records.join(",")))
}

fn run_payload_scaling(backend: Backend, samples: usize) -> Result<String, PilError> {
    let mut records = Vec::with_capacity(PAYLOAD_LENGTHS.len());
    for length in PAYLOAD_LENGTHS {
        let mut measurements = Vec::with_capacity(samples);
        for _ in 0..samples {
            reset_allocations();
            let graph_started = Instant::now();
            let image = build_lut_graph(length)?.use_backend(backend);
            let graph_ns = graph_started.elapsed().as_nanos();
            let graph_allocations = allocation_sample();

            let clone_started = Instant::now();
            let cloned = std::hint::black_box(image.clone());
            let clone_ns = clone_started.elapsed().as_nanos();

            let mode_started = Instant::now();
            let mode = cloned.mode()?;
            let mode_ns = mode_started.elapsed().as_nanos();

            reset_allocations();
            let execute_started = Instant::now();
            let bytes = cloned.tobytes()?;
            let execute_ns = execute_started.elapsed().as_nanos();
            let (operation_count, fused_operation_count, execute_allocations) =
                execution_telemetry();
            let signature = digest(&bytes) ^ digest(mode.as_bytes()) ^ length as u64;
            measurements.push((
                graph_ns,
                clone_ns,
                mode_ns,
                execute_ns,
                operation_count,
                fused_operation_count,
                graph_allocations,
                execute_allocations,
                signature,
            ));
        }
        let first = measurements.first().ok_or_else(|| {
            PilError::InternalError("payload benchmark produced no samples".to_owned())
        })?;
        records.push(format!(
            "{{\"workload_id\":\"pipeline.core.payload-scaling.{length}\",\"chain_length\":{length},\"samples\":{samples},\"graph_median_ns\":{},\"clone_median_ns\":{},\"mode_median_ns\":{},\"execute_median_ns\":{},\"operation_count\":{},\"fused_operation_count\":{},\"graph_alloc_calls_median\":{},\"graph_allocated_bytes_median\":{},\"execute_alloc_calls_median\":{},\"execute_allocated_bytes_median\":{},\"signature\":{}}}",
            median(&measurements, |item| item.0),
            median(&measurements, |item| item.1),
            median(&measurements, |item| item.2),
            median(&measurements, |item| item.3),
            first.4,
            first.5,
            median_allocation(&measurements.iter().map(|item| item.6).collect::<Vec<_>>(), |item| item.calls),
            median_allocation(&measurements.iter().map(|item| item.6).collect::<Vec<_>>(), |item| item.bytes),
            median_allocation(&measurements.iter().map(|item| item.7).collect::<Vec<_>>(), |item| item.calls),
            median_allocation(&measurements.iter().map(|item| item.7).collect::<Vec<_>>(), |item| item.bytes),
            first.8,
        ));
    }
    Ok(format!("[{}]", records.join(",")))
}

fn run_branch_cache(backend: Backend, samples: usize) -> Result<String, PilError> {
    let mut measurements = Vec::with_capacity(samples);
    for _ in 0..samples {
        let (invert_branch, mirror_branch) = build_branch_workload(backend)?;

        reset_allocations();
        let first_started = Instant::now();
        let invert_bytes = invert_branch.tobytes()?;
        let first_ns = first_started.elapsed().as_nanos();
        let (first_operation_count, first_fused_operation_count, first_allocations) =
            execution_telemetry();

        reset_allocations();
        let second_started = Instant::now();
        let mirror_bytes = mirror_branch.tobytes()?;
        let second_ns = second_started.elapsed().as_nanos();
        let (second_operation_count, second_fused_operation_count, second_allocations) =
            execution_telemetry();

        measurements.push((
            first_ns,
            second_ns,
            first_operation_count,
            first_fused_operation_count,
            second_operation_count,
            second_fused_operation_count,
            first_allocations,
            second_allocations,
            digest(&invert_bytes) ^ digest(&mirror_bytes),
        ));
    }
    let first = measurements.first().ok_or_else(|| {
        PilError::InternalError("branch benchmark produced no samples".to_owned())
    })?;
    Ok(format!(
        "{{\"workload_id\":\"pipeline.core.branch-cache.rgb-1024\",\"samples\":{samples},\"prefix_operation_count\":1,\"first_branch_logical_operation_count\":2,\"second_branch_logical_operation_count\":2,\"first_branch_execute_median_ns\":{},\"second_branch_execute_median_ns\":{},\"first_branch_suffix_operation_count\":{},\"first_branch_fused_operation_count\":{},\"second_branch_suffix_operation_count\":{},\"second_branch_fused_operation_count\":{},\"first_branch_alloc_calls_median\":{},\"first_branch_allocated_bytes_median\":{},\"second_branch_alloc_calls_median\":{},\"second_branch_allocated_bytes_median\":{},\"signature\":{}}}",
        median(&measurements, |item| item.0),
        median(&measurements, |item| item.1),
        first.2,
        first.3,
        first.4,
        first.5,
        median_allocation(
            &measurements.iter().map(|item| item.6).collect::<Vec<_>>(),
            |item| item.calls,
        ),
        median_allocation(
            &measurements.iter().map(|item| item.6).collect::<Vec<_>>(),
            |item| item.bytes,
        ),
        median_allocation(
            &measurements.iter().map(|item| item.7).collect::<Vec<_>>(),
            |item| item.calls,
        ),
        median_allocation(
            &measurements.iter().map(|item| item.7).collect::<Vec<_>>(),
            |item| item.bytes,
        ),
        first.8,
    ))
}

fn parse_args() -> Result<(Backend, usize, Vec<String>), String> {
    let mut backend = Backend::Cpu;
    let mut samples = 20usize;
    let mut workloads = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--backend" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--backend requires cpu, simd, or gpu".to_owned())?;
                backend = Backend::parse(&value)
                    .ok_or_else(|| format!("unsupported backend: {value}"))?;
            }
            "--samples" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--samples requires a positive integer".to_owned())?;
                samples = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid sample count: {value}"))?;
                if samples == 0 {
                    return Err("--samples must be positive".to_owned());
                }
            }
            "--workload" => workloads.push(
                args.next()
                    .ok_or_else(|| "--workload requires a workload id".to_owned())?,
            ),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if workloads.is_empty() {
        workloads.extend(WORKLOADS.iter().map(|name| (*name).to_owned()));
        workloads.push(GRAPH_SCALING_WORKLOAD.to_owned());
        workloads.push(PAYLOAD_SCALING_WORKLOAD.to_owned());
        workloads.push(BRANCH_CACHE_WORKLOAD.to_owned());
    }
    for workload in &workloads {
        if workload != GRAPH_SCALING_WORKLOAD
            && workload != PAYLOAD_SCALING_WORKLOAD
            && workload != BRANCH_CACHE_WORKLOAD
            && !WORKLOADS.contains(&workload.as_str())
        {
            return Err(format!("unknown workload: {workload}"));
        }
    }
    Ok((backend, samples, workloads))
}

fn run_workload(name: &str, backend: Backend, samples: usize) -> Result<String, PilError> {
    let _ = build_workload(name, backend)?.tobytes()?;
    let mut measurements = Vec::with_capacity(samples);
    for _ in 0..samples {
        reset_allocations();
        let total_started = Instant::now();
        let graph_started = Instant::now();
        let image = build_workload(name, backend)?;
        let graph_ns = graph_started.elapsed().as_nanos();
        let graph_allocations = allocation_sample();
        reset_allocations();
        let execute_started = Instant::now();
        let bytes = image.tobytes()?;
        let execute_ns = execute_started.elapsed().as_nanos();
        let (_, _, execute_allocations) = execution_telemetry();
        measurements.push(Sample {
            graph_ns,
            execute_ns,
            total_ns: total_started.elapsed().as_nanos(),
            graph_allocations,
            execute_allocations,
            digest: digest(&bytes),
        });
    }
    let sample = measurements
        .first()
        .ok_or_else(|| PilError::InternalError("core benchmark produced no samples".to_owned()))?;
    Ok(format!(
        "{{\"workload_id\":\"pipeline.quick.{name}.rgb-1024\",\"samples\":{samples},\"graph_median_ns\":{},\"execute_median_ns\":{},\"total_median_ns\":{},\"graph_alloc_calls_median\":{},\"graph_allocated_bytes_median\":{},\"execute_alloc_calls_median\":{},\"execute_allocated_bytes_median\":{},\"digest\":{}}}",
        median(&measurements, |item| item.graph_ns),
        median(&measurements, |item| item.execute_ns),
        median(&measurements, |item| item.total_ns),
        median_allocation(
            &measurements
                .iter()
                .map(|item| item.graph_allocations)
                .collect::<Vec<_>>(),
            |item| item.calls
        ),
        median_allocation(
            &measurements
                .iter()
                .map(|item| item.graph_allocations)
                .collect::<Vec<_>>(),
            |item| item.bytes
        ),
        median_allocation(
            &measurements
                .iter()
                .map(|item| item.execute_allocations)
                .collect::<Vec<_>>(),
            |item| item.calls
        ),
        median_allocation(
            &measurements
                .iter()
                .map(|item| item.execute_allocations)
                .collect::<Vec<_>>(),
            |item| item.bytes
        ),
        sample.digest,
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (backend, samples, workloads) = parse_args().map_err(PilError::ValueError)?;
    let previous_telemetry = Backend::set_pipeline_telemetry_enabled(true);
    let graph_scaling = workloads
        .iter()
        .any(|workload| workload == GRAPH_SCALING_WORKLOAD)
        .then(|| run_graph_scaling(samples))
        .transpose()?;
    let payload_scaling = workloads
        .iter()
        .any(|workload| workload == PAYLOAD_SCALING_WORKLOAD)
        .then(|| run_payload_scaling(backend, samples))
        .transpose()?;
    let branch_cache = workloads
        .iter()
        .any(|workload| workload == BRANCH_CACHE_WORKLOAD)
        .then(|| run_branch_cache(backend, samples))
        .transpose()?;
    let records = workloads
        .iter()
        .filter(|workload| {
            workload.as_str() != GRAPH_SCALING_WORKLOAD
                && workload.as_str() != PAYLOAD_SCALING_WORKLOAD
                && workload.as_str() != BRANCH_CACHE_WORKLOAD
        })
        .map(|workload| run_workload(workload, backend, samples))
        .collect::<Result<Vec<_>, _>>()?;
    println!(
        "{{\"schema\":\"pillow-rs/pipeline-core-benchmark@4\",\"backend\":\"{:?}\",\"workloads\":[{}],\"graph_scaling\":{},\"payload_scaling\":{},\"branch_cache\":{}}}",
        backend,
        records.join(","),
        graph_scaling.as_deref().unwrap_or("[]"),
        payload_scaling.as_deref().unwrap_or("[]"),
        branch_cache.as_deref().unwrap_or("[]")
    );
    Backend::set_pipeline_telemetry_enabled(previous_telemetry);
    Ok(())
}
