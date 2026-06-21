//! IUP — Interpolation of Unscaled Points.
//!
//! After hinting touched points, untouched points must be interpolated
//! from their original outline positions relative to the nearest touched
//! reference points in each contour.
//!
//! The algorithm mirrors FreeType's Ins_IUP exactly.

#![allow(missing_docs)]
#![allow(unused_assignments)]

use super::graphics::*;

#[inline]
fn mul_div(a: i32, b: i32, c: i32) -> i32 {
    if c == 0 {
        return a;
    }
    // FreeType FT_MulDiv_No_Round: ((int64_t)a * b + ((int64_t)c >> 1)) / c
    (((a as i64 * b as i64) + (c as i64 >> 1)) / c as i64) as i32
}

// ---------------------------------------------------------------------------
// Shifting: used when a contour has only ONE touched point.
// Equivalent to FreeType's iup_worker_shift_.
// ---------------------------------------------------------------------------
fn iup_shift(zone: &mut Zone, p1: usize, p2: usize, p: usize, dir: u8) {
    let dx = if dir == 0 {
        zone.points[p].x - zone.org[p].x
    } else {
        zone.points[p].y - zone.org[p].y
    };
    if dx != 0 {
        for i in p1..p {
            if dir == 0 {
                zone.points[i].x += dx;
            } else {
                zone.points[i].y += dx;
            }
        }
        for i in (p + 1)..=p2 {
            if dir == 0 {
                zone.points[i].x += dx;
            } else {
                zone.points[i].y += dx;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Interpolation: handle points in [p1, p2] with references ref1 and ref2.
// Equivalent to FreeType's iup_worker_interpolate_.
//
// FreeType uses three arrays:
//   orgs – original (pre-hinting) positions  -> used for range classification
//   curs – current (post-hinting) positions  -> used for final values
//   orus – original unscaled positions        -> used for interpolation ratio
//
// We use zone.org for both orgs and orus (ratio), since we don't have a
// separate unscaled array.  This gives the same structural algorithm;
// the ratio approximates FreeType's to within rounding.
// ---------------------------------------------------------------------------
fn iup_interpolate(
    zone: &mut Zone,
    p1: usize,
    p2: usize,
    ref1: usize,
    ref2: usize,
    dir: u8,
) {
    if p1 > p2 {
        return;
    }
    if ref1 >= zone.points.len() || ref2 >= zone.points.len() {
        return;
    }

    // Grab the reference-frame orus-values (we use org in place of orus).
    let (mut orus1, mut orus2) = if dir == 0 {
        (zone.org[ref1].x, zone.org[ref2].x)
    } else {
        (zone.org[ref1].y, zone.org[ref2].y)
    };

    // FreeType sorts orus1 <= orus2 and swaps refs accordingly.
    let (r1, r2) = if orus1 > orus2 {
        std::mem::swap(&mut orus1, &mut orus2);
        (ref2, ref1)
    } else {
        (ref1, ref2)
    };

    let (org1, org2) = if dir == 0 {
        (zone.org[r1].x, zone.org[r2].x)
    } else {
        (zone.org[r1].y, zone.org[r2].y)
    };
    let (cur1, cur2) = if dir == 0 {
        (zone.points[r1].x, zone.points[r2].x)
    } else {
        (zone.points[r1].y, zone.points[r2].y)
    };

    let delta1 = cur1 - org1;
    let delta2 = cur2 - org2;

    if cur1 == cur2 || orus1 == orus2 {
        // Trivial snap-or-shift path.
        for i in p1..=p2 {
            if i >= zone.points.len() {
                break;
            }
            let x = if dir == 0 {
                zone.org[i].x
            } else {
                zone.org[i].y
            };
            let new_val = if x <= org1 {
                x + delta1
            } else if x >= org2 {
                x + delta2
            } else {
                cur1
            };
            if dir == 0 {
                zone.points[i].x = new_val;
            } else {
                zone.points[i].y = new_val;
            }
        }
    } else {
        // Interpolation path.
        let delta_cur = cur2 - cur1;
        let delta_orus = orus2 - orus1;

        for i in p1..=p2 {
            if i >= zone.points.len() {
                break;
            }
            let x = if dir == 0 {
                zone.org[i].x
            } else {
                zone.org[i].y
            };
            let new_val = if x <= org1 {
                x + delta1
            } else if x >= org2 {
                x + delta2
            } else {
                let orus_val = if dir == 0 {
                    zone.org[i].x
                } else {
                    zone.org[i].y
                };
                cur1 + mul_div(orus_val - orus1, delta_cur, delta_orus)
            };
            if dir == 0 {
                zone.points[i].x = new_val;
            } else {
                zone.points[i].y = new_val;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry-point — mirrors FreeType's Ins_IUP.
// ---------------------------------------------------------------------------
pub fn iup_zone(zone: &mut Zone, direction: u8) {
    let n_contours = zone.n_contours as usize;
    if n_contours == 0 {
        return;
    }

    let mut point = 0usize;

    for ci in 0..n_contours {
        let end_point = zone.contours[ci] as usize;
        if end_point >= zone.n_points as usize {
            point = end_point + 1;
            continue;
        }

        let first_point = point;

        // Advance past untouched points to find the first touched one.
        while point <= end_point && !is_touched(zone, point, direction) {
            point += 1;
        }

        if point <= end_point {
            let first_touched = point;
            let mut cur_touched = point;

            point += 1;

            // Interpolate between consecutive touched points.
            while point <= end_point {
                if is_touched(zone, point, direction) {
                    iup_interpolate(
                        zone,
                        cur_touched + 1,
                        point - 1,
                        cur_touched,
                        point,
                        direction,
                    );
                    cur_touched = point;
                }
                point += 1;
            }

            // Handle wrap-around in the contour.
            if cur_touched == first_touched {
                // Only one touched point found — shift every point by that
                // point's delta.
                iup_shift(zone, first_point, end_point, cur_touched, direction);
            } else {
                // Interpolate the tail segment (after last touched -> contour end).
                iup_interpolate(
                    zone,
                    cur_touched + 1,
                    end_point,
                    cur_touched,
                    first_touched,
                    direction,
                );

                // Interpolate the head segment (contour start -> before first touched).
                if first_touched > first_point {
                    iup_interpolate(
                        zone,
                        first_point,
                        first_touched - 1,
                        cur_touched,
                        first_touched,
                        direction,
                    );
                }
            }
        }

        point = end_point + 1;
    }
}

fn is_touched(zone: &Zone, idx: usize, dir: u8) -> bool {
    if idx >= zone.tags.len() {
        return false;
    }
    if dir == 0 {
        (zone.tags[idx] & TOUCH_X) != 0
    } else {
        (zone.tags[idx] & TOUCH_Y) != 0
    }
}
