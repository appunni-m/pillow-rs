//! CPU implementations for median, min, max, and arbitrary-rank filters.

use super::uniform_image::{SMALL_UNIFORM_BYTE_MAX_PIXELS, native_small_uniform_byte_image};
use super::{CheckedDims, DynamicImage, PilError, preserve_mode, raw_bytes_to_image};

/// Push one source index into a monotonic queue used by a byte min/max line.
/// The queue stores indices rather than samples so stale values can be
/// removed when the replicated-edge window advances.
#[inline]
fn push_extreme_index(
    queue: &mut Vec<usize>,
    head: &mut usize,
    input: &[u8],
    channels: usize,
    channel: usize,
    index: usize,
    select_max: bool,
) {
    let value = input[index * channels + channel];
    while queue.len() > *head {
        let Some(&back) = queue.last() else {
            break;
        };
        let back_value = input[back * channels + channel];
        let remove_back = if select_max {
            back_value <= value
        } else {
            back_value >= value
        };
        if !remove_back {
            break;
        }
        queue.pop();
    }
    queue.push(index);
}

/// Apply a replicated-edge min/max window to one contiguous byte line.
///
/// A queue is reused for every channel, so the pass has no allocation in the
/// output-pixel loop and its work is linear in the line length, independent of
/// the square filter area.
fn rank_filter_bytes_extreme_line(
    input: &[u8],
    output: &mut [u8],
    length: usize,
    channels: usize,
    half: usize,
    select_max: bool,
    queues: &mut [Vec<usize>; 4],
) {
    if length == 0 || channels == 0 || channels > queues.len() {
        return;
    }
    let last_index = length - 1;
    let initial_end = half.min(last_index);
    for channel in 0..channels {
        let queue = &mut queues[channel];
        queue.clear();
        let mut head = 0usize;
        for index in 0..=initial_end {
            push_extreme_index(
                queue, &mut head, input, channels, channel, index, select_max,
            );
        }
        output[channel] = input[queue[head] * channels + channel];
        let mut last_added = initial_end;
        for x in 1..length {
            let left = x.saturating_sub(half);
            while head < queue.len() && queue[head] < left {
                head += 1;
            }
            let entering = x.saturating_add(half).min(last_index);
            if entering > last_added {
                push_extreme_index(
                    queue, &mut head, input, channels, channel, entering, select_max,
                );
                last_added = entering;
            }
            output[x * channels + channel] = input[queue[head] * channels + channel];
        }
    }
}

/// Evaluate one output row of the vertical extrema pass. Rebuilding a
/// monotonic queue for the bounded vertical window reduces the square scan to
/// one horizontal and one vertical window traversal per pixel.
fn rank_filter_bytes_extreme_vertical_row(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    select_max: bool,
    y: usize,
    queues: &mut [Vec<usize>; 4],
) {
    let row_stride = width * channels;
    let top = y.saturating_sub(half);
    let bottom = y.saturating_add(half).min(height - 1);
    for x in 0..width {
        for channel in 0..channels {
            let queue = &mut queues[channel];
            queue.clear();
            let head = 0usize;
            for source_y in top..=bottom {
                let value = input[source_y * row_stride + x * channels + channel];
                while queue.len() > head {
                    let Some(&back) = queue.last() else {
                        break;
                    };
                    let back_value = input[back * row_stride + x * channels + channel];
                    let remove_back = if select_max {
                        back_value <= value
                    } else {
                        back_value >= value
                    };
                    if !remove_back {
                        break;
                    }
                    queue.pop();
                }
                queue.push(source_y);
            }
            output[x * channels + channel] =
                input[queue[head] * row_stride + x * channels + channel];
        }
    }
}

