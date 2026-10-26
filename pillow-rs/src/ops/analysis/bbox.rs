//! Bounding-box scans for materialized images.

use crate::error::PilError;
use crate::image::Image;

struct Bounds {
    width: usize,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

#[inline]
fn pixel_index_coordinate(index: usize) -> u32 {
    u32::try_from(index).expect("pixel indices are bounded by u32 image dimensions")
}

#[inline]
fn pixel_index_exclusive_edge(index: usize) -> u32 {
    pixel_index_coordinate(index)
        .checked_add(1)
        .expect("exclusive image edges fit in u32")
}

impl Bounds {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width: width as usize,
            left: width,
            top: height,
            right: 0,
            bottom: 0,
        }
    }

    #[inline(always)]
    fn include(&mut self, x: u32, y: u32) {
        self.left = self.left.min(x);
        self.top = self.top.min(y);
        self.right = self.right.max(x);
        self.bottom = self.bottom.max(y);
    }

    // Callers derive `index` from exact-size pixel iterators whose dimensions
    // are u32; retaining these casts avoids range checks for every nonzero pixel.
    #[allow(clippy::cast_possible_truncation)]
    #[inline(always)]
    fn include_index(&mut self, index: usize) {
        let x = (index % self.width) as u32;
        let y = (index / self.width) as u32;
        self.include(x, y);
    }

    #[inline(always)]
    fn include_row(&mut self, y: u32, left: u32, right: u32) {
        self.include(left, y);
        self.include(right, y);
    }

    fn finish(self) -> Option<(u32, u32, u32, u32)> {
        (self.left <= self.right).then_some((self.left, self.top, self.right + 1, self.bottom + 1))
    }
}

/// Finds the horizontal bounds of non-zero F-mode pixels in one tightly
/// packed row. The raw-word predicate keeps negative zero observable.
#[inline(always)]
fn getbbox_f_row_bounds(row: &[u8]) -> Option<(u32, u32)> {
    let mut left = 0usize;
    let mut right = row.len() / 4;
    let is_nonzero = |x: usize| {
        let offset = x * 4;
        u32::from_le_bytes([
            row[offset],
            row[offset + 1],
            row[offset + 2],
            row[offset + 3],
        ]) != 0
    };

    // Check both row edges first. Dense rows usually find both bounds in two
    // reads; blank and sparse rows converge toward the center without reading
    // any sample twice.
    while left < right {
        let right_x = right - 1;
        let left_nonzero = is_nonzero(left);
        let right_nonzero = if right_x == left {
            left_nonzero
        } else {
            is_nonzero(right_x)
        };

        if left_nonzero && right_nonzero {
            return Some((
                pixel_index_coordinate(left),
                pixel_index_coordinate(right_x),
            ));
        }
        if left_nonzero {
            let mut x = right_x;
            while x > left {
                x -= 1;
                if is_nonzero(x) {
                    return Some((pixel_index_coordinate(left), pixel_index_coordinate(x)));
                }
            }
            return Some((pixel_index_coordinate(left), pixel_index_coordinate(left)));
        }
        if right_nonzero {
            let mut x = left;
            while x < right_x {
                x += 1;
                if is_nonzero(x) {
                    return Some((pixel_index_coordinate(x), pixel_index_coordinate(right_x)));
                }
            }
            return Some((
                pixel_index_coordinate(right_x),
                pixel_index_coordinate(right_x),
            ));
        }

        left += 1;
        right -= 1;
    }

    None
}

/// Finds the horizontal bounds of non-zero I-mode pixels in one tightly
/// packed row. Grouped raw-word ORs cheaply skip blank rows; hit rows are
/// resolved with the exact signed-integer nonzero predicate.
#[inline]
fn getbbox_i_row_bounds(row: &[u8]) -> Option<(u32, u32)> {
    let mut groups = row.chunks_exact(32);
    let (mut word0, mut word1, mut word2, mut word3) = (0u64, 0u64, 0u64, 0u64);
    for group in &mut groups {
        word0 |= u64::from_le_bytes(group[0..8].try_into().expect("8-byte word"));
        word1 |= u64::from_le_bytes(group[8..16].try_into().expect("8-byte word"));
        word2 |= u64::from_le_bytes(group[16..24].try_into().expect("8-byte word"));
        word3 |= u64::from_le_bytes(group[24..32].try_into().expect("8-byte word"));
    }
    if word0 | word1 | word2 | word3 == 0 && !groups.remainder().iter().any(|&byte| byte != 0) {
        return None;
    }

    let mut samples = row.chunks_exact(4);
    let is_nonzero =
        |sample: &[u8]| u32::from_le_bytes(sample.try_into().expect("4-byte I-mode sample")) != 0;
    let left = samples.clone().position(is_nonzero)?;
    let right = samples.rposition(is_nonzero).unwrap_or(left);
    Some((pixel_index_coordinate(left), pixel_index_coordinate(right)))
}

