//! Image quantization — reduce color palette using median-cut algorithm.
//!
//! Implements PIL's median-cut quantization:
//! 1. Build 3D histogram (RGB, 5-bit precision = 32x32x32 = 32768 buckets)
//! 2. Find leaf boxes (non-empty buckets) to start
//! 3. Recursively split the largest-volume box at the median pixel count
//! 4. Compute palette centroids as weighted averages
//! 5. Map each pixel to nearest palette color

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Histogram bucket index for 5-bit channels.
fn hist_index(r5: u8, g5: u8, b5: u8) -> usize {
    (r5 as usize) << 10 | (g5 as usize) << 5 | b5 as usize
}

/// Number of bins per channel (5-bit)
const BINS: usize = 32;
const TOTAL_BINS: usize = BINS * BINS * BINS; // 32768

/// A rectangular volume in 5-bit (32-level) RGB color space.
#[derive(Debug, Clone, Copy)]
struct VBox {
    r_min: u8, // 0..31
    r_max: u8,
    g_min: u8,
    g_max: u8,
    b_min: u8,
    b_max: u8,
    pixel_count: u32,
    volume: u32,
}

impl VBox {
    fn new(r_min: u8, r_max: u8, g_min: u8, g_max: u8, b_min: u8, b_max: u8) -> Self {
        let volume = (r_max - r_min + 1) as u32
            * (g_max - g_min + 1) as u32
            * (b_max - b_min + 1) as u32;
        VBox {
            r_min,
            r_max,
            g_min,
            g_max,
            b_min,
            b_max,
            pixel_count: 0,
            volume,
        }
    }

    fn vibe(&self) -> u64 {
        self.volume as u64 * self.pixel_count as u64
    }
}

/// Median cut with 5-bit histogram buckets.
pub fn median_cut_quantize_rgb(pixels: &[u8], n_colors: usize) -> (Vec<u8>, Vec<[u8; 3]>) {
    let n = pixels.len() / 3;
    if n == 0 || n_colors < 2 {
        let p = if n_colors < 2 {
            vec![[0u8; 3]; n_colors.max(1)]
        } else {
            vec![[0u8; 3]; 1]
        };
        return (vec![0u8; n], p);
    }

    let n_colors = n_colors.min(256);

    // Step 1: Build 5-bit histogram
    let mut hist = [0u32; TOTAL_BINS];
    for i in 0..n {
        let base = i * 3;
        let r5 = pixels[base] >> 3;
        let g5 = pixels[base + 1] >> 3;
        let b5 = pixels[base + 2] >> 3;
        let idx = hist_index(r5, g5, b5);
        hist[idx] += 1;
    }

    // Step 2: Find initial bounding box
    let (r_min, r_max, g_min, g_max, b_min, b_max) = match find_initial_bounds(&hist) {
        Some(bounds) => bounds,
        None => return (vec![0u8; n], vec![[0u8; 3]; 1]),
    };

    // Step 3: Create initial box
    let mut boxes: Vec<VBox> = vec![VBox::new(r_min, r_max, g_min, g_max, b_min, b_max)];
    let mut total = 0u32;
    for r5 in r_min..=r_max {
        for g5 in g_min..=g_max {
            for b5 in b_min..=b_max {
                total += hist[hist_index(r5, g5, b5)];
            }
        }
    }
    boxes[0].pixel_count = total;

    // Recursively split
    split_boxes(&mut boxes, &hist, n_colors);

    // Step 4: Sort boxes by vibe for stable palette ordering
    boxes.sort_by(|a, b| b.vibe().cmp(&a.vibe()));

    // Step 5: Compute palette centroids
    let palette = compute_palette(&boxes, &hist);
    let n_boxes = palette.len();

    if n_boxes == 0 {
        return (vec![0u8; n], vec![[0u8; 3]; 1]);
    }

    // Pad palette
    let mut final_palette = Vec::with_capacity(n_colors);
    for i in 0..n_colors {
        if i < n_boxes {
            final_palette.push(palette[i]);
        } else {
            final_palette.push(palette[n_boxes - 1]);
        }
    }

    // Step 6: Map pixels to nearest palette color
    let indices = map_pixels_to_palette(pixels, &final_palette);

    (indices, final_palette)
}

/// Find the initial bounding box from non-empty histogram bins.
fn find_initial_bounds(hist: &[u32; TOTAL_BINS]) -> Option<(u8, u8, u8, u8, u8, u8)> {
    let mut r_min = 31u8;
    let mut r_max = 0u8;
    let mut g_min = 31u8;
    let mut g_max = 0u8;
    let mut b_min = 31u8;
    let mut b_max = 0u8;
    let mut found = false;

    for r5 in 0..BINS {
        for g5 in 0..BINS {
            for b5 in 0..BINS {
                if hist[hist_index(r5 as u8, g5 as u8, b5 as u8)] > 0 {
                    found = true;
                    r_min = r_min.min(r5 as u8);
                    r_max = r_max.max(r5 as u8);
                    g_min = g_min.min(g5 as u8);
                    g_max = g_max.max(g5 as u8);
                    b_min = b_min.min(b5 as u8);
                    b_max = b_max.max(b5 as u8);
                }
            }
        }
    }

    if found {
        Some((r_min, r_max, g_min, g_max, b_min, b_max))
    } else {
        None
    }
}

