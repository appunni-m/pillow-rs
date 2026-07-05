//! IUP — Interpolate Untouched Points.
//!
//! Implements FreeType's `Ins_IUP` contour rules from `ttinterp.c`: untouched
//! points are either shifted by a single touched reference point or linearly
//! interpolated between two touched reference points using original
//! coordinates.
use crate::fixed::{ft_div_fix, ft_mul_fix};

use super::zone::GlyphZone;

pub fn iup_x(zone: &mut GlyphZone) {
    iup_impl(zone, true);
}
pub fn iup_y(zone: &mut GlyphZone) {
    iup_impl(zone, false);
}

fn iup_impl(zone: &mut GlyphZone, do_x: bool) {
    let touch_bit: u8 = if do_x { 0x01 } else { 0x02 };
    let cur_arr = if do_x {
        &mut zone.cur_x
    } else {
        &mut zone.cur_y
    };
    let org_arr = if do_x { &zone.org_x } else { &zone.org_y };
    let orus_arr = if do_x { &zone.orus_x } else { &zone.orus_y };
    let n = zone.n_points as usize;
    if n == 0 {
        return;
    }

    let ends: Vec<usize> = if zone.contours.is_empty() {
        vec![n]
    } else {
        zone.contours.iter().map(|&e| e as usize + 1).collect()
    };

    let mut p = 0usize;
    for &ep in &ends {
        let ep = ep.min(n);
        if ep <= p {
            p = ep;
            continue;
        }
        let fp = p;
        while p < ep && p < zone.tags.len() && zone.tags[p] & touch_bit == 0 {
            p += 1;
        }
        if p >= ep {
            p = ep;
            continue;
        }
        let ft = p;
        let mut ct = p;
        p += 1;
        while p < ep {
            if p < zone.tags.len() && zone.tags[p] & touch_bit != 0 {
                seg(
                    cur_arr,
                    org_arr,
                    orus_arr,
                    ct + 1,
                    p.saturating_sub(1),
                    ct,
                    p,
                );
                ct = p;
            }
            p += 1;
        }
        if ct == ft {
            let delta = cur_arr[ft].wrapping_sub(org_arr[ft]);
            if delta != 0 {
                for (i, cur) in cur_arr.iter_mut().enumerate().take(ep).skip(fp) {
                    if i != ft {
                        *cur = cur.wrapping_add(delta);
                    }
                }
            }
        } else {
            if ct + 1 < ep {
                seg(cur_arr, org_arr, orus_arr, ct + 1, ep - 1, ct, ft);
            }
            if ft > fp {
                seg(cur_arr, org_arr, orus_arr, fp, ft - 1, ct, ft);
            }
        }
        p = ep;
    }
}

fn seg(cur: &mut [i32], org: &[i32], orus: &[i32], a: usize, b: usize, r1: usize, r2: usize) {
    if a > b || a >= cur.len() || r1 >= orus.len() || r2 >= orus.len() {
        return;
    }
    let b = b.min(cur.len() - 1);
    let (mut o1, mut o2) = (orus[r1], orus[r2]);
    let (ra, rb) = if o1 > o2 {
        std::mem::swap(&mut o1, &mut o2);
        (r2, r1)
    } else {
        (r1, r2)
    };
    let (g1, g2) = (org[ra], org[rb]);
    let (c1, c2) = (cur[ra], cur[rb]);
    let d1 = c1.wrapping_sub(g1);
    let d2 = c2.wrapping_sub(g2);
    if c1 == c2 || o1 == o2 {
        for i in a..=b {
            let x = org[i];
            cur[i] = if x <= g1 {
                x.wrapping_add(d1)
            } else if x >= g2 {
                x.wrapping_add(d2)
            } else {
                c1
            };
        }
    } else {
        let scale = ft_div_fix(c2.wrapping_sub(c1), o2.wrapping_sub(o1));
        for i in a..=b {
            let x = org[i];
            if x <= g1 {
                cur[i] = x.wrapping_add(d1);
            } else if x >= g2 {
                cur[i] = x.wrapping_add(d2);
            } else {
                cur[i] = c1.wrapping_add(ft_mul_fix(orus[i].wrapping_sub(o1), scale));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_single_touch() {
        let mut z = GlyphZone {
            cur_x: vec![150, 200, 300],
            cur_y: vec![0, 0, 0],
            org_x: vec![100, 200, 300],
            org_y: vec![0, 0, 0],
            orus_x: vec![100, 200, 300],
            orus_y: vec![0, 0, 0],
            tags: vec![0x01, 0x00, 0x00],
            contours: vec![2],
            n_points: 3,
            n_contours: 1,
            first_point: 0,
        };
        iup_x(&mut z);
        assert_eq!(z.cur_x[1], 250);
        assert_eq!(z.cur_x[2], 350);
    }
    #[test]
    fn test_two_touch() {
        let mut z = GlyphZone {
            cur_x: vec![100, 200, 300, 700],
            cur_y: vec![0, 0, 0, 0],
            org_x: vec![100, 200, 300, 400],
            org_y: vec![0, 0, 0, 0],
            orus_x: vec![100, 200, 300, 400],
            orus_y: vec![0, 0, 0, 0],
            tags: vec![0x01, 0x00, 0x00, 0x01],
            contours: vec![3],
            n_points: 4,
            n_contours: 1,
            first_point: 0,
        };
        iup_x(&mut z);
        assert_eq!(z.cur_x[1], 300);
        assert_eq!(z.cur_x[2], 500);
    }
}
