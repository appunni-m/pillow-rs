use std::hint::black_box;
use std::time::Instant;

fn current(source: &[u8], pixels: usize, channel: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(pixels);
    for quad in source.chunks_exact(12).take(pixels / 4) {
        output.extend_from_slice(&[
            quad[channel], quad[channel + 3], quad[channel + 6], quad[channel + 9],
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

fn unrolled_push(source: &[u8], pixels: usize, channel: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(pixels);
    for quad in source.chunks_exact(12).take(pixels / 4) {
        output.push(quad[channel]);
        output.push(quad[channel + 3]);
        output.push(quad[channel + 6]);
        output.push(quad[channel + 9]);
    }
    let tail_start = (pixels / 4) * 12;
    if let Some(tail) = source.get(tail_start..) {
        for pixel in tail.chunks_exact(3).take(pixels % 4) {
            output.push(pixel[channel]);
        }
    }
    output
}

fn pixel_chunks(source: &[u8], pixels: usize, channel: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(pixels);
    for pixel in source.chunks_exact(3).take(pixels) {
        output.push(pixel[channel]);
    }
    output
}

fn stride_iter(source: &[u8], pixels: usize, channel: usize) -> Vec<u8> {
    source
        .iter()
        .skip(channel)
        .step_by(3)
        .take(pixels)
        .copied()
        .collect()
}

fn check_all() {
    for pixels in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 1023, 1024] {
        let mut source: Vec<u8> = (0..pixels * 3 + 5)
            .map(|i| ((i * 73 + 19) % 256) as u8)
            .collect();
        if source.is_empty() {
            source.push(0);
        }
        for channel in 0..3 {
            let expected = current(&source, pixels, channel);
            for candidate in [unrolled_push as fn(&[u8], usize, usize) -> Vec<u8>, pixel_chunks, stride_iter] {
                assert_eq!(candidate(&source, pixels, channel), expected, "pixels={pixels}, channel={channel}");
            }
        }
    }
}

fn main() {
    check_all();
    let pixels = 1024 * 768;
    let mut source: Vec<u8> = (0..pixels * 3)
        .map(|i| ((i * 73 + 19) % 256) as u8)
        .collect();
    black_box(&mut source);
    let candidates: [(&str, fn(&[u8], usize, usize) -> Vec<u8>); 4] = [
        ("current_extend_array", current),
        ("unrolled_push", unrolled_push),
        ("pixel_chunks", pixel_chunks),
        ("stride_iter_collect", stride_iter),
    ];
    println!("host={} pixels={} input_bytes={} allocation_included=true unit=us_per_channel_call", std::env::consts::ARCH, pixels, source.len());
    for (name, kernel) in candidates {
        for _ in 0..5 {
            for channel in 0..3 {
                black_box(kernel(black_box(&source), pixels, channel));
            }
        }
        let mut samples = Vec::with_capacity(5);
        for _ in 0..5 {
            let started = Instant::now();
            let mut checksum = 0u8;
            for _ in 0..20 {
                for channel in 0..3 {
                    let output = kernel(black_box(&source), pixels, channel);
                    checksum ^= output[pixels / 2];
                    black_box(output);
                }
            }
            black_box(checksum);
            samples.push(started.elapsed().as_nanos() as u64 / 60 / 1000);
        }
        samples.sort_unstable();
        println!("{name}: median={} max_of_5={} samples={samples:?}", samples[samples.len()/2], samples[4]);
    }
}
