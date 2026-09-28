// Set the alpha byte in each native LA sample without transporting RGBA.
// Input/output bytes are [L0, A0, L1, A1, ...], packed into storage words.

struct Params {
    columns: u32,
    rows: u32,
    mode: u32,
    words: u32,
    alpha: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.columns || gid.y >= params.rows {
        return;
    }

    let word_index = gid.y * params.columns + gid.x;
    if word_index >= params.words {
        return;
    }

    let source = input[word_index];
    let alpha = params.alpha & 0xffu;
    let alpha_lanes = 0xFF00FF00u;
    let replacement = (alpha << 8u) | (alpha << 24u);
    output[word_index] = (source & ~alpha_lanes) | replacement;
}
