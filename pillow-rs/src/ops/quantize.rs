//! Image quantization — reduce color palette using median-cut algorithm.
//!
//! Implements PIL's exact median-cut quantization:
//! 1. Build adaptive 3D color histogram using PIL's PIXEL_HASH table
//!    (adaptive precision — starts at 8-bit, increases scale when
//!    unique entries exceed 65536, merging duplicate scaled bins)
//! 2. Find leaf boxes (non-empty histogram bins) to start
//! 3. Recursively split the largest-volume box at the median pixel count
//! 4. Compute palette centroids from ORIGINAL pixel value averages
//!    (matching PIL's (int)(.5 + avg / count) rounding)
//! 5. Map each pixel to nearest palette color

use std::sync::Arc;

use crate::raster::DynamicImage;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;

// ── VBox: rectangular volume in scaled RGB color space ──

#[derive(Debug, Clone, Copy)]
struct VBox {
    r_min: u8,
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
        // Cast to u32 before adding 1 to avoid u8 overflow when the range
        // is the full 0..255 (e.g., at hash table scale=0).
        let r_ext = r_max as u32 - r_min as u32 + 1;
        let g_ext = g_max as u32 - g_min as u32 + 1;
        let b_ext = b_max as u32 - b_min as u32 + 1;
        let volume = r_ext * g_ext * b_ext;
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
}

// ── PIL's PIXEL_HASH ──
// #define PIXEL_HASH(r, g, b) (r * 463 ^ (g << 8) * 10069 ^ (b << 16) * 64997)

fn pixel_hash(r: u8, g: u8, b: u8) -> u32 {
    (r as u32).wrapping_mul(463)
        ^ ((g as u32) << 8).wrapping_mul(10069)
        ^ ((b as u32) << 16).wrapping_mul(64997)
}

// ── Histogram entry (PIL hash table) ──

/// A single entry in PIL's adaptive color histogram hash table.
///
/// `r`, `g`, `b` are the scaled values (`r >> scale`) used for box splitting.
/// `r_sum`, `g_sum`, `b_sum` are sums of ORIGINAL (unscaled) pixel values —
/// these are preserved through rehashing so that palette centroids are
/// computed from true pixel averages, matching PIL's `avg[][]`.
#[derive(Debug, Clone)]
struct HistEntry {
    r: u8,
    g: u8,
    b: u8,
    count: u64,
    r_sum: u64,
    g_sum: u64,
    b_sum: u64,
}

// ── PIL-style adaptive hash table ──
//
// Uses open addressing with linear probing.
// Hash function: PIXEL_HASH(scaled_r, scaled_g, scaled_b).
// When unique entries exceed 65536, increases `scale` (right-shifts
// color channels by one more bit) and rehashes — entries whose
// scaled values collapse into the same bin are merged.

struct QuantHash {
    entries: Vec<Option<HistEntry>>,
    mask: usize,
    num_entries: usize,
    scale: u32,
}

impl QuantHash {
    const MAX_ENTRIES: usize = 65536;

    fn new() -> Self {
        let size = (Self::MAX_ENTRIES * 2).next_power_of_two(); // 131072
        QuantHash {
            entries: (0..size).map(|_| None).collect(),
            mask: size - 1,
            num_entries: 0,
            scale: 0,
        }
    }

    fn add_pixel(&mut self, r: u8, g: u8, b: u8) {
        let sr = r >> self.scale;
        let sg = g >> self.scale;
        let sb = b >> self.scale;

        let hash = pixel_hash(sr, sg, sb);
        let mut idx = (hash as usize) & self.mask;

        loop {
            match &mut self.entries[idx] {
                None => {
                    self.entries[idx] = Some(HistEntry {
                        r: sr,
                        g: sg,
                        b: sb,
                        count: 1,
                        r_sum: r as u64,
                        g_sum: g as u64,
                        b_sum: b as u64,
                    });
                    self.num_entries += 1;
                    if self.num_entries > Self::MAX_ENTRIES {
                        self.rebuild();
                    }
                    return;
                }
                Some(e) if e.r == sr && e.g == sg && e.b == sb => {
                    e.count += 1;
                    e.r_sum += r as u64;
                    e.g_sum += g as u64;
                    e.b_sum += b as u64;
                    return;
                }
                Some(_) => {
                    idx = (idx + 1) & self.mask;
                }
            }
        }
    }

    fn rebuild(&mut self) {
        self.scale += 1;
        let old = std::mem::take(&mut self.entries);
        let size = old.len();
        self.entries = vec![None; size];
        self.num_entries = 0;
        for entry in old.into_iter().flatten() {
            self.reinsert(entry);
        }
        if self.num_entries > Self::MAX_ENTRIES {
            self.rebuild();
        }
    }

    fn reinsert(&mut self, entry: HistEntry) {
        let new_r = entry.r >> 1;
        let new_g = entry.g >> 1;
        let new_b = entry.b >> 1;
        let hash = pixel_hash(new_r, new_g, new_b);
        let mut idx = (hash as usize) & self.mask;

        loop {
            match &mut self.entries[idx] {
                None => {
                    self.entries[idx] = Some(HistEntry {
                        r: new_r,
                        g: new_g,
                        b: new_b,
                        count: entry.count,
                        r_sum: entry.r_sum,
                        g_sum: entry.g_sum,
                        b_sum: entry.b_sum,
                    });
                    self.num_entries += 1;
                    return;
                }
                Some(e) if e.r == new_r && e.g == new_g && e.b == new_b => {
                    e.count += entry.count;
                    e.r_sum += entry.r_sum;
                    e.g_sum += entry.g_sum;
                    e.b_sum += entry.b_sum;
                    return;
                }
                Some(_) => {
                    idx = (idx + 1) & self.mask;
                }
            }
        }
    }

    fn collect_entries(&self) -> Vec<HistEntry> {
        self.entries.iter().filter_map(|e| e.clone()).collect()
    }
}

// ── Main quantize function ──

