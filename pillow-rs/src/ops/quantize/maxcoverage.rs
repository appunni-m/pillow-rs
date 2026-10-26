//! Pillow-compatible MAXCOVERAGE quantization.

// ── MAXCOVERAGE (Pillow Quant.c quantize2) ────────────────────────────────

/// Pillow's unshifted_pixel_hash: XOR-mix of channels (Quant.c PIXEL_HASH).
#[inline]
fn maxcoverage_pixel_hash(r: u8, g: u8, b: u8) -> u32 {
    // Quant.c PIXEL_HASH uses unsigned int arithmetic, which wraps on
    // overflow; the (b << 16) * 64997 term overflows u32, so wrap explicitly
    // to keep debug builds panic-free and identical to C.
    (u32::from(r) * 463)
        ^ (u32::from(g) << 8).wrapping_mul(10069)
        ^ (u32::from(b) << 16).wrapping_mul(64997)
}

/// Pillow's unshifted_pixel_cmp: lexicographic RGB ordering.
#[inline]
fn maxcoverage_pixel_cmp(a: [u8; 3], b: [u8; 3]) -> i32 {
    if a[0] != b[0] {
        return i32::from(a[0]) - i32::from(b[0]);
    }
    if a[1] != b[1] {
        return i32::from(a[1]) - i32::from(b[1]);
    }
    i32::from(a[2]) - i32::from(b[2])
}

#[inline]
fn maxcoverage_dist_sqr(a: [u8; 3], b: [u8; 3]) -> u32 {
    let dr = i32::from(a[0]) - i32::from(b[0]);
    let dg = i32::from(a[1]) - i32::from(b[1]);
    let db = i32::from(a[2]) - i32::from(b[2]);
    (dr * dr + dg * dg + db * db) as u32
}

/// Pillow's prime-finding helper for hashtable resizing (QuantHash.c).
fn maxcoverage_find_prime(start: u32, dir: i32) -> u32 {
    let unit = [0u8, 1, 0, 1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0];
    let mut s = start;
    while s > 1 {
        let u = unit[(s & 0x0f) as usize];
        if u == 0 {
            s = s.wrapping_add(dir as u32);
            continue;
        }
        // Keep the observable behavior of Pillow 12.2.0's QuantHash.c.
        // Its `if (!start % t)` expression is parsed as `(!start) % t`,
        // which is always zero for the positive candidates reaching this
        // helper.  Consequently the loop accepts the first candidate whose
        // low-nibble sieve entry is set; replacing this with a mathematically
        // correct primality test changes hash bucket order and quantizer
        // palette/index output.
        return s;
    }
    s
}

/// Exact port of Pillow's QuantHash.c table used by MAXCOVERAGE.
struct MaxCoverageHash<'a> {
    table: Vec<Vec<(u32, u32)>>, // bucket -> sorted chain of (pixel_v, distance)
    length: u32,
    count: u32,
    pixels: &'a [[u8; 3]],
}

impl<'a> MaxCoverageHash<'a> {
    fn new(pixels: &'a [[u8; 3]]) -> Self {
        Self {
            table: vec![Vec::new(); 11],
            length: 11,
            count: 0,
            // QuantHash only needs an indexed read of the source colors. The
            // old owned copy doubled the color-list memory and copied every
            // pixel before the first MAXCOVERAGE phase.
            pixels,
        }
    }

    fn hash_of(&self, v: u32) -> u32 {
        let p = self.pixels[v as usize];
        maxcoverage_pixel_hash(p[0], p[1], p[2]) % self.length
    }

    fn lookup(&self, v: u32) -> Option<u32> {
        let hash = self.hash_of(v);
        let p = self.pixels[v as usize];
        for &(k, val) in &self.table[hash as usize] {
            let cmp = maxcoverage_pixel_cmp(self.pixels[k as usize], p);
            if cmp == 0 {
                // QuantHash.c hashtable_lookup returns the stored *value*
                // (the cached palette index), not the chain key.
                return Some(val);
            }
            if cmp > 0 {
                break;
            }
        }
        None
    }

    /// QuantHash.c `_hashtable_insert` with sorted-chain insertion and
    /// Pillow's resize rules.
    fn insert(&mut self, v: u32, val: u32) {
        let hash = self.hash_of(v);
        if self.chain_insert(hash as usize, v, val) {
            return;
        }
        self.count += 1;
        self.resize();
    }

