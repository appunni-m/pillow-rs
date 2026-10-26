//! Color counting helpers for Pillow-compatible image statistics.

use super::super::color_count::{
    PillowColorCounts, RgbGrayscaleColorCounts, SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS,
    ScalarColorDomainCounts, count_pillow_color_scalar_samples, getcolors_rgb8_grayscale,
    getcolors_scalar_byte_domain, getcolors_scalar_i_repeated_runs, pillow_color_components,
    pillow_color_direct_table_slots, pillow_color_pixel, pillow_color_table_parameters,
    pillow_image_pixel_count, scalar_byte_domain_is_plausible, scalar_i_prefix_has_long_run,
    scalar_prefix_has_repeated_samples,
};
use super::super::{FormattedPixelValue, Image};
use crate::error::PilError;
use crate::raster::DynamicImage;

impl Image {
    #[allow(clippy::type_complexity)]
    pub(super) fn getcolors_multiband(
        maxcolors: u32,
        img: &DynamicImage,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
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
        let pixel_count = pillow_image_pixel_count(img);
        if maxcolors > 1023
            && pixel_count >= 64 * 1024
            && let DynamicImage::ImageRgb8(rgb) = img
        {
            match getcolors_rgb8_grayscale(rgb, max_distinct_colors, code_mask, code_polynomial) {
                RgbGrayscaleColorCounts::NotGrayscale => {}
                RgbGrayscaleColorCounts::TooManyColors => return Ok(None),
                RgbGrayscaleColorCounts::Colors(result) => return Ok(Some(result)),
            }
        }
        let mut counts = PillowColorCounts::new(code_mask, initial_capacity, pixel_count);
        let mut distinct_colors = 0usize;
        let counted = Self::count_multiband_pixels(n_bands, img, |pixel| {
            counts.count(
                &mut distinct_colors,
                pixel,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            )
        });
        if !counted {
            return Ok(None);
        }
        let result = counts.into_slot_order(distinct_colors);
        Ok(Some(
            result
                .into_iter()
                .map(|(_, (pixel, count))| (count, pillow_color_components(pixel, n_bands)))
                .collect(),
        ))
    }

    #[inline(always)]
    fn count_multiband_pixels(
        n_bands: usize,
        img: &DynamicImage,
        mut count_pixel: impl FnMut(u32) -> bool,
    ) -> bool {
        match (n_bands, img) {
            (2, crate::raster::DynamicImage::ImageLumaA8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return false;
                    }
                }
            }
            (3, crate::raster::DynamicImage::ImageRgb8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return false;
                    }
                }
            }
            (4, crate::raster::DynamicImage::ImageRgba8(image)) => {
                for pixel in image.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return false;
                    }
                }
            }
            (2, _) => {
                let la = img.to_luma_alpha8();
                for pixel in la.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return false;
                    }
                }
            }
            (3, _) => {
                let rgb = img.to_rgb8();
                for pixel in rgb.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return false;
                    }
                }
            }
            (4, _) => {
                let rgba = img.to_rgba8();
                for pixel in rgba.pixels() {
                    if !count_pixel(pillow_color_pixel(&pixel.0)) {
                        return false;
                    }
                }
            }
            _ => unreachable!("getcolors only supports one through four bands"),
        }
        true
    }

    /// Histogram-based getcolors for 1, L, P modes.
    /// Matches PIL's Python-level implementation:
    ///   h = self.im.histogram()
    ///   out = [(h[i], i) for i in range(256) if h[i]]
    pub(super) fn getcolors_histogram(
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
    pub(super) fn getcolors_scalar(
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
        match Self::scalar_color_domain_counts(
            mode,
            bytes,
            max_distinct_colors,
            code_mask,
            code_polynomial,
            pixel_count,
            has_repeated_prefix,
        ) {
            ScalarColorDomainCounts::NotApplicable => {}
            ScalarColorDomainCounts::TooManyColors => return Ok(None),
            ScalarColorDomainCounts::Colors(colors) => return Ok(Some(colors)),
        }
        let initial_capacity = if maxcolors <= 256 && bytes.len() >= 64 * 1024 {
            max_distinct_colors + 1
        } else {
            0
        };
        let mut counts = PillowColorCounts::new(code_mask, initial_capacity, pixel_count);
        let mut distinct_colors = 0usize;
        if has_repeated_prefix {
            if !Self::count_repeated_scalar_samples(
                &mut counts,
                &mut distinct_colors,
                bytes,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            ) {
                return Ok(None);
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
        Ok(Some(Self::format_scalar_color_counts(
            counts,
            distinct_colors,
            mode,
        )))
    }

    fn scalar_color_domain_counts(
        mode: &str,
        bytes: &[u8],
        max_distinct_colors: usize,
        code_mask: u32,
        code_polynomial: u32,
        pixel_count: usize,
        has_repeated_prefix: bool,
    ) -> ScalarColorDomainCounts {
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
                result => return result,
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
                result => return result,
            }
        }
        ScalarColorDomainCounts::NotApplicable
    }

    #[inline(always)]
    fn count_repeated_scalar_samples(
        counts: &mut PillowColorCounts,
        distinct_colors: &mut usize,
        bytes: &[u8],
        max_distinct_colors: usize,
        code_mask: u32,
        code_polynomial: u32,
    ) -> bool {
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
                    distinct_colors,
                    pixel,
                    max_distinct_colors,
                    code_mask,
                    code_polynomial,
                ) {
                    return false;
                }
                continue;
            }

            match counts.count_repeated(
                distinct_colors,
                pixel,
                repetitions,
                max_distinct_colors,
                code_mask,
                code_polynomial,
            ) {
                Some(true) => {}
                Some(false) => return false,
                None => {
                    for _ in 0..repetitions {
                        if !counts.count(
                            distinct_colors,
                            pixel,
                            max_distinct_colors,
                            code_mask,
                            code_polynomial,
                        ) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    #[inline(always)]
    fn format_scalar_color_counts(
        counts: PillowColorCounts,
        distinct_colors: usize,
        mode: &str,
    ) -> Vec<(u32, FormattedPixelValue)> {
        counts
            .into_slot_order(distinct_colors)
            .into_iter()
            .map(|(_, (pixel, count))| {
                let value = if mode == "I" {
                    FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes()))
                } else {
                    FormattedPixelValue::Float(f32::from_bits(pixel) as f64)
                };
                (count, value)
            })
            .collect()
    }
}