/// Quantizes tightly packed RGB pixels with Pillow-style median cut.
///
/// # Inputs
///
/// - `pixels`: RGB byte triplets.
/// - `n_colors`: requested palette size in Pillow's 1..=256 range.
///
/// # Returns
///
/// A tuple containing one palette index per input pixel and a palette of RGB
/// triples.
pub fn median_cut_quantize_rgb(pixels: &[u8], n_colors: usize) -> (Vec<u8>, Vec<[u8; 3]>) {
    let n = pixels.len() / 3;
    if n == 0 {
        return (Vec::new(), vec![[0u8; 3]; 1]);
    }

    let n_colors = n_colors.clamp(1, 256);

    // Step 1: Build PIL-style adaptive hash histogram
    let mut qh = QuantHash::new();
    for i in 0..n {
        let base = i * 3;
        qh.add_pixel(pixels[base], pixels[base + 1], pixels[base + 2]);
    }
    let entries = qh.collect_entries();
    let scale = qh.scale;

    if entries.is_empty() {
        return (vec![0u8; n], vec![[0u8; 3]; 1]);
    }

    // Step 3: Find initial bounding box using hash-table scale.
    // PIL operates at the hash table's scale (starts at 8-bit, increases
    // only when unique entries exceed 65536). No fixed 5-bit reduction.
    let (r_min, r_max, g_min, g_max, b_min, b_max) = match find_initial_bounds(&entries) {
        Some(bounds) => bounds,
        None => return (vec![0u8; n], vec![[0u8; 3]; 1]),
    };

    // Step 4: Create initial box and count pixels
    let mut boxes = vec![VBox::new(r_min, r_max, g_min, g_max, b_min, b_max)];
    let total: u32 = entries
        .iter()
        .filter(|e| vbox_contains(e, &boxes[0]))
        .map(|e| e.count as u32)
        .sum();
    boxes[0].pixel_count = total;

    // Step 5: Build tree using PIL-compatible max-heap by pixelCount.
    // PIL extracts the box with the MOST pixels first (max-heap,
    // box_heap_cmp: `a->pixelCount - b->pixelCount`).
    // Tie-breaking for equal pixelCount: earlier-inserted boxes win
    // (PIL's heap_sift picks left child when children are equal).
    // After splitting, collect leaf boxes in DFS left-to-right order
    // to match PIL's annotate_hash_table palette ordering.
    //
    // NOTE: `split_boxes` function below is no longer used but kept
    // for reference — all logic is now inline here.
    struct TreeNode {
        vbox: VBox,
        left: Option<usize>,
        right: Option<usize>,
    }

    let mut tree: Vec<TreeNode> = Vec::new();
    tree.push(TreeNode {
        vbox: boxes[0],
        left: None,
        right: None,
    });

    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    // Max-heap: primary key pixelCount (descending),
    // tie-break key Reverse(index) so older boxes (lower index) come first.
    let mut heap: BinaryHeap<(u32, Reverse<usize>)> = BinaryHeap::new();
    heap.push((boxes[0].pixel_count, Reverse(0)));

    let max_colors = n_colors;
    let mut num_splits = 0usize;

    while num_splits + 1 < max_colors {
        // Pop leaf with largest pixelCount (splittable only)
        let leaf_idx = loop {
            match heap.pop() {
                Some((_, Reverse(idx))) => {
                    if tree[idx].left.is_none()
                        && tree[idx].vbox.pixel_count > 1
                        && tree[idx].vbox.volume > 0
                    {
                        break idx;
                    }
                    // Not splittable — skip this box (already split,
                    // single pixel, or single color)
                }
                None => break usize::MAX,
            }
        };

        if leaf_idx == usize::MAX {
            break;
        }

        match try_split(&tree[leaf_idx].vbox, &entries) {
            Some((left, right)) => {
                let left_idx = tree.len();
                tree.push(TreeNode {
                    vbox: left,
                    left: None,
                    right: None,
                });
                let right_idx = tree.len();
                tree.push(TreeNode {
                    vbox: right,
                    left: None,
                    right: None,
                });
                tree[leaf_idx].left = Some(left_idx);
                tree[leaf_idx].right = Some(right_idx);

                heap.push((left.pixel_count, Reverse(left_idx)));
                heap.push((right.pixel_count, Reverse(right_idx)));
                num_splits += 1;
            }
            None => {
                // Can't split this leaf further; stays as leaf.
            }
        }
    }

    // Collect leaf boxes in DFS left-to-right order (PIL's
    // annotate_hash_table traversal — this determines palette order).
    fn collect_tree_leaves(tree: &[TreeNode], idx: usize, leaves: &mut Vec<usize>) {
        match (tree[idx].left, tree[idx].right) {
            (Some(l), Some(r)) => {
                // PIL visits right (higher values) before left (lower values).
                collect_tree_leaves(tree, r, leaves);
                collect_tree_leaves(tree, l, leaves);
            }
            (None, None) => leaves.push(idx),
            (_, _) => unreachable!("node must have both or no children"),
        }
    }

    let mut leaf_order: Vec<usize> = Vec::new();
    collect_tree_leaves(&tree, 0, &mut leaf_order);
    boxes = leaf_order.iter().map(|&i| tree[i].vbox).collect();

    // Step 6: Compute palette centroids from ORIGINAL pixel value averages
    let palette = compute_palette(&boxes, &entries);
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

    // Step 7: Map pixels to nearest palette color
    // (box-guided nearest-neighbor search using hash-table scale)
    let indices = map_pixels_to_palette(pixels, &final_palette, &boxes, scale);

    (indices, final_palette)
}

// ── Helper: check if HistEntry falls inside VBox ──

#[inline]
fn vbox_contains(e: &HistEntry, v: &VBox) -> bool {
    e.r >= v.r_min
        && e.r <= v.r_max
        && e.g >= v.g_min
        && e.g <= v.g_max
        && e.b >= v.b_min
        && e.b <= v.b_max
}

// ── Find initial bounding box from histogram entries ──

fn find_initial_bounds(entries: &[HistEntry]) -> Option<(u8, u8, u8, u8, u8, u8)> {
    let mut bounds: Option<(u8, u8, u8, u8, u8, u8)> = None;
    for e in entries {
        bounds = match bounds {
            None => Some((e.r, e.r, e.g, e.g, e.b, e.b)),
            Some((rmin, rmax, gmin, gmax, bmin, bmax)) => Some((
                rmin.min(e.r),
                rmax.max(e.r),
                gmin.min(e.g),
                gmax.max(e.g),
                bmin.min(e.b),
                bmax.max(e.b),
            )),
        };
    }
    bounds
}

// ── Try to split a VBox along the best axis ──