    /// Inserts into the sorted chain; returns true when an existing key was
    /// updated in place (no count change).
    fn chain_insert(&mut self, bucket: usize, v: u32, val: u32) -> bool {
        let p = self.pixels[v as usize];
        let chain = &mut self.table[bucket];
        let mut pos = 0usize;
        while pos < chain.len() {
            let existing = self.pixels[chain[pos].0 as usize];
            let cmp = maxcoverage_pixel_cmp(existing, p);
            if cmp == 0 {
                chain[pos].1 = val;
                return true;
            }
            if cmp > 0 {
                break;
            }
            pos += 1;
        }
        chain.insert(pos, (v, val));
        false
    }

    fn for_each_update(&mut self, mut f: impl FnMut(u32, &mut u32)) {
        for chain in &mut self.table {
            for (k, val) in chain.iter_mut() {
                f(*k, val);
            }
        }
    }

    fn resize(&mut self) {
        let mut new_size = self.length;
        if self.count * 3 < self.length {
            new_size = maxcoverage_find_prime(self.length / 2 - 1, -1);
        } else if self.length * 3 < self.count {
            new_size = maxcoverage_find_prime(self.length * 2 + 1, 1);
        }
        if new_size < 11 {
            new_size = self.length;
        }
        if new_size != self.length {
            let old = std::mem::take(&mut self.table);
            self.table = vec![Vec::new(); new_size as usize];
            self.length = new_size;
            self.count = 0;
            for chain in old {
                for (k, val) in chain {
                    let hash = self.hash_of(k);
                    let _ = self.chain_insert(hash as usize, k, val);
                    self.count += 1;
                }
            }
        }
    }
}

/// Pillow's MAXCOVERAGE quantizer (Quant.c `quantize2`).
///
/// Returns (palette RGB triples, per-pixel palette indices).
pub(crate) fn maxcoverage_quantize_rgb(
    pixels: &[u8],
    n_colors: usize,
    kmeans: u32,
) -> (Vec<u8>, Vec<[u8; 3]>) {
    let n_pixels = pixels.len() / 3;
    let pixel_list: Vec<[u8; 3]> = pixels.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    let n_quant = n_colors.clamp(1, 256);
    if n_pixels == 0 {
        return (Vec::new(), vec![[0u8; 3]; 1]);
    }

    // MAXCOVERAGE's palette loop emits the same furthest pixel for every
    // requested slot when the source has one color. Preserve that observable
    // result without building the hash or distance tables.
    let first = pixel_list[0];
    if pixel_list.iter().all(|pixel| *pixel == first) {
        return (vec![0u8; n_pixels], vec![first; n_quant]);
    }

    // Collect unique colors and the RGB mean (Quant.c quantize2 setup).
    let mut mean = [0u64; 3];
    let mut h = MaxCoverageHash::new(&pixel_list);
    for p in &pixel_list {
        mean[0] += u64::from(p[0]);
        mean[1] += u64::from(p[1]);
        mean[2] += u64::from(p[2]);
    }
    let mut new_color = [
        (0.5 + mean[0] as f64 / n_pixels as f64) as u8,
        (0.5 + mean[1] as f64 / n_pixels as f64) as u8,
        (0.5 + mean[2] as f64 / n_pixels as f64) as u8,
    ];
    let mut unique: Vec<u32> = Vec::new();
    for (idx, _p) in pixel_list.iter().enumerate() {
        if h.lookup(idx as u32).is_none() {
            h.insert(idx as u32, 0xffff_ffff);
            unique.push(idx as u32);
        }
    }

    // Iteratively pick the pixel farthest from the current palette set.
    let mut palette: Vec<[u8; 3]> = Vec::with_capacity(n_quant);
    for i in 0..n_quant {
        let mut furthest_distance = 0u32;
        let mut furthest_v = pixel_list[0];
        let second_pixel = i == 1;
        h.for_each_update(|v, dist| {
            let px = pixel_list[v as usize];
            let new_dist = maxcoverage_dist_sqr(new_color, px);
            if second_pixel || new_dist < *dist {
                *dist = new_dist;
            }
            if *dist > furthest_distance {
                furthest_distance = *dist;
                furthest_v = px;
            }
        });
        palette.push(furthest_v);
        new_color = furthest_v;
    }

    // Build distance tables and map each pixel to its nearest palette entry.
    let mut avg_dist = vec![0u32; n_quant * n_quant];
    let mut avg_dist_sort: Vec<Vec<usize>> = vec![Vec::new(); n_quant];
    for i in 0..n_quant {
        avg_dist[i * n_quant + i] = 0;
        for j in 0..i {
            let d = maxcoverage_dist_sqr(palette[i], palette[j]);
            avg_dist[j * n_quant + i] = d;
            avg_dist[i * n_quant + j] = d;
        }
    }
    for i in 0..n_quant {
        let mut row: Vec<(u32, usize)> = (0..n_quant)
            .map(|j| (avg_dist[i * n_quant + j], j))
            .collect();
        // _distance_index_cmp: distance ascending, then index ascending.
        row.sort_by_key(|&(d, idx)| (d, idx));
        avg_dist_sort[i] = row.into_iter().map(|(_, idx)| idx).collect();
    }

    // map_image_pixels: nearest palette index per pixel, with early exit.
    let mut quantized = vec![0u32; n_pixels];
    let mut lookup = MaxCoverageHash::new(&pixel_list);
    for (i, px) in pixel_list.iter().enumerate() {
        let bestmatch = if let Some(v) = lookup.lookup(i as u32) {
            v
        } else {
            // Pillow's map_image_pixels fixes the scan row to the *starting*
            // bestmatch: `aDSK = avgDistSortKey + bestmatch*n` is computed
            // before the loop, so the cutoff tests and candidate order keep
            // using the original row even while bestmatch is updated.
            let start = 0usize;
            let mut bestmatch = 0u32;
            let mut bestdist = maxcoverage_dist_sqr(palette[start], *px);
            let initialdist = bestdist << 2;
            for &idx in &avg_dist_sort[start] {
                if avg_dist[start * n_quant + idx] <= initialdist {
                    let d = maxcoverage_dist_sqr(palette[idx], *px);
                    if d < bestdist {
                        bestdist = d;
                        bestmatch = idx as u32;
                    }
                } else {
                    break;
                }
            }
            lookup.insert(i as u32, bestmatch);
            bestmatch
        };
        quantized[i] = bestmatch;
    }

    if kmeans > 0 {
        kmeans_refine(&pixel_list, &mut palette, &mut quantized, kmeans - 1);
    }

    (quantized.iter().map(|&i| i as u8).collect(), palette)
}

