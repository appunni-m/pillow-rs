# Autohinter Algorithm Reference (Phase A–F)

Faithful pseudo-Rust transcription of FreeType 2.14.1 `src/autofit/` (aflatin.c,
afhints.c, afblue.c). This is the **spec** to port against. All coordinates are
26.6 fixed unless noted; `FT_Pos` ≡ `i32`. Line numbers reference the pinned C oracle.

Reference provenance: fixtures are **FreeType `FT_LOAD_RENDER`**
on bytecode-stripped `fonts_autohint/` → **AUTOHINTED**. The autohinter must run.

Citation key:
- `aflatin.c:N` — `freetype/src/autofit/aflatin.c` in the pinned C oracle
- `afhints.c:N` — `.../afhints.c`
- `aflatin.h:N`, `afblue.c:N`, `afblue.h:N`, `afblue.dat`

---

## Shared constants & macros

```rust
// aflatin.h:34  — c scaled by upem/2048
fn af_latin_constant(upem: i32, c: i32) -> i32 {
    (c * upem) / 2048
}
// aflatin.h:69
const AF_LATIN_MAX_WIDTHS: usize = 16;

// aflatin.h:72-83  blue zone flags
const BLUE_ACTIVE:       u32 = 1 << 0; // zone height <= 3/4px
const BLUE_TOP:          u32 = 1 << 1;
const BLUE_SUB_TOP:      u32 = 1 << 2;
const BLUE_NEUTRAL:      u32 = 1 << 3;
const BLUE_ADJUSTMENT:   u32 = 1 << 4; // x-height scale opt flag
const BLUE_BOTTOM:       u32 = 1 << 5; // capital bottom
const BLUE_BOTTOM_SMALL: u32 = 1 << 6; // small-letter bottom

// aflatin.h:317-324  blue-string properties (from afblue stringset table)
const PROP_TOP:           u32 = 1 << 0;
const PROP_SUB_TOP:       u32 = 1 << 1;
const PROP_NEUTRAL:       u32 = 1 << 2;
const PROP_X_HEIGHT:      u32 = 1 << 3;
const PROP_LONG:          u32 = 1 << 4;
const PROP_CAPITAL_BOTTOM:u32 = 1 << 5;
const PROP_SMALL_BOTTOM:  u32 = 1 << 6;

// aflatin.c:39
fn flat_threshold(upem: i32) -> i32 { upem / 14 }
```

## Data structures (new — add to types.rs)

```rust
// afhints.h AF_WidthRec
#[derive(Clone, Copy, Default)]
pub struct AfWidth { pub org: i32, pub cur: i32, pub fit: i32 }

// aflatin.h:86 AF_LatinBlueRec
#[derive(Clone, Copy, Default)]
pub struct AfLatinBlue {
    pub ref_:   AfWidth,  // reference (flat) position
    pub shoot:  AfWidth,  // overshoot (round) position
    pub ascender:  i32,
    pub descender: i32,
    pub flags: u32,
}

// Per-axis metrics (one for Horz, one for Vert). aflatin.h:97
#[derive(Clone)]
pub struct AfLatinAxis {
    pub scale: i32,        // 16.16 (the metrics' axis scale, = x/y_scale)
    pub delta: i32,        // 26.6
    pub width_count: usize,
    pub widths: [AfWidth; AF_LATIN_MAX_WIDTHS],
    pub edge_distance_threshold: i32, // font units
    pub standard_width: i32,
    pub extra_light: bool,
    // Vert axis only:
    pub blue_count: usize,
    pub blues: Vec<AfLatinBlue>,
    pub org_scale: i32,
    pub org_delta: i32,
}

// aflatin.h:118 — font-wide metrics, computed ONCE per font+size, reused per glyph.
#[derive(Clone)]
pub struct AfLatinMetrics {
    pub units_per_em: i32,
    pub axis: [AfLatinAxis; 2], // [Horz, Vert]
}
```

`GlyphHints` gains `pub metrics: Option<&AfLatinMetrics>` (borrowed, set by `apply_hints`).
`AFEdge` gains `pub blue_edge: Option<AfWidth>` (the fit value to snap to) and
`AFEdgeFlags::NEUTRAL` bit. AFSegment already has `height`.

## Latin blue stringset (afblue.c:646-653, afblue.dat:347-358)

