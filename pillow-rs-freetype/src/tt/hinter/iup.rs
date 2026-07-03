//! IUP — Interpolation of Untouched Points.
//! C: Ins_IUP at ttinterp.c:6189+
//! ✅ VERIFIED: Per-contour walk, orus ratio, single-touch shift, wrap-around.
//! Ported from pillow-rs-font-legacy-attempt/src/hinting/iup.rs.

use super::zone::GlyphZone;

/// IUP for X direction (opcode 0x30)
pub fn iup_x(zone: &mut GlyphZone) {
    iup_zone(zone, 0);
}

/// IUP for Y direction (opcode 0x31)
pub fn iup_y(zone: &mut GlyphZone) {
    iup_zone(zone, 1);
}

fn iup_zone(zone: &mut GlyphZone, direction: u8) {
    let touch_bit = if direction == 0 { 0x01u8 } else { 0x02u8 };
    let n = zone.n_points as usize;
    if n == 0 { return; }
    
    let cur = if direction == 0 { &mut zone.cur_x } else { &mut zone.cur_y };
    let org = if direction == 0 { &zone.org_x } else { &zone.org_y };
    let orus = if direction == 0 { &zone.orus_x } else { &zone.orus_y };
    
    let contour_ends: Vec<usize> = if zone.contours.is_empty() {
        vec![n]
    } else {
        zone.contours.iter().map(|&e| e as usize + 1).collect()
    };
    
    let mut point = 0usize;
    for &end_point in &contour_ends {
        let ep = end_point.min(n);
        if ep <= point { point = ep; continue; }
        let first_point = point;
        
        while point < ep && point < zone.tags.len() && zone.tags[point] & touch_bit == 0 {
            point += 1;
        }
        
        if point < ep {
            let first_touched = point;
            let mut cur_touched = point;
            point += 1;
            
            while point < ep {
                if zone.tags[point] & touch_bit != 0 {
                    iup_interp(cur, org, orus, cur_touched + 1, point.saturating_sub(1), cur_touched, point);
                    cur_touched = point;
                }
                point += 1;
            }
            
            if cur_touched == first_touched {
                let delta = cur[first_touched] - org[first_touched];
                if delta != 0 {
                    for i in first_point..ep {
                        if i != first_touched { cur[i] = org[i] + delta; }
                    }
                }
            } else {
                if cur_touched + 1 < ep {
                    iup_interp(cur, org, orus, cur_touched + 1, ep - 1, cur_touched, first_touched);
                }
                if first_touched > first_point {
                    iup_interp(cur, org, orus, first_point, first_touched - 1, cur_touched, first_touched);
                }
            }
        }
        point = ep;
    }
}

fn iup_interp(cur: &mut [i32], org: &[i32], orus: &[i32], p1: usize, p2: usize, ref1: usize, ref2: usize) {
    if p1 > p2 || p1 >= cur.len() || ref1 >= orus.len() || ref2 >= orus.len() { return; }
    let p2 = p2.min(cur.len() - 1);
    
    let (mut orus1, mut orus2) = (orus[ref1], orus[ref2]);
    let (r1, r2) = if orus1 > orus2 { std::mem::swap(&mut orus1, &mut orus2); (ref2, ref1) } else { (ref1, ref2) };
    
    let (org1, org2) = (org[r1], org[r2]);
    let (cur1, cur2) = (cur[r1], cur[r2]);
    let delta1 = cur1 - org1;
    let delta2 = cur2 - org2;
    
    if cur1 == cur2 || orus1 == orus2 {
        for i in p1..=p2 {
            let x = org[i];
            cur[i] = if x <= org1 { x + delta1 } else if x >= org2 { x + delta2 } else { cur1 };
        }
    } else {
        let delta_cur = cur2 - cur1;
        let delta_orus = orus2 - orus1;
        for i in p1..=p2 {
            let x = org[i];
            if x <= org1 { cur[i] = x + delta1; }
            else if x >= org2 { cur[i] = x + delta2; }
            else {
                let v = orus[i];
                let frac = if delta_orus != 0 { (((v - orus1) as i64 * delta_cur as i64 + (delta_orus as i64 >> 1)) / delta_orus as i64) as i32 } else { 0 };
                cur[i] = cur1 + frac;
            }
        }
    }
}
