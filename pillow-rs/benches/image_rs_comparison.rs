//! Focused serial-CPU and SIMD comparison against the `image` crate.
#![allow(unused_crate_dependencies)] // Package dependencies are used by the linked library target.

use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use image::{RgbImage, imageops};
use pillow_rs::{Backend, DynamicImage, Image};

type BenchResult<T> = Result<T, Box<dyn Error>>;
type PillowOperation = fn(&Image, Backend) -> BenchResult<DynamicImage>;
type ImageRsOperation = fn(&RgbImage) -> RgbImage;

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;
const CROP: (u32, u32, u32, u32) = (96, 48, 928, 720);
const WARMUP_CALLS: usize = 4;
const SAMPLES: usize = 21;
const CALLS_PER_SAMPLE: usize = 20;

struct Workload {
    id: &'static str,
    operation: &'static str,
    require_simd_vectors: bool,
    pillow_rs: PillowOperation,
    image_rs: ImageRsOperation,
}

const WORKLOADS: [Workload; 4] = [
    Workload {
        id: "crop-rgb-1024x768-to-832x672",
        operation: "Crop",
        require_simd_vectors: false,
        pillow_rs: pillow_crop,
        image_rs: image_rs_crop,
    },
    Workload {
        id: "flip-left-right-rgb-1024x768",
        operation: "Transpose",
        require_simd_vectors: true,
        pillow_rs: pillow_flip_left_right,
        image_rs: image_rs_flip_left_right,
    },
    Workload {
        id: "rotate-90-rgb-1024x768",
        operation: "Transpose",
        require_simd_vectors: true,
        pillow_rs: pillow_rotate_90,
        image_rs: image_rs_rotate_90,
    },
    Workload {
        id: "rotate-180-rgb-1024x768",
        operation: "Transpose",
        require_simd_vectors: true,
        pillow_rs: pillow_rotate_180,
        image_rs: image_rs_rotate_180,
    },
];

fn pillow_crop(image: &Image, backend: Backend) -> BenchResult<DynamicImage> {
    Ok(image
        .crop(Some((
            CROP.0 as i32,
            CROP.1 as i32,
            CROP.2 as i32,
            CROP.3 as i32,
        )))?
        .use_backend(backend)
        .materialize()?)
}

fn image_rs_crop(image: &RgbImage) -> RgbImage {
    imageops::crop_imm(image, CROP.0, CROP.1, CROP.2 - CROP.0, CROP.3 - CROP.1).to_image()
}

fn pillow_flip_left_right(image: &Image, backend: Backend) -> BenchResult<DynamicImage> {
    Ok(image
        .transpose("FLIP_LEFT_RIGHT")?
        .use_backend(backend)
        .materialize()?)
}

fn image_rs_flip_left_right(image: &RgbImage) -> RgbImage {
    imageops::flip_horizontal(image)
}

fn pillow_rotate_90(image: &Image, backend: Backend) -> BenchResult<DynamicImage> {
    Ok(image
        .transpose("ROTATE_90")?
        .use_backend(backend)
        .materialize()?)
}

fn image_rs_rotate_90(image: &RgbImage) -> RgbImage {
    imageops::rotate270(image)
}

fn pillow_rotate_180(image: &Image, backend: Backend) -> BenchResult<DynamicImage> {
    Ok(image
        .transpose("ROTATE_180")?
        .use_backend(backend)
        .materialize()?)
}

fn image_rs_rotate_180(image: &RgbImage) -> RgbImage {
    imageops::rotate180(image)
}

struct TelemetryGuard {
    previous: bool,
}

impl TelemetryGuard {
    fn enable() -> Self {
        Self {
            previous: Backend::set_pipeline_telemetry_enabled(true),
        }
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.previous);
    }
}

struct ExecutionProof {
    path: &'static str,
    vector_blocks: u64,
}