The standard Latin script uses exactly these 6 blue zones, in order:

| zone | property flags | reference chars |
|------|----------------|-----------------|
| 0 capital top | `TOP` | T H E Z O C Q S |
| 1 capital bottom | `CAPITAL_BOTTOM` | H E Z L O C U S |
| 2 small f-top | `TOP` | f i j k d b h |
| 3 small top | `TOP \| X_HEIGHT` | u v x z o e s c |
| 4 small bottom | `SMALL_BOTTOM` | n r x z o e s c |
| 5 small descender | (none) | p q g j y |

Static table in Rust: `[(chars: &[char], props: u32); 6]`. (No HarfBuzz: each char →
its glyph index directly via cmap. No clusters/num_idx>1 in practice for Latin.)

---

## PHASE A — metrics_init_widths + fix link_segments

### A.1 `af_latin_metrics_init_widths` (aflatin.c:55-265)

Builds the stem-width histogram for **one standard character** (first of
`"oO0"`-like standard string present in the font) loaded with `FT_LOAD_NO_SCALE`
(scale = 1.0, i.e. raw font units).

```
1. width_count = 0 for both axes.
2. Find standard char: try glyphs in the script's standard_charstring until one
   resolves to a glyph_index != 0. For Latin the standard string is the lowercase
   "o"-class; if absent, goto Exit (fallback widths).
3. Load that glyph OUTLINE in font units (no scaling). Reload hints with
   scaler.x_scale = y_scale = 0x10000 (1.0), delta 0.
4. FOR dim in [Horz, Vert]:
     compute_segments(dim)
     link_segments(width_count=0)         // unparameterized, but link pairs still form
     FOR seg in axis.segments:
        link = seg.link
        if link && link.link == seg && link > seg:   // mutual, count once
           dist = |seg.pos - link.pos|
           if num_widths < 16: axis.widths[num_widths++].org = dist
     sort_and_quantize_widths(&num_widths, axis.widths, upem/100)
     axis.width_count = num_widths
5. Exit: FOR dim:
     stdw = (width_count>0) ? widths[0].org : af_latin_constant(upem,50)
     axis.edge_distance_threshold = stdw / 5
     axis.standard_width = stdw
     axis.extra_light = false
```

`standard_charstring` is script-specific.  With HarfBuzz disabled,
`af_shaper_get_cluster_nohb` consumes one UTF-8 candidate and calls
`FT_Get_Char_Index`; `aflatin.c` and `afcjk.c` repeat until the first nonzero
glyph index.  For Latin the pinned list is `"o O 0"`; for Arabic it is
`"Lam Ha Tatweel"` (U+0644, U+062D, U+0640).  If every candidate is missing,
the constant fallback remains in effect (`width_count` stays 0, so
`max_width=0`).

The CJK/Indic scaler has a separate pinned-C detail: `af_cjk_metrics_scale_dim`
scales blue zones but leaves `axis->widths[].cur/fit` and `extra_light`
unchanged.  The selected standard glyph still supplies the font-unit
`standard_width` and `edge_distance_threshold`, but its width list is not
substituted into `af_cjk_compute_stem_width` at glyph-load time.

### A.2 `af_sort_and_quantize_widths` (afhints.c:58-131)

Insertion-sort by `.org`, then collapse clusters whose spread ≤ threshold (upem/100)
into their mean, then compress out zeros. See C above — port verbatim. Sets
`widths[0]` = the dominant (standard) stem width.

### A.3 `af_latin_hints_link_segments` (aflatin.c:2016-2148) — THE FIX

Currently our port uses `dist_demerit = dist` (the `else` branch, max_width==0).
With Phase A.1 populated, `max_width = widths[width_count-1].org` and we take
the real scoring branch:

