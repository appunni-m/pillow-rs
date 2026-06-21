//! IUP — Interpolation of Unscaled Points.
//!
//! After hinting touched points, untouched points must be interpolated
//! from their original outline positions relative to the nearest touched
//! reference points in each contour.

#![allow(missing_docs)]
#![allow(unused_assignments)]

use super::graphics::*;

#[inline]
fn mul_div(a: i32, b: i32, c: i32) -> i32 {
    if c == 0 {
        return a;
    }
    ((a as i64 * b as i64) / c as i64) as i32
}

pub fn iup_zone(zone: &mut Zone, direction: u8) {
    let n_contours = zone.n_contours as usize;
    if n_contours == 0 {
        return;
    }

    let mut contour_start = 0usize;

    for ci in 0..n_contours {
        let contour_end = zone.contours[ci] as usize;
        if contour_end >= zone.n_points as usize {
            contour_start = contour_end + 1;
            continue;
        }

        let first_touched = find_first_touched(zone, contour_start, contour_end, direction);

        let first = match first_touched {
            Some(f) => f,
            None => {
                contour_start = contour_end + 1;
                continue;
            }
        };

        let mut last_touched = first;
        let mut curr_touched = first;

        for p in contour_start..=contour_end {
            if is_touched(zone, p, direction) {
                last_touched = curr_touched;
                curr_touched = p;
                if curr_touched != last_touched {
                    do_interpolate(zone, last_touched, curr_touched, direction);
                }
            }
        }

        if curr_touched != first {
            do_interpolate_wrap(zone, curr_touched, first, direction, contour_end);
        }

        handle_prefix(zone, first, direction, contour_start);
        contour_start = contour_end + 1;
    }
}

fn find_first_touched(zone: &Zone, start: usize, end: usize, dir: u8) -> Option<usize> {
    for p in start..=end {
        if is_touched(zone, p, dir) {
            return Some(p);
        }
    }
    None
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

fn do_interpolate(zone: &mut Zone, a: usize, b: usize, dir: u8) {
    let (a_org, b_org) = if dir == 0 {
        (zone.org[a].x, zone.org[b].x)
    } else {
        (zone.org[a].y, zone.org[b].y)
    };
    let (a_cur, b_cur) = if dir == 0 {
        (zone.points[a].x, zone.points[b].x)
    } else {
        (zone.points[a].y, zone.points[b].y)
    };
    let delta_org = b_org - a_org;
    let delta_cur = b_cur - a_cur;

    for p in (a + 1)..b {
        if p >= zone.points.len() {
            break;
        }
        if is_touched(zone, p, dir) {
            continue;
        }
        let org_dist = if dir == 0 {
            zone.org[p].x - a_org
        } else {
            zone.org[p].y - a_org
        };
        let new_pos = if delta_org != 0 {
            a_cur + mul_div(org_dist, delta_cur, delta_org)
        } else {
            a_cur
        };
        if dir == 0 {
            zone.points[p].x = new_pos;
        } else {
            zone.points[p].y = new_pos;
        }
    }
}

fn do_interpolate_wrap(zone: &mut Zone, a: usize, b: usize, dir: u8, contour_end: usize) {
    let (a_org, b_org) = if dir == 0 {
        (zone.org[a].x, zone.org[b].x)
    } else {
        (zone.org[a].y, zone.org[b].y)
    };
    let (a_cur, b_cur) = if dir == 0 {
        (zone.points[a].x, zone.points[b].x)
    } else {
        (zone.points[a].y, zone.points[b].y)
    };
    let delta_org = b_org - a_org;
    let delta_cur = b_cur - a_cur;

    for p in (a + 1)..=contour_end {
        if p >= zone.points.len() {
            break;
        }
        if is_touched(zone, p, dir) {
            continue;
        }
        let org_dist = if dir == 0 {
            zone.org[p].x - a_org
        } else {
            zone.org[p].y - a_org
        };
        let new_pos = if delta_org != 0 {
            a_cur + mul_div(org_dist, delta_cur, delta_org)
        } else {
            a_cur
        };
        if dir == 0 {
            zone.points[p].x = new_pos;
        } else {
            zone.points[p].y = new_pos;
        }
    }
}

fn handle_prefix(zone: &mut Zone, first: usize, dir: u8, contour_start: usize) {
    if first == contour_start {
        return;
    }
    for p in contour_start..first {
        if p >= zone.points.len() {
            break;
        }
        if is_touched(zone, p, dir) {
            continue;
        }
        let org_dist = if dir == 0 {
            zone.org[p].x - zone.org[first].x
        } else {
            zone.org[p].y - zone.org[first].y
        };
        let cur_val = if dir == 0 {
            zone.points[first].x
        } else {
            zone.points[first].y
        };
        let new_val = cur_val + org_dist;
        if dir == 0 {
            zone.points[p].x = new_val;
        } else {
            zone.points[p].y = new_val;
        }
    }
}
