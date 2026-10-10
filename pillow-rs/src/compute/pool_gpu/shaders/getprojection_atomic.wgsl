struct ProjectionParams {
    width: u32,
    height: u32,
    write_count: u32,
    has_zero_override: u32,
    channels: u32,
    source_is_zero: u32,
    packed_output: u32,
    _padding2: u32,
};

struct PixelOverride {
    index: u32,
    nonzero: u32,
};

@group(0) @binding(0) var<storage, read> source: array<u32>;
@group(0) @binding(1) var<storage, read_write> projections: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> params: ProjectionParams;
@group(0) @binding(3) var<storage, read> overrides: array<PixelOverride>;

fn source_pixel_nonzero(index: u32) -> bool {
    if params.channels == 4u {
        return source[index] != 0u;
    }

    let byte_index = index * params.channels;
    let word_index = byte_index / 4u;
    let shift = (byte_index % 4u) * 8u;
    var packed = source[word_index] >> shift;
    if shift > 8u {
        packed = packed | (source[word_index + 1u] << (32u - shift));
    }
    let channel_mask = (1u << (params.channels * 8u)) - 1u;
    return (packed & channel_mask) != 0u;
}

fn pixel_nonzero_after_overrides(index: u32) -> bool {
    var low = 0u;
    var high = params.write_count;
    while low < high {
        let middle = low + (high - low) / 2u;
        if overrides[middle].index < index {
            low = middle + 1u;
        } else {
            high = middle;
        }
    }
    if low < params.write_count && overrides[low].index == index {
        return overrides[low].nonzero != 0u;
    }
    return source_pixel_nonzero(index);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = global_id.x;
    let y = global_id.y;

    if params.source_is_zero != 0u {
        let override_index = y * 16u + x;
        if override_index >= params.write_count {
            return;
        }
        let item = overrides[override_index];
        if item.nonzero != 0u {
            let override_x = item.index % params.width;
            let override_y = item.index / params.width;
            if params.packed_output != 0u {
                let packed_columns = (params.width + 31u) / 32u;
                atomicOr(
                    &projections[override_x / 32u],
                    1u << (override_x % 32u),
                );
                atomicOr(
                    &projections[packed_columns + override_y / 32u],
                    1u << (override_y % 32u),
                );
            } else {
                atomicStore(&projections[override_x], 1u);
                atomicStore(&projections[params.width + override_y], 1u);
            }
        }
        return;
    }

    // Reserve one extra workgroup row for up to 128 sparse overrides. A
    // single 16x16 workgroup already provides enough lanes, even at width 1.
    let padded_width = ((params.width + 15u) / 16u) * 16u;
    let padded_height = ((params.height + 15u) / 16u) * 16u;
    if y >= padded_height {
        let override_index = (y - padded_height) * padded_width + x;
        if override_index >= params.write_count {
            return;
        }
        let item = overrides[override_index];
        if item.nonzero != 0u {
            let override_x = item.index % params.width;
            let override_y = item.index / params.width;
            atomicStore(&projections[override_x], 1u);
            atomicStore(&projections[params.width + override_y], 1u);
        }
        return;
    }

    if x >= params.width || y >= params.height {
        return;
    }

    let index = y * params.width + x;
    var nonzero = source_pixel_nonzero(index);
    if params.has_zero_override != 0u {
        nonzero = pixel_nonzero_after_overrides(index);
    }
    if nonzero {
        atomicStore(&projections[x], 1u);
        atomicStore(&projections[params.width + y], 1u);
    }
}