```
max_width = width_count ? widths[width_count-1].org : 0
len_threshold = af_latin_constant(upem, 8); if 0 → 1
len_score     = af_latin_constant(upem, 6000)
dist_score    = 3000

for seg1 in segments:
  if seg1.dir != major_dir: continue
  for seg2 in segments:
    if seg1.dir + seg2.dir == 0 and seg2.pos > seg1.pos:   // opposite dirs, seg2 right of seg1
      min = max(seg1.min_coord, seg2.min_coord)            // intersection
      max = min(seg1.max_coord, seg2.max_coord)
      len = max - min                                       // overlap along cross-axis
      if len >= len_threshold:
        dist = seg2.pos - seg1.pos                          // along-axis gap
        if max_width:
          delta = (dist << 10) / max_width - (1<<10)        // multiples of max_width, ×1024
          if delta > 10000:  dist_demerit = 32000
          elif delta > 0:    dist_demerit = delta*delta / dist_score
          else:              dist_demerit = 0
        else:
          dist_demerit = dist
        score = dist_demerit + len_score / len
        if score < seg1.score: seg1.score=score; seg1.link=seg2
        if score < seg2.score: seg2.score=score; seg2.link=seg1

// serif pass (unchanged from current port):
for seg1: seg2=seg1.link; if seg2 && seg2.link != seg1: seg1.link=NULL; seg1.serif=seg2.link
```

This re-enables `link_segments` in `apply_hints` (uncomment the two calls).

### A.4 Plumbing

`scale_glyph` (scaler.rs:89) builds `ScaleMetrics` per glyph but the **font-wide**
metrics depend only on (font, size) → compute once and cache. Add to `Font`:
`pub latin_metrics_cache: HashMap<(size_pt-ish key), AfLatinMetrics>` OR compute
lazily and store on the `Font` for the active size. Pass `&AfLatinMetrics` into
`autohint_glyph` → `apply_hints` → `GlyphHints.metrics`.

`apply_hints` signature gains `metrics: &AfLatinMetrics`.

---

## PHASE B — segment height extension (aflatin.c:1959-2005)

After `compute_segments` builds the per-contour segments, extend each segment's
`height` by half the adjacent half-tint, so serifs can be detected/ignored:

```
for seg in axis.segments:
  first = points[seg.first]; last = points[seg.last]
  fv = first.v; lv = last.v          // v = cross-axis coord (set by compute_segments u/v swap)
  if fv < lv:
    p = points[first.prev]
    if p.v < fv: seg.height += (fv - p.v) >> 1
    p = points[last.next]
    if p.v > lv: seg.height += (p.v - lv) >> 1
  else:
    p = points[first.prev]
    if p.v > fv: seg.height += (p.v - fv) >> 1
    p = points[last.next]
    if p.v < lv: seg.height += (lv - p.v) >> 1
```

`seg.height` is `i16`; cast result through `as i16`. Insert at end of
`compute_segments`, before return.

---

## PHASE C — segment filtering in compute_edges (aflatin.c:2190-2251)

When forming edges, skip noise segments. Thresholds in font units:

```
// scale = dim==Horz ? x_scale : y_scale   (aflatin.c:2182-2183)
if dim == Horz:
  segment_length_threshold = FT_DivFix(64, hints.y_scale)   // 1px in font units
else:  // Vert axis: NO length filtering
  segment_length_threshold = 0
segment_width_threshold = FT_DivFix(32, scale)              // 0.5px in font units

edge_distance_threshold = FT_MulFix(axis.edge_distance_threshold, scale)
if > 16: = 16                            // cap at 0.25px (64/4)
edge_distance_threshold = FT_DivFix(edge_distance_threshold, scale)       // back to font units

for seg in segments:
  // FILTER:
  if seg.height < segment_length_threshold: continue
  if seg.delta > segment_width_threshold:  continue
  if seg.dir == None:                      continue
  if seg.serif != MAX and 2*seg.height < 3*segment_length_threshold: continue   // tiny serif
  ... find/create edge by |seg.pos - edge.fpos| < edge_distance_threshold && same dir ...
```

`FT_DivFix(64, scale)` converts the 26.6 value 64 (=1px) back into font units so it
compares against `seg.height`/`seg.delta` (font units). NOTE: Horz length-threshold
uses `hints.y_scale` (not x_scale) because Horz-axis segments run vertically and
their height is measured in Y font units — scale must be the Y scale.

Our current `compute_edges` uses a fixed `EDGE_DISTANCE_THRESHOLD=50` and no
length/width/serif filtering → replace with the thresholded version.

---

## PHASE D — proper IUP (afhints.c:1592-1808)

Replace the hand-rolled linear `align_weak_points` with the TrueType IUP
specification: `af_iup_shift` + `af_iup_interp`, driven per-contour.

