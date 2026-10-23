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

#[derive(Debug, Clone, Copy)]
struct MedianCutNode {
    vbox: VBox,
    left: Option<usize>,
    right: Option<usize>,
    split_axis: Option<usize>,
    split_at: u8,
    palette_index: Option<u8>,
}

// Exact port of QuantHeap.c: 1-indexed max-heap on pixelCount with the same
// sift rules, so equal-count boxes pop in Pillow's order.
struct BoxHeap {
    heap: Vec<usize>, // index 0 unused, 1-indexed like C
    counts: Vec<u32>, // tree node pixelCount by node index
}

impl BoxHeap {
    fn new(root_count: u32) -> Self {
        Self {
            heap: vec![0],
            counts: vec![root_count],
        }
    }

    fn cmp(&self, a: usize, b: usize) -> i64 {
        i64::from(self.counts[a]) - i64::from(self.counts[b])
    }

    fn note_count(&mut self, count: u32) {
        self.counts.push(count);
    }

    fn add(&mut self, val: usize) {
        self.heap.push(0);
        let mut k = self.heap.len() - 1;
        while k != 1 {
            if self.cmp(val, self.heap[k >> 1]) <= 0 {
                break;
            }
            self.heap[k] = self.heap[k >> 1];
            k >>= 1;
        }
        self.heap[k] = val;
    }

