//! Image quantization — reduce color palette using median-cut algorithm.
//!
//! Implements PIL's median-cut quantization:
//! 1. Build 3D histogram (RGB, 5-bit precision = 32x32x32 = 32768 buckets)
//! 2. Create sorted lists of unique colors for each axis
//! 3. Recursively split the largest-volume box at the median pixel count
//! 4. Compute palette centroids as weighted averages
//! 5. Map each pixel to nearest palette color

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use std::collections::BTreeMap;

// ── Data structures ──

/// A rectangular volume (box) in RGB space containing a set of colors.
#[derive(Debug, Clone)]
struct VBox {
    r_min: u8,
    r_max: u8,
    g_min: u8,
    g_max: u8,
    b_min: u8,
    b_max: u8,
    pixel_count: u32,
}

// ── Median cut state ──

struct MedianCutState {
    /// All unique histogram entries: (r, g, b, count)
    entries: Vec<(u8, u8, u8, u32)>,
    /// Index entries sorted by each axis: Vec<(value, entry_index)>
    sorted_r: Vec<(u8, usize)>,
    sorted_g: Vec<(u8, usize)>,
    sorted_b: Vec<(u8, usize)>,
    /// Final boxes after splitting.
    boxes: Vec<VBox>,
}

impl MedianCutState {
    /// Build histogram from raw RGB pixel data.
    fn from_pixels(pixels: &[u8]) -> Self {
        let n = pixels.len() / 3;
        let mut hist: BTreeMap<u32, (u8, u8, u8, u32)> = BTreeMap::new();
        for i in 0..n {
            let base = i * 3;
            let r = pixels[base];
            let g = pixels[base + 1];
            let b = pixels[base + 2];
            let key = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            let e = hist.entry(key).or_insert((r, g, b, 0));
            e.3 += 1;
        }

        let n_entries = hist.len();
        let mut entries = Vec::with_capacity(n_entries);
        let mut sorted_r = Vec::with_capacity(n_entries);
        let mut sorted_g = Vec::with_capacity(n_entries);
        let mut sorted_b = Vec::with_capacity(n_entries);

        let mut r_min = u8::MAX;
        let mut r_max = u8::MIN;
        let mut g_min = u8::MAX;
        let mut g_max = u8::MIN;
        let mut b_min = u8::MAX;
        let mut b_max = u8::MIN;
        let mut total = 0u32;

        for (_key, &(r, g, b, count)) in &hist {
            let idx = entries.len();
            entries.push((r, g, b, count));
            sorted_r.push((r, idx));
            sorted_g.push((g, idx));
            sorted_b.push((b, idx));
            r_min = r_min.min(r);
            r_max = r_max.max(r);
            g_min = g_min.min(g);
            g_max = g_max.max(g);
            b_min = b_min.min(b);
            b_max = b_max.max(b);
            total += count;
        }

        sorted_r.sort_by_key(|&(v, _)| v);
        sorted_g.sort_by_key(|&(v, _)| v);
        sorted_b.sort_by_key(|&(v, _)| v);

        MedianCutState {
            entries,
            sorted_r,
            sorted_g,
            sorted_b,
            boxes: vec![VBox {
                r_min,
                r_max,
                g_min,
                g_max,
                b_min,
                b_max,
                pixel_count: total,
            }],
        }
    }

    /// Perform median cut splitting until we have `n_colors` boxes.
    fn split(&mut self, n_colors: usize) {
        let n_colors = n_colors.min(256).max(1);

        while self.boxes.len() < n_colors {
            let mut li = None;
            let mut lc = 0u32;
            for (i, b) in self.boxes.iter().enumerate() {
                if b.pixel_count > lc {
                    lc = b.pixel_count;
                    li = Some(i);
                }
            }

            let idx = match li {
                Some(i) if self.boxes[i].pixel_count > 1 => i,
                _ => break,
            };

            let vbox = self.boxes[idx].clone();
            match self.try_split(&vbox) {
                Some((left, right)) => {
                    self.boxes[idx] = left;
                    self.boxes.push(right);
                }
                None => {
                    // Can't split this box; mark it so we don't try again.
                    self.boxes[idx].pixel_count = 0;
                }
            }
        }

        self.boxes.retain(|b| b.pixel_count > 0);
    }