### D.1 `af_iup_shift(p1..p2, ref)` (afhints.c:1592)
```
delta = ref.u - ref.v
if delta == 0: return
for p in p1..ref-1:   p.u = p.v + delta
for p in ref+1..p2:   p.u = p.v + delta
```

### D.2 `af_iup_interp(p1..p2, ref1, ref2)` (afhints.c:1619)
```
if p1 > p2: return
if ref1.v > ref2.v: swap(ref1, ref2)
v1=ref1.v; v2=ref2.v; u1=ref1.u; u2=ref2.u
d1 = u1 - v1; d2 = u2 - v2
if u1==u2 or v1==v2:
  for p in p1..p2:
    u = p.v
    if u <= v1: u += d1
    elif u >= v2: u += d2
    else: u = u1
    p.u = u
else:
  scale = FT_DivFix(u2-u1, v2-v1)     // 16.16
  for p in p1..p2:
    u = p.v
    if u <= v1: u += d1
    elif u >= v2: u += d2
    else: u = u1 + FT_MulFix(u - v1, scale)
    p.u = u
```

### D.3 driver `af_glyph_hints_align_weak_points` (afhints.c:1687)
```
touch = (dim==Horz) ? TOUCH_X : TOUCH_Y
PASS1: for p in points: p.u = (Horz? p.x : p.y);  p.v = (Horz? p.ox : p.oy)
for each contour:
  point = contour_start; end_point = point.prev; first_point = point
  // find first touched point:
  loop { if point > end_point: goto NextContour;  if point.flags&touch: break;  point = next_in_storage_order }
  first_touched = point
  loop {
    // skip consecutive touched:
    while point < end_point and point.next.flags&touch: point = next
    last_touched = point
    point = next
    loop { if point > end_point: goto EndContour; if point.flags&touch: break; point = next }
    iup_interp(last_touched.next .. point.prev, last_touched, point)
  }
  EndContour:
    if last_touched == first_touched:
      iup_shift(first_point .. end_point, first_touched)
    else:
      if last_touched < end_point:
        iup_interp(last_touched.next .. end_point, last_touched, first_touched)
      if first_touched > points[0]:
        iup_interp(first_point .. first_touched.prev, last_touched, first_touched)
  NextContour:
PASS2: for p: if Horz: p.x = p.u  else p.y = p.u
```

NOTE on "next" / pointer order: the C code advances `point++` (storage order),
NOT the contour `next` link. IUP operates on the **storage array** within a
contour's point range `[contour_start .. contour_start.prev]`. Our `points` Vec
is already in storage order and contours are contiguous, so iterate indices.
`end_point` = last index of the contour (= `end_pts_of_contours[c]`).

---

## PHASE E — blue zones (the largest piece, ~unlocks 25%)

### E.1 `af_latin_metrics_init_blues` (aflatin.c:311-1039, ~220 lines core)

For the VERT axis only, scan the 6 blue character strings; for each, find the
median flat (reference) and round (overshoot) Y extremum across the string's
glyphs. Algorithm per blue zone:

```
flats[], rounds[] empty; ascender=0; descender=0
flat_threshold = upem/14
for each char c in this blue's string:
  gid = cmap[c]; load outline (NO_SCALE, font units); skip if <=2 points
  best_y_extremum = TOP/SUB_TOP ? LONG_MIN : LONG_MAX
  best_round = false; best_point=-1
  // over glyph elements (num_idx; Latin has 1):
  for each contour (skip single-point):
    if TOP or SUB_TOP:
      for pp in contour: track MAX y → best_point/best_y; ascender=max(ascender, y+off)
                      else descender=min(descender, y+off)
    else (bottom):
      for pp in contour: track MIN y → best_point/best_y; descender=min(...); else ascender=max(...)
    if best_point > best_contour_last: record contour first/last
  // classify flat vs round at the extremum:
  if best_point >= 0:
    best_x = points[best_point].x
    // walk prev while |dy|<=5 or |dx| <= 20*|dy|  (expand flat segment)
    // walk next likewise → best_segment_first/last, best_on_point_first/last
    // [LONG blue variant: aflatin.c:645-822 — search a longer same-direction
    //  segment within height_threshold upem/4; SKIP for first cut, only 'h'/'b'
    //  style glyphs use it and it rarely changes DejaVu/Liberation blues]
    round = (|on_last.x - on_first.x| > flat_threshold) ? false
          : (tag[first]!=ON || tag[last]!=ON)
    if round and NEUTRAL: continue
  best_y += y_offset  // =0 for Latin
  if TOP: if best_y > best_y_extremum: update (+best_round)
  else:   if best_y < best_y_extremum: update
  // after all chars: median
if flats empty and rounds empty: skip this blue
sort_pos(rounds); sort_pos(flats)
if flats==0: ref=shoot=rounds[n/2]
elif rounds==0: ref=shoot=flats[n/2]
else: ref=flats[nf/2]; shoot=rounds[nr/2]
// overshoot sanity:
if shoot != ref:
  over_ref = shoot > ref
  if (TOP||SUB_TOP) ^ over_ref:    ref=shoot=(ref+shoot)/2
blue.ascender=ascender; blue.descender=descender
flags from properties (TOP/SUB_TOP/NEUTRAL/BOTTOM/BOTTOM_SMALL); X_HEIGHT→ADJUSTMENT
push blue; blue_count++
```