fn getbbox_rgb_bounds(image: &crate::raster::RgbImage) -> Option<(u32, u32, u32, u32)> {
    if image.width() == 0 || image.height() == 0 {
        return None;
    }
    let row_stride = image.width() as usize * 3;
    let raw = image.as_raw();
    let has_nonzero = raw[..3].iter().any(|&sample| sample != 0) || {
        let (words, remainder) = raw[3..].as_chunks::<8>();
        words.iter().any(|word| u64::from_ne_bytes(*word) != 0)
            || remainder.iter().any(|&sample| sample != 0)
    };
    if !has_nonzero {
        return None;
    }
    let rows = raw.chunks_exact(row_stride);
    let is_nonzero = |pixel: &[u8]| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0;
    let first = rows.clone().enumerate().find_map(|(y, row)| {
        let mut pixels = row.chunks_exact(3);
        let left = pixels.clone().position(is_nonzero)?;
        let right = pixels.rposition(is_nonzero).unwrap_or(left);
        Some((y, left, right))
    });
    let (top, left, right) = first?;
    if left == 0
        && right + 1 == image.width() as usize
        && let Some(bottom) = rows
            .clone()
            .enumerate()
            .rposition(|(_, row)| row.chunks_exact(3).any(is_nonzero))
    {
        return Some((
            0,
            pixel_index_coordinate(top),
            image.width(),
            pixel_index_exclusive_edge(bottom),
        ));
    }

    let (mut bbox_left, mut bbox_right, mut bbox_bottom) = (
        pixel_index_coordinate(left),
        pixel_index_exclusive_edge(right),
        pixel_index_exclusive_edge(top),
    );
    for (y, row) in rows.enumerate().skip(top + 1) {
        let mut pixels = row.chunks_exact(3);
        let Some(row_left) = pixels.clone().position(is_nonzero) else {
            continue;
        };
        let row_right = pixels.rposition(is_nonzero).unwrap_or(row_left);
        bbox_left = bbox_left.min(pixel_index_coordinate(row_left));
        bbox_right = bbox_right.max(pixel_index_exclusive_edge(row_right));
        bbox_bottom = pixel_index_exclusive_edge(y);
    }
    Some((
        bbox_left,
        pixel_index_coordinate(top),
        bbox_right,
        bbox_bottom,
    ))
}

fn getbbox_i_bounds(image: &crate::raster::DynamicImage) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = (image.width(), image.height());
    let row_bytes = width as usize * 4;
    let rows = image.as_bytes().chunks_exact(row_bytes);
    let mut bounds = Bounds::new(width, height);

    // Materialization validates the scalar storage. Group raw words by row so
    // blank rows avoid per-pixel coordinate updates. Once a row establishes
    // full-width bounds, only a reverse scan for the last nonzero row remains.
    for (y, row) in rows.clone().enumerate() {
        if let Some((left, right)) = getbbox_i_row_bounds(row) {
            bounds.include_row(pixel_index_coordinate(y), left, right);
            if bounds.left == 0 && bounds.right == width - 1 {
                if let Some((last_y, _)) = rows
                    .clone()
                    .enumerate()
                    .rev()
                    .find(|(_, row)| getbbox_i_row_bounds(row).is_some())
                {
                    bounds.bottom = pixel_index_coordinate(last_y);
                }
                break;
            }
        }
    }
    bounds.finish()
}

