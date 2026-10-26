//! Pillow-compatible image statistics methods for [`Image`].

mod colors;
mod extrema;
mod result;

pub use result::{StatResult, StatValue, stat_from_histogram, stat_from_list};

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::DynamicImage;

use super::{
    FormattedExtrema, FormattedPixelValue, Image, ScalarImageSamples, decode_float_samples,
    decode_integer_samples, is_l16_mode, pillow_band_count,
};

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
            let img = self.materialized_shared()?;
            return Ok(vec![Self::stat_scalar_materialized(img.as_ref(), is_f)?]);
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
            return Ok(Self::stat_from_byte_histogram(&histogram));
        }

        let n_bands = img.color().channel_count() as usize;
        // Preserve public mode conversion semantics while keeping histogram
        // storage independent of image dimensions.
        let histograms = Self::stat_fallback_histograms(img.as_ref(), n_bands);
        Ok(Self::stat_from_u64_histograms(&histograms))
    }

    fn stat_scalar_materialized(img: &DynamicImage, is_f: bool) -> Result<Vec<f64>, PilError> {
        // F mode: float32, I mode: int32. Both are single-band values stored
        // as 4 RGBA bytes per pixel. Pillow's Stat scales a 256-bin histogram
        // linearly from [min, max] to [0, 255] and computes stats from bins.
        let rgba = img.as_bytes();
        let n_pixels = rgba.len() / 4;
        if n_pixels == 0 {
            // Pillow's I/F histogram path requires a finite min/max and rejects
            // an empty image before constructing ImageStat.Stat.
            return Err(PilError::ValueError("min/max not given".into()));
        }
        // Avoid retaining a pixel-sized value vector and its sorted clone.
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
            // Pillow's I/F histogram is empty for a constant image. Its Stat
            // defaults count and aggregate values to zero, median/min to 255,
            // and max to zero.
            return Ok(vec![0.0, 0.0, 0.0, 0.0, 255.0, 0.0, 0.0, 0.0, 255.0, 0.0]);
        }
        let histogram = Self::stat_scalar_histogram(rgba, is_f, min_val, max_val);
        Ok(Self::stat_from_scalar_histogram(&histogram, n_pixels))
    }

    fn stat_scalar_histogram(rgba: &[u8], is_f: bool, min_val: f64, max_val: f64) -> [i64; 256] {
        let mut hist = [0i64; 256];
        if is_f {
            // Keep samples, range, and reciprocal in float32. Widening before
            // multiply can move an endpoint from bin 254 to 255. Pillow does
            // not force the maximum to bin 255 after reciprocal rounding.
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
            let decode =
                |bytes: &[u8]| i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f64;
            for bytes in rgba.chunks_exact(4) {
                let value = decode(bytes);
                // Integer samples have an exact endpoint; only F mode
                // observes source float reciprocal rounding above.
                let bin = if value == max_val {
                    255
                } else {
                    ((value - min_val) * scale).clamp(0.0, 255.0) as usize
                };
                hist[bin] += 1;
            }
        }
        hist
    }

    fn stat_from_scalar_histogram(hist: &[i64; 256], n_pixels: usize) -> Vec<f64> {
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
        let mut cumulative = 0i64;
        let half = (count / 2.0) as i64;
        let mut median = 0.0;
        for (i, &frequency) in hist.iter().enumerate() {
            cumulative += frequency;
            if cumulative > half {
                median = i as f64;
                break;
            }
        }
        let mut min_bin = 255usize;
        let mut max_bin = 0usize;
        for (i, &frequency) in hist.iter().enumerate() {
            if frequency > 0 {
                min_bin = min_bin.min(i);
                max_bin = max_bin.max(i);
            }
        }
        vec![
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
        ]
    }

    fn stat_from_byte_histogram(histogram: &[u32]) -> Vec<Vec<f64>> {
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
        results
    }

    fn stat_fallback_histograms(img: &DynamicImage, n_bands: usize) -> Vec<[u64; 256]> {
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
                    for band in 0..4 {
                        histograms[band][px[band] as usize] += 1;
                    }
                }
            }
        }
        histograms
    }

    fn stat_from_u64_histograms(histograms: &[[u64; 256]]) -> Vec<Vec<f64>> {
        let mut results = Vec::with_capacity(histograms.len());
        for histogram in histograms {
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
        results
    }

    /// Returns unique colors and their counts.
    ///
    /// The result is `None` when the image contains more than `maxcolors`
    /// distinct colors. Each color is returned in the scalar or tuple shape
    /// matching the image mode. `I` values retain signed 32-bit precision,
    /// `I;16*` values follow Pillow's wrong-mode error contract, and `F` values
    /// retain their 32-bit floating-point value.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection or materialization fails.
    #[allow(clippy::type_complexity)]
    pub fn getcolors(
        &self,
        maxcolors: u32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        // GetColors over scalar samples is an eager host-CPU scan. Emit a
        // selected-step receipt only when the pixels were already materialized:
        // a first-time pipeline materialization has its own backend receipt and
        // must not be overwritten by this terminal CPU scan.
        let eager_receipt_eligible =
            crate::compute::Backend::pipeline_telemetry_enabled() && self.is_materialized();
        let mode = self.mode()?;
        if is_l16_mode(&mode) {
            // Pillow's ImagingCore rejects unsigned 16-bit luma modes here;
            // this is the public mode contract, independent of TIFF decoding.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let img = self.materialized_shared()?;
        if mode == "I" || mode == "F" {
            let telemetry = if eager_receipt_eligible {
                match crate::compute::preferred_active_backend() {
                    Ok(selected) => Some(crate::compute::begin_eager_cpu_operation_telemetry(
                        "GetColors",
                        selected.map(|(backend, _)| backend),
                    )),
                    Err(_) => None,
                }
            } else {
                None
            };
            let result = Self::getcolors_scalar(maxcolors, &mode, &img);
            if let Some(telemetry) = telemetry {
                if result.is_ok() {
                    crate::compute::finish_eager_cpu_operation_telemetry(telemetry, 0);
                } else {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                }
            }
            return result;
        }
        // For 1, L, P modes, PIL uses histogram (pixel value ascending)
        if mode == "1" || mode == "L" || mode == "P" {
            return Self::getcolors_histogram(maxcolors, &img);
        }
        Self::getcolors_multiband(maxcolors, img.as_ref())
    }

    /// Applies Pillow's signed `maxcolors` argument semantics for bindings.
    /// Negative limits still load and validate the image, then return `None`.
    pub fn getcolors_with_signed_limit(
        &self,
        maxcolors: i32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        if maxcolors >= 0 {
            return self.getcolors(maxcolors as u32);
        }
        let mode = self.mode()?;
        if is_l16_mode(&mode) {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        self.materialized_shared()?;
        Ok(None)
    }

    /// Returns `getcolors` with Pillow's scalar/tuple color shape applied.
    ///
    /// Color counting and mode-specific truncation remain in the core; the
    /// binding is responsible only for constructing the host list and tuple
    /// objects.
    pub fn getcolors_formatted(
        &self,
        maxcolors: u32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors(maxcolors)
    }

    /// Returns formatted colors using Pillow's signed `maxcolors` limit.
    pub fn getcolors_formatted_with_signed_limit(
        &self,
        maxcolors: i32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors_with_signed_limit(maxcolors)
    }

    /// Returns extrema using Pillow's one-pair versus per-band result shape.
    pub fn getextrema_formatted(&self) -> Result<FormattedExtrema, PilError> {
        let (width, height) = self.size()?;
        if width == 0 || height == 0 {
            let bands = pillow_band_count(&self.mode()?);
            return Ok(if bands == 1 {
                FormattedExtrema::Empty
            } else {
                FormattedExtrema::EmptyMultiple(bands)
            });
        }

        let mode = self.mode()?;
        if mode == "I" {
            let image = self.materialized_shared()?;
            return extrema::integer(image.as_ref());
        }
        if matches!(mode.as_str(), "I;16L" | "I;16B" | "I;16N") {
            // Pillow exposes the native I;16 mode to getextrema, but the
            // explicit byte-order variants are rejected by ImagingCore with
            // the public wrong-mode error.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        if is_l16_mode(&mode) {
            let image = self.materialized_shared()?;
            return extrema::luma16(image.as_ref());
        }
        if mode == "F" {
            let image = self.materialized_shared()?;
            return extrema::float(image.as_ref());
        }

        let extrema = self.getextrema()?;
        if extrema.len() == 1 {
            Ok(FormattedExtrema::Single(extrema[0]))
        } else {
            Ok(FormattedExtrema::Multiple(extrema))
        }
    }

    /// Reads the retained four-byte scalar storage for `I` and `F` modes.
    pub(crate) fn scalar_samples(&self, mode: &str) -> Result<ScalarImageSamples, PilError> {
        if mode == "I" {
            return self
                .read_scalar_storage(mode, decode_integer_samples)
                .map(ScalarImageSamples::Integer);
        }
        self.read_scalar_storage(mode, decode_float_samples)
            .map(ScalarImageSamples::Float)
    }

    fn read_scalar_storage<T>(
        &self,
        mode: &str,
        decode: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, PilError> {
        let img = self.materialized_shared()?;
        let expected = CheckedDims::new(img.width(), img.height(), 4)?.total_bytes();
        let raw = img.as_bytes();
        if raw.len() != expected {
            return Err(PilError::InternalError(format!(
                "{mode} image has invalid scalar storage"
            )));
        }
        Ok(decode(raw))
    }
}