    fn remove(&mut self) -> Option<usize> {
        if self.heap.len() <= 1 {
            return None;
        }
        let root = self.heap[1];
        let value = self.heap.pop().expect("heap non-empty");
        if self.heap.len() == 1 {
            return Some(root);
        }
        let mut k = 1usize;
        let count = self.heap.len() - 1;
        while k * 2 <= count {
            let mut left = k * 2;
            if left < count && self.cmp(self.heap[left], self.heap[left + 1]) < 0 {
                left += 1;
            }
            if self.cmp(value, self.heap[left]) > 0 {
                break;
            }
            self.heap[k] = self.heap[left];
            k = left;
        }
        self.heap[k] = value;
        Some(root)
    }
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
pub(crate) fn median_cut_quantize_rgb(pixels: &[u8], n_colors: usize) -> (Vec<u8>, Vec<[u8; 3]>) {
    let n = pixels.len() / 3;
    if n == 0 {
        return (Vec::new(), vec![[0u8; 3]; 1]);
    }

    let n_colors = n_colors.clamp(1, 256);
    let first = [pixels[0], pixels[1], pixels[2]];
    if pixels[..n * 3].chunks_exact(3).all(|pixel| pixel == first) {
        return (vec![0u8; n], vec![first; n_colors]);
    }

    let (entries, scale) = build_histogram(pixels, n);
    if entries.is_empty() {
        return (vec![0u8; n], vec![[0u8; 3]; 1]);
    }
    let Some(root) = initial_vbox(&entries) else {
        return (vec![0u8; n], vec![[0u8; 3]; 1]);
    };

    let mut tree = build_median_cut_tree(root, &entries, n_colors);
    let mut leaf_order = Vec::new();
    collect_tree_leaves(&tree, 0, &mut leaf_order);
    for (palette_index, &node_index) in leaf_order.iter().enumerate() {
        tree[node_index].palette_index = Some(palette_index as u8);
    }
    let boxes: Vec<VBox> = leaf_order.iter().map(|&idx| tree[idx].vbox).collect();
    let palette = compute_palette(&boxes, &entries);
    if palette.is_empty() {
        return (vec![0u8; n], vec![[0u8; 3]; 1]);
    }

    let final_palette = pad_palette(palette, n_colors);
    let indices = map_pixels_to_palette(pixels, &final_palette, &tree, scale);
    (indices, final_palette)
}

fn build_histogram(pixels: &[u8], pixel_count: usize) -> (Vec<HistEntry>, u32) {
    let mut histogram = QuantHash::new();
    for pixel in pixels[..pixel_count * 3].chunks_exact(3) {
        histogram.add_pixel(pixel[0], pixel[1], pixel[2]);
    }
    (histogram.collect_entries(), histogram.scale)
}

fn initial_vbox(entries: &[HistEntry]) -> Option<VBox> {
    let (r_min, r_max, g_min, g_max, b_min, b_max) = find_initial_bounds(entries)?;
    let mut vbox = VBox::new(r_min, r_max, g_min, g_max, b_min, b_max);
    vbox.pixel_count = entries
        .iter()
        .filter(|entry| vbox_contains(entry, &vbox))
        .map(|entry| entry.count as u32)
        .sum();
    Some(vbox)
}

fn build_median_cut_tree(
    root: VBox,
    entries: &[HistEntry],
    max_colors: usize,
) -> Vec<MedianCutNode> {
    let mut tree = vec![MedianCutNode {
        vbox: root,
        left: None,
        right: None,
        split_axis: None,
        split_at: 0,
        palette_index: None,
    }];
    let mut heap = BoxHeap::new(root.pixel_count);
    heap.add(0);
    let mut num_splits = 0usize;

    while num_splits + 1 < max_colors {
        let Some(leaf_idx) = next_splittable_leaf(&mut heap, &tree) else {
            break;
        };
        let Some((left, right, split_axis, split_at)) = try_split(&tree[leaf_idx].vbox, entries)
        else {
            continue;
        };
        add_split_nodes(
            &mut tree, &mut heap, leaf_idx, left, right, split_axis, split_at,
        );
        num_splits += 1;
    }
    tree
}

fn next_splittable_leaf(heap: &mut BoxHeap, tree: &[MedianCutNode]) -> Option<usize> {
    loop {
        let idx = heap.remove()?;
        if tree[idx].left.is_none() && tree[idx].vbox.pixel_count > 1 && tree[idx].vbox.volume > 0 {
            return Some(idx);
        }
    }
}

fn add_split_nodes(
    tree: &mut Vec<MedianCutNode>,
    heap: &mut BoxHeap,
    leaf_idx: usize,
    left: VBox,
    right: VBox,
    split_axis: usize,
    split_at: u8,
) {
    let left_idx = tree.len();
    tree.push(new_leaf(left));
    let right_idx = tree.len();
    tree.push(new_leaf(right));
    tree[leaf_idx].left = Some(left_idx);
    tree[leaf_idx].right = Some(right_idx);
    tree[leaf_idx].split_axis = Some(split_axis);
    tree[leaf_idx].split_at = split_at;
    heap.note_count(left.pixel_count);
    heap.note_count(right.pixel_count);
    heap.add(left_idx);
    heap.add(right_idx);
}

fn new_leaf(vbox: VBox) -> MedianCutNode {
    MedianCutNode {
        vbox,
        left: None,
        right: None,
        split_axis: None,
        split_at: 0,
        palette_index: None,
    }
}

fn collect_tree_leaves(tree: &[MedianCutNode], idx: usize, leaves: &mut Vec<usize>) {
    match (tree[idx].left, tree[idx].right) {
        (Some(left), Some(right)) => {
            // Pillow annotates the left subtree first, defining palette order.
            collect_tree_leaves(tree, left, leaves);
            collect_tree_leaves(tree, right, leaves);
        }
        (None, None) => leaves.push(idx),
        (_, _) => unreachable!("node must have both or no children"),
    }
}

fn pad_palette(mut palette: Vec<[u8; 3]>, n_colors: usize) -> Vec<[u8; 3]> {
    let last = *palette.last().expect("non-empty palette");
    palette.resize(n_colors, last);
    palette
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

type ColorBounds = (u8, u8, u8, u8, u8, u8);

fn try_split(vbox: &VBox, entries: &[HistEntry]) -> Option<(VBox, VBox, usize, u8)> {
    if vbox.pixel_count <= 1 || vbox.volume <= 1 {
        return None;
    }
    let axis = choose_split_axis(vbox);
    let high_min = axis_split_point(vbox, entries, axis)?;
    let (left, right) = split_vbox_entries(vbox, entries, axis, high_min)?;
    Some((left, right, axis, high_min))
}

fn choose_split_axis(vbox: &VBox) -> usize {
    let r_weighted = (vbox.r_max - vbox.r_min) as u32 * 77;
    let g_weighted = (vbox.g_max - vbox.g_min) as u32 * 150;
    let b_weighted = (vbox.b_max - vbox.b_min) as u32 * 29;
    if r_weighted >= g_weighted && r_weighted >= b_weighted {
        0
    } else if g_weighted >= r_weighted && g_weighted >= b_weighted {
        1
    } else {
        2
    }
}

fn axis_value(entry: &HistEntry, axis: usize) -> u8 {
    match axis {
        0 => entry.r,
        1 => entry.g,
        2 => entry.b,
        _ => unreachable!(),
    }
}

fn axis_split_point(vbox: &VBox, entries: &[HistEntry], axis: usize) -> Option<u8> {
    let mut counts = [0u32; 256];
    for entry in entries.iter().filter(|entry| vbox_contains(entry, vbox)) {
        counts[axis_value(entry, axis) as usize] += entry.count as u32;
    }

    let mut values = [0u8; 256];
    let mut value_count = 0usize;
    for (value, &count) in counts.iter().enumerate() {
        if count > 0 {
            values[value_count] = value as u8;
            value_count += 1;
        }
    }
    if value_count <= 1 {
        return None;
    }

    // Pillow walks high to low. The first group crossing half remains on the
    // high side; if none crosses, move the lowest group to the low side.
    let mut cumulative = 0u32;
    let mut high_start = value_count;
    for index in (0..value_count).rev() {
        cumulative += counts[values[index] as usize];
        if cumulative * 2 > vbox.pixel_count {
            high_start = index;
            break;
        }
    }
    if high_start == 0 {
        high_start = 1;
    }
    let high_min = values[high_start];
    let low_max = values[high_start - 1];
    let (box_min, box_max) = axis_bounds(vbox, axis);
    if high_min <= box_min || low_max >= box_max || high_min > box_max || low_max < box_min {
        return None;
    }
    Some(high_min)
}

fn axis_bounds(vbox: &VBox, axis: usize) -> (u8, u8) {
    match axis {
        0 => (vbox.r_min, vbox.r_max),
        1 => (vbox.g_min, vbox.g_max),
        2 => (vbox.b_min, vbox.b_max),
        _ => unreachable!(),
    }
}

fn split_vbox_entries(
    vbox: &VBox,
    entries: &[HistEntry],
    axis: usize,
    high_min: u8,
) -> Option<(VBox, VBox)> {
    let mut left_bounds = None;
    let mut right_bounds = None;
    let mut left_count = 0u32;
    let mut right_count = 0u32;
    for entry in entries.iter().filter(|entry| vbox_contains(entry, vbox)) {
        if axis_value(entry, axis) >= high_min {
            include_entry(&mut left_bounds, entry);
            left_count += entry.count as u32;
        } else {
            include_entry(&mut right_bounds, entry);
            right_count += entry.count as u32;
        }
    }
    let left_bounds = left_bounds?;
    let right_bounds = right_bounds?;
    if left_count == 0 || right_count == 0 {
        return None;
    }
    Some((
        vbox_from_bounds(left_bounds, left_count),
        vbox_from_bounds(right_bounds, right_count),
    ))
}

fn include_entry(bounds: &mut Option<ColorBounds>, entry: &HistEntry) {
    *bounds = Some(match *bounds {
        None => (entry.r, entry.r, entry.g, entry.g, entry.b, entry.b),
        Some((rmin, rmax, gmin, gmax, bmin, bmax)) => (
            rmin.min(entry.r),
            rmax.max(entry.r),
            gmin.min(entry.g),
            gmax.max(entry.g),
            bmin.min(entry.b),
            bmax.max(entry.b),
        ),
    });
}

fn vbox_from_bounds(bounds: ColorBounds, pixel_count: u32) -> VBox {
    let (r_min, r_max, g_min, g_max, b_min, b_max) = bounds;
    let mut vbox = VBox::new(r_min, r_max, g_min, g_max, b_min, b_max);
    vbox.pixel_count = pixel_count;
    vbox
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
    tree: &[MedianCutNode],
    scale: u32,
) -> Vec<u8> {
    let n = pixels.len() / 3;
    let n_colors = palette.len();

    if n_colors <= 1 || tree.is_empty() {
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
        let mut node_idx = 0;
        let start_idx = loop {
            let node = &tree[node_idx];
            match (node.left, node.right) {
                (Some(left), Some(right)) => {
                    let axis = node.split_axis.expect("split node has an axis");
                    let value = match axis {
                        0 => sr,
                        1 => sg,
                        2 => sb,
                        _ => unreachable!("median-cut split axis must be RGB"),
                    };
                    node_idx = if value >= node.split_at { left } else { right };
                }
                (None, None) => {
                    break usize::from(node.palette_index.unwrap_or(0));
                }
                _ => unreachable!("median-cut node must have both or no children"),
            }
        };

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
