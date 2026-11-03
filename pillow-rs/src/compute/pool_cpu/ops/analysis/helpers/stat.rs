//! Native histogram reduction and scalar I/F statistics shared by exact modes.

use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) type StatOutput = Vec<Vec<f64>>;

/// Input selected by the operation-level Stat preparation.
pub(crate) enum StatRequest<'a> {
    /// The prepared Histogram child for the native Stat reducer.
    Histogram(&'a [u32]),
    /// The prepared Histogram child for Pillow's formatted masked-Stat path.
    FormattedHistogram(&'a [u32]),
    /// Native scalar I/F samples, whose Pillow scaling needs an exact path.
    Scalar,
}

/// Reduces the prepared 256-bin bands without materializing converted rasters.
pub(crate) fn histogram(request: StatRequest<'_>) -> Result<StatOutput, PilError> {
    match request {
        StatRequest::Histogram(histogram) => Ok(stat_from_byte_histogram(histogram)),
        StatRequest::FormattedHistogram(histogram) => Ok(formatted_histogram(histogram)),
        StatRequest::Scalar => Err(PilError::InternalError(
            "Stat histogram implementation received scalar input".into(),
        )),
    }
}

/// Matches the public f64-bin reducer without allocating a widened histogram.
pub(crate) fn formatted_histogram(histogram: &[u32]) -> StatOutput {
    histogram
        .chunks(256)
        .map(|band| {
            let count: f64 = band.iter().map(|&frequency| f64::from(frequency)).sum();
            let sum: f64 = band
                .iter()
                .enumerate()
                .map(|(value, &frequency)| value as f64 * f64::from(frequency))
                .sum();
            let sum2: f64 = band
                .iter()
                .enumerate()
                .map(|(value, &frequency)| {
                    let value = value as f64;
                    value * value * f64::from(frequency)
                })
                .sum();
            let mean = if count > 0.0 { sum / count } else { 0.0 };
            let rms = if count > 0.0 {
                (sum2 / count).sqrt()
            } else {
                0.0
            };
            let variance = if count > 0.0 {
                ((sum2 - (sum * sum) / count) / count).max(0.0)
            } else {
                0.0
            };
            let stddev = variance.sqrt();
            let half = (count / 2.0).floor();
            let (mut cumulative, mut median) = (0.0, 255.0);
            let (mut minimum, mut maximum) = (255.0, 0.0);
            for (value, &frequency) in band.iter().enumerate() {
                let frequency = f64::from(frequency);
                cumulative += frequency;
                if cumulative > half && median == 255.0 {
                    median = value as f64;
                }
                if frequency > 0.0 {
                    minimum = minimum.min(value as f64);
                    maximum = maximum.max(value as f64);
                }
            }
            vec![
                count, sum, sum2, mean, median, rms, variance, stddev, minimum, maximum,
            ]
        })
        .collect()
}

/// Computes Pillow's scaled I-mode statistics from native four-byte samples.
pub(crate) fn scalar_i32(image: &RgbaImage) -> Result<Vec<f64>, PilError> {
    scalar_materialized(image.as_raw(), false)
}

/// Computes Pillow's scaled F-mode statistics from native four-byte samples.
pub(crate) fn scalar_f32(image: &RgbaImage) -> Result<Vec<f64>, PilError> {
    scalar_materialized(image.as_raw(), true)
}

fn scalar_materialized(rgba: &[u8], is_f: bool) -> Result<Vec<f64>, PilError> {
    let n_pixels = rgba.len() / 4;
    if n_pixels == 0 {
        return Err(PilError::ValueError("min/max not given".into()));
    }
    let decode = |bytes: &[u8]| {
        let bytes: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if is_f {
            f32::from_le_bytes(bytes) as f64
        } else {
            i32::from_le_bytes(bytes) as f64
        }
    };
    let mut samples = rgba.chunks_exact(4);
    let first = decode(samples.next().expect("n_pixels guarantees one sample"));
    let (mut min_val, mut max_val) = (first, first);
    for bytes in samples {
        let value = decode(bytes);
        if value.partial_cmp(&min_val) == Some(std::cmp::Ordering::Less) {
            min_val = value;
        }
        if value.partial_cmp(&max_val) == Some(std::cmp::Ordering::Greater) {
            max_val = value;
        }
    }
    if (max_val - min_val).abs() < f64::EPSILON {
        return Ok(vec![0.0, 0.0, 0.0, 0.0, 255.0, 0.0, 0.0, 0.0, 255.0, 0.0]);
    }
    let histogram = scalar_histogram(rgba, is_f, min_val, max_val);
    Ok(stat_from_scalar_histogram(&histogram, n_pixels))
}

fn scalar_histogram(rgba: &[u8], is_f: bool, min_val: f64, max_val: f64) -> [i64; 256] {
    let mut histogram = [0i64; 256];
    if is_f {
        // NO_PILLOW_SUPPORT: Pillow Histo.c directly casts scaled F32 values
        // to C int, which is undefined for NaN, infinity, and out-of-range
        // values. Rust's clamp and float-to-usize cast define those samples.
        let min_f = min_val as f32;
        let max_f = max_val as f32;
        let scale = 255.0_f32 / (max_f - min_f);
        for bytes in rgba.chunks_exact(4) {
            let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let bin = ((value - min_f) * scale).clamp(0.0, 255.0) as usize;
            histogram[bin] += 1;
        }
    } else {
        // NO_PILLOW_SUPPORT: Pillow Histo.c subtracts I extrema and samples
        // in signed C arithmetic, whose overflow is undefined. This reducer
        // uses the established f64 calculation across the full I32 range.
        let scale = 255.0 / (max_val - min_val);
        for bytes in rgba.chunks_exact(4) {
            let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64;
            let bin = if value == max_val {
                255
            } else {
                ((value - min_val) * scale).clamp(0.0, 255.0) as usize
            };
            histogram[bin] += 1;
        }
    }
    histogram
}

fn stat_from_scalar_histogram(histogram: &[i64; 256], n_pixels: usize) -> Vec<f64> {
    let count = n_pixels as f64;
    let sum: f64 = histogram
        .iter()
        .enumerate()
        .map(|(value, &frequency)| value as f64 * frequency as f64)
        .sum();
    let sum2: f64 = histogram
        .iter()
        .enumerate()
        .map(|(value, &frequency)| value as f64 * value as f64 * frequency as f64)
        .sum();
    let mean = sum / count;
    let rms = (sum2 / count).sqrt();
    let variance = (sum2 - sum * sum / count) / count;
    let variance = if variance < 0.0 { 0.0 } else { variance };
    let stddev = variance.sqrt();
    let half = (count / 2.0) as i64;
    let (mut cumulative, mut median) = (0i64, 0.0);
    for (value, &frequency) in histogram.iter().enumerate() {
        cumulative += frequency;
        if cumulative > half {
            median = value as f64;
            break;
        }
    }
    let (mut minimum, mut maximum) = (255usize, 0usize);
    for (value, &frequency) in histogram.iter().enumerate() {
        if frequency > 0 {
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
    }
    vec![
        count,
        sum,
        sum2,
        mean,
        median,
        rms,
        variance,
        stddev,
        minimum as f64,
        maximum as f64,
    ]
}

fn stat_from_byte_histogram(histogram: &[u32]) -> StatOutput {
    let mut results = Vec::with_capacity(histogram.len() / 256);
    for band in histogram.chunks_exact(256) {
        let count_u64: u64 = band.iter().map(|&value| u64::from(value)).sum();
        let count = count_u64 as f64;
        if count_u64 == 0 {
            results.push(vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 255.0, 0.0]);
            continue;
        }
        let sum: f64 = band
            .iter()
            .enumerate()
            .map(|(value, &frequency)| value as f64 * frequency as f64)
            .sum();
        let sum2: f64 = band
            .iter()
            .enumerate()
            .map(|(value, &frequency)| {
                let value = value as f64;
                value * value * frequency as f64
            })
            .sum();
        let mean = sum / count;
        let rms = (sum2 / count).sqrt();
        let variance = (sum2 - sum * sum / count) / count;
        let stddev = variance.sqrt();
        let half = count_u64 / 2;
        let (mut cumulative, mut median) = (0u64, None);
        let (mut minimum, mut maximum) = (255.0, 0.0);
        for (value, &frequency) in band.iter().enumerate() {
            if frequency > 0 {
                minimum = minimum.min(value as f64);
                maximum = maximum.max(value as f64);
            }
            cumulative += u64::from(frequency);
            if median.is_none() && cumulative > half {
                median = Some(value as f64);
            }
        }
        results.push(vec![
            count,
            sum,
            sum2,
            mean,
            median.unwrap_or(255.0),
            rms,
            variance,
            stddev,
            minimum,
            maximum,
        ]);
    }
    results
}