fn try_split(vbox: &VBox, entries: &[HistEntry]) -> Option<(VBox, VBox)> {
    if vbox.pixel_count <= 1 || vbox.volume <= 1 {
        return None;
    }

    // Axis selection with PIL luminance weights (*77, *150, *29)
    let r_weighted = (vbox.r_max - vbox.r_min) as u32 * 77;
    let g_weighted = (vbox.g_max - vbox.g_min) as u32 * 150;
    let b_weighted = (vbox.b_max - vbox.b_min) as u32 * 29;

    let best_axis = if r_weighted >= g_weighted && r_weighted >= b_weighted {
        0
    } else if g_weighted >= r_weighted && g_weighted >= b_weighted {
        1
    } else {
        2
    };

    // Collect entries in this box, along the chosen axis
    let mut axis_entries: Vec<(u8, u32)> = entries
        .iter()
        .filter(|e| vbox_contains(e, vbox))
        .map(|e| {
            let val = match best_axis {
                0 => e.r,
                1 => e.g,
                2 => e.b,
                _ => unreachable!(),
            };
            (val, e.count as u32)
        })
        .collect();

    if axis_entries.is_empty() {
        return None;
    }

    // Sort by channel value, then deduplicate by merging same values
    axis_entries.sort_by_key(|&(v, _)| v);
    let mut deduped: Vec<(u8, u32)> = Vec::new();
    for (val, cnt) in axis_entries {
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

    // Determine the axis min/max for validation
    let (box_min, box_max) = match best_axis {
        0 => (vbox.r_min, vbox.r_max),
        1 => (vbox.g_min, vbox.g_max),
        2 => (vbox.b_min, vbox.b_max),
        _ => unreachable!(),
    };

    // Compute split value: split BETWEEN the last group that is at/below
    // midpoint (group split_idx-1) and the first group above midpoint
    // (group split_idx).  The entry at split_idx goes to the RIGHT box.
    //
    // When split_idx == 0 the first group alone exceeds midpoint — we
    // split AFTER it (value+1) so the group goes to the LEFT box.
    let split_val = if split_idx > 0 {
        deduped[split_idx - 1].0.saturating_add(1)
    } else {
        deduped[0].0.saturating_add(1)
    };

    // Validate: left and right must both be non-empty.
    if split_val <= box_min || split_val > box_max {
        return None;
    }

    // Helper to compute volume, avoiding u8 overflow.
    fn box_volume(r_min: u8, r_max: u8, g_min: u8, g_max: u8, b_min: u8, b_max: u8) -> u32 {
        let r_ext = r_max as u32 - r_min as u32 + 1;
        let g_ext = g_max as u32 - g_min as u32 + 1;
        let b_ext = b_max as u32 - b_min as u32 + 1;
        r_ext * g_ext * b_ext
    }

    // Build left and right boxes
    let (mut left, mut right) = match best_axis {
        0 => {
            if split_val <= vbox.r_min || split_val > vbox.r_max {
                return None;
            }
            let mut l = *vbox;
            l.r_max = split_val - 1;
            l.volume = box_volume(l.r_min, l.r_max, l.g_min, l.g_max, l.b_min, l.b_max);
            let mut r = *vbox;
            r.r_min = split_val;
            r.volume = box_volume(r.r_min, r.r_max, r.g_min, r.g_max, r.b_min, r.b_max);
            (l, r)
        }
        1 => {
            if split_val <= vbox.g_min || split_val > vbox.g_max {
                return None;
            }
            let mut l = *vbox;
            l.g_max = split_val - 1;
            l.volume = box_volume(l.r_min, l.r_max, l.g_min, l.g_max, l.b_min, l.b_max);
            let mut r = *vbox;
            r.g_min = split_val;
            r.volume = box_volume(r.r_min, r.r_max, r.g_min, r.g_max, r.b_min, r.b_max);
            (l, r)
        }
        2 => {
            if split_val <= vbox.b_min || split_val > vbox.b_max {
                return None;
            }
            let mut l = *vbox;
            l.b_max = split_val - 1;
            l.volume = box_volume(l.r_min, l.r_max, l.g_min, l.g_max, l.b_min, l.b_max);
            let mut r = *vbox;
            r.b_min = split_val;
            r.volume = box_volume(r.r_min, r.r_max, r.g_min, r.g_max, r.b_min, r.b_max);
            (l, r)
        }
        _ => unreachable!(),
    };

    // Count pixels and recompute actual bounds from entries (PIL computes
    // bounds from pixel data, not from split values — this avoids looser
    // explicit bounds that would affect axis-weight calculation).
    let left_bounds = entries.iter().filter(|e| vbox_contains(e, &left)).fold(
        None::<(u8, u8, u8, u8, u8, u8)>,
        |acc, e| match acc {
            None => Some((e.r, e.r, e.g, e.g, e.b, e.b)),
            Some((rmin, rmax, gmin, gmax, bmin, bmax)) => Some((
                rmin.min(e.r),
                rmax.max(e.r),
                gmin.min(e.g),
                gmax.max(e.g),
                bmin.min(e.b),
                bmax.max(e.b),
            )),
        },
    );
    let right_bounds = entries.iter().filter(|e| vbox_contains(e, &right)).fold(
        None::<(u8, u8, u8, u8, u8, u8)>,
        |acc, e| match acc {
            None => Some((e.r, e.r, e.g, e.g, e.b, e.b)),
            Some((rmin, rmax, gmin, gmax, bmin, bmax)) => Some((
                rmin.min(e.r),
                rmax.max(e.r),
                gmin.min(e.g),
                gmax.max(e.g),
                bmin.min(e.b),
                bmax.max(e.b),
            )),
        },
    );

    let (left_bounds, left_count) = match left_bounds {
        Some((rmin, rmax, gmin, gmax, bmin, bmax)) => {
            let cnt: u32 = entries
                .iter()
                .filter(|e| vbox_contains(e, &left))
                .map(|e| e.count as u32)
                .sum();
            ((rmin, rmax, gmin, gmax, bmin, bmax), cnt)
        }
        None => return None,
    };
    let (right_bounds, right_count) = match right_bounds {
        Some((rmin, rmax, gmin, gmax, bmin, bmax)) => {
            let cnt: u32 = entries
                .iter()
                .filter(|e| vbox_contains(e, &right))
                .map(|e| e.count as u32)
                .sum();
            ((rmin, rmax, gmin, gmax, bmin, bmax), cnt)
        }
        None => return None,
    };

    if left_count == 0 || right_count == 0 {
        return None;
    }

    let (l_rmin, l_rmax, l_gmin, l_gmax, l_bmin, l_bmax) = left_bounds;
    let (r_rmin, r_rmax, r_gmin, r_gmax, r_bmin, r_bmax) = right_bounds;

    left = VBox {
        r_min: l_rmin,
        r_max: l_rmax,
        g_min: l_gmin,
        g_max: l_gmax,
        b_min: l_bmin,
        b_max: l_bmax,
        pixel_count: left_count,
        volume: box_volume(l_rmin, l_rmax, l_gmin, l_gmax, l_bmin, l_bmax),
    };
    right = VBox {
        r_min: r_rmin,
        r_max: r_rmax,
        g_min: r_gmin,
        g_max: r_gmax,
        b_min: r_bmin,
        b_max: r_bmax,
        pixel_count: right_count,
        volume: box_volume(r_rmin, r_rmax, r_gmin, r_gmax, r_bmin, r_bmax),
    };

    Some((left, right))
}

// ── Compute palette from box centroids using original pixel value sums ──

fn compute_palette(boxes: &[VBox], entries: &[HistEntry]) -> Vec<[u8; 3]> {
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

        for e in entries {
            if vbox_contains(e, vbox) {
                sum_r += e.r_sum;
                sum_g += e.g_sum;
                sum_b += e.b_sum;
                count += e.count;
            }
        }

        if count > 0 {
            // PIL: (int)(.5 + (double)avg / (double)count)
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

// ── Map pixels to palette with PIL-style box-guided NN search ──

fn map_pixels_to_palette(
    pixels: &[u8],
    palette: &[[u8; 3]],
    boxes: &[VBox],
    scale: u32,
) -> Vec<u8> {
    let n = pixels.len() / 3;
    let n_colors = palette.len();

    if n_colors <= 1 || boxes.is_empty() {
        return vec![0u8; n];
    }

    // ── Step 2: Pre-compute pairwise distance matrix ──────────────────
    let mut dists: Vec<u32> = vec![0u32; n_colors * n_colors];
    for i in 0..n_colors {
        let pi = &palette[i];
        let row_base = i * n_colors;
        for j in 0..n_colors {
            let pj = &palette[j];
            let dr = pi[0] as i32 - pj[0] as i32;
            let dg = pi[1] as i32 - pj[1] as i32;
            let db = pi[2] as i32 - pj[2] as i32;
            dists[row_base + j] = (dr * dr + dg * dg + db * db) as u32;
        }
    }

    // ── Step 3: Sorted distance pointers per palette entry ──────────
    let mut dist_sort: Vec<Vec<(u32, usize)>> = Vec::with_capacity(n_colors);
    for i in 0..n_colors {
        let row_base = i * n_colors;
        let mut entries: Vec<(u32, usize)> = (0..n_colors)
            .filter(|&j| j != i)
            .map(|j| (dists[row_base + j], j))
            .collect();
        entries.sort_by_key(|&(d, _)| d);
        dist_sort.push(entries);
    }

    // ── Step 4: Map pixels using box-guided nearest-neighbor search ──
    let mut indices = Vec::with_capacity(n);

    for i in 0..n {
        let base = i * 3;
        let r = pixels[base];
        let g = pixels[base + 1];
        let b = pixels[base + 2];

        // Find containing box at hash-table scale (exact, matching PIL's
        // medianBoxHash). Project pixel to scale and check box bounds.
        let sr = (r as u32 >> scale) as u8;
        let sg = (g as u32 >> scale) as u8;
        let sb = (b as u32 >> scale) as u8;
        let start_idx = boxes
            .iter()
            .position(|b| {
                sr >= b.r_min
                    && sr <= b.r_max
                    && sg >= b.g_min
                    && sg <= b.g_max
                    && sb >= b.b_min
                    && sb <= b.b_max
            })
            .unwrap_or(0);

        let dr = r as i32 - palette[start_idx][0] as i32;
        let dg = g as i32 - palette[start_idx][1] as i32;
        let db = b as i32 - palette[start_idx][2] as i32;
        let initial_dist = (dr * dr + dg * dg + db * db) as u32;

        let bound = initial_dist << 2;

        let mut best_dist = initial_dist;
        let mut best_idx = start_idx as u8;

        for &(ref_dist, j) in &dist_sort[start_idx] {
            if ref_dist > bound {
                break;
            }
            let dr = r as i32 - palette[j][0] as i32;
            let dg = g as i32 - palette[j][1] as i32;
            let db = b as i32 - palette[j][2] as i32;
            let d = (dr * dr + dg * dg + db * db) as u32;
            if d < best_dist {
                best_dist = d;
                best_idx = j as u8;
            }
        }

        indices.push(best_idx);
    }

    indices
}

// ═══════════════════════════════════════════════════════════════════════════════
// ── Octree quantization (FASTOCTREE) — PIL's algorithm for RGBA images ──
// ═══════════════════════════════════════════════════════════════════════════════
//
// PIL's Quantize.FASTOCTREE (method=2) uses a multi-dimensional color cube
// approach. Two cubes are created at different bit depths:
//   - Fine cube:   higher resolution  (e.g., 4×4×4×0 = 4096 buckets for RGB)
//   - Coarse cube: lower resolution   (e.g., 2×2×2×0 = 64 buckets for RGB)
//
// With alpha, a 4th dimension is added:
//   - Fine:   3×4×3×3 = 8192 buckets
//   - Coarse: 2×2×2×2 = 256 buckets
//
// Algorithm:
//   1. Populate fine cube from pixel data
//   2. Derive coarse cube from fine cube (resolution reduction)
//   3. Count used coarse buckets, limit to n_colors
//   4. Sort fine buckets by popularity, take remaining slots
//   5. Subtract fine colors from coarse to avoid double-counting
//   6. Build lookup cube (coarse resolution first, then fine overlay)
//   7. Map each pixel to its palette index via lookup cube

/// Bit depths for fine and coarse cubes — RGBA mode (with alpha).
const CUBE_LEVELS_RGBA: [u32; 8] = [3, 4, 3, 3, 2, 2, 2, 2];

// Behavioral port of Pillow 12.2.0 src/libImaging/QuantOctree.c. In
// particular, the cube is laid out R-major and copy_octree_cube handles both
// reduction and expansion. Palette order for equally populated buckets is
// observable, so the sorter reproduces the Apple Libc qsort used by the pinned
// macOS Pillow oracle rather than adding an artificial color tie-breaker.

#[derive(Clone, Default)]
struct OctreeBucket {
    count: u32,
    sums: [u64; 4],
}

impl OctreeBucket {
    fn add_color(&mut self, color: [u8; 4]) {
        self.count = self.count.saturating_add(1);
        for (sum, channel) in self.sums.iter_mut().zip(color) {
            *sum = sum.saturating_add(u64::from(channel));
        }
    }

    fn add_bucket(&mut self, other: &Self) {
        self.count = self.count.saturating_add(other.count);
        for (sum, other_sum) in self.sums.iter_mut().zip(other.sums) {
            *sum = sum.saturating_add(other_sum);
        }
    }

    fn average(&self) -> [u8; 4] {
        if self.count == 0 {
            return [0; 4];
        }
        // Pillow 12.2.0 QuantOctree.c:avg_color_from_color_bucket converts
        // both operands to C `float`, truncates the quotient to `int`, then
        // applies CLIP8. The f32 rounding before division is observable for
        // large buckets and must not be replaced with integer division.
        let count = self.count as f32;
        std::array::from_fn(|channel| {
            let average = (self.sums[channel] as f32 / count) as i32;
            average.clamp(0, 255) as u8
        })
    }
}

struct OctreeCube {
    bits: [u32; 4],
    widths: [usize; 4],
    offsets: [u32; 4],
    buckets: Vec<OctreeBucket>,
}

impl OctreeCube {
    fn new(bits: [u32; 4]) -> Self {
        debug_assert!(bits.iter().all(|&value| value < usize::BITS));
        debug_assert!(bits.iter().copied().sum::<u32>() < usize::BITS);
        let widths = bits.map(|value| 1usize << value);
        let offsets = [
            bits[1].saturating_add(bits[2]).saturating_add(bits[3]),
            bits[2].saturating_add(bits[3]),
            bits[3],
            0,
        ];
        let size = widths.into_iter().product();
        Self {
            bits,
            widths,
            offsets,
            buckets: vec![OctreeBucket::default(); size],
        }
    }

    fn offset_position(&self, values: [usize; 4]) -> usize {
        values
            .into_iter()
            .zip(self.offsets)
            .fold(0usize, |offset, (value, shift)| offset | (value << shift))
    }

    fn offset(&self, color: [u8; 4]) -> usize {
        let values = std::array::from_fn(|channel| {
            (usize::from(color[channel]) >> 8u32.saturating_sub(self.bits[channel]))
                & self.widths[channel].saturating_sub(1)
        });
        self.offset_position(values)
    }

    fn add_color(&mut self, color: [u8; 4]) {
        let offset = self.offset(color);
        self.buckets[offset].add_color(color);
    }

    fn used(&self) -> usize {
        self.buckets
            .iter()
            .filter(|bucket| bucket.count > 0)
            .count()
    }
}

fn copy_octree_cube(cube: &OctreeCube, bits: [u32; 4]) -> OctreeCube {
    let mut result = OctreeCube::new(bits);
    let mut source_reduce = [0u32; 4];
    let mut destination_reduce = [0u32; 4];
    let widths: [usize; 4] = std::array::from_fn(|channel| {
        if cube.bits[channel] > bits[channel] {
            destination_reduce[channel] = cube.bits[channel].saturating_sub(bits[channel]);
            cube.widths[channel]
        } else {
            source_reduce[channel] = bits[channel].saturating_sub(cube.bits[channel]);
            result.widths[channel]
        }
    });
    for r in 0..widths[0] {
        for g in 0..widths[1] {
            for b in 0..widths[2] {
                for a in 0..widths[3] {
                    let values = [r, g, b, a];
                    let source = cube.offset_position(std::array::from_fn(|channel| {
                        values[channel] >> source_reduce[channel]
                    }));
                    let destination = result.offset_position(std::array::from_fn(|channel| {
                        values[channel] >> destination_reduce[channel]
                    }));
                    result.buckets[destination].add_bucket(&cube.buckets[source]);
                }
            }
        }
    }
    result
}

fn octree_bucket_order(left: &OctreeBucket, right: &OctreeBucket) -> std::cmp::Ordering {
    right.count.cmp(&left.count)
}

fn median_octree_bucket(values: &[OctreeBucket], a: usize, b: usize, c: usize) -> usize {
    if octree_bucket_order(&values[a], &values[b]).is_lt() {
        if octree_bucket_order(&values[b], &values[c]).is_lt() {
            b
        } else if octree_bucket_order(&values[a], &values[c]).is_lt() {
            c
        } else {
            a
        }
    } else if octree_bucket_order(&values[b], &values[c]).is_gt() {
        b
    } else if octree_bucket_order(&values[a], &values[c]).is_lt() {
        a
    } else {
        c
    }
}

fn insertion_sort_octree_buckets(values: &mut [OctreeBucket], swap_limit: Option<usize>) -> bool {
    let mut swaps = 0usize;
    for right in 1..values.len() {
        let mut cursor = right;
        while cursor > 0
            && octree_bucket_order(&values[cursor.saturating_sub(1)], &values[cursor]).is_gt()
        {
            values.swap(cursor, cursor.saturating_sub(1));
            swaps = swaps.saturating_add(1);
            if swap_limit.is_some_and(|limit| swaps > limit) {
                return false;
            }
            cursor = cursor.saturating_sub(1);
        }
    }
    true
}

fn swap_octree_bucket_ranges(
    values: &mut [OctreeBucket],
    left: usize,
    right: usize,
    length: usize,
) {
    for offset in 0..length {
        values.swap(left.saturating_add(offset), right.saturating_add(offset));
    }
}

fn apple_qsort_octree_buckets(values: &mut [OctreeBucket]) {
    let mut start = 0usize;
    let mut length = values.len();
    loop {
        if length <= 7 {
            insertion_sort_octree_buckets(&mut values[start..start.saturating_add(length)], None);
            return;
        }
        let mut low = start;
        let mut middle = start.saturating_add(length.div_euclid(2));
        let mut high = start.saturating_add(length).saturating_sub(1);
        if length > 40 {
            let distance = length.div_euclid(8);
            low = median_octree_bucket(
                values,
                low,
                low.saturating_add(distance),
                low.saturating_add(distance.saturating_mul(2)),
            );
            middle = median_octree_bucket(
                values,
                middle.saturating_sub(distance),
                middle,
                middle.saturating_add(distance),
            );
            high = median_octree_bucket(
                values,
                high.saturating_sub(distance.saturating_mul(2)),
                high.saturating_sub(distance),
                high,
            );
        }
        middle = median_octree_bucket(values, low, middle, high);
        values.swap(start, middle);
        let mut equal_left = start.saturating_add(1);
        let mut scan_left = start.saturating_add(1);
        let mut scan_right = start.saturating_add(length).saturating_sub(1);
        let mut equal_right = scan_right;
        let mut swapped = false;
        loop {
            while scan_left <= scan_right {
                let ordering = octree_bucket_order(&values[scan_left], &values[start]);
                if ordering.is_gt() {
                    break;
                }
                if ordering.is_eq() {
                    values.swap(equal_left, scan_left);
                    equal_left = equal_left.saturating_add(1);
                    swapped = true;
                }
                scan_left = scan_left.saturating_add(1);
            }
            while scan_left <= scan_right {
                let ordering = octree_bucket_order(&values[scan_right], &values[start]);
                if ordering.is_lt() {
                    break;
                }
                if ordering.is_eq() {
                    values.swap(scan_right, equal_right);
                    equal_right = equal_right.saturating_sub(1);
                    swapped = true;
                }
                scan_right = scan_right.saturating_sub(1);
            }
            if scan_left > scan_right {
                break;
            }
            values.swap(scan_left, scan_right);
            swapped = true;
            scan_left = scan_left.saturating_add(1);
            scan_right = scan_right.saturating_sub(1);
        }
        let end = start.saturating_add(length);
        let left_equal = equal_left
            .saturating_sub(start)
            .min(scan_left.saturating_sub(equal_left));
        swap_octree_bucket_ranges(
            values,
            start,
            scan_left.saturating_sub(left_equal),
            left_equal,
        );
        let right_equal = equal_right
            .saturating_sub(scan_right)
            .min(end.saturating_sub(equal_right).saturating_sub(1));
        swap_octree_bucket_ranges(
            values,
            scan_left,
            end.saturating_sub(right_equal),
            right_equal,
        );
        if !swapped {
            let limit = 1usize.saturating_add(length.div_euclid(4));
            if insertion_sort_octree_buckets(&mut values[start..end], Some(limit)) {
                return;
            }
        }
        let left_length = scan_left.saturating_sub(equal_left);
        let right_length = equal_right.saturating_sub(scan_right);
        if left_length <= right_length {
            if left_length > 1 {
                apple_qsort_octree_buckets(&mut values[start..start.saturating_add(left_length)]);
            }
            if right_length <= 1 {
                return;
            }
            start = end.saturating_sub(right_length);
            length = right_length;
        } else {
            if right_length > 1 {
                apple_qsort_octree_buckets(&mut values[end.saturating_sub(right_length)..end]);
            }
            if left_length <= 1 {
                return;
            }
            length = left_length;
        }
    }
}

fn sorted_octree_buckets(cube: &OctreeCube) -> Vec<OctreeBucket> {
    let mut buckets = cube.buckets.clone();
    apple_qsort_octree_buckets(&mut buckets);
    buckets
}

fn subtract_octree_buckets(cube: &mut OctreeCube, buckets: &[OctreeBucket]) {
    for bucket in buckets.iter().filter(|bucket| bucket.count > 0) {
        let offset = cube.offset(bucket.average());
        let destination = &mut cube.buckets[offset];
        destination.count = destination.count.saturating_sub(bucket.count);
        for (sum, value) in destination.sums.iter_mut().zip(bucket.sums) {
            *sum = sum.saturating_sub(value);
        }
    }
}

fn add_octree_lookup(cube: &mut OctreeCube, palette: &[OctreeBucket], offset: usize) {
    for index in (offset..palette.len()).rev() {
        let bucket = &palette[index];
        let position = cube.offset(bucket.average());
        cube.buckets[position].count = index as u32;
    }
}

/// Main octree quantization function, matching Pillow's Quantize.FASTOCTREE.
///
/// `pixels` is flat RGBA byte data (4 bytes per pixel).
/// Returns (palette_indices, palette_bytes_rgba).
fn quantize_octree_rgba(
    pixels: &[u8],
    w: u32,
    h: u32,
    n_colors: usize,
) -> Result<(Vec<u8>, Vec<u8>), PilError> {
    let dimensions = CheckedDims::new(w, h, 1)?;
    let fine_bits = [
        CUBE_LEVELS_RGBA[0],
        CUBE_LEVELS_RGBA[1],
        CUBE_LEVELS_RGBA[2],
        CUBE_LEVELS_RGBA[3],
    ];
    let coarse_bits = [
        CUBE_LEVELS_RGBA[4],
        CUBE_LEVELS_RGBA[5],
        CUBE_LEVELS_RGBA[6],
        CUBE_LEVELS_RGBA[7],
    ];
    let mut colors = pixels
        .chunks_exact(4)
        .take(dimensions.total_pixels())
        .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
        .collect::<Vec<_>>();
    // Quant.c normalizes every fully transparent pixel to the first one's RGB
    // before FASTOCTREE. Invisible garbage channels must not consume colors or
    // cause distinct indices for pixels that Pillow treats as equivalent.
    if let Some(first) = colors.iter().find(|color| color[3] == 0).copied() {
        for color in &mut colors {
            if color[3] == 0 {
                color[..3].copy_from_slice(&first[..3]);
            }
        }
    }

    let mut fine = OctreeCube::new(fine_bits);
    for &color in &colors {
        fine.add_color(color);
    }
    let mut coarse = copy_octree_cube(&fine, coarse_bits);
    let mut coarse_count = coarse.used().min(n_colors);
    let mut fine_count = n_colors.saturating_sub(coarse_count);
    let fine_palette = sorted_octree_buckets(&fine);
    subtract_octree_buckets(&mut coarse, &fine_palette[..fine_count]);
    while coarse_count > coarse.used() {
        let already_subtracted = fine_count;
        coarse_count = coarse.used();
        fine_count = n_colors.saturating_sub(coarse_count);
        subtract_octree_buckets(&mut coarse, &fine_palette[already_subtracted..fine_count]);
    }
    let coarse_palette = sorted_octree_buckets(&coarse);
    let mut palette = coarse_palette[..coarse_count].to_vec();
    palette.extend_from_slice(&fine_palette[..fine_count]);

    let mut coarse_lookup = OctreeCube::new(coarse_bits);
    add_octree_lookup(&mut coarse_lookup, &palette[..coarse_count], 0);
    let mut lookup = copy_octree_cube(&coarse_lookup, fine_bits);
    add_octree_lookup(&mut lookup, &palette, coarse_count);

    let indices = colors
        .iter()
        .map(|&color| lookup.buckets[lookup.offset(color)].count as u8)
        .collect();
    let palette_bytes = palette
        .iter()
        .flat_map(OctreeBucket::average)
        .collect::<Vec<_>>();
    Ok((indices, palette_bytes))
}

// ═══════════════════════════════════════════════════════════════════════════════
// ── Image.quantize method ──
// ═══════════════════════════════════════════════════════════════════════════════

// ── WEB palette (PIL's default fixed palette for convert("P")) ──
//
// PIL's convert("P") with default palette=Palette.WEB uses a fixed 226-color palette:
// - Indices 0-10:   reserved (black: 0,0,0)
// - Indices 11-225: web-safe color cube at values {0,51,102,153,204,255}
//   PIL ordering: for b in [0,51,102,153,204,255], for g in [0,51,102,153,204,255],
//   for r in [0,51,102,153,204,255], skipping (0,0,0) at the first position.
//   216 cube entries minus the (0,0,0) duplicate = 215 + 11 reserved = 226 total.

const WEB_PALETTE: [u8; 678] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 51, 0, 0, 102, 0, 0, 153, 0, 0, 204, 0, 0, 255, 0, 0, 0, 51, 0, 51, 51, 0, 102, 51, 0, 153,
    51, 0, 204, 51, 0, 255, 51, 0, 0, 102, 0, 51, 102, 0, 102, 102, 0, 153, 102, 0, 204, 102, 0,
    255, 102, 0, 0, 153, 0, 51, 153, 0, 102, 153, 0, 153, 153, 0, 204, 153, 0, 255, 153, 0, 0, 204,
    0, 51, 204, 0, 102, 204, 0, 153, 204, 0, 204, 204, 0, 255, 204, 0, 0, 255, 0, 51, 255, 0, 102,
    255, 0, 153, 255, 0, 204, 255, 0, 255, 255, 0, 0, 0, 51, 51, 0, 51, 102, 0, 51, 153, 0, 51,
    204, 0, 51, 255, 0, 51, 0, 51, 51, 51, 51, 51, 102, 51, 51, 153, 51, 51, 204, 51, 51, 255, 51,
    51, 0, 102, 51, 51, 102, 51, 102, 102, 51, 153, 102, 51, 204, 102, 51, 255, 102, 51, 0, 153,
    51, 51, 153, 51, 102, 153, 51, 153, 153, 51, 204, 153, 51, 255, 153, 51, 0, 204, 51, 51, 204,
    51, 102, 204, 51, 153, 204, 51, 204, 204, 51, 255, 204, 51, 0, 255, 51, 51, 255, 51, 102, 255,
    51, 153, 255, 51, 204, 255, 51, 255, 255, 51, 0, 0, 102, 51, 0, 102, 102, 0, 102, 153, 0, 102,
    204, 0, 102, 255, 0, 102, 0, 51, 102, 51, 51, 102, 102, 51, 102, 153, 51, 102, 204, 51, 102,
    255, 51, 102, 0, 102, 102, 51, 102, 102, 102, 102, 102, 153, 102, 102, 204, 102, 102, 255, 102,
    102, 0, 153, 102, 51, 153, 102, 102, 153, 102, 153, 153, 102, 204, 153, 102, 255, 153, 102, 0,
    204, 102, 51, 204, 102, 102, 204, 102, 153, 204, 102, 204, 204, 102, 255, 204, 102, 0, 255,
    102, 51, 255, 102, 102, 255, 102, 153, 255, 102, 204, 255, 102, 255, 255, 102, 0, 0, 153, 51,
    0, 153, 102, 0, 153, 153, 0, 153, 204, 0, 153, 255, 0, 153, 0, 51, 153, 51, 51, 153, 102, 51,
    153, 153, 51, 153, 204, 51, 153, 255, 51, 153, 0, 102, 153, 51, 102, 153, 102, 102, 153, 153,
    102, 153, 204, 102, 153, 255, 102, 153, 0, 153, 153, 51, 153, 153, 102, 153, 153, 153, 153,
    153, 204, 153, 153, 255, 153, 153, 0, 204, 153, 51, 204, 153, 102, 204, 153, 153, 204, 153,
    204, 204, 153, 255, 204, 153, 0, 255, 153, 51, 255, 153, 102, 255, 153, 153, 255, 153, 204,
    255, 153, 255, 255, 153, 0, 0, 204, 51, 0, 204, 102, 0, 204, 153, 0, 204, 204, 0, 204, 255, 0,
    204, 0, 51, 204, 51, 51, 204, 102, 51, 204, 153, 51, 204, 204, 51, 204, 255, 51, 204, 0, 102,
    204, 51, 102, 204, 102, 102, 204, 153, 102, 204, 204, 102, 204, 255, 102, 204, 0, 153, 204, 51,
    153, 204, 102, 153, 204, 153, 153, 204, 204, 153, 204, 255, 153, 204, 0, 204, 204, 51, 204,
    204, 102, 204, 204, 153, 204, 204, 204, 204, 204, 255, 204, 204, 0, 255, 204, 51, 255, 204,
    102, 255, 204, 153, 255, 204, 204, 255, 204, 255, 255, 204, 0, 0, 255, 51, 0, 255, 102, 0, 255,
    153, 0, 255, 204, 0, 255, 255, 0, 255, 0, 51, 255, 51, 51, 255, 102, 51, 255, 153, 51, 255,
    204, 51, 255, 255, 51, 255, 0, 102, 255, 51, 102, 255, 102, 102, 255, 153, 102, 255, 204, 102,
    255, 255, 102, 255, 0, 153, 255, 51, 153, 255, 102, 153, 255, 153, 153, 255, 204, 153, 255,
    255, 153, 255, 0, 204, 255, 51, 204, 255, 102, 204, 255, 153, 204, 255, 204, 204, 255, 255,
    204, 255, 0, 255, 255, 51, 255, 255, 102, 255, 255, 153, 255, 255, 204, 255, 255, 255, 255,
    255,
];

/// Quantizes RGB pixels to Pillow's default web palette.
///
/// `pixels` is tightly packed RGB data for a `w` by `h` image. When `dither` is
/// true, Pillow-compatible Floyd-Steinberg error diffusion is applied.
///
/// # Returns
///
/// A tuple of palette index bytes and flat RGB palette bytes.
pub fn web_palette_quantize(
    pixels: &[u8],
    w: u32,
    h: u32,
    dither: bool,
) -> Result<(Vec<u8>, Vec<u8>), PilError> {
    let dims_pixels = CheckedDims::new(w, h, 1)?;
    let mut out = dims_pixels.alloc_buffer();

    if dither {
        // PIL-identical Floyd-Steinberg dither with WEB palette.
        // Uses PIL's 3x/5x/7x accumulator pattern (single division by 16 at read time)
        // instead of dividing each propagated fraction separately.
        let wu = w as usize;
        // Error array: one row + 1 column, 3 channels interleaved
        let mut errors = vec![0i32; (wu + 1) * 3];

        #[inline]
        fn clip8(v: i32) -> u8 {
            if v < 0 {
                0
            } else if v > 255 {
                255
            } else {
                v as u8
            }
        }

        for y in 0..h as usize {
            // Per-channel accumulators (carry 7x, delayed 5x, delayed 1x)
            let mut l = [0i32; 3];
            let mut l0 = [0i32; 3];
            let mut l1 = [0i32; 3];

            for x in 0..wu {
                let src_i = (y * wu + x) * 3;
                let e_ptr = x * 3; // base of errors for this column

                // Read pixel + accumulated error (divided by 16 once)
                for ch in 0..3 {
                    let val = pixels[src_i + ch] as i32;
                    let acc = l[ch] + errors[e_ptr + 3 + ch]; // errors[x+1] = next col
                    l[ch] = clip8(val + acc / 16) as i32;
                }
                let r = l[0] as u8;
                let g = l[1] as u8;
                let b = l[2] as u8;

                let (best_idx, pr, pg, pb) = find_nearest_web(r, g, b);
                out[y * wu + x] = best_idx;

                // Compute error = corrected_input - palette_output
                for ch in 0..3 {
                    l[ch] -= [pr as i32, pg as i32, pb as i32][ch];
                }

                // PIL's 3x/5x/7x accumulation pattern
                for ch in 0..3 {
                    let err = l[ch];
                    let r2 = err;
                    let d2 = err + err;
                    l[ch] = err + d2; // 3x
                    errors[e_ptr + ch] = l[ch] + l0[ch]; // store 3x + l0
                    l[ch] += d2; // 5x
                    l0[ch] = l[ch] + l1[ch]; // 5x + l1
                    l1[ch] = r2; // 1x (delayed)
                    l[ch] += d2; // 7x (carry to next pixel)
                }
            }

            // Post-loop: PIL's topalette writes ONLY B-channel accumulators
            // at errors[w*3 + 0..2] — b0, b1, b2 (NOT per-channel l0 values).
            // This effectively loses R and G error at the right edge, but we
            // must match PIL's exact behavior for pixel-level parity.
            // b0 = 5*e_B_last + e_B_second_last, b1 = b2 = e_B_last
            let e_end = wu * 3;
            errors[e_end] = l0[2]; // b0: B channel's 5x delay
            errors[e_end + 1] = l1[2]; // b1: B channel's 1x delay (original error)
            errors[e_end + 2] = l1[2]; // b2: B channel's original error (same as b1)
        }
    } else {
        // No dither: nearest-neighbor mapping
        for (pi, out_pixel) in out.iter_mut().enumerate() {
            let src_i = pi * 3;
            let (best_idx, _, _, _) =
                find_nearest_web(pixels[src_i], pixels[src_i + 1], pixels[src_i + 2]);
            *out_pixel = best_idx;
        }
    }

    Ok((out, WEB_PALETTE.to_vec()))
}

/// Number of colors in the WEB palette (11 reserved + 215 web-safe cube).
const WEB_PALETTE_COLORS: usize = 226;

/// PIL-identical palette lookup cache.
/// PIL divides color space into 8×8×8 cells (size 32×32×32 each).
/// Within each cell, 8×8×8 sub-positions (step 4) are pre-cached.
/// Each pixel maps to its cell+sub-position for the cached result.
struct WebPaletteCache {
    /// Cache per cell: 512 cells × 512 entries per cell
    cells: Vec<Option<[u8; 512]>>,
}

impl WebPaletteCache {
    fn new() -> Self {
        let mut cells = Vec::with_capacity(512);
        for _ in 0..512 {
            cells.push(None);
        }
        WebPaletteCache { cells }
    }

    fn cell_index(r: u8, g: u8, b: u8) -> usize {
        ((r as usize) >> 5) | (((g as usize) >> 5) << 3) | (((b as usize) >> 5) << 6)
    }

    fn sub_index(r: u8, g: u8, b: u8, r0: u8, g0: u8, b0: u8) -> usize {
        let ri = ((r.saturating_sub(r0)) >> 2) as usize;
        let gi = ((g.saturating_sub(g0)) >> 2) as usize;
        let bi = ((b.saturating_sub(b0)) >> 2) as usize;
        ri | (gi << 3) | (bi << 6)
    }

    fn get_or_build(&mut self, r: u8, g: u8, b: u8) -> u8 {
        let ci = Self::cell_index(r, g, b);
        let cell_cache = self.cells[ci].get_or_insert_with(|| build_cell_cache(ci));
        let r0 = ((ci & 7) as u8) << 5;
        let g0 = (((ci >> 3) & 7) as u8) << 5;
        let b0 = ((ci >> 6) as u8) << 5;
        let si = Self::sub_index(r, g, b, r0, g0, b0);
        cell_cache[si]
    }
}

/// Build PIL-identical palette cache for one cell.
/// Each cell is a 32×32×32 color box. We compute the closest palette entry
/// for 8×8×8 sub-positions within the box (step 4 per channel).
fn build_cell_cache(cell_idx: usize) -> [u8; 512] {
    let r0 = ((cell_idx & 7) as u8) << 5;
    let g0 = (((cell_idx >> 3) & 7) as u8) << 5;
    let b0 = ((cell_idx >> 6) as u8) << 5;
    let r1 = r0 + 31;
    let g1 = g0 + 31;
    let b1 = b0 + 31;

    // Compute dmin[i] and tmax[i] for each palette entry
    let mut dmin = [0i32; WEB_PALETTE_COLORS];
    let mut tmax = [0i32; WEB_PALETTE_COLORS];
    let mut dmax = i32::MAX;

    for i in 0..WEB_PALETTE_COLORS {
        let base = i * 3;
        let pr = WEB_PALETTE[base] as i32;
        let pg = WEB_PALETTE[base + 1] as i32;
        let pb = WEB_PALETTE[base + 2] as i32;

        let dr_min = if pr < r0 as i32 {
            r0 as i32 - pr
        } else if pr > r1 as i32 {
            pr - r1 as i32
        } else {
            0
        };
        let dg_min = if pg < g0 as i32 {
            g0 as i32 - pg
        } else if pg > g1 as i32 {
            pg - g1 as i32
        } else {
            0
        };
        let db_min = if pb < b0 as i32 {
            b0 as i32 - pb
        } else if pb > b1 as i32 {
            pb - b1 as i32
        } else {
            0
        };
        dmin[i] = dr_min * dr_min + dg_min * dg_min + db_min * db_min;

        let dr_max = (r1 as i32 - pr).abs().max((pr - r0 as i32).abs());
        let dg_max = (g1 as i32 - pg).abs().max((pg - g0 as i32).abs());
        let db_max = (b1 as i32 - pb).abs().max((pb - b0 as i32).abs());
        tmax[i] = dr_max * dr_max + dg_max * dg_max + db_max * db_max;

        if tmax[i] < dmax {
            dmax = tmax[i];
        }
    }

    // Build 8×8×8 sub-position cache
    let mut cache = [0u8; 512];
    for ri in 0..8u8 {
        for gi in 0..8u8 {
            for bi in 0..8u8 {
                let r = r0 + ri * 4;
                let g = g0 + gi * 4;
                let b = b0 + bi * 4;
                let ci = (ri as usize) | ((gi as usize) << 3) | ((bi as usize) << 6);

                let mut best_dist = i32::MAX;
                let mut best_idx = 0u8;
                for pi in 0..WEB_PALETTE_COLORS {
                    if dmin[pi] <= dmax {
                        let base = pi * 3;
                        let dr = r as i32 - WEB_PALETTE[base] as i32;
                        let dg = g as i32 - WEB_PALETTE[base + 1] as i32;
                        let db = b as i32 - WEB_PALETTE[base + 2] as i32;
                        let dist = dr * dr + dg * dg + db * db;
                        if dist < best_dist {
                            best_dist = dist;
                            best_idx = pi as u8;
                        }
                    }
                }
                cache[ci] = best_idx;
            }
        }
    }
    cache
}

/// PIL-style cached nearest-palette lookup.
/// Uses thread-local cache to avoid Mutex overhead.
fn find_nearest_web(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    use std::cell::RefCell;
    thread_local! {
        static CACHE: RefCell<WebPaletteCache> = RefCell::new(WebPaletteCache::new());
    }
    let idx = CACHE.with(|cache| cache.borrow_mut().get_or_build(r, g, b));
    let base = idx as usize * 3;
    (
        idx,
        WEB_PALETTE[base],
        WEB_PALETTE[base + 1],
        WEB_PALETTE[base + 2],
    )
}

// ── Image method ──

impl Image {
    /// Reduces the image to a `P` image with at most `colors` palette entries.
    ///
    /// RGB-family images use Pillow-compatible median cut. RGBA images use the
    /// default Pillow fast-octree path. The returned image carries palette index
    /// bytes plus RGB or RGBA palette data matching the selected path.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `colors` is outside 1..=256, or
    /// another [`PilError`] when materialization fails.
    pub fn quantize(
        &self,
        colors: u32,
        _kmeans: u32,
        _palette: Option<&Image>,
        _dither: bool,
    ) -> Result<Image, PilError> {
        // KNOWN DIVERGENCE (pillow-rs parity ledger): Pillow's Image.quantize
        // produces different palettes per method (MEDIANCUT=0, MAXCOVERAGE=1,
        // FASTOCTREE=2) and honors kmeans/palette/dither.  The current core
        // always runs one median-cut variant and ignores the other public
        // arguments; on diverse images the output differs from Pillow 12.2.0.
        // The active corpus intentionally covers only low-diversity inputs
        // where the variants agree, so the gap stays visible in the ledger
        // instead of being hidden by failing cases.
        if !(1..=256).contains(&colors) {
            return Err(PilError::ValueError("bad number of colors".to_owned()));
        }
        let n_colors = colors as usize;
        let img = self.materialize()?;
        let (w, h) = (img.width(), img.height());

        // PIL uses FASTOCTREE for RGBA mode (method=2 by default for RGBA).
        let is_rgba = matches!(img.color(), crate::raster::ColorType::Rgba8)
            || (self.explicit_mode() == Some("RGBA"));

        let (indices, palette_bytes, palette_alpha) = if is_rgba {
            // Use FASTOCTREE (octree) algorithm for RGBA
            let rgba = img.to_rgba8();
            let rgba_raw = rgba.into_raw();
            let (idx, pal) = quantize_octree_rgba(&rgba_raw, w, h, n_colors)?;
            // Pillow's FASTOCTREE result owns an RGBA palette. Core represents
            // that interleaved layout as RGB triples plus an alpha sidecar.
            let pal_rgb = pal
                .chunks_exact(4)
                .flat_map(|color| [color[0], color[1], color[2]])
                .collect();
            let alpha = pal.chunks_exact(4).map(|color| color[3]).collect();
            (idx, pal_rgb, Some(alpha))
        } else {
            // Standard median cut for RGB and other modes
            let rgb = img.to_rgb8();
            let rgb_raw = rgb.into_raw();
            let (idx, pal) = median_cut_quantize_rgb(&rgb_raw, n_colors);
            let pal_bytes: Vec<u8> = pal.iter().flat_map(|c| [c[0], c[1], c[2]]).collect();
            (idx, pal_bytes, None)
        };

        let mut out = crate::raster::GrayImage::new(w, h);
        for (i, pixel) in out.pixels_mut().enumerate() {
            pixel[0] = indices.get(i).copied().unwrap_or(0);
        }
        Ok(Image::Pipeline {
            source: Arc::new(Image::from_dynamic(
                DynamicImage::ImageLuma8(out),
                Some("P".to_string()),
            )),
            ops: vec![],
            format: None,
            explicit_mode: Some("P".to_string()),
            backend: None,
            palette: Some(palette_bytes),
            palette_alpha,
            materialized: crate::image::materialization_cache(),
        })
    }
}
