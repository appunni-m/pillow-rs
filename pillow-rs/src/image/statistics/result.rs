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

/// Computes Pillow `ImageStat.Stat` values from a precomputed histogram.
///
/// A histogram is partitioned into 256-bin bands. Each returned band contains
/// `[count, sum, sum2, mean, median, rms, var, stddev, min, max]`, matching the
/// representation used by [`crate::image::Image::stat`].
pub fn stat_from_histogram(data: &[f64]) -> StatResult {
    let bands: Vec<Vec<f64>> = data
        .chunks(256)
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
