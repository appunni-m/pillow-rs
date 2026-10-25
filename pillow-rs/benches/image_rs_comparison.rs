//! Focused serial-CPU comparison against the `image` crate's core operations.
#![allow(unused_crate_dependencies)] // Package dependencies are used by the linked library target.

use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use image::{RgbImage, imageops};
use pillow_rs::{Backend, DynamicImage, Image};

type BenchResult<T> = Result<T, Box<dyn Error>>;
type PillowOperation = fn(&Image) -> BenchResult<DynamicImage>;
type ImageRsOperation = fn(&RgbImage) -> RgbImage;

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;
const CROP: (u32, u32, u32, u32) = (96, 48, 928, 720);
const WARMUP_CALLS: usize = 4;
const SAMPLES: usize = 21;
const CALLS_PER_SAMPLE: usize = 20;

struct Workload {
    id: &'static str,
    pillow_rs: PillowOperation,
    image_rs: ImageRsOperation,
}

const WORKLOADS: [Workload; 3] = [
    Workload {
        id: "crop-rgb-1024x768-to-832x672",
        pillow_rs: pillow_crop,
        image_rs: image_rs_crop,
    },
    Workload {
        id: "flip-left-right-rgb-1024x768",
        pillow_rs: pillow_flip_left_right,
        image_rs: image_rs_flip_left_right,
    },
    Workload {
        id: "rotate-90-rgb-1024x768",
        pillow_rs: pillow_rotate_90,
        image_rs: image_rs_rotate_90,
    },
];

fn pillow_crop(image: &Image) -> BenchResult<DynamicImage> {
    Ok(image
        .crop(Some((
            CROP.0 as i32,
            CROP.1 as i32,
            CROP.2 as i32,
            CROP.3 as i32,
        )))?
        .use_backend(Backend::Cpu)
        .materialize()?)
}

fn image_rs_crop(image: &RgbImage) -> RgbImage {
    imageops::crop_imm(image, CROP.0, CROP.1, CROP.2 - CROP.0, CROP.3 - CROP.1).to_image()
}

fn pillow_flip_left_right(image: &Image) -> BenchResult<DynamicImage> {
    Ok(image
        .transpose("FLIP_LEFT_RIGHT")?
        .use_backend(Backend::Cpu)
        .materialize()?)
}

fn image_rs_flip_left_right(image: &RgbImage) -> RgbImage {
    imageops::flip_horizontal(image)
}

fn pillow_rotate_90(image: &Image) -> BenchResult<DynamicImage> {
    Ok(image
        .transpose("ROTATE_90")?
        .use_backend(Backend::Cpu)
        .materialize()?)
}

fn image_rs_rotate_90(image: &RgbImage) -> RgbImage {
    imageops::rotate270(image)
}

fn verify_equal_output(workload: &Workload, pillow: &Image, image: &RgbImage) -> BenchResult<()> {
    let pillow_output = (workload.pillow_rs)(pillow)?;
    let image_output = (workload.image_rs)(image);
    if pillow_output.as_bytes() != image_output.as_raw() {
        return Err(std::io::Error::other(format!(
            "{} produced different RGB bytes; refusing to time unlike outputs",
            workload.id
        ))
        .into());
    }
    Ok(())
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

fn compare(workload: &Workload, pillow: &Image, image: &RgbImage) -> BenchResult<(f64, f64)> {
    for _ in 0..WARMUP_CALLS {
        black_box((workload.pillow_rs)(pillow)?);
        black_box((workload.image_rs)(image));
    }

    let mut pillow_samples = Vec::with_capacity(SAMPLES);
    let mut image_samples = Vec::with_capacity(SAMPLES);
    for sample in 0..SAMPLES {
        if sample % 2 == 0 {
            pillow_samples.push(time_batch(|| (workload.pillow_rs)(pillow))?);
            image_samples.push(time_batch(|| Ok((workload.image_rs)(image)))?);
        } else {
            image_samples.push(time_batch(|| Ok((workload.image_rs)(image)))?);
            pillow_samples.push(time_batch(|| (workload.pillow_rs)(pillow))?);
        }
    }
    Ok((median(pillow_samples), median(image_samples)))
}

fn main() -> BenchResult<()> {
    let pixels = (0..(WIDTH as usize * HEIGHT as usize))
        .flat_map(|index| [index as u8, (index >> 8) as u8, (index >> 16) as u8]);
    let input: Vec<u8> = pixels.collect();
    let pillow = Image::frombytes("RGB", (WIDTH, HEIGHT), &input)?.use_backend(Backend::Cpu);
    let image = RgbImage::from_raw(WIDTH, HEIGHT, input).ok_or_else(|| {
        std::io::Error::other("deterministic RGB input did not match its declared dimensions")
    })?;

    println!("profile: serial CPU; pillow-rs Backend::Cpu; image default features disabled");
    println!("input: RGB {WIDTH}x{HEIGHT}; deterministic bytes; construction excluded");
    println!(
        "sampling: {WARMUP_CALLS} warmups; {SAMPLES} alternating batches; {CALLS_PER_SAMPLE} calls per batch; median"
    );
    println!("case | pillow-rs CPU (µs) | image-rs (µs) | image-rs / pillow-rs");

    for workload in &WORKLOADS {
        verify_equal_output(workload, &pillow, &image)?;
        let (pillow_us, image_us) = compare(workload, &pillow, &image)?;
        println!(
            "{} | {:.2} | {:.2} | {:.2}x",
            workload.id,
            pillow_us,
            image_us,
            image_us / pillow_us
        );
    }
    Ok(())
}
