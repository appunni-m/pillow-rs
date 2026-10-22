//! Pillow-compatible image statistics methods for [`Image`].

mod result;

pub use result::{StatResult, StatValue, stat_from_histogram, stat_from_list};

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::DynamicImage;

use super::color_count::{
    PillowColorCounts, RgbGrayscaleColorCounts, SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS,
    ScalarColorDomainCounts, count_pillow_color_scalar_samples, getcolors_rgb8_grayscale,
    getcolors_scalar_byte_domain, getcolors_scalar_i_repeated_runs, pillow_color_components,
    pillow_color_direct_table_slots, pillow_color_pixel, pillow_color_table_parameters,
    pillow_image_pixel_count, scalar_byte_domain_is_plausible, scalar_i_prefix_has_long_run,
    scalar_prefix_has_repeated_samples,
};
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
        // For multi-channel modes, use pixel-level counting
        let n_bands = match img.color() {
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let (code_mask, code_polynomial) = pillow_color_table_parameters(maxcolors)
            .ok_or_else(|| PilError::MemoryError(String::new()))?;
        // Pillow emits multiband colors in its ImagingGetColors hash-table
        // slot order, which is neither insertion nor lexicographic order.
        // Keep Pillow's probe sequence while storing only occupied slots.
        let max_distinct_colors = maxcolors as usize;
        // Large images with the usual small color limit reject after at most
        // 257 distinct colors. Reserve only that bounded working set once.
        let initial_capacity = if maxcolors <= 256 && img.as_bytes().len() >= 64 * 1024 {
            max_distinct_colors + 1
        } else {
            0
        };
        let pixel_count = pillow_image_pixel_count(img.as_ref());
        if maxcolors > 1023 && pixel_count >= 64 * 1024 {
            if let DynamicImage::ImageRgb8(rgb) = img.as_ref() {
                match getcolors_rgb8_grayscale(rgb, max_distinct_colors, code_mask, code_polynomial)
                {
                    RgbGrayscaleColorCounts::NotGrayscale => {}
                    RgbGrayscaleColorCounts::TooManyColors => return Ok(None),
                    RgbGrayscaleColorCounts::Colors(result) => return Ok(Some(result)),
                }
            }
        }
        let mut counts = PillowColorCounts::new(code_mask, initial_capacity, pixel_count);
        let mut distinct_colors = 0usize;
        let mut count_pixel = |pixel: u32| {
            counts.count(
                &mut distinct_colors,
                pixel,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            )
        };
        match (n_bands, img.as_ref()) {
            (2, crate::raster::DynamicImage::ImageLumaA8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (3, crate::raster::DynamicImage::ImageRgb8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (4, crate::raster::DynamicImage::ImageRgba8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (2, _) => {
                let la = img.to_luma_alpha8();
                for pixel in la.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (3, _) => {
                let rgb = img.to_rgb8();
                for pixel in rgb.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            (4, _) => {
                let rgba = img.to_rgba8();
                for pixel in rgba.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return Ok(None);
                    }
                }
            }
            _ => unreachable!("getcolors only supports one through four bands"),
        }
        let result = counts.into_slot_order(distinct_colors);
        Ok(Some(
            result
                .into_iter()
                .map(|(_, (pixel, count))| (count, pillow_color_components(pixel, n_bands)))
                .collect(),
        ))
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
            let (minimum, maximum) = image
                .as_bytes()
                .chunks_exact(4)
                .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
                .fold(None, |extrema: Option<(i32, i32)>, value| {
                    Some(match extrema {
                        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                        None => (value, value),
                    })
                })
                .ok_or_else(|| PilError::InternalError("I image has no scalar samples".into()))?;
            return Ok(FormattedExtrema::Integer((minimum, maximum)));
        }
        if matches!(mode.as_str(), "I;16L" | "I;16B" | "I;16N") {
            // Pillow exposes the native I;16 mode to getextrema, but the
            // explicit byte-order variants are rejected by ImagingCore with
            // the public wrong-mode error.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        if is_l16_mode(&mode) {
            let image = self.materialized_shared()?;
            let luma = image.to_luma16();
            let values = luma.as_raw().iter().copied().map(i32::from);
            let (minimum, maximum) = values
                .fold(None, |extrema: Option<(i32, i32)>, value| {
                    Some(match extrema {
                        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                        None => (value, value),
                    })
                })
                .ok_or_else(|| PilError::InternalError("I;16 image has no samples".into()))?;
            return Ok(FormattedExtrema::Integer((minimum, maximum)));
        }
        if mode == "F" {
            let image = self.materialized_shared()?;
            let (minimum, maximum) = image
                .as_bytes()
                .chunks_exact(4)
                .map(|sample| {
                    f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) as f64
                })
                .fold(None, |extrema: Option<(f64, f64)>, value| {
                    Some(match extrema {
                        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                        None => (value, value),
                    })
                })
                .ok_or_else(|| PilError::InternalError("F image has no scalar samples".into()))?;
            return Ok(FormattedExtrema::Float((minimum, maximum)));
        }

        let extrema = self.getextrema()?;
        if extrema.len() == 1 {
            Ok(FormattedExtrema::Single(extrema[0]))
        } else {
            Ok(FormattedExtrema::Multiple(extrema))
        }
    }

    /// Histogram-based getcolors for 1, L, P modes.
    /// Matches PIL's Python-level implementation:
    ///   h = self.im.histogram()
    ///   out = [(h[i], i) for i in range(256) if h[i]]
    fn getcolors_histogram(
        maxcolors: u32,
        img: &DynamicImage,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        // Compute a 256-bin histogram.  All Rust-backed single-band modes,
        // including P and 1, expose their samples through the luma view after
        // materialization, so a second color-type arm only added an unreachable
        // coverage region without changing the result.
        let mut hist = [0u32; 256];
        let luma = img.to_luma8();
        let max_distinct_colors = maxcolors as usize;
        let mut distinct_colors = 0usize;
        for p in luma.pixels() {
            let count = &mut hist[p[0] as usize];
            if *count == 0 {
                distinct_colors += 1;
                if distinct_colors > max_distinct_colors {
                    return Ok(None);
                }
            }
            *count += 1;
        }
        // Build result: [(count, pixel_value)] in pixel value ascending order
        let result: Vec<(u32, FormattedPixelValue)> = (0..=255u8)
            .filter(|&i| hist[i as usize] > 0)
            .map(|i| (hist[i as usize], FormattedPixelValue::Scalar(i)))
            .collect();
        if result.len() > maxcolors as usize {
            return Ok(None);
        }
        Ok(Some(result))
    }

    /// Counts Pillow's scalar `I` and `F` modes without converting their
    /// four-byte samples through the byte-oriented RGBA view.
    fn getcolors_scalar(
        maxcolors: u32,
        mode: &str,
        img: &DynamicImage,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        let (code_mask, code_polynomial) = pillow_color_table_parameters(maxcolors)
            .ok_or_else(|| PilError::MemoryError(String::new()))?;
        let max_distinct_colors = maxcolors as usize;
        let bytes = img.as_bytes();
        let pixel_count = pillow_image_pixel_count(img);
        let has_repeated_prefix = if mode == "I" {
            scalar_i_prefix_has_long_run(bytes)
        } else {
            scalar_prefix_has_repeated_samples(bytes)
        };
        let direct_table_slots = pillow_color_direct_table_slots(code_mask, pixel_count);
        if mode == "I"
            && pixel_count >= 4096
            && has_repeated_prefix
            && direct_table_slots
                .is_some_and(|slots| slots > SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS)
            && scalar_byte_domain_is_plausible("I", bytes)
        {
            match getcolors_scalar_i_repeated_runs(
                bytes,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            ) {
                ScalarColorDomainCounts::NotApplicable => {}
                ScalarColorDomainCounts::TooManyColors => return Ok(None),
                ScalarColorDomainCounts::Colors(colors) => return Ok(Some(colors)),
            }
        }
        if pixel_count >= 4096
            && !has_repeated_prefix
            && scalar_byte_domain_is_plausible(mode, bytes)
        {
            match getcolors_scalar_byte_domain(
                mode,
                bytes,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            ) {
                ScalarColorDomainCounts::NotApplicable => {}
                ScalarColorDomainCounts::TooManyColors => return Ok(None),
                ScalarColorDomainCounts::Colors(colors) => return Ok(Some(colors)),
            }
        }
        let initial_capacity = if maxcolors <= 256 && bytes.len() >= 64 * 1024 {
            max_distinct_colors + 1
        } else {
            0
        };
        let mut counts = PillowColorCounts::new(code_mask, initial_capacity, pixel_count);
        let mut distinct_colors = 0usize;
        if has_repeated_prefix {
            let mut samples = bytes.chunks_exact(4).peekable();
            while let Some(sample) = samples.next() {
                let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
                let mut repetitions = 1usize;
                while let Some(next) = samples.peek() {
                    if *next != sample {
                        break;
                    }
                    samples.next();
                    repetitions += 1;
                }

                if repetitions == 1 {
                    if !counts.count(
                        &mut distinct_colors,
                        pixel,
                        max_distinct_colors,
                        code_mask,
                        code_polynomial,
                    ) {
                        return Ok(None);
                    }
                    continue;
                }

                match counts.count_repeated(
                    &mut distinct_colors,
                    pixel,
                    repetitions,
                    max_distinct_colors,
                    code_mask,
                    code_polynomial,
                ) {
                    Some(true) => {}
                    Some(false) => return Ok(None),
                    None => {
                        for _ in 0..repetitions {
                            if !counts.count(
                                &mut distinct_colors,
                                pixel,
                                max_distinct_colors,
                                code_mask,
                                code_polynomial,
                            ) {
                                return Ok(None);
                            }
                        }
                    }
                }
            }
        } else {
            if !count_pillow_color_scalar_samples(
                &mut counts,
                &mut distinct_colors,
                bytes,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            ) {
                return Ok(None);
            }
        }
        let result = counts.into_slot_order(distinct_colors);
        Ok(Some(
            result
                .into_iter()
                .map(|(_, (pixel, count))| {
                    let value = if mode == "I" {
                        FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes()))
                    } else {
                        FormattedPixelValue::Float(f32::from_bits(pixel) as f64)
                    };
                    (count, value)
                })
                .collect(),
        ))
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