fn getbbox_f_bounds(image: &crate::raster::DynamicImage) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = (image.width(), image.height());
    let row_bytes = width as usize * 4;
    let raw = image.as_bytes();
    let mut bounds = Bounds::new(width, height);
    let mut full_width = false;
    for (y, row) in raw.chunks_exact(row_bytes).enumerate() {
        if let Some((left, right)) = getbbox_f_row_bounds(row) {
            bounds.include_row(pixel_index_coordinate(y), left, right);
            if bounds.left == 0 && bounds.right == width - 1 {
                // No later row can widen the horizontal bounds. Stop and find
                // the bottom edge with a reverse row scan.
                full_width = true;
                break;
            }
        }
    }
    if full_width {
        for (y, row) in raw.chunks_exact(row_bytes).enumerate().rev() {
            if getbbox_f_row_bounds(row).is_some() {
                bounds.bottom = pixel_index_coordinate(y);
                break;
            }
        }
    }
    bounds.finish()
}

// `x` and `y` range directly over the image's u32 dimensions. Keep the
// narrowing casts in this pixel loop so each nonzero sample stays branch-free.
#[allow(clippy::cast_possible_truncation)]
fn getbbox_l16_byte_bounds(image: &crate::raster::DynamicImage) -> Option<(u32, u32, u32, u32)> {
    // Pillow's ImagingCore getbbox dispatches I;16* through its byte-oriented
    // image8 scan. Each row contributes its first `width` stored bytes rather
    // than one predicate per decoded 16-bit sample.
    let (width, height) = (image.width(), image.height());
    let row_stride = width as usize * 2;
    let raw = image.as_bytes();
    let mut bounds = Bounds::new(width, height);
    for y in 0..height as usize {
        let row = &raw[y * row_stride..(y + 1) * row_stride];
        for x in 0..width as usize {
            if row[x] != 0 {
                bounds.include(x as u32, y as u32);
            }
        }
    }
    bounds.finish()
}

fn getbbox_luma8_bounds(image: &crate::raster::GrayImage) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = image.dimensions();
    let mut bounds = Bounds::new(width, height);
    for (index, pixel) in image.pixels().enumerate() {
        if pixel[0] != 0 {
            bounds.include_index(index);
        }
    }
    bounds.finish()
}

fn getbbox_luma_alpha8_bounds(
    image: &crate::raster::GrayAlphaImage,
    alpha_only: bool,
) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = image.dimensions();
    let mut bounds = Bounds::new(width, height);
    for (index, pixel) in image.pixels().enumerate() {
        let nonzero = if alpha_only {
            pixel[1] != 0
        } else {
            pixel[0] != 0 || pixel[1] != 0
        };
        if nonzero {
            bounds.include_index(index);
        }
    }
    bounds.finish()
}

fn getbbox_rgba8_bounds(
    image: &crate::raster::RgbaImage,
    mode: &str,
    alpha_only: bool,
) -> Option<(u32, u32, u32, u32)> {
    // CMYK and RGBX also use four-byte storage but Pillow inspects all stored
    // bands for those modes. Only RGBA/RGBa select band 3 alone.
    let has_alpha = matches!(mode, "RGBA" | "RGBa");
    let (width, height) = image.dimensions();
    let mut bounds = Bounds::new(width, height);
    for (index, pixel) in image.pixels().enumerate() {
        let nonzero = if alpha_only && has_alpha {
            pixel[3] != 0
        } else {
            pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 || pixel[3] != 0
        };
        if nonzero {
            bounds.include_index(index);
        }
    }
    bounds.finish()
}

fn getbbox_converted_bounds(
    image: &crate::raster::DynamicImage,
    alpha_only: bool,
) -> Option<(u32, u32, u32, u32)> {
    let has_alpha = matches!(
        image.color(),
        crate::raster::ColorType::La8
            | crate::raster::ColorType::La16
            | crate::raster::ColorType::Rgba8
            | crate::raster::ColorType::Rgba16
            | crate::raster::ColorType::Rgba32F
    );
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut bounds = Bounds::new(width, height);
    for (index, pixel) in rgba.pixels().enumerate() {
        // Pillow GetBBox.c uses alpha only for alpha-bearing modes; RGB keeps
        // all three visible channels in its mask.
        let nonzero = if alpha_only && has_alpha {
            pixel[3] > 0
        } else if has_alpha {
            pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0 || pixel[3] > 0
        } else {
            pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0
        };
        if nonzero {
            bounds.include_index(index);
        }
    }
    bounds.finish()
}

