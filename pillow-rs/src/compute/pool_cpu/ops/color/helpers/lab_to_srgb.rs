//! Native Rust implementation of Pillow's built-in LAB-to-sRGB conversion.

use std::sync::OnceLock;

use crate::raster::RgbImage;

const GRID: usize = 33;
const GRID_LAST: usize = GRID - 1;
const CLUT_SIZE: usize = GRID * GRID * GRID;
const MAX_ENCODEABLE_XYZ: f64 = (2.0 * 65_535.0) / 65_536.0;

// D50 XYZ to linear sRGB for the virtual D65 Rec. 709 profile.
const XYZ_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
    [
        3.134_186_364_236_820_4,
        -1.617_208_958_998_275_4,
        -0.490_694_064_006_384,
    ],
    [
        -0.978_748_504_190_693_7,
        1.916_130_096_773_585_5,
        0.033_433_399_159_995_52,
    ],
    [
        0.071_963_927_802_246_74,
        -0.228_993_873_453_203_07,
        1.405_753_732_896_443_3,
    ],
];

static LAB_TO_SRGB_CLUT: OnceLock<Vec<[u16; 3]>> = OnceLock::new();

/// Converts packed LAB8 samples through a cached 33³ sRGB lookup table.
pub(crate) fn srgb_pixels(source: &RgbImage) -> impl ExactSizeIterator<Item = [u8; 3]> + '_ {
    let clut = LAB_TO_SRGB_CLUT.get_or_init(build_clut).as_slice();
    source
        .pixels()
        .map(move |pixel| lab_to_srgb_pixel(pixel.0, clut))
}

fn build_clut() -> Vec<[u16; 3]> {
    let mut clut = Vec::with_capacity(CLUT_SIZE);
    for l in 0..GRID {
        for a in 0..GRID {
            for b in 0..GRID {
                clut.push(lab_v4_node_to_srgb(
                    grid_coordinate(l),
                    grid_coordinate(a),
                    grid_coordinate(b),
                ));
            }
        }
    }
    clut
}

fn grid_coordinate(index: usize) -> u16 {
    ((index * u16::MAX as usize + GRID_LAST / 2) / GRID_LAST) as u16
}

fn lab_v4_node_to_srgb(l: u16, a: u16, b: u16) -> [u16; 3] {
    // LittleCMS evaluates the sampled pipeline as f32 between its Lab, XYZ,
    // matrix, and tone-curve stages. Keep those boundaries: calculating the
    // same equations in f64 end-to-end changes a few final 8-bit channels.
    let lab = [l, a, b].map(|value| f32::from(value) / f32::from(u16::MAX));
    let lightness = f64::from(lab[0]) * 100.0;
    let a = f64::from(lab[1]) * 255.0 - 128.0;
    let b = f64::from(lab[2]) * 255.0 - 128.0;
    let fy = (lightness + 16.0) / 116.0;
    let xyz = [
        lab_inverse_curve(fy + a / 500.0) * 0.9642,
        lab_inverse_curve(fy),
        lab_inverse_curve(fy - b / 200.0) * 0.8249,
    ]
    .map(|value| (value / MAX_ENCODEABLE_XYZ) as f32);

    XYZ_TO_LINEAR_SRGB.map(|row| {
        let linear = f64::from(xyz[0]) * row[0] * MAX_ENCODEABLE_XYZ
            + f64::from(xyz[1]) * row[1] * MAX_ENCODEABLE_XYZ
            + f64::from(xyz[2]) * row[2] * MAX_ENCODEABLE_XYZ;
        encode_srgb(linear as f32)
    })
}

#[inline]
fn lab_inverse_curve(value: f64) -> f64 {
    if value > 6.0 / 29.0 {
        value * value * value
    } else {
        (108.0 / 841.0) * (value - 4.0 / 29.0)
    }
}

#[inline]
fn encode_srgb(linear: f32) -> u16 {
    let linear = f64::from(linear);
    let encoded = if linear <= (1.0 / 12.92) * 0.04045 {
        linear / (1.0 / 12.92)
    } else {
        (linear.powf(1.0 / 2.4) - (0.055 / 1.055)) / (1.0 / 1.055)
    };
    let encoded = (encoded as f32).clamp(0.0, 1.0);
    (f64::from(encoded) * f64::from(u16::MAX) + 0.5).floor() as u16
}

fn lab_to_srgb_pixel(lab: [u8; 3], clut: &[[u16; 3]]) -> [u8; 3] {
    let encoded = [
        u16::from(lab[0]) * 257,
        u16::from(lab[1] ^ 128) * 257,
        u16::from(lab[2] ^ 128) * 257,
    ];
    let axes = encoded.map(grid_axis);
    let base = axes[0].cell * GRID * GRID + axes[1].cell * GRID + axes[2].cell;
    let steps = [
        axes[0].step * GRID * GRID,
        axes[1].step * GRID,
        axes[2].step,
    ];
    let fractions = [axes[0].fraction, axes[1].fraction, axes[2].fraction];
    let order = tetrahedron_order(fractions);
    let nodes = [
        base,
        base + steps[order[0]],
        base + steps[order[0]] + steps[order[1]],
        base + steps[order[0]] + steps[order[1]] + steps[order[2]],
    ];

    std::array::from_fn(|channel| {
        let first = i32::from(clut[nodes[0]][channel]);
        let second = i32::from(clut[nodes[1]][channel]);
        let third = i32::from(clut[nodes[2]][channel]);
        let fourth = i32::from(clut[nodes[3]][channel]);
        let delta = interpolate_channel(
            first,
            second,
            third,
            fourth,
            fractions[order[0]],
            fractions[order[1]],
            fractions[order[2]],
        );
        let value = (first.wrapping_add(delta)) as u16;
        ((u32::from(value) * 65_281 + 8_388_608) >> 24) as u8
    })
}

#[derive(Clone, Copy)]
struct GridAxis {
    cell: usize,
    fraction: i64,
    step: usize,
}

fn grid_axis(value: u16) -> GridAxis {
    let scaled = u32::from(value) * GRID_LAST as u32;
    let fixed = scaled + ((scaled + 0x7fff) / u32::from(u16::MAX));
    GridAxis {
        cell: (fixed >> 16) as usize,
        fraction: i64::from(fixed & 0xffff),
        step: if value == u16::MAX { 0 } else { 1 },
    }
}

fn tetrahedron_order(fractions: [i64; 3]) -> [usize; 3] {
    let [first, second, third] = fractions;
    if first >= second {
        if second >= third {
            [0, 1, 2]
        } else if third >= first {
            [2, 0, 1]
        } else {
            [0, 2, 1]
        }
    } else if first >= third {
        [1, 0, 2]
    } else if second >= third {
        [1, 2, 0]
    } else {
        [2, 1, 0]
    }
}

#[inline]
fn interpolate_channel(
    first: i32,
    second: i32,
    third: i32,
    fourth: i32,
    first_fraction: i64,
    second_fraction: i64,
    third_fraction: i64,
) -> i32 {
    let first_delta = second.wrapping_sub(first);
    let second_delta = third.wrapping_sub(second);
    let third_delta = fourth.wrapping_sub(third);
    let weighted = first_delta
        .wrapping_mul(first_fraction as i32)
        .wrapping_add(second_delta.wrapping_mul(second_fraction as i32))
        .wrapping_add(third_delta.wrapping_mul(third_fraction as i32))
        .wrapping_add(0x8001);
    weighted.wrapping_add(weighted >> 16) >> 16
}