    /// Try to split a box into two at the median along the best axis.
    fn try_split(&self, vbox: &VBox) -> Option<(VBox, VBox)> {
        if vbox.pixel_count <= 1 {
            return None;
        }

        // Choose best axis: luminance-weighted range (matches PIL).
        let r_range = vbox.r_max as u32 - vbox.r_min as u32;
        let g_range = vbox.g_max as u32 - vbox.g_min as u32;
        let b_range = vbox.b_max as u32 - vbox.b_min as u32;
        let weighted = [r_range * 77, g_range * 150, b_range * 29];
        let mut best_axis = 0;
        let mut best_val = weighted[0];
        for i in 1..3 {
            if weighted[i] > best_val {
                best_val = weighted[i];
                best_axis = i;
            }
        }
        if best_val == 0 {
            return None;
        }

        // Get the sorted indices for the chosen axis.
        let sorted = match best_axis {
            0 => &self.sorted_r,
            1 => &self.sorted_g,
            2 => &self.sorted_b,
            _ => unreachable!(),
        };

        let get_val = |idx: usize| -> u8 {
            let (r, g, b, _) = self.entries[idx];
            match best_axis {
                0 => r,
                1 => g,
                2 => b,
                _ => unreachable!(),
            }
        };

        let in_box = |idx: usize| -> bool {
            let (r, g, b, _) = self.entries[idx];
            r >= vbox.r_min
                && r <= vbox.r_max
                && g >= vbox.g_min
                && g <= vbox.g_max
                && b >= vbox.b_min
                && b <= vbox.b_max
        };

        // Collect entries in this box, in sorted order along chosen axis.
        let mut box_entries: Vec<(usize, u32)> = Vec::new();
        for &(_, idx) in sorted.iter() {
            if in_box(idx) {
                box_entries.push((idx, self.entries[idx].3));
            }
        }

        if box_entries.len() <= 1 {
            return None;
        }

        // Follow PIL's median split approach:
        // Walk entries until cum*2 > total pixel count, then extend
        // LEFT to include same-value entries after the median.
        let mut cum = 0u32;
        let mut split_at = 0usize;

        for (j, &(_, cnt)) in box_entries.iter().enumerate() {
            cum += cnt;
            if cum * 2 > vbox.pixel_count {
                split_at = j + 1;
                break;
            }
        }

        // Extend LEFT to include same-value entries after median (PIL behavior).
        if split_at > 0 && split_at < box_entries.len() {
            let median_val = get_val(box_entries[split_at - 1].0);
            for k in split_at..box_entries.len() {
                if get_val(box_entries[k].0) == median_val {
                    split_at = k + 1;
                } else {
                    break;
                }
            }
        }

        // If extension consumed all entries, trim back to keep at least one
        // entry on each side.
        if split_at >= box_entries.len() {
            split_at = box_entries.len() - 1;
            if split_at == 0 { return None; }
        }

        if split_at == 0 {
            let mid = box_entries.len() / 2;
            if mid == 0 || mid >= box_entries.len() { return None; }
            split_at = mid;
        }

        let (left_e, right_e) = box_entries.split_at(split_at);
        Some((self.build_box(left_e), self.build_box(right_e)))
    }

    /// Build a VBox from a list of histogram entry indices.
    fn build_box(&self, entries: &[(usize, u32)]) -> VBox {
        let mut r_min = u8::MAX;
        let mut r_max = u8::MIN;
        let mut g_min = u8::MAX;
        let mut g_max = u8::MIN;
        let mut b_min = u8::MAX;
        let mut b_max = u8::MIN;
        let mut total = 0u32;
        for &(idx, count) in entries {
            let (r, g, b, _) = self.entries[idx];
            r_min = r_min.min(r);
            r_max = r_max.max(r);
            g_min = g_min.min(g);
            g_max = g_max.max(g);
            b_min = b_min.min(b);
            b_max = b_max.max(b);
            total += count;
        }
        VBox {
            r_min,
            r_max,
            g_min,
            g_max,
            b_min,
            b_max,
            pixel_count: total,
        }
    }