fn getbbox_native_bounds(
    image: &crate::raster::DynamicImage,
    mode: &str,
    alpha_only: bool,
) -> Option<(u32, u32, u32, u32)> {
    match image {
        crate::raster::DynamicImage::ImageLuma8(image) => getbbox_luma8_bounds(image),
        crate::raster::DynamicImage::ImageLumaA8(image) => {
            getbbox_luma_alpha8_bounds(image, alpha_only)
        }
        crate::raster::DynamicImage::ImageRgb8(image) => getbbox_rgb_bounds(image),
        crate::raster::DynamicImage::ImageRgba8(image) => {
            getbbox_rgba8_bounds(image, mode, alpha_only)
        }
        _ => getbbox_converted_bounds(image, alpha_only),
    }
}

#[cfg(test)]
mod getbbox_i_row_tests {
    use super::getbbox_i_row_bounds;

    #[test]
    fn grouped_row_scan_preserves_nonzero_bounds_with_tails() {
        for width in 1..=19 {
            let mut row = vec![0u8; width * 4];
            assert_eq!(getbbox_i_row_bounds(&row), None, "blank width {width}");

            let left = width / 3;
            let right = width - 1;
            row[left * 4..left * 4 + 4].copy_from_slice(&(-17i32).to_le_bytes());
            row[right * 4..right * 4 + 4].copy_from_slice(&i32::MIN.to_le_bytes());
            assert_eq!(
                getbbox_i_row_bounds(&row),
                Some((left as u32, right as u32)),
                "nonzero width {width}"
            );

            row.fill(0);
            row[(width / 2) * 4..(width / 2) * 4 + 4].copy_from_slice(&i32::MAX.to_le_bytes());
            assert_eq!(
                getbbox_i_row_bounds(&row),
                Some(((width / 2) as u32, (width / 2) as u32)),
                "single hit width {width}"
            );
        }
    }
}

impl Image {
    /// Returns the bounding box of non-zero image content.
    ///
    /// The result is `(left, top, right, bottom)` with `right` and `bottom`
    /// exclusive, matching Pillow `getbbox`. `None` means every inspected pixel
    /// is zero. When `alpha_only` is true and the image has alpha, only alpha is
    /// inspected.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn getbbox(&self, alpha_only: bool) -> Result<Option<(u32, u32, u32, u32)>, PilError> {
        // Direct materialized images use this host scan as their terminal
        // operation. Report that CPU execution only when the benchmark has
        // enabled receipts. Leave lazy pipelines on their existing receipt
        // path; their mixed materialization-plus-host-scan boundary needs a
        // combined terminal fallback receipt before it can earn accelerated
        // backend credit.
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled()
            && !matches!(self, Image::Pipeline { .. })
        {
            match crate::compute::preferred_active_backend() {
                Ok(selected) => Some(crate::compute::begin_eager_cpu_operation_telemetry(
                    "GetBBox",
                    selected.map(|(backend, _)| backend),
                )),
                Err(_) => None,
            }
        } else {
            None
        };
        let result = self.getbbox_materialized(alpha_only);
        if let Some(telemetry) = telemetry {
            if result.is_ok() {
                crate::compute::finish_eager_cpu_operation_telemetry(telemetry, 0);
            } else {
                crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
            }
        }
        result
    }

    fn getbbox_materialized(
        &self,
        alpha_only: bool,
    ) -> Result<Option<(u32, u32, u32, u32)>, PilError> {
        let image = self.materialized_shared()?;
        let (width, height) = (image.width(), image.height());
        if width == 0 || height == 0 {
            return Ok(None);
        }

        let mode = self.mode_from_materialized(&image);
        let bounds = if mode == "I" {
            getbbox_i_bounds(&image)
        } else if mode == "F" {
            getbbox_f_bounds(&image)
        } else if image.color() == crate::raster::ColorType::L16
            && matches!(mode.as_str(), "I;16" | "I;16L" | "I;16B" | "I;16N")
        {
            getbbox_l16_byte_bounds(&image)
        } else {
            getbbox_native_bounds(&image, &mode, alpha_only)
        };
        Ok(bounds)
    }
}
