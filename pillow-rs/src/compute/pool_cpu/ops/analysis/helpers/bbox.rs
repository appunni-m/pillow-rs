//! Native GetBBox scans shared by exact-mode Default strategies.

use crate::raster::{GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

pub(crate) type BBox = Option<(u32, u32, u32, u32)>;

struct Bounds {
    width: usize,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
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

    // Callers derive `index` from exact-size pixel iterators bounded by u32 dimensions.
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

    fn finish(self) -> BBox {
        (self.left <= self.right).then_some((self.left, self.top, self.right + 1, self.bottom + 1))
    }
}

#[inline]
fn coordinate(index: usize) -> u32 {
    u32::try_from(index).expect("pixel indices are bounded by u32 image dimensions")
}

#[inline]
fn exclusive_edge(index: usize) -> u32 {
    coordinate(index)
        .checked_add(1)
        .expect("exclusive image edges fit in u32")
}

pub(crate) fn luma8(image: &GrayImage) -> BBox {
    let (width, height) = image.dimensions();
    let mut bounds = Bounds::new(width, height);
    for (index, pixel) in image.pixels().enumerate() {
        if pixel[0] != 0 {
            bounds.include_index(index);
        }
    }
    bounds.finish()
}

pub(crate) fn luma_alpha8(image: &GrayAlphaImage, alpha_only: bool) -> BBox {
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

/// Pillow's byte RGB scan keeps a grouped early blank test and row-bound scan.
pub(crate) fn rgb8(image: &RgbImage) -> BBox {
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
            coordinate(top),
            image.width(),
            exclusive_edge(bottom),
        ));
    }

    let (mut bbox_left, mut bbox_right, mut bbox_bottom) = (
        coordinate(left),
        exclusive_edge(right),
        exclusive_edge(top),
    );
    for (y, row) in rows.enumerate().skip(top + 1) {
        let mut pixels = row.chunks_exact(3);
        let Some(row_left) = pixels.clone().position(is_nonzero) else {
            continue;
        };
        let row_right = pixels.rposition(is_nonzero).unwrap_or(row_left);
        bbox_left = bbox_left.min(coordinate(row_left));
        bbox_right = bbox_right.max(exclusive_edge(row_right));
        bbox_bottom = exclusive_edge(y);
    }
    Some((bbox_left, coordinate(top), bbox_right, bbox_bottom))
}