/// Pillow's k-means refinement (Quant.c `k_means`): recompute palette entries
/// as the mean of assigned pixels and remap until changes stabilize.
pub(crate) fn kmeans_refine(
    pixel_list: &[[u8; 3]],
    palette: &mut [[u8; 3]],
    quantized: &mut [u32],
    threshold: u32,
) {
    let n_entries = palette.len();
    let mut count = vec![0u32; n_entries];
    let mut avg = vec![[0u64; 3]; n_entries];
    let mut built = false;
    let mut avg_dist = vec![0u32; n_entries * n_entries];
    let mut avg_dist_sort: Vec<Vec<usize>> = vec![Vec::new(); n_entries];

    loop {
        if !built {
            // compute_palette_from_quantized_pixels
            count.fill(0);
            for a in avg.iter_mut() {
                *a = [0u64; 3];
            }
            for (i, px) in pixel_list.iter().enumerate() {
                let q = quantized[i] as usize;
                if q >= n_entries {
                    return;
                }
                avg[q][0] += u64::from(px[0]);
                avg[q][1] += u64::from(px[1]);
                avg[q][2] += u64::from(px[2]);
                count[q] += 1;
            }
            for i in 0..n_entries {
                let c = count[i].max(1);
                palette[i] = [
                    (0.5 + avg[i][0] as f64 / c as f64) as u8,
                    (0.5 + avg[i][1] as f64 / c as f64) as u8,
                    (0.5 + avg[i][2] as f64 / c as f64) as u8,
                ];
            }
            build_distance_tables(palette, &mut avg_dist, &mut avg_dist_sort);
            built = true;
        } else {
            // recompute_palette_from_averages
            for i in 0..n_entries {
                let c = count[i].max(1);
                palette[i] = [
                    (0.5 + avg[i][0] as f64 / c as f64) as u8,
                    (0.5 + avg[i][1] as f64 / c as f64) as u8,
                    (0.5 + avg[i][2] as f64 / c as f64) as u8,
                ];
            }
            resort_distance_tables(palette, &mut avg_dist, &mut avg_dist_sort);
        }
        let changes = remap_pixels(
            pixel_list,
            palette,
            quantized,
            &avg_dist,
            &avg_dist_sort,
            &mut avg,
            &mut count,
        );
        if changes <= threshold {
            break;
        }
    }
}