    /// Compute palette from leaf boxes (weighted average of pixels in each box).
    fn compute_palette(&self, pixels: &[u8]) -> Vec<[u8; 3]> {
        let n_boxes = self.boxes.len().min(256);
        if n_boxes == 0 {
            return vec![[0u8; 3]; 1];
        }

        // Annotate each pixel to its containing box.
        let pixel_boxes = self.annotate_pixels(pixels);

        // Accumulate sums and counts per box.
        let mut sums = vec![(0u64, 0u64, 0u64); n_boxes];
        let mut counts = vec![0u64; n_boxes];

        for i in 0..pixels.len() / 3 {
            let bi = pixel_boxes[i];
            if bi < n_boxes {
                let base = i * 3;
                sums[bi].0 += pixels[base] as u64;
                sums[bi].1 += pixels[base + 1] as u64;
                sums[bi].2 += pixels[base + 2] as u64;
                counts[bi] += 1;
            }
        }

        // Compute centroids with rounding (matching PIL's `+ .5` approach).
        let mut palette = vec![[0u8; 3]; n_boxes];
        for bi in 0..n_boxes {
            if counts[bi] > 0 {
                palette[bi][0] = ((sums[bi].0 as f64 / counts[bi] as f64) + 0.5) as u8;
                palette[bi][1] = ((sums[bi].1 as f64 / counts[bi] as f64) + 0.5) as u8;
                palette[bi][2] = ((sums[bi].2 as f64 / counts[bi] as f64) + 0.5) as u8;
            }
        }

        palette
    }

    /// Map each original pixel to its box index by bounds checking.
    fn annotate_pixels(&self, pixels: &[u8]) -> Vec<usize> {
        let n = pixels.len() / 3;
        let mut indices = vec![0usize; n];
        for i in 0..n {
            let base = i * 3;
            let r = pixels[base];
            let g = pixels[base + 1];
            let b = pixels[base + 2];
            for (bi, vbox) in self.boxes.iter().enumerate() {
                if r >= vbox.r_min
                    && r <= vbox.r_max
                    && g >= vbox.g_min
                    && g <= vbox.g_max
                    && b >= vbox.b_min
                    && b <= vbox.b_max
                {
                    indices[i] = bi;
                    break;
                }
            }
        }
        indices
    }
}

/// Map each pixel to its nearest palette color using Euclidean distance in RGB.
fn map_pixels_to_palette(pixels: &[u8], palette: &[[u8; 3]]) -> Vec<u8> {
    let n = pixels.len() / 3;
    let mut indices = Vec::with_capacity(n);

    for i in 0..n {
        let base = i * 3;
        let r = pixels[base];
        let g = pixels[base + 1];
        let b = pixels[base + 2];

        let mut best_dist = u32::MAX;
        let mut best_idx = 0u8;

        for (pi, pc) in palette.iter().enumerate() {
            let dr = r as i32 - pc[0] as i32;
            let dg = g as i32 - pc[1] as i32;
            let db = b as i32 - pc[2] as i32;
            let dist = (dr * dr + dg * dg + db * db) as u32;
            if dist < best_dist {
                best_dist = dist;
                best_idx = pi as u8;
            }
        }
        indices.push(best_idx);
    }

    indices
}

// ── Public entry point ──

/// Run median-cut quantization on RGB pixel data.
/// Returns (palette_indices, palette_colors) where:
/// - palette_indices: 1 byte per pixel (index into palette)
/// - palette_colors: Vec of [r,g,b] arrays
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

    let mut mc = MedianCutState::from_pixels(pixels);
    mc.split(n_colors);

    // Compute palette from the leaf boxes.
    let palette = mc.compute_palette(pixels);
    let n_boxes = palette.len();

    if n_boxes == 0 {
        return (vec![0u8; n], vec![[0u8; 3]; 1]);
    }

    // Pad palette if we got fewer boxes than requested.
    let mut final_palette = Vec::with_capacity(n_colors);
    for i in 0..n_colors {
        if i < n_boxes {
            final_palette.push(palette[i]);
        } else {
            final_palette.push(palette[n_boxes - 1]);
        }
    }

    // Map each pixel to the nearest palette color.
    let indices = map_pixels_to_palette(pixels, &final_palette);

    (indices, final_palette)
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
        Ok(Image::push_op(
            self,
            PipelineOp::Quantize {
                colors,
                dither: _dither,
            },
        ))
    }
}