fn execute_with_proof(
    workload: &Workload,
    image: &Image,
    backend: Backend,
) -> BenchResult<(DynamicImage, ExecutionProof)> {
    let telemetry_guard = TelemetryGuard::enable();
    let output = (workload.pillow_rs)(image, backend);
    let pipeline = Backend::take_pipeline_telemetry();
    let operations = Backend::take_pipeline_operation_telemetry();
    drop(telemetry_guard);

    let output = output?;
    let (requested_backend, actual_backend, operation_count, _, _, _, _, fallback_reason, _, _, _) =
        pipeline.ok_or_else(|| format!("{} produced no pipeline receipt", workload.id))?;
    if requested_backend != Some(backend)
        || actual_backend != backend
        || operation_count != 1
        || fallback_reason.is_some()
    {
        return Err(format!(
            "{} requested {backend:?}, actual {actual_backend:?}, operations={operation_count}, fallback={fallback_reason:?}",
            workload.id
        )
        .into());
    }
    if operations.len() != 1 || operations[0].operation != workload.operation {
        return Err(format!(
            "{} expected one {} operation receipt, got {operations:?}",
            workload.id, workload.operation
        )
        .into());
    }
    let operation = &operations[0];
    Ok((
        output,
        ExecutionProof {
            path: operation.path,
            vector_blocks: operation.vector_block_count,
        },
    ))
}

fn verify_equal_output(
    workload: &Workload,
    pillow: &Image,
    image_rs_output: &RgbImage,
    backend: Backend,
) -> BenchResult<ExecutionProof> {
    let (pillow_output, proof) = execute_with_proof(workload, pillow, backend)?;
    if pillow_output.as_bytes() != image_rs_output.as_raw() {
        return Err(std::io::Error::other(format!(
            "{} produced different RGB bytes under {backend:?}; refusing to time unlike outputs",
            workload.id
        ))
        .into());
    }
    Ok(proof)
}

fn has_vector_work(proof: &ExecutionProof) -> bool {
    proof.vector_blocks > 0 && matches!(proof.path, "vector" | "native-copy")
}

