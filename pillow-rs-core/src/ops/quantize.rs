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

use image::DynamicImage;

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
            match &self.entries[idx] {
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
                    let entry = self.entries[idx].as_mut().unwrap();
                    entry.count += 1;
                    entry.r_sum += r as u64;
                    entry.g_sum += g as u64;
                    entry.b_sum += b as u64;
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
            match &self.entries[idx] {
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
                    let existing = self.entries[idx].as_mut().unwrap();
                    existing.count += entry.count;
                    existing.r_sum += entry.r_sum;
                    existing.g_sum += entry.g_sum;
                    existing.b_sum += entry.b_sum;
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

// ── WEB palette (PIL's default fixed palette for convert("P")) ──
//
// PIL's convert("P") with default palette=Palette.WEB uses a fixed 226-color palette:
// - Index 0: (0,0,0) reserved for transparency
// - Indices 1-10: reserved (black)
// - Indices 11-225: 6x6x6 web-safe color cube at values {0,51,102,153,204,255}
//   (216 colors minus (0,0,0) which is at index 0, plus 10 additional reserved slots = 226)

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

/// Convert RGB pixels to P-mode using PIL's default WEB palette.
/// Applies Floyd-Steinberg dither when `dither` is true (matching PIL default).
/// Returns (palette_indices, palette_bytes).
pub fn web_palette_quantize(pixels: &[u8], w: u32, h: u32, dither: bool) -> (Vec<u8>, Vec<u8>) {
    let n_pixels = (w * h) as usize;
    let mut out = vec![0u8; n_pixels];

    if dither {
        // Floyd-Steinberg dither with WEB palette
        let wu = w as usize;
        let mut err_r = vec![0i32; n_pixels];
        let mut err_g = vec![0i32; n_pixels];
        let mut err_b = vec![0i32; n_pixels];

        for y in 0..h as usize {
            for x in 0..wu {
                let pi = y * wu + x;
                let src_i = pi * 3;
                let r = (pixels[src_i] as i32 + err_r[pi]).clamp(0, 255) as u8;
                let g = (pixels[src_i + 1] as i32 + err_g[pi]).clamp(0, 255) as u8;
                let b = (pixels[src_i + 2] as i32 + err_b[pi]).clamp(0, 255) as u8;

                let (best_idx, pr, pg, pb) = find_nearest_web(r, g, b);
                out[pi] = best_idx;

                let er = r as i32 - pr as i32;
                let eg = g as i32 - pg as i32;
                let eb = b as i32 - pb as i32;

                if x + 1 < wu {
                    err_r[pi + 1] += er * 7 / 16;
                    err_g[pi + 1] += eg * 7 / 16;
                    err_b[pi + 1] += eb * 7 / 16;
                }
                if y + 1 < h as usize {
                    let next_row = (y + 1) * wu;
                    if x > 0 {
                        err_r[next_row + x - 1] += er * 3 / 16;
                        err_g[next_row + x - 1] += eg * 3 / 16;
                        err_b[next_row + x - 1] += eb * 3 / 16;
                    }
                    err_r[next_row + x] += er * 5 / 16;
                    err_g[next_row + x] += eg * 5 / 16;
                    err_b[next_row + x] += eb * 5 / 16;
                    if x + 1 < wu {
                        err_r[next_row + x + 1] += er / 16;
                        err_g[next_row + x + 1] += eg / 16;
                        err_b[next_row + x + 1] += eb / 16;
                    }
                }
            }
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

    (out, WEB_PALETTE.to_vec())
}

/// Number of colors in the WEB palette.
const WEB_PALETTE_COLORS: usize = 226;

/// Find the closest WEB palette entry for an RGB color.
fn find_nearest_web(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    let mut best_dist = i32::MAX;
    let mut best_idx = 0u8;
    for i in 0..WEB_PALETTE_COLORS {
        let base = i * 3;
        let dr = r as i32 - WEB_PALETTE[base] as i32;
        let dg = g as i32 - WEB_PALETTE[base + 1] as i32;
        let db = b as i32 - WEB_PALETTE[base + 2] as i32;
        let dist = dr * dr + dg * dg + db * db;
        if dist < best_dist {
            best_dist = dist;
            best_idx = i as u8;
        }
    }
    let base = best_idx as usize * 3;
    (
        best_idx,
        WEB_PALETTE[base],
        WEB_PALETTE[base + 1],
        WEB_PALETTE[base + 2],
    )
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
        let n_colors = colors.clamp(2, 256) as usize;
        let img = self.materialize()?;
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let rgb_raw = rgb.into_raw();
        let (indices, palette) = median_cut_quantize_rgb(&rgb_raw, n_colors);
        let mut out = image::GrayImage::new(w, h);
        for (i, pixel) in out.pixels_mut().enumerate() {
            pixel[0] = indices.get(i).copied().unwrap_or(0);
        }
        let palette_bytes: Vec<u8> = palette.iter().flat_map(|c| [c[0], c[1], c[2]]).collect();
        Ok(Image::Pipeline {
            source: Arc::new(Image::Loaded(
                DynamicImage::ImageLuma8(out),
                Some("P".to_string()),
            )),
            ops: vec![],
            format: None,
            explicit_mode: Some("P".to_string()),
            backend: None,
            palette: Some(palette_bytes),
        })
    }
}
