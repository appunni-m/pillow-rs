struct ProjectionParams {
    width: u32,
    height: u32,
    write_count: u32,
    has_zero_override: u32,
};

struct PixelOverride {
    index: u32,
    nonzero: u32,
};

@group(0) @binding(0) var<storage, read> source: array<u32>;
@group(0) @binding(1) var<storage, read_write> projections: array<u32>;
@group(0) @binding(2) var<uniform> params: ProjectionParams;
@group(0) @binding(3) var<storage, read> overrides: array<PixelOverride>;

var<workgroup> row_nonzero: array<u32, 256>;

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
    return source[index] != 0u;
}

fn has_nonzero_write_on_axis(axis_index: u32, horizontal: bool) -> bool {
    for (var write_index = 0u; write_index < params.write_count; write_index += 1u) {
        let item = overrides[write_index];
        if item.nonzero != 0u {
            let item_axis_index = select(item.index / params.width, item.index % params.width, horizontal);
            if item_axis_index == axis_index {
                return true;
            }
        }
    }
    return false;
}

@compute @workgroup_size(256, 1, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    if workgroup_id.y == 0u {
        let x = global_id.x;
        if x >= params.width {
            return;
        }
        var nonzero = false;
        for (var y = 0u; y < params.height; y += 1u) {
            let index = y * params.width + x;
            if params.has_zero_override != 0u {
                nonzero = pixel_nonzero_after_overrides(index);
            } else {
                nonzero = source[index] != 0u;
            }
            if nonzero {
                break;
            }
        }
        if !nonzero {
            nonzero = has_nonzero_write_on_axis(x, true);
        }
        projections[x] = select(0u, 1u, nonzero);
    } else if workgroup_id.x == 0u {
        let y = workgroup_id.y - 1u;
        let lane = local_id.x;
        var lane_nonzero = false;
        for (var x = lane; x < params.width; x += 256u) {
            let index = y * params.width + x;
            if params.has_zero_override != 0u {
                lane_nonzero = pixel_nonzero_after_overrides(index);
            } else {
                lane_nonzero = source[index] != 0u;
            }
            if lane_nonzero {
                break;
            }
        }
        if params.has_zero_override == 0u {
            for (var write_index = 0u; write_index < params.write_count; write_index += 1u) {
                let item = overrides[write_index];
                if item.nonzero != 0u && item.index / params.width == y {
                    lane_nonzero = true;
                }
            }
        }
        row_nonzero[lane] = select(0u, 1u, lane_nonzero);
        workgroupBarrier();

        var stride = 128u;
        loop {
            if stride == 0u {
                break;
            }
            if lane < stride {
                row_nonzero[lane] = row_nonzero[lane] | row_nonzero[lane + stride];
            }
            workgroupBarrier();
            stride = stride / 2u;
        }
        if lane == 0u {
            projections[params.width + y] = row_nonzero[0];
        }
    }
}
