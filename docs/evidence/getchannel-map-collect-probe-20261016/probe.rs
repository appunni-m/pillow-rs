use std::hint::black_box;
use std::time::Instant;

type Kernel = fn(&[u8], usize, usize) -> Vec<u8>;

fn current_grouped(source: &[u8], pixels: usize, index: usize) -> Vec<u8> {
    let channel = index.min(3);
    if channel >= 3 {
        return vec![u8::MAX; pixels];
    }
    let mut output = Vec::with_capacity(pixels);
    for quad in source.chunks_exact(12).take(pixels / 4) {
        output.extend_from_slice(&[
            quad[channel],
            quad[channel + 3],
            quad[channel + 6],
            quad[channel + 9],
        ]);
    }
    let tail_start = (pixels / 4) * 12;
    if let Some(tail) = source.get(tail_start..) {
        for pixel in tail.chunks_exact(3).take(pixels % 4) {
            output.push(pixel[channel]);
        }
    }
    output
}

fn chunks_collect(source: &[u8], pixels: usize, index: usize) -> Vec<u8> {
    let channel = index.min(3);
    if channel >= 3 {
        return vec![u8::MAX; pixels];
    }
    source
        .chunks_exact(3)
        .take(pixels)
        .map(|pixel| pixel[channel])
        .collect()
}

fn check_all() {
    for width in 0..=17 {
        for height in 0..=7 {
            let pixels = width * height;
            let source: Vec<u8> = (0..pixels * 3 + 11)
                .map(|i| ((i * 73 + 19) % 256) as u8)
                .collect();
            for channel in 0..=4 {
                assert_eq!(
                    chunks_collect(&source, pixels, channel),
                    current_grouped(&source, pixels, channel),
                    "width={width}, height={height}, channel={channel}"
                );
            }
        }
    }
    let pixels = 1024 * 768;
    let source: Vec<u8> = (0..pixels * 3)
        .map(|i| ((i * 73 + 19) % 256) as u8)
        .collect();
    for channel in 0..=3 {
        assert_eq!(
            chunks_collect(&source, pixels, channel),
            current_grouped(&source, pixels, channel),
            "large image channel={channel}"
        );
    }
}

fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn main() {
    check_all();
    let pixels = 1024 * 768;
    let source: Vec<u8> = (0..pixels * 3)
        .map(|i| ((i * 73 + 19) % 256) as u8)
        .collect();
    let kernels: [(&str, Kernel); 2] = [
        ("current_grouped", current_grouped),
        ("chunks_map_collect", chunks_collect),
    ];
    const WARMUPS: usize = 4;
    const SAMPLES: usize = 9;
    const ITERATIONS: usize = 20;
    let mut values = [[[0u64; SAMPLES]; 3]; 2];

    for (_, kernel) in kernels {
        for _ in 0..WARMUPS {
            for channel in 0..3 {
                black_box(kernel(black_box(&source), pixels, channel));
            }
        }
    }

    for sample in 0..SAMPLES {
        let order = if sample % 2 == 0 { [0, 1] } else { [1, 0] };
        for kernel_idx in order {
            let kernel = kernels[kernel_idx].1;
            for channel in 0..3 {
                let started = Instant::now();
                let mut checksum = 0u8;
                for _ in 0..ITERATIONS {
                    let output = kernel(black_box(&source), pixels, channel);
                    checksum ^= output[pixels / 2];
                    black_box(output);
                }
                black_box(checksum);
                values[kernel_idx][channel][sample] =
                    started.elapsed().as_nanos() as u64 / ITERATIONS as u64;
            }
        }
    }

    println!(
        "host={} pixels={} input_bytes={} allocation_and_drop_included=true unit=ns_per_channel_call warmups={WARMUPS} samples={SAMPLES} iterations={ITERATIONS}",
        std::env::consts::ARCH,
        pixels,
        source.len()
    );
    for (kernel_idx, (name, _)) in kernels.iter().enumerate() {
        for channel in 0..3 {
            let median = median(&mut values[kernel_idx][channel]);
            println!(
                "{name} channel={channel} median={median} samples={:?}",
                values[kernel_idx][channel]
            );
        }
    }
}