/// Exact native-byte min/max implementation using horizontal and vertical
/// monotonic windows. Pillow's square window is separable for extrema, and
/// duplicating the edge samples does not change a min or max. The intermediate
/// buffer is one full frame, while the per-line queues are bounded by one
/// image dimension and reused across channels.
fn rank_filter_bytes_extreme_separable(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    select_max: bool,
) -> bool {
    let Ok(width) = usize::try_from(w) else {
        return false;
    };
    let Ok(height) = usize::try_from(h) else {
        return false;
    };
    let Ok(half) = usize::try_from(half) else {
        return false;
    };
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    let Some(total_bytes) = row_stride.checked_mul(height) else {
        return false;
    };
    if channels == 0
        || channels > 4
        || raw.len() < total_bytes
        || out.len() < total_bytes
        || width == 0
        || height == 0
    {
        return false;
    }

    let mut horizontal = vec![0u8; total_bytes];

    for y in 0..height {
        let row_start = y * row_stride;
        let mut queues: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::with_capacity(width));
        rank_filter_bytes_extreme_line(
            &raw[row_start..row_start + row_stride],
            &mut horizontal[row_start..row_start + row_stride],
            width,
            channels,
            half,
            select_max,
            &mut queues,
        );
    }

    let queue_capacity = half.saturating_mul(2).saturating_add(1).min(height);

    for y in 0..height {
        let row_start = y * row_stride;
        let mut queues: [Vec<usize>; 4] =
            std::array::from_fn(|_| Vec::with_capacity(queue_capacity));
        rank_filter_bytes_extreme_vertical_row(
            &horizontal,
            &mut out[row_start..row_start + row_stride],
            width,
            height,
            channels,
            half,
            select_max,
            y,
            &mut queues,
        );
    }
    true
}

const SMALL_RANK_AREA: usize = 49;

#[path = "rank/float.rs"]
mod float;
use float::{RankFilterWindow, rank_filter_f_impl};
#[cfg(test)]
use float::{
    native_uniform_f_mode_image, rank_filter_f_requires_legacy_sort, select_rank_filter_f_value,
};

#[inline]
fn select_rank_histogram(histogram: &[usize; 256], rank: usize) -> u8 {
    let mut seen = 0usize;
    for (value, count) in histogram.iter().enumerate() {
        seen += *count;
        if seen > rank {
            return value as u8;
        }
    }
    255
}

fn rank_filter_bytes_extreme_row(
    raw: &[u8],
    row: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    select_max: bool,
    y: i32,
) {
    for x in 0..w {
        let mut selected = if select_max { [0u8; 4] } else { [u8::MAX; 4] };
        for dy in -half..=half {
            for dx in -half..=half {
                let sx = (x + dx).clamp(0, w - 1);
                let sy = (y + dy).clamp(0, h - 1);
                let base = (sy * w + sx) as usize * channels;
                for c in 0..channels {
                    let value = raw[base + c];
                    if select_max {
                        selected[c] = selected[c].max(value);
                    } else {
                        selected[c] = selected[c].min(value);
                    }
                }
            }
        }
        let out_base = x as usize * channels;
        row[out_base..out_base + channels].copy_from_slice(&selected[..channels]);
    }
}

fn rank_filter_bytes_extreme(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    select_max: bool,
) {
    let row_stride = w.max(0) as usize * channels;

    for y in 0..h {
        let row_start = y as usize * row_stride;
        rank_filter_bytes_extreme_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            w,
            h,
            channels,
            half,
            select_max,
            y,
        );
    }
}

fn rank_filter_bytes_small_row(
    raw: &[u8],
    row: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    area: usize,
    rank: usize,
    y: i32,
) {
    debug_assert!(area <= SMALL_RANK_AREA);
    for x in 0..w {
        let mut values = [[0u8; SMALL_RANK_AREA]; 4];
        let mut value_index = 0usize;
        for dy in -half..=half {
            for dx in -half..=half {
                let sx = (x + dx).clamp(0, w - 1);
                let sy = (y + dy).clamp(0, h - 1);
                let base = (sy * w + sx) as usize * channels;
                for c in 0..channels {
                    values[c][value_index] = raw[base + c];
                }
                value_index += 1;
            }
        }
        let out_base = x as usize * channels;
        for c in 0..channels {
            // Only the requested order statistic is observable. Selecting it
            // avoids sorting the complete byte neighborhood while preserving
            // the exact value returned for duplicate samples.
            let (_, selected, _) = values[c][..area].select_nth_unstable(rank);
            row[out_base + c] = *selected;
        }
    }
}

fn rank_filter_bytes_small(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    area: usize,
    rank: usize,
) {
    let row_stride = w.max(0) as usize * channels;

    for y in 0..h {
        let row_start = y as usize * row_stride;
        rank_filter_bytes_small_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            w,
            h,
            channels,
            half,
            area,
            rank,
            y,
        );
    }
}