/// Recursively split boxes until we have enough.
fn split_boxes(boxes: &mut Vec<VBox>, hist: &[u32; TOTAL_BINS], n_colors: usize) {
    while boxes.len() < n_colors {
        let mut best_idx = None;
        let mut best_vibe = 0u64;

        for (i, b) in boxes.iter().enumerate() {
            if b.pixel_count > 1 && b.volume > 0 {
                let v = b.vibe();
                if v > best_vibe {
                    best_vibe = v;
                    best_idx = Some(i);
                }
            }
        }

        let idx = match best_idx {
            Some(i) => i,
            None => break,
        };

        let vbox = boxes[idx];
        match try_split(&vbox, hist) {
            Some((left, right)) => {
                boxes[idx] = left;
                boxes.push(right);
            }
            None => {
                break;
            }
        }
    }
}

/// Try to split a VBox into two along the best axis.
fn try_split(vbox: &VBox, hist: &[u32; TOTAL_BINS]) -> Option<(VBox, VBox)> {
    if vbox.pixel_count <= 1 || vbox.volume <= 1 {
        return None;
    }

    // Choose the axis with the largest range
    let r_range = vbox.r_max - vbox.r_min;
    let g_range = vbox.g_max - vbox.g_min;
    let b_range = vbox.b_max - vbox.b_min;

    let best_axis = if r_range >= g_range && r_range >= b_range {
        0
    } else if g_range >= r_range && g_range >= b_range {
        1
    } else {
        2
    };

    // Build a sorted list of (channel_value, pixel_count) for entries in this box
    let mut axis_entries: Vec<(u8, u32)> = Vec::new();
    for r5 in vbox.r_min..=vbox.r_max {
        for g5 in vbox.g_min..=vbox.g_max {
            for b5 in vbox.b_min..=vbox.b_max {
                let count = hist[hist_index(r5, g5, b5)];
                if count > 0 {
                    let val = match best_axis {
                        0 => r5,
                        1 => g5,
                        2 => b5,
                        _ => unreachable!(),
                    };
                    axis_entries.push((val, count));
                }
            }
        }
    }

    // Sort by channel value, then deduplicate by merging same values
    axis_entries.sort_by_key(|&(v, _)| v);
    let mut deduped: Vec<(u8, u32)> = Vec::new();
    for &(val, cnt) in &axis_entries {
        if let Some(last) = deduped.last_mut() {
            if last.0 == val {
                last.1 += cnt;
                continue;
            }
        }
        deduped.push((val, cnt));
    }

    if deduped.len() <= 1 {
        return None;
    }

    // Find median split
    let mut cum = 0u32;
    let mut split_idx = 0usize;
    let midpoint = vbox.pixel_count / 2;

    for (j, &(_, cnt)) in deduped.iter().enumerate() {
        cum += cnt;
        if cum > midpoint {
            split_idx = j;
            break;
        }
    }

    if split_idx >= deduped.len() - 1 {
        return None;
    }

    let split_val = deduped[split_idx].0.saturating_add(1);

    // Build left and right boxes
    let (left, right) = match best_axis {
        0 => {
            if split_val <= vbox.r_min || split_val > vbox.r_max {
                return None;
            }
            let mut left = *vbox;
            left.r_max = split_val - 1;
            left.volume = (left.r_max - left.r_min + 1) as u32
                * (left.g_max - left.g_min + 1) as u32
                * (left.b_max - left.b_min + 1) as u32;
            let mut right = *vbox;
            right.r_min = split_val;
            right.volume = (right.r_max - right.r_min + 1) as u32
                * (right.g_max - right.g_min + 1) as u32
                * (right.b_max - right.b_min + 1) as u32;
            (left, right)
        }
        1 => {
            if split_val <= vbox.g_min || split_val > vbox.g_max {
                return None;
            }
            let mut left = *vbox;
            left.g_max = split_val - 1;
            left.volume = (left.r_max - left.r_min + 1) as u32
                * (left.g_max - left.g_min + 1) as u32
                * (left.b_max - left.b_min + 1) as u32;
            let mut right = *vbox;
            right.g_min = split_val;
            right.volume = (right.r_max - right.r_min + 1) as u32
                * (right.g_max - right.g_min + 1) as u32
                * (right.b_max - right.b_min + 1) as u32;
            (left, right)
        }
        2 => {
            if split_val <= vbox.b_min || split_val > vbox.b_max {
                return None;
            }
            let mut left = *vbox;
            left.b_max = split_val - 1;
            left.volume = (left.r_max - left.r_min + 1) as u32
                * (left.g_max - left.g_min + 1) as u32
                * (left.b_max - left.b_min + 1) as u32;
            let mut right = *vbox;
            right.b_min = split_val;
            right.volume = (right.r_max - right.r_min + 1) as u32
                * (right.g_max - right.g_min + 1) as u32
                * (right.b_max - right.b_min + 1) as u32;
            (left, right)
        }
        _ => unreachable!(),
    };

    // Count pixels in each box
    let mut left_count = 0u32;
    let mut right_count = 0u32;
    for r5 in vbox.r_min..=vbox.r_max {
        for g5 in vbox.g_min..=vbox.g_max {
            for b5 in vbox.b_min..=vbox.b_max {
                let count = hist[hist_index(r5, g5, b5)];
                if count > 0 {
                    match best_axis {
                        0 => {
                            if r5 < split_val {
                                left_count += count;
                            } else {
                                right_count += count;
                            }
                        }
                        1 => {
                            if g5 < split_val {
                                left_count += count;
                            } else {
                                right_count += count;
                            }
                        }
                        2 => {
                            if b5 < split_val {
                                left_count += count;
                            } else {
                                right_count += count;
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    if left_count == 0 || right_count == 0 {
        return None;
    }

    let mut left = left;
    left.pixel_count = left_count;
    let mut right = right;
    right.pixel_count = right_count;

    Some((left, right))
}

/// Compute palette from box centroids using histogram data.
fn compute_palette(boxes: &[VBox], hist: &[u32; TOTAL_BINS]) -> Vec<[u8; 3]> {
    let n = boxes.len().min(256);
    if n == 0 {
        return vec![[0u8; 3]; 1];
    }

    let mut palette = Vec::with_capacity(n);

    for vbox in boxes.iter().take(n) {
        let mut sum_r = 0u64;
        let mut sum_g = 0u64;
        let mut sum_b = 0u64;
        let mut count = 0u64;

        for r5 in vbox.r_min..=vbox.r_max {
            for g5 in vbox.g_min..=vbox.g_max {
                for b5 in vbox.b_min..=vbox.b_max {
                    let hc = hist[hist_index(r5, g5, b5)];
                    if hc > 0 {
                        // Use bin center: (bin * 8 + 4) maps 0..31 to 4..252
                        let r = (r5 as u64) * 8 + 4;
                        let g = (g5 as u64) * 8 + 4;
                        let b = (b5 as u64) * 8 + 4;
                        sum_r += r * hc as u64;
                        sum_g += g * hc as u64;
                        sum_b += b * hc as u64;
                        count += hc as u64;
                    }
                }
            }
        }

        if count > 0 {
            palette.push([
                ((sum_r + count / 2) / count) as u8,
                ((sum_g + count / 2) / count) as u8,
                ((sum_b + count / 2) / count) as u8,
            ]);
        } else {
            palette.push([0u8; 3]);
        }
    }

    palette
}

/// Map each pixel to its nearest palette color using Euclidean distance in RGB.
fn map_pixels_to_palette(pixels: &[u8], palette: &[[u8; 3]]) -> Vec<u8> {
    let n = pixels.len() / 3;
    let mut indices = Vec::with_capacity(n);

    for i in 0..n {
        let base = i * 3;
        let r = pixels[base] as i32;
        let g = pixels[base + 1] as i32;
        let b = pixels[base + 2] as i32;

        let mut best_dist = i32::MAX;
        let mut best_idx = 0u8;

        for (pi, pc) in palette.iter().enumerate() {
            let dr = r - pc[0] as i32;
            let dg = g - pc[1] as i32;
            let db = b - pc[2] as i32;
            let dist = dr * dr + dg * dg + db * db;
            if dist < best_dist {
                best_dist = dist;
                best_idx = pi as u8;
            }
        }
        indices.push(best_idx);
    }

    indices
}

// ── Image method ──

impl Image {
    /// Reduce the number of colors in the image using median cut.
    /// PIL-compatible: `quantize(colors=256, method=None, kmeans=0, palette=None, dither=1)`.
    pub fn quantize(
        &self,
        colors: u32,
        _kmeans: u32,
        _palette: Option<&Image>,
        _dither: bool,
    ) -> Result<Image, PilError> {
        let colors = colors.clamp(2, 256);
        let mut result = Image::push_op(
            self,
            PipelineOp::Quantize {
                colors,
                dither: _dither,
            },
        );
        // Set explicit_mode to "P" for quantize output (matches PIL)
        if let Image::Pipeline {
            explicit_mode: ref mut em_field,
            ..
        } = &mut result
        {
            *em_field = Some("P".to_string());
        }
        Ok(result)
    }
}
