//! Public statistics result values and standalone summary helpers.

/// PIL-compatible statistics result. Every statistic is a per-band list.
#[derive(Debug, Clone)]
pub struct StatResult {
    /// Number of pixels contributing to each band.
    pub count: StatValue,
    /// Sum of channel values.
    pub sum: StatValue,
    /// Sum of squared channel values.
    pub sum2: StatValue,
    /// Mean channel value.
    pub mean: StatValue,
    /// Median channel value.
    pub median: StatValue,
    /// Root-mean-square channel value.
    pub rms: StatValue,
    /// Variance per channel.
    pub var: StatValue,
    /// Standard deviation per channel.
    pub stddev: StatValue,
    /// Minimum and maximum value per channel.
    pub extrema: StatValue,
}

/// Scalar or per-band statistic value.
#[derive(Debug, Clone)]
pub enum StatValue {
    /// Single integer statistic.
    Int(i64),
    /// Single floating-point statistic.
    Float(f64),
    /// Per-band integer statistics.
    IntList(Vec<i64>),
    /// Per-band floating-point statistics.
    FloatList(Vec<f64>),
    /// Single-band extrema pair.
    ExtremaSingle((i64, i64)),
    /// Per-band extrema pairs.
    ExtremaList(Vec<(i64, i64)>),
}

impl StatResult {
    pub(super) fn from_bands(bands: &[Vec<f64>]) -> Self {
        let fi = |idx: usize| -> StatValue {
            StatValue::IntList(bands.iter().map(|b| b[idx] as i64).collect())
        };
        let ff = |idx: usize| -> StatValue {
            StatValue::FloatList(bands.iter().map(|b| b[idx]).collect())
        };
        let extrema = |min_idx, max_idx| -> StatValue {
            // Always use list format for extrema: [[min, max]] for single, [[min,max], ...] for multi
            StatValue::ExtremaList(
                bands
                    .iter()
                    .map(|b| (b[min_idx] as i64, b[max_idx] as i64))
                    .collect(),
            )
        };
        StatResult {
            count: fi(0),
            sum: ff(1),
            sum2: ff(2),
            mean: ff(3),
            median: fi(4),
            rms: ff(5),
            var: ff(6),
            stddev: ff(7),
            extrema: extrema(8, 9),
        }
    }
}

/// Computes Pillow's integer-valued ImageStat path from histogram counts.
///
/// Pillow 13 accumulates integer histogram bins into integer `count`, `sum`,
/// and `sum2` values. Keeping the sum integer until after squaring preserves
/// the exact quotient rounding used by its variance formula.
// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
pub fn stat_from_integer_histogram(data: &[u64]) -> StatResult {
    let mut count_values = Vec::new();
    let mut sum_values = Vec::new();
    let mut sum2_values = Vec::new();
    let mut means = Vec::new();
    let mut medians = Vec::new();
    let mut rms_values = Vec::new();
    let mut variances = Vec::new();
    let mut stddev_values = Vec::new();
    let mut extrema_values = Vec::new();

    for histogram in data.chunks_exact(256) {
        let (count, sum, sum2, median, minimum, maximum) = integer_histogram_moments(histogram);
        let mean = (count > 0)
            .then(|| integer_ratio_to_f64(sum, count))
            .unwrap_or(0.0);
        let rms = (count > 0)
            .then(|| integer_ratio_to_f64(sum2, count).sqrt())
            .unwrap_or(0.0);
        let variance = if count > 0 {
            let square = sum.checked_mul(sum).unwrap_or(u128::MAX);
            ((sum2 as f64 - integer_ratio_to_f64(square, count)) / count as f64).max(0.0)
        } else {
            0.0
        };

        count_values.push(count as i64);
        sum_values.push(sum as i64);
        sum2_values.push(sum2 as i64);
        means.push(mean);
        medians.push(median as i64);
        rms_values.push(rms);
        variances.push(variance);
        stddev_values.push(variance.sqrt());
        extrema_values.push((minimum as i64, maximum as i64));
    }

    StatResult {
        count: StatValue::IntList(count_values),
        sum: StatValue::IntList(sum_values),
        sum2: StatValue::IntList(sum2_values),
        mean: StatValue::FloatList(means),
        median: StatValue::IntList(medians),
        rms: StatValue::FloatList(rms_values),
        var: StatValue::FloatList(variances),
        stddev: StatValue::FloatList(stddev_values),
        extrema: StatValue::ExtremaList(extrema_values),
    }
}