/// Apply a 3x3 byte rank filter without constructing the maximum-sized
/// 7x7 scratch array for every output pixel. The clamped coordinates preserve
/// Pillow's replicated-edge window, and only the requested order statistic is
/// selected from the nine samples per channel.
#[inline(always)]
fn compare_swap_median9(values: &mut [u8; 9], left: usize, right: usize) {
    let a = values[left];
    let b = values[right];
    values[left] = a.min(b);
    values[right] = a.max(b);
}

#[inline(always)]
fn median9_u8(mut values: [u8; 9]) -> u8 {
    // A fixed 19-comparator selection network leaves the median at index 4.
    compare_swap_median9(&mut values, 1, 2);
    compare_swap_median9(&mut values, 4, 5);
    compare_swap_median9(&mut values, 7, 8);
    compare_swap_median9(&mut values, 0, 1);
    compare_swap_median9(&mut values, 3, 4);
    compare_swap_median9(&mut values, 6, 7);
    compare_swap_median9(&mut values, 0, 3);
    compare_swap_median9(&mut values, 1, 2);
    compare_swap_median9(&mut values, 4, 5);
    compare_swap_median9(&mut values, 7, 8);
    compare_swap_median9(&mut values, 3, 6);
    compare_swap_median9(&mut values, 4, 7);
    compare_swap_median9(&mut values, 5, 8);
    compare_swap_median9(&mut values, 1, 4);
    compare_swap_median9(&mut values, 2, 5);
    compare_swap_median9(&mut values, 4, 7);
    compare_swap_median9(&mut values, 4, 2);
    compare_swap_median9(&mut values, 6, 4);
    compare_swap_median9(&mut values, 4, 2);
    values[4]
}

fn rank_filter_bytes_3x3_row(
    raw: &[u8],
    row: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    rank: usize,
    y: usize,
) {
    let row_stride = width * channels;
    let top = y.saturating_sub(1) * row_stride;
    let middle = y * row_stride;
    let bottom = (y + 1).min(height - 1) * row_stride;

    for x in 0..width {
        let left = x.saturating_sub(1) * channels;
        let center = x * channels;
        let right = (x + 1).min(width - 1) * channels;
        let output = center;
        for channel in 0..channels {
            let mut values = [
                raw[top + left + channel],
                raw[top + center + channel],
                raw[top + right + channel],
                raw[middle + left + channel],
                raw[middle + center + channel],
                raw[middle + right + channel],
                raw[bottom + left + channel],
                raw[bottom + center + channel],
                raw[bottom + right + channel],
            ];
            row[output + channel] = if rank == 4 {
                median9_u8(values)
            } else if matches!(channels, 1 | 2) && rank == 1 {
                // Native L/LA rank 1 only needs the two smallest samples, not
                // a complete introselect partition. This min/max reduction
                // keeps duplicate minima exact and avoids data-dependent
                // selection.
                let mut smallest = values[0].min(values[1]);
                let mut second = values[0].max(values[1]);
                for value in values.into_iter().skip(2) {
                    let lower = smallest.min(value);
                    let upper = smallest.max(value);
                    second = second.min(upper);
                    smallest = lower;
                }
                second
            } else {
                let (_, selected, _) = values.select_nth_unstable(rank);
                *selected
            };
        }
    }
}

fn rank_filter_bytes_3x3(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    rank: usize,
) {
    let row_stride = width * channels;

    for y in 0..height {
        let row_start = y * row_stride;
        rank_filter_bytes_3x3_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            width,
            height,
            channels,
            rank,
            y,
        );
    }
}