After all blues: sort zones bottom→top (`af_latin_sort_blue`, aflatin.c:268-304)
and resolve overlaps (clamp upper zone's top to lower zone's top if inverted).
`af_sort_pos` = insertion sort ascending (afhints.c:36).

### E.2 `af_latin_metrics_scale_dim` blue scaling (aflatin.c:1357-1437)

After computing `axis.scale`/`delta` for the requested size, scale the blue
zones (VERT axis only):

```
for blue in blues:
  blue.ref.cur   = FT_MulFix(blue.ref.org, scale) + delta
  blue.ref.fit   = blue.ref.cur
  blue.shoot.cur = FT_MulFix(blue.shoot.org, scale) + delta
  blue.shoot.fit = blue.shoot.cur
  blue.flags &= ~ACTIVE
  dist = FT_MulFix(blue.ref.org - blue.shoot.org, scale)   // scaled overshoot
  if -48 <= dist <= 48:                                     // zone < 3/4px tall
    delta2 = |dist|
    if delta2 < 32: delta2 = 0
    elif delta2 < 48: delta2 = 32
    else: delta2 = 64
    if dist < 0: delta2 = -delta2
    blue.ref.fit   = FT_PIX_ROUND(blue.ref.cur)             // (x+32)&~63
    blue.shoot.fit = blue.ref.fit - delta2
    blue.flags |= ACTIVE
// sub-top overlap suppression (aflatin.c:1443+): drop a SUB_TOP+ACTIVE blue
// if it overlaps a non-SUB-TOP active blue (used by Khmer subscript tops).
```

### E.3 x-height scale optimization (aflatin.c:1238-1306)

For the blue with `ADJUSTMENT` flag (the small-top / x-height zone), if its scaled
`ref` doesn't land on a pixel, nudge the **vertical scale** so it does (within ±2px
of total height). Optional but affects ~5% of glyphs:

```
for blue in blues where ADJUSTMENT and ACTIVE:
  scaled = FT_MulFix(blue.ref.org, scale) + delta
  // increase-x-height property threshold (Latin: limit=0 → threshold stays 40)
  threshold = 40; if limit and ppem<=limit and ppem>=MIN: threshold=52
  fitted = (scaled + threshold) & ~63
  if scaled != fitted and dim==VERT:
    new_scale = FT_MulDiv(scale, fitted, scaled)
    max_height = upem
    for b in blues: max_height=max(max_height, b.ascender, -b.descender)
    dist = FT_MulFix(max_height, new_scale - scale)
    if -128 < dist < 128: scale = new_scale
axis.scale = scale; axis.delta = delta
```
(limit comes from `face->internal->increase_x_height` — for FreeType/non-instructed fonts
this is 0, so threshold stays 40 and the branch is just `FT_PIX_ROUND(scaled)`-ish.)

### E.4 `af_latin_hints_compute_blue_edges` (aflatin.c:2529-2640)

After `compute_edges(VERT)`, assign each horizontal edge a `blue_edge` (the
`.fit` AfWidth to snap to):