fn time_batch<T>(mut operation: impl FnMut() -> BenchResult<T>) -> BenchResult<f64> {
    let started = Instant::now();
    for _ in 0..CALLS_PER_SAMPLE {
        black_box(operation()?);
    }
    Ok(started.elapsed().as_secs_f64() * 1_000_000.0 / CALLS_PER_SAMPLE as f64)
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

#[derive(Clone, Copy)]
enum Subject {
    Cpu,
    Simd,
    ImageRs,
}

fn time_subject(
    subject: Subject,
    workload: &Workload,
    pillow: &Image,
    image: &RgbImage,
) -> BenchResult<f64> {
    match subject {
        Subject::Cpu => time_batch(|| (workload.pillow_rs)(pillow, Backend::Cpu)),
        Subject::Simd => time_batch(|| (workload.pillow_rs)(pillow, Backend::Simd)),
        Subject::ImageRs => time_batch(|| Ok((workload.image_rs)(image))),
    }
}

struct Measurements {
    cpu_us: f64,
    simd_us: Option<f64>,
    image_rs_us: f64,
}

fn compare(
    workload: &Workload,
    pillow: &Image,
    image: &RgbImage,
    measure_simd: bool,
) -> BenchResult<Measurements> {
    for _ in 0..WARMUP_CALLS {
        black_box((workload.pillow_rs)(pillow, Backend::Cpu)?);
        if measure_simd {
            black_box((workload.pillow_rs)(pillow, Backend::Simd)?);
        }
        black_box((workload.image_rs)(image));
    }

    let mut cpu_samples = Vec::with_capacity(SAMPLES);
    let mut simd_samples = Vec::with_capacity(SAMPLES);
    let mut image_rs_samples = Vec::with_capacity(SAMPLES);
    const SUBJECT_ORDERS: [[Subject; 3]; 6] = [
        [Subject::Cpu, Subject::Simd, Subject::ImageRs],
        [Subject::Simd, Subject::ImageRs, Subject::Cpu],
        [Subject::ImageRs, Subject::Cpu, Subject::Simd],
        [Subject::Cpu, Subject::ImageRs, Subject::Simd],
        [Subject::ImageRs, Subject::Simd, Subject::Cpu],
        [Subject::Simd, Subject::Cpu, Subject::ImageRs],
    ];
    for sample in 0..SAMPLES {
        if measure_simd {
            for subject in SUBJECT_ORDERS[sample % SUBJECT_ORDERS.len()] {
                let duration = time_subject(subject, workload, pillow, image)?;
                match subject {
                    Subject::Cpu => cpu_samples.push(duration),
                    Subject::Simd => simd_samples.push(duration),
                    Subject::ImageRs => image_rs_samples.push(duration),
                }
            }
        } else if sample % 2 == 0 {
            cpu_samples.push(time_subject(Subject::Cpu, workload, pillow, image)?);
            image_rs_samples.push(time_subject(Subject::ImageRs, workload, pillow, image)?);
        } else {
            image_rs_samples.push(time_subject(Subject::ImageRs, workload, pillow, image)?);
            cpu_samples.push(time_subject(Subject::Cpu, workload, pillow, image)?);
        }
    }

    Ok(Measurements {
        cpu_us: median(cpu_samples),
        simd_us: measure_simd.then(|| median(simd_samples)),
        image_rs_us: median(image_rs_samples),
    })
}

fn main() -> BenchResult<()> {
    let pixels = (0..(WIDTH as usize * HEIGHT as usize))
        .flat_map(|index| [index as u8, (index >> 8) as u8, (index >> 16) as u8]);
    let input: Vec<u8> = pixels.collect();
    let pillow = Image::frombytes("RGB", (WIDTH, HEIGHT), &input)?;
    let image = RgbImage::from_raw(WIDTH, HEIGHT, input).ok_or_else(|| {
        std::io::Error::other("deterministic RGB input did not match its declared dimensions")
    })?;

    println!(
        "profiles: pillow-rs Backend::Cpu, pillow-rs Backend::Simd, image-rs with defaults disabled"
    );
    println!("input: RGB {WIDTH}x{HEIGHT}; deterministic bytes; construction excluded");
    println!(
        "sampling: {WARMUP_CALLS} warmups; {SAMPLES} alternating samples; {CALLS_PER_SAMPLE} calls per sample; median"
    );
    println!("case | CPU µs | SIMD µs | SIMD proof | image-rs µs | image-rs/CPU | image-rs/SIMD");

    for workload in &WORKLOADS {
        let image_rs_output = (workload.image_rs)(&image);
        verify_equal_output(workload, &pillow, &image_rs_output, Backend::Cpu)?;
        let simd_proof = verify_equal_output(workload, &pillow, &image_rs_output, Backend::Simd)?;
        let measure_simd = has_vector_work(&simd_proof);
        if workload.require_simd_vectors && !measure_simd {
            return Err(format!(
                "{} requested SIMD but its receipt did not prove vector work: path={}, blocks={}",
                workload.id, simd_proof.path, simd_proof.vector_blocks
            )
            .into());
        }

        let measurements = compare(workload, &pillow, &image, measure_simd)?;
        let simd_latency = measurements
            .simd_us
            .map_or_else(|| "—".to_owned(), |value| format!("{value:.2}"));
        let simd_ratio = measurements.simd_us.map_or_else(
            || "—".to_owned(),
            |value| format!("{:.2}x", measurements.image_rs_us / value),
        );
        let simd_evidence = if measure_simd {
            format!("{} ({} blocks)", simd_proof.path, simd_proof.vector_blocks)
        } else {
            format!(
                "{} ({} blocks; no SIMD timing)",
                simd_proof.path, simd_proof.vector_blocks
            )
        };
        println!(
            "{} | {:.2} | {} | {} | {:.2} | {:.2}x | {}",
            workload.id,
            measurements.cpu_us,
            simd_latency,
            simd_evidence,
            measurements.image_rs_us,
            measurements.image_rs_us / measurements.cpu_us,
            simd_ratio
        );
    }
    Ok(())
}