fn rank_filter_bytes_histogram_row(
    raw: &[u8],
    row: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    rank: usize,
    y: i32,
) {
    let mut histogram = [[0usize; 256]; 4];
    for dy in -half..=half {
        let sy = (y + dy).clamp(0, h - 1);
        for dx in -half..=half {
            let sx = dx.clamp(0, w - 1);
            let base = (sy * w + sx) as usize * channels;
            for c in 0..channels {
                histogram[c][raw[base + c] as usize] += 1;
            }
        }
    }

    for x in 0..w {
        let out_base = x as usize * channels;
        for c in 0..channels {
            row[out_base + c] = select_rank_histogram(&histogram[c], rank);
        }

        if x + 1 < w {
            let remove_x = (x - half).clamp(0, w - 1);
            let add_x = (x + half + 1).clamp(0, w - 1);
            for dy in -half..=half {
                let sy = (y + dy).clamp(0, h - 1);
                let remove_base = (sy * w + remove_x) as usize * channels;
                let add_base = (sy * w + add_x) as usize * channels;
                for c in 0..channels {
                    histogram[c][raw[remove_base + c] as usize] -= 1;
                    histogram[c][raw[add_base + c] as usize] += 1;
                }
            }
        }
    }
}

fn rank_filter_bytes_histogram(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    rank: usize,
) {
    let row_stride = w.max(0) as usize * channels;

    for y in 0..h {
        let row_start = y as usize * row_stride;
        rank_filter_bytes_histogram_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            w,
            h,
            channels,
            half,
            rank,
            y,
        );
    }
}

/// Generic rank filter: selects the neighborhood value at `rank`.
/// PIL uses clamping for border pixels and preserves each byte mode's channels.
fn rank_filter_impl(
    img: &DynamicImage,
    size: u32,
    rank: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w_u32, h_u32) = (img.width(), img.height());
    let (w, h) = (w_u32 as i32, h_u32 as i32);
    let half = (size / 2) as i32;
    let area = (size * size) as usize;
    let rank = rank.min((area - 1) as u32) as usize;
    let window = RankFilterWindow {
        width: w_u32,
        height: h_u32,
        width_i32: w,
        height_i32: h,
        half,
        area,
        rank,
    };
    if mode == Some("F") {
        return rank_filter_f_impl(img, window);
    }

    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();

    let native_byte_layout = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_layout
        && native_small_uniform_byte_image(img, channels, SMALL_UNIFORM_BYTE_MAX_PIXELS)
    {
        // Every rank of a uniform byte image is the same source tuple. The
        // bounded scan avoids gathering and selecting an identical window at
        // every output pixel, and cloning preserves the source's native mode.
        return Ok(img.clone());
    }
    let mut out = CheckedDims::new(w as u32, h as u32, channels as u8)?.alloc_buffer();
    let separable_extreme =
        native_byte_layout && size >= 5 && w.max(h) > 512 && (rank == 0 || rank == area - 1);
    if separable_extreme
        && rank_filter_bytes_extreme_separable(
            raw,
            &mut out,
            w,
            h,
            channels,
            half,
            rank == area - 1,
        )
    {
        // The separable path completed the output. Keep the original direct
        // path below for typed layouts and small windows where its lower setup
        // cost is preferable.
    } else if rank == 0 {
        rank_filter_bytes_extreme(raw, &mut out, w, h, channels, half, false);
    } else if rank == area - 1 {
        rank_filter_bytes_extreme(raw, &mut out, w, h, channels, half, true);
    } else if size == 3 {
        rank_filter_bytes_3x3(
            raw,
            &mut out,
            w_u32 as usize,
            h_u32 as usize,
            channels,
            rank,
        );
    } else if area <= SMALL_RANK_AREA {
        rank_filter_bytes_small(raw, &mut out, w, h, channels, half, area, rank);
    } else {
        rank_filter_bytes_histogram(raw, &mut out, w, h, channels, half, rank);
    }
    let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
    Ok(preserve_mode(img, result))
}

// ── Execute filter ops ──

/// Execute a 3x3 convolution filter.
/// Execute a median filter with explicit mode.
pub fn execute_median_filter_with_mode(
    img: &DynamicImage,
    size: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, size * size / 2, mode)
}

/// Execute a max filter with explicit mode.
pub fn execute_max_filter_with_mode(
    img: &DynamicImage,
    size: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, size * size - 1, mode)
}

/// Execute a min filter with explicit mode.
pub fn execute_min_filter_with_mode(
    img: &DynamicImage,
    size: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, 0, mode)
}

/// Execute a rank filter with explicit mode.
pub fn execute_rank_filter_with_mode(
    img: &DynamicImage,
    size: u32,
    rank: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, rank, mode)
}

#[cfg(test)]
#[path = "rank/tests.rs"]
mod tests;
