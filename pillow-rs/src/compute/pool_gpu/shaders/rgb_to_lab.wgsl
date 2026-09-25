// Pillow's RGB -> LAB byte transform: LCMS's 33^3 LabV4 CLUT, tetrahedral
// interpolation, then Pillow's three-byte LabV2 pack. The table stores two
// u32 words per vertex: (L | A << 16), then B in the low half.

struct Params {
    width: u32,
    height: u32,
    mode: u32,
    _pad: u32,
}

struct Axis {
    cell: u32,
    fraction: u32,
    step: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read> table: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn axis(value: u32) -> Axis {
    let scaled = value * 8224u;
    let fixed = scaled + (scaled + 32767u) / 65535u;
    return Axis(fixed >> 16u, fixed & 65535u, select(1u, 0u, value == 255u));
}

fn vertex(index: u32) -> vec3<i32> {
    let first = table[index * 2u];
    let second = table[index * 2u + 1u];
    return vec3<i32>(
        i32(first & 65535u),
        i32(first >> 16u),
        i32(second & 65535u),
    );
}

fn packed_byte(value: i32) -> u32 {
    let wide = u32(value);
    return (wide * 65281u + 8388608u) >> 24u;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }

    let index = gid.y * params.width + gid.x;
    let pixel = input[index];
    let r = axis(pixel & 255u);
    let g = axis((pixel >> 8u) & 255u);
    let b = axis((pixel >> 16u) & 255u);
    let base = r.cell * 1089u + g.cell * 33u + b.cell;

    var p = 0u;
    var q = 1u;
    var t = 2u;
    if r.fraction >= g.fraction {
        if g.fraction >= b.fraction {
            p = 0u; q = 1u; t = 2u;
        } else if b.fraction >= r.fraction {
            p = 2u; q = 0u; t = 1u;
        } else {
            p = 0u; q = 2u; t = 1u;
        }
    } else if r.fraction >= b.fraction {
        p = 1u; q = 0u; t = 2u;
    } else if g.fraction >= b.fraction {
        p = 1u; q = 2u; t = 0u;
    } else {
        p = 2u; q = 1u; t = 0u;
    }

    let fractions = array<u32, 3>(r.fraction, g.fraction, b.fraction);
    let steps = array<u32, 3>(r.step * 1089u, g.step * 33u, b.step);
    let v0 = vertex(base);
    let v1 = vertex(base + steps[p]);
    let v2 = vertex(base + steps[p] + steps[q]);
    let v3 = vertex(base + steps[p] + steps[q] + steps[t]);
    let rest = (v1 - v0) * vec3<i32>(i32(fractions[p]))
        + (v2 - v1) * vec3<i32>(i32(fractions[q]))
        + (v3 - v2) * vec3<i32>(i32(fractions[t]))
        + vec3<i32>(32769);
    // The bundled CLUT's largest adjacent delta is 2259, so all three
    // products and their sum remain inside signed 32-bit range. This is the
    // same signed correction used by LittleCMS's 15.16 tetrahedral evaluator.
    let values = v0 + ((rest + (rest >> vec3<u32>(16u))) >> vec3<u32>(16u));
    let l = packed_byte(values.x);
    let a = (packed_byte(values.y) + 128u) & 255u;
    let bb = (packed_byte(values.z) + 128u) & 255u;
    output[index] = l | (a << 8u) | (bb << 16u) | (255u << 24u);
}