fn build_distance_tables(
    palette: &[[u8; 3]],
    avg_dist: &mut [u32],
    avg_dist_sort: &mut [Vec<usize>],
) {
    let n = palette.len();
    for i in 0..n {
        avg_dist[i * n + i] = 0;
        for j in 0..i {
            let d = maxcoverage_dist_sqr(palette[i], palette[j]);
            avg_dist[j * n + i] = d;
            avg_dist[i * n + j] = d;
        }
    }
    for i in 0..n {
        let mut row: Vec<(u32, usize)> = (0..n).map(|j| (avg_dist[i * n + j], j)).collect();
        row.sort_by_key(|&(d, idx)| (d, idx));
        avg_dist_sort[i] = row.into_iter().map(|(_, idx)| idx).collect();
    }
}

fn resort_distance_tables(
    palette: &[[u8; 3]],
    avg_dist: &mut [u32],
    avg_dist_sort: &mut [Vec<usize>],
) {
    let n = palette.len();
    for i in 0..n {
        avg_dist[i * n + i] = 0;
        for j in 0..i {
            let d = maxcoverage_dist_sqr(palette[i], palette[j]);
            avg_dist[j * n + i] = d;
            avg_dist[i * n + j] = d;
        }
    }
    for i in 0..n {
        // Pillow Quant.c `resort_distance_tables` uses an insertion sort whose
        // loop condition reads `*(skRow[k-1]) > *(skRow[k])` *after* moving
        // `skRow[k] = skRow[k-1]`, so the comparison is between adjacent
        // elements rather than against the pivot.  Rows therefore end up only
        // partially sorted, and the scan order (and its early exit) depends on
        // the previous row order.  Replicate that behavior exactly.
        let row = &mut avg_dist_sort[i];
        for j in 1..n {
            let sk_elt = row[j];
            let mut k = j;
            while k > 0 {
                let left = row[k - 1];
                let right = row[k];
                if avg_dist[i * n + left] > avg_dist[i * n + right] {
                    row[k] = left;
                    k -= 1;
                } else {
                    break;
                }
            }
            if k != j {
                row[k] = sk_elt;
            }
        }
    }
}

fn remap_pixels(
    pixel_list: &[[u8; 3]],
    palette: &[[u8; 3]],
    quantized: &mut [u32],
    avg_dist: &[u32],
    avg_dist_sort: &[Vec<usize>],
    avg: &mut [[u64; 3]],
    count: &mut [u32],
) -> u32 {
    let n_entries = palette.len();
    let mut changes = 0u32;
    let mut lookup = MaxCoverageHash::new(pixel_list);
    for (i, px) in pixel_list.iter().enumerate() {
        let bestmatch = if let Some(v) = lookup.lookup(i as u32) {
            v
        } else {
            // Pillow's map_image_pixels_from_quantized_pixels pins the scan
            // row to the current assignment (`aDSK`/`aD` are read once from
            // `bestmatch = pixelArray[i]`), so the cutoff and candidate order
            // must keep using the original row while bestmatch updates.
            let start = quantized[i] as usize;
            let mut bestmatch = quantized[i];
            let mut bestdist = maxcoverage_dist_sqr(palette[start], *px);
            let initialdist = bestdist << 2;
            for &idx in &avg_dist_sort[start] {
                if avg_dist[start * n_entries + idx] <= initialdist {
                    let d = maxcoverage_dist_sqr(palette[idx], *px);
                    if d < bestdist {
                        bestdist = d;
                        bestmatch = idx as u32;
                    }
                } else {
                    break;
                }
            }
            lookup.insert(i as u32, bestmatch);
            bestmatch
        };
        if bestmatch != quantized[i] {
            let old = quantized[i] as usize;
            let new = bestmatch as usize;
            avg[new][0] += u64::from(px[0]);
            avg[new][1] += u64::from(px[1]);
            avg[new][2] += u64::from(px[2]);
            avg[old][0] -= u64::from(px[0]);
            avg[old][1] -= u64::from(px[1]);
            avg[old][2] -= u64::from(px[2]);
            count[new] += 1;
            count[old] -= 1;
            quantized[i] = bestmatch;
            changes += 1;
        }
    }
    changes
}