fn integer_histogram_moments(histogram: &[u64]) -> (u128, u128, u128, usize, usize, usize) {
    let count = histogram.iter().map(|&value| u128::from(value)).sum();
    let mut sum = 0u128;
    let mut sum2 = 0u128;
    let mut cumulative = 0u128;
    let mut median = 255;
    let mut minimum = 255;
    let mut maximum = 0;
    for (index, &frequency) in histogram.iter().enumerate() {
        let frequency = u128::from(frequency);
        sum += index as u128 * frequency;
        sum2 += (index * index) as u128 * frequency;
        cumulative += frequency;
        if median == 255 && cumulative > count / 2 {
            median = index;
        }
        if frequency > 0 {
            minimum = minimum.min(index);
            maximum = maximum.max(index);
        }
    }
    (count, sum, sum2, median, minimum, maximum)
}

fn integer_ratio_to_f64(numerator: u128, denominator: u128) -> f64 {
    if numerator == 0 || denominator == 0 {
        return 0.0;
    }
    let mut exponent = (u128::BITS - numerator.leading_zeros()) as i32
        - (u128::BITS - denominator.leading_zeros()) as i32;
    let ratio_below_power = if exponent >= 0 {
        numerator < (denominator << exponent as u32)
    } else {
        (numerator << (-exponent) as u32) < denominator
    };
    if ratio_below_power {
        exponent -= 1;
    }

    let shift = 52 - exponent;
    let (scaled_numerator, scaled_denominator) = if shift >= 0 {
        let Some(value) = numerator.checked_shl(shift as u32) else {
            return numerator as f64 / denominator as f64;
        };
        (value, denominator)
    } else {
        let Some(value) = denominator.checked_shl((-shift) as u32) else {
            return numerator as f64 / denominator as f64;
        };
        (numerator, value)
    };
    let mut significand = scaled_numerator / scaled_denominator;
    let remainder = scaled_numerator % scaled_denominator;
    let complement = scaled_denominator - remainder;
    if remainder > complement || (remainder == complement && significand & 1 != 0) {
        significand += 1;
    }
    if significand == (1u128 << 53) {
        significand >>= 1;
        exponent += 1;
    }
    let exponent_bits = (exponent + 1023) as u64;
    let fraction_bits = (significand - (1u128 << 52)) as u64;
    f64::from_bits((exponent_bits << 52) | fraction_bits)
}

/// Computes Pillow `ImageStat.Stat` values from a precomputed histogram.
///
/// A histogram is partitioned into 256-bin bands. Each returned band contains
/// `[count, sum, sum2, mean, median, rms, var, stddev, min, max]`, matching the
/// representation used by [`crate::image::Image::stat`]. An incomplete final
/// band is ignored.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub fn stat_from_histogram(data: &[f64]) -> StatResult {
    let bands: Vec<Vec<f64>> = data
        // Pillow creates one band per complete 256-entry histogram segment.
        .chunks_exact(256)
        .map(|histogram| {
            let count: f64 = histogram.iter().sum();
            let sum: f64 = histogram
                .iter()
                .enumerate()
                .map(|(index, &frequency)| index as f64 * frequency)
                .sum();
            let sum2: f64 = histogram
                .iter()
                .enumerate()
                .map(|(index, &frequency)| {
                    let value = index as f64;
                    value * value * frequency
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

            let mut cumulative = 0.0;
            let half = (count / 2.0).floor();
            let mut median: f64 = 255.0;
            let mut min_bin: f64 = 255.0;
            let mut max_bin: f64 = 0.0;
            for (index, &frequency) in histogram.iter().enumerate() {
                cumulative += frequency;
                if cumulative > half && median == 255.0 {
                    median = index as f64;
                }
                if frequency > 0.0 {
                    min_bin = min_bin.min(index as f64);
                    max_bin = max_bin.max(index as f64);
                }
            }

            vec![
                count, sum, sum2, mean, median, rms, variance, stddev, min_bin, max_bin,
            ]
        })
        .collect();
    StatResult::from_bands(&bands)
}

/// Computes basic statistics for non-image numeric input.
///
/// This is the Pillow `ImageStat` fallback for plain lists. The returned tuple
/// is `(count, sum, mean, min, max)`.
pub fn stat_from_list(data: &[f64]) -> (f64, f64, f64, f64, f64) {
    let count = data.len() as f64;
    let sum: f64 = data.iter().sum();
    let mean = if count > 0.0 { sum / count } else { 0.0 };
    let min_val = if count > 0.0 {
        data.iter().cloned().fold(f64::MAX, f64::min)
    } else {
        0.0
    };
    let max_val = if count > 0.0 {
        data.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    } else {
        0.0
    };
    (count, sum, mean, min_val, max_val)
}
