// Flip native one- and two-byte pixel rows without color conversion.
// Each input row is padded to a four-byte boundary by the host, and every
// invocation copies one complete storage word to one distinct output word.
struct Params {
    height: u32,
    words_per_row: u32,
    word_count: u32,
    groups_x: u32,
}

@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

@compute @workgroup_size(64)
fn main(
    @builtin(local_invocation_id) local: vec3<u32>,
    @builtin(workgroup_id) group: vec3<u32>,
) {
    // Flatten the bounded 2D dispatch grid. Its product is checked by the
    // planner, so the multiplication stays within the u32 word index.
    let workgroup = group.y * params.groups_x + group.x;
    let output_word = workgroup * 64u + local.x;
    if output_word >= params.word_count {
        return;
    }

    let output_row = output_word / params.words_per_row;
    let column_word = output_word % params.words_per_row;
    let source_row = params.height - 1u - output_row;
    let source_word = source_row * params.words_per_row + column_word;
    output[output_word] = input[source_word];
}