pub(crate) fn rgba8(image: &RgbaImage, alpha_only: bool) -> BBox {
    let (width, height) = image.dimensions();
    let mut bounds = Bounds::new(width, height);
    for (index, pixel) in image.pixels().enumerate() {
        let nonzero = if alpha_only {
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

pub(crate) fn four_byte8(image: &RgbaImage) -> BBox {
    let (width, height) = image.dimensions();
    let mut bounds = Bounds::new(width, height);
    for (index, pixel) in image.pixels().enumerate() {
        if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 || pixel[3] != 0 {
            bounds.include_index(index);
        }
    }
    bounds.finish()
}

/// Pillow's `I` mode treats each stored four-byte signed sample as one word.
pub(crate) fn i32_carrier(raw: &[u8], width: u32, height: u32) -> BBox {
    let (width, height) = (width as usize, height as usize);
    if width == 0 || height == 0 {
        return None;
    }
    let rows = raw.chunks_exact(width * 4);
    let mut bounds = Bounds::new(width as u32, height as u32);
    for (y, row) in rows.clone().enumerate() {
        if let Some((left, right)) = i32_row_bounds(row) {
            bounds.include_row(coordinate(y), left, right);
            if bounds.left == 0 && bounds.right == width as u32 - 1 {
                if let Some((last_y, _)) = rows
                    .clone()
                    .enumerate()
                    .rev()
                    .find(|(_, row)| i32_row_bounds(row).is_some())
                {
                    bounds.bottom = coordinate(last_y);
                }
                break;
            }
        }
    }
    bounds.finish()
}

#[inline]
fn i32_row_bounds(row: &[u8]) -> Option<(u32, u32)> {
    let mut groups = row.chunks_exact(32);
    let (mut word0, mut word1, mut word2, mut word3) = (0u64, 0u64, 0u64, 0u64);
    for group in &mut groups {
        word0 |= u64::from_le_bytes(group[0..8].try_into().expect("8-byte word"));
        word1 |= u64::from_le_bytes(group[8..16].try_into().expect("8-byte word"));
        word2 |= u64::from_le_bytes(group[16..24].try_into().expect("8-byte word"));
        word3 |= u64::from_le_bytes(group[24..32].try_into().expect("8-byte word"));
    }
    if word0 | word1 | word2 | word3 == 0
        && !groups.remainder().iter().any(|&byte| byte != 0)
    {
        return None;
    }

    let mut samples = row.chunks_exact(4);
    let is_nonzero =
        |sample: &[u8]| u32::from_le_bytes(sample.try_into().expect("4-byte I-mode sample")) != 0;
    let left = samples.clone().position(is_nonzero)?;
    let right = samples.rposition(is_nonzero).unwrap_or(left);
    Some((coordinate(left), coordinate(right)))
}

/// Pillow's `F` mode predicate observes the stored word, including negative zero.
pub(crate) fn f32_carrier(raw: &[u8], width: u32, height: u32) -> BBox {
    let (width, height) = (width as usize, height as usize);
    if width == 0 || height == 0 {
        return None;
    }
    let row_bytes = width * 4;
    let mut bounds = Bounds::new(width as u32, height as u32);
    let mut full_width = false;
    for (y, row) in raw.chunks_exact(row_bytes).enumerate() {
        if let Some((left, right)) = f32_row_bounds(row) {
            bounds.include_row(coordinate(y), left, right);
            if bounds.left == 0 && bounds.right == width as u32 - 1 {
                full_width = true;
                break;
            }
        }
    }
    if full_width {
        for (y, row) in raw.chunks_exact(row_bytes).enumerate().rev() {
            if f32_row_bounds(row).is_some() {
                bounds.bottom = coordinate(y);
                break;
            }
        }
    }
    bounds.finish()
}

#[inline(always)]
fn f32_row_bounds(row: &[u8]) -> Option<(u32, u32)> {
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

    while left < right {
        let right_x = right - 1;
        let left_nonzero = is_nonzero(left);
        let right_nonzero = if right_x == left {
            left_nonzero
        } else {
            is_nonzero(right_x)
        };
        if left_nonzero && right_nonzero {
            return Some((coordinate(left), coordinate(right_x)));
        }
        if left_nonzero {
            let mut x = right_x;
            while x > left {
                x -= 1;
                if is_nonzero(x) {
                    return Some((coordinate(left), coordinate(x)));
                }
            }
            return Some((coordinate(left), coordinate(left)));
        }
        if right_nonzero {
            let mut x = left;
            while x < right_x {
                x += 1;
                if is_nonzero(x) {
                    return Some((coordinate(x), coordinate(right_x)));
                }
            }
            return Some((coordinate(right_x), coordinate(right_x)));
        }
        left += 1;
        right -= 1;
    }
    None
}

/// Scans native I;16 samples; byte order does not affect zero detection.
pub(crate) fn u16_samples<const CHANNELS: usize>(
    samples: &[u16],
    width: u32,
    height: u32,
    selected_channel: Option<usize>,
) -> BBox {
    scan_samples::<u16, CHANNELS, _>(
        samples,
        width,
        height,
        selected_channel,
        |sample| *sample != 0,
    )
}

pub(crate) fn f32_samples<const CHANNELS: usize>(
    samples: &[f32],
    width: u32,
    height: u32,
    selected_channel: Option<usize>,
) -> BBox {
    scan_samples::<f32, CHANNELS, _>(
        samples,
        width,
        height,
        selected_channel,
        |sample| {
            sample.to_bits() != 0
        },
    )
}

fn scan_samples<T, const CHANNELS: usize, F>(
    samples: &[T],
    width: u32,
    height: u32,
    selected_channel: Option<usize>,
    is_nonzero: F,
) -> BBox
where
    F: Fn(&T) -> bool,
{
    if width == 0 || height == 0 || CHANNELS == 0 {
        return None;
    }
    let mut bounds = Bounds::new(width, height);
    if let Some(channel) = selected_channel {
        for (index, pixel) in samples.chunks_exact(CHANNELS).enumerate() {
            if is_nonzero(&pixel[channel]) {
                bounds.include_index(index);
            }
        }
    } else {
        for (index, pixel) in samples.chunks_exact(CHANNELS).enumerate() {
            if pixel.iter().any(|sample| is_nonzero(sample)) {
                bounds.include_index(index);
            }
        }
    }
    bounds.finish()
}

macro_rules! declare_getbbox_owner {
    ($module:ident, $mode:ident, $image_ty:ty $(,)?) => {
        pub(crate) fn execute_getbbox(
            image: &$image_ty,
            alpha_only: bool,
            selection: crate::compute::registry::support::StrategySelection,
        ) -> Result<crate::compute::pool_cpu::ops::analysis::helpers::bbox::BBox, crate::error::PilError> {
            let implementation = selection.implementation;
            if implementation.mode != crate::raster::Mode::$mode
                || implementation.operation
                    != crate::compute::registry::support::OperationId::GetBBox
            {
                return Err(crate::error::PilError::InternalError(
                    "GetBBox strategy selection did not match its operation and mode".into(),
                ));
            }
            let architecture = implementation.architecture;
            match architecture {
                crate::compute::Architecture::Default => Ok(
                    crate::compute::pool_cpu::ops::analysis::$module::default::getbbox_impl::execute(
                        image,
                        alpha_only,
                    ),
                ),
                architecture => Err(crate::error::PilError::InternalError(format!(
                    "GetBBox has no {:?} {architecture:?} strategy",
                    crate::raster::Mode::$mode,
                ))),
            }
        }
    };
}

pub(crate) use declare_getbbox_owner;