```
scale = metrics.axis[VERT].scale
for edge in vert edges:
  if edge.flags & NO_BLUE: continue
  best_dist = FT_MulFix(upem/40, scale); if > 32: best_dist = 32   // ≤0.5px
  best_blue = None; best_neutral=false
  for blue in vert blues:
    if !(blue.flags & ACTIVE): continue
    is_top    = blue.flags & (TOP|SUB_TOP) != 0
    is_neutral= blue.flags & NEUTRAL != 0
    is_major  = edge.dir == major_dir
    if (is_top ^ is_major) or is_neutral:
      dist = |edge.fpos - blue.ref.org|; dist = FT_MulFix(dist, scale)
      if dist < best_dist: best_dist=dist; best_blue=blue.ref; best_neutral=is_neutral
      if edge.flags&ROUND and dist!=0 and !is_neutral:
        is_under_ref = edge.fpos < blue.ref.org
        if is_top ^ is_under_ref:
          dist = |edge.fpos - blue.shoot.org|; dist = FT_MulFix(dist, scale)
          if dist < best_dist: best_dist=dist; best_blue=blue.shoot; best_neutral=is_neutral
  if best_blue: edge.blue_edge = best_blue; if best_neutral: edge.flags|=NEUTRAL
```

### E.5 hint_edges Phase 1 — blue alignment (aflatin.c:4247-4336)

Already partially scaffolded in our `hint_edges`. Activate:

```
if dim==VERT and AF_HINTS_DO_BLUES:
  for edge in edges:
    if edge.flags&DONE: continue
    edge1=None; edge2=edge.link
    // neutral dedup
    if edge.blue_edge and edge2 and edge2.blue_edge:
       if edge2 NEUTRAL: edge2.blue_edge=None; edge2.flags&=!NEUTRAL
       elif edge NEUTRAL: edge.blue_edge=None; edge.flags&=!NEUTRAL
    blue = edge.blue_edge
    if blue: edge1=edge
    elif edge2 and edge2.blue_edge: blue=edge2.blue_edge; edge1=edge2; edge2=edge
    if !edge1: continue
    edge1.pos = blue.fit; edge1.flags|=DONE
    if edge2 and !edge2.blue_edge: align_linked_edge(edge1,edge2); edge2.flags|=DONE
    if anchor==None: anchor=edge_index   // blue edge becomes the anchor
```

`AF_HINTS_DO_BLUES(hints)` is true unless `NO_HINTING`/`MONO`-style disables — for
our smooth path it's always true.

---

## PHASE F — polish

### F.1 lowercase 'm' symmetry (aflatin.c:4582-4627)
Between Phase 2 and Phase 4, if `num_edges==6 or 12` and edges form the 'm'
pattern (3 humps: edges[0,2,4] linked to [1,3,5]), enforce equal stem spacing.
Needs link_segments active (Phase A). Port only if 'm' parity is wrong after A–E.

### F.2 second pass for directionless segments (aflatin.c ~2306-2342)
After the main segment pass, a second pass re-scans for segments whose direction
wasn't classified (dir==None) and tries to assign them. Low impact for Latin;
port if specific glyphs still miss.

### F.3 strong-point IP uses `ft_mul_div` (already does in our align_strong_points)
Confirm interpolation uses FT_MulDiv not i64 division — current code uses i64
linear; switch to `ft_mul_div` for bit-exactness.

---

## Implementation order & wiring

```
apply_hints(outline, raw, x_scale, y_scale, x_delta, y_delta, metrics):
  hints.metrics = metrics
  reload(...)
  // VERT (horizontal edges — Y):
  compute_segments(VERT);  height_extension(VERT)        // Phase B
  link_segments(VERT, metrics.axis[VERT])                 // Phase A (re-enabled)
  compute_edges(VERT)        // Phase C filtering + threshold
  compute_blue_edges(hints, metrics)                      // Phase E.4
  hint_edges(VERT)           // Phase 1 blue + Phase 2 stems + Phase 4 non-stem
  align_edge_points(VERT)
  align_strong_points(VERT)  // Phase F.3: ft_mul_div
  align_weak_points(VERT)    // Phase D: real IUP
  // HORZ (vertical edges — X): same minus blue zones
  ... compute_segments(HORz); height_extension; link_segments(Horz); compute_edges;
      hint_edges; align_edge_points; align_strong_points; align_weak_points ...
  save_to_outline
```

`metrics` computed once per (font, size): `af_latin_metrics_init_widths` then
`af_latin_metrics_init_blues` then `af_latin_metrics_scale_dim` (applies the
size's scale + scales widths + scales blues). Cache on `Font`.
