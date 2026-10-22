//! Pillow-compatible image statistics methods for [`Image`].

use crate::error::PilError;

use super::{Image, StatResult, is_l16_mode, stat_from_histogram};

impl Image {
    // ── Immediate ops (force materialize) ──

    /// Returns Pillow-compatible image statistics in structured form.
    ///
    /// Results use list variants in band order, including single-band images,
    /// matching Pillow's `ImageStat.Stat` attribute contract.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn stat_formatted(&self) -> Result<StatResult, PilError> {
        let bands = self.stat()?;
        Ok(StatResult::from_bands(&bands))
    }

    /// Computes Pillow statistics after applying an optional transparency mask.
    ///
    /// `ImageStat.Stat` is defined in terms of `Image.histogram(mask)`, not an
    /// unmasked scan followed by a validation-only mask check. Reusing the
    /// histogram path keeps the count, extrema, and all derived values
    /// restricted to the non-zero mask pixels.
    pub fn stat_formatted_with_mask(
        &self,
        mask: crate::ops::imageops::ImageOpsMask,
    ) -> Result<StatResult, PilError> {
        match mask {
            crate::ops::imageops::ImageOpsMask::None => self.stat_formatted(),
            crate::ops::imageops::ImageOpsMask::Invalid(type_name) => Err(
                PilError::AttributeError(format!("'{type_name}' object has no attribute 'load'")),
            ),
            crate::ops::imageops::ImageOpsMask::Image(mask) => {
                let histogram = self.histogram_with_mask(Some(&mask))?;
                let histogram: Vec<f64> = histogram.into_iter().map(f64::from).collect();
                Ok(stat_from_histogram(&histogram))
            }
        }
    }

    /// Computes per-band statistics.
    ///
    /// The return value is indexed by band. Each band contains:
    /// `[count, sum, sum2, mean, median, rms, var, stddev, min, max]`.
    /// Integer (`"I"`) and floating (`"F"`) modes follow Pillow's histogram
    /// fallback behavior instead of reporting raw numeric-domain statistics.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn stat(&self) -> Result<Vec<Vec<f64>>, PilError> {
        let explicit_mode = self.explicit_mode();
        let is_f = explicit_mode == Some("F");
        let is_i = explicit_mode == Some("I");

        if is_f || is_i {
            // F mode: float32, I mode: int32. Both are single-band values
            // stored as 4 RGBA bytes per pixel. PIL's Stat uses a 256-bin
            // histogram with linear scaling from [min, max] to [0, 255]:
            //   bin = (int)((value - min) * 255 / (max - min))
            // Stats are computed from bin indices, not original values.
            let img = self.materialized_shared()?;
            let rgba = img.as_bytes();
            let n_pixels = rgba.len() / 4;
            if n_pixels == 0 {
                // Pillow's I/F histogram path requires a finite min/max and
                // rejects an empty image before constructing ImageStat.Stat.
                return Err(PilError::ValueError("min/max not given".into()));
            }
            // The histogram pass only needs the extrema and then a second
            // read of each packed sample. Avoid retaining both a pixel-sized
            // value vector and its sorted clone; this path is commonly used
            // as a terminal read on large F/I images.
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
            let mut min_val = first;
            let mut max_val = first;
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
                // Pillow's I/F histogram is empty when every value is equal
                // (max == min), so its Stat min/max fall back to the histogram
                // defaults of 255 and 0 for every band. The empty histogram
                // also reports count zero and leaves the median at its
                // initialized value of 255.
                return Ok(vec![vec![
                    0.0, 0.0, 0.0, 0.0, 255.0, 0.0, 0.0, 0.0, 255.0, 0.0,
                ]]);
            }
            let mut hist = [0i64; 256];
            if is_f {
                // Pillow's F-mode histogram keeps the sample, range, and
                // reciprocal in float32. Widening those values before the
                // multiply can move an endpoint from bin 254 to 255, so keep
                // this operation order in the source sample type. In
                // particular, Pillow does not force the maximum into bin 255
                // after the multiply; its reciprocal rounding is observable.
                let min_f = min_val as f32;
                let max_f = max_val as f32;
                let scale = 255.0_f32 / (max_f - min_f);
                for bytes in rgba.chunks_exact(4) {
                    let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    let bin = ((value - min_f) * scale).clamp(0.0, 255.0) as usize;
                    hist[bin] += 1;
                }
            } else {
                let scale = 255.0 / (max_val - min_val);
                for bytes in rgba.chunks_exact(4) {
                    let value = decode(bytes);
                    // Integer samples have an exact endpoint. Preserve the
                    // existing Pillow-compatible rule that their maximum is
                    // represented by bin 255; only F-mode observes source
                    // float reciprocal rounding above.
                    let bin = if value == max_val {
                        255
                    } else {
                        ((value - min_val) * scale).clamp(0.0, 255.0) as usize
                    };
                    hist[bin] += 1;
                }
            }
            let count = n_pixels as f64;
            let sum: f64 = hist
                .iter()
                .enumerate()
                .map(|(i, &c)| i as f64 * c as f64)
                .sum();
            let sum2: f64 = hist
                .iter()
                .enumerate()
                .map(|(i, &c)| (i as f64) * (i as f64) * c as f64)
                .sum();
            let mean = sum / count;
            let rms = (sum2 / count).sqrt();
            let var = (sum2 - sum * sum / count) / count;
            let var = if var < 0.0 { 0.0 } else { var };
            let stddev = var.sqrt();
            let mut cum = 0i64;
            let half = (count / 2.0) as i64;
            let mut median = 0.0;
            for (i, &c) in hist.iter().enumerate() {
                cum += c;
                if cum > half {
                    median = i as f64;
                    break;
                }
            }
            let mut min_bin = 255usize;
            let mut max_bin = 0usize;
            for (i, &c) in hist.iter().enumerate() {
                if c > 0 {
                    min_bin = min_bin.min(i);
                    max_bin = max_bin.max(i);
                }
            }
            return Ok(vec![vec![
                count,
                sum,
                sum2,
                mean,
                median,
                rms,
                var,
                stddev,
                min_bin as f64,
                max_bin as f64,
            ]]);
        }

        let img = self.materialized_shared()?;
        let is_l16_stat = img.color() == crate::raster::ColorType::L16
            && matches!(explicit_mode, Some(mode) if is_l16_mode(mode));
        if is_l16_stat
            || matches!(
                img.color(),
                crate::raster::ColorType::L8
                    | crate::raster::ColorType::La8
                    | crate::raster::ColorType::Rgb8
                    | crate::raster::ColorType::Rgba8
            )
        {
            // Byte-oriented ImageStat is fully described by the fixed
            // 256-bin histogram. For I;16* this deliberately reuses the
            // legacy byte histogram dispatch before reducing the bins, just
            // as Pillow's ImageStat.Stat does. Reusing it avoids one Vec<u8>
            // per band and the pixel-sized sort that the old path used for
            // every read.
            let histogram = self.histogram()?;
            let mut results = Vec::with_capacity(histogram.len() / 256);
            for band in histogram.chunks_exact(256) {
                let count_u64: u64 = band.iter().map(|&value| u64::from(value)).sum();
                let count = count_u64 as f64;
                if count == 0.0 {
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
                let var = (sum2 - sum * sum / count) / count;
                let stddev = var.sqrt();
                let half = count_u64 / 2;
                let mut cumulative = 0u64;
                let mut median = None;
                let mut min: f64 = 255.0;
                let mut max: f64 = 0.0;
                for (value, &frequency) in band.iter().enumerate() {
                    if frequency > 0 {
                        min = min.min(value as f64);
                        max = max.max(value as f64);
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
                    var,
                    stddev,
                    min,
                    max,
                ]);
            }
            return Ok(results);
        }

        let n_bands = img.color().channel_count() as usize;

        // The fallback conversions below preserve the existing public mode
        // semantics, but the reduction itself only needs fixed 8-bit
        // histograms. Keep memory independent of pixel count and avoid
        // sorting a full value vector for every converted band.
        let mut histograms = vec![[0u64; 256]; n_bands];

        match n_bands {
            1 => {
                let gray = img.to_luma8();
                for px in gray.pixels() {
                    histograms[0][px[0] as usize] += 1;
                }
            }
            2 => {
                // LA mode: channel 0 = L (from R), channel 1 = A (from A)
                let rgba = img.to_rgba8();
                for px in rgba.pixels() {
                    histograms[0][px[0] as usize] += 1; // L = R
                    histograms[1][px[3] as usize] += 1; // A = A
                }
            }
            3 => {
                let rgb = img.to_rgb8();
                for px in rgb.pixels() {
                    histograms[0][px[0] as usize] += 1;
                    histograms[1][px[1] as usize] += 1;
                    histograms[2][px[2] as usize] += 1;
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                for px in rgba.pixels() {
                    for b in 0..4 {
                        histograms[b][px[b] as usize] += 1;
                    }
                }
            }
        }

        let mut results = Vec::with_capacity(n_bands);
        for histogram in &histograms {
            let count_u64: u64 = histogram.iter().sum();
            let count = count_u64 as f64;
            if count_u64 == 0 {
                // Pillow's byte-oriented histogram starts an empty band at
                // extrema (255, 0), while all aggregate statistics remain 0.
                results.push(vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 255.0, 0.0]);
                continue;
            }
            let sum: f64 = histogram
                .iter()
                .enumerate()
                .map(|(value, &frequency)| value as f64 * frequency as f64)
                .sum();
            let sum2: f64 = histogram
                .iter()
                .enumerate()
                .map(|(value, &frequency)| {
                    let value = value as f64;
                    value * value * frequency as f64
                })
                .sum();
            let mean = sum / count;
            let rms = (sum2 / count).sqrt();
            // Pillow 12.2 returns the raw floating-point expression here. It
            // intentionally preserves tiny negative round-off values (for
            // example, a 7,292,605-pixel uniform L=182 image produces
            // -4.184729342258356e-12), so do not clamp the result.
            let var = (sum2 - sum * sum / count) / count;
            let stddev = var.sqrt();
            let half = count_u64 / 2;
            let mut cumulative = 0u64;
            let mut median = None;
            let mut min = 255.0;
            let mut max = 0.0;
            for (value, &frequency) in histogram.iter().enumerate() {
                if frequency > 0 {
                    min = value as f64;
                    break;
                }
            }
            for (value, &frequency) in histogram.iter().enumerate() {
                if frequency > 0 {
                    max = value as f64;
                }
                cumulative += frequency;
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
                var,
                stddev,
                min,
                max,
            ]);
        }
        Ok(results)
    }
}
