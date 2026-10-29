use super::{
    GPU_TRANSFORM_GEOMETRY_WORDS, PilError, PipelineOp, ResampleFilter, TransformMethod,
    gpu_transform_geometry_is_admitted,
};
pub(in crate::compute::pool_gpu) fn gpu_transform_geometry_words(
    op: &PipelineOp,
    source: (u32, u32),
    mode: Option<&str>,
) -> Result<Vec<u32>, PilError> {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        fill_is_none,
        ..
    } = op
    else {
        return Err(PilError::InternalError(
            "GPU geometry requires a transform".into(),
        ));
    };
    if !gpu_transform_geometry_is_admitted(op, mode, source) {
        return Err(PilError::ValueError(
            "GPU transform geometry exceeds bounded storage or sample layout".into(),
        ));
    }
    let nearest = matches!(filter, ResampleFilter::Nearest) || matches!(mode, Some("P" | "1"));
    let cubic = matches!(filter, ResampleFilter::Bicubic) && !nearest;
    let mut geometry = GpuTransformGeometryOutput::new(*w, *h, source, nearest, cubic);
    if matches!(method, TransformMethod::Mesh) {
        gpu_transform_mesh_geometry_words(&mut geometry, data, *fill_is_none);
    } else {
        gpu_transform_regular_geometry_words(&mut geometry, method, data);
    }
    Ok(geometry.words)
}

struct GpuTransformGeometryOutput {
    width: u32,
    height: u32,
    source: (u32, u32),
    nearest: bool,
    cubic: bool,
    words: Vec<u32>,
}

impl GpuTransformGeometryOutput {
    fn new(width: u32, height: u32, source: (u32, u32), nearest: bool, cubic: bool) -> Self {
        Self {
            width,
            height,
            source,
            nearest,
            cubic,
            words: vec![0; (width as usize) * (height as usize) * GPU_TRANSFORM_GEOMETRY_WORDS],
        }
    }

    #[inline(always)]
    fn record(&mut self, x: u32, y: u32, sx: f64, sy: f64, clear_failed: bool) {
        let row = &mut self.words[((y * self.width + x) as usize) * GPU_TRANSFORM_GEOMETRY_WORDS..]
            [..GPU_TRANSFORM_GEOMETRY_WORDS];
        if !sx.is_finite()
            || !sy.is_finite()
            || sx < 0.0
            || sy < 0.0
            || sx >= f64::from(self.source.0)
            || sy >= f64::from(self.source.1)
        {
            if clear_failed {
                row[0] = 2;
            }
            return;
        }
        row[0] = 1;
        if self.nearest {
            row[1] = sy as u32 * self.source.0 + sx as u32;
            return;
        }
        let sample_x = sx - 0.5;
        let sample_y = sy - 0.5;
        let floor_x = sample_x.floor() as i64;
        let floor_y = sample_y.floor() as i64;
        let dx = (sample_x - floor_x as f64).to_bits();
        let dy = (sample_y - floor_y as f64).to_bits();
        row[9..13].copy_from_slice(&[dx as u32, (dx >> 32) as u32, dy as u32, (dy >> 32) as u32]);
        for tap in 0..4 {
            row[1 + tap] = (floor_x + tap as i64 - i64::from(self.cubic))
                .clamp(0, i64::from(self.source.0) - 1) as u32;
            row[5 + tap] = (floor_y + tap as i64 - i64::from(self.cubic))
                .clamp(0, i64::from(self.source.1) - 1) as u32
                * self.source.0;
        }
    }
}

fn gpu_transform_geometry_coefficients(q: &[f64], width: f64, height: f64) -> [f64; 8] {
    let iw = 1.0 / width;
    let ih = 1.0 / height;
    [
        q[0],
        (q[6] - q[0]) * iw,
        (q[2] - q[0]) * ih,
        (q[4] - q[2] - q[6] + q[0]) * iw * ih,
        q[1],
        (q[7] - q[1]) * iw,
        (q[3] - q[1]) * ih,
        (q[5] - q[3] - q[7] + q[1]) * iw * ih,
    ]
}

#[inline(always)]
fn gpu_transform_geometry_quad(c: [f64; 8], x: f64, y: f64) -> (f64, f64) {
    (
        (c[3] * x).mul_add(y, c[2].mul_add(y, c[1].mul_add(x, c[0]))),
        (c[7] * x).mul_add(y, c[6].mul_add(y, c[5].mul_add(x, c[4]))),
    )
}

fn gpu_transform_mesh_geometry_words(
    output: &mut GpuTransformGeometryOutput,
    data: &[f64],
    fill_is_none: bool,
) {
    for mesh in data.chunks_exact(12) {
        let (x0, y0, x1, y1) = (
            mesh[0] as i64,
            mesh[1] as i64,
            mesh[2] as i64,
            mesh[3] as i64,
        );
        let width = x1.saturating_sub(x0);
        let height = y1.saturating_sub(y0);
        if width <= 0 || height <= 0 {
            continue;
        }
        let c = gpu_transform_geometry_coefficients(&mesh[4..], width as f64, height as f64);
        let bx0 = x0.clamp(0, i64::from(output.width)) as u32;
        let by0 = y0.clamp(0, i64::from(output.height)) as u32;
        let bx1 = x1.clamp(0, i64::from(output.width)) as u32;
        let by1 = y1.clamp(0, i64::from(output.height)) as u32;
        // Geometry.c uses coordinates relative to the clipped box. With
        // explicit fill, a failed later record preserves an earlier sample;
        // omitted fill clears it. Resolve only this geometry precedence.
        for y in by0..by1 {
            for x in bx0..bx1 {
                let (sx, sy) = gpu_transform_geometry_quad(
                    c,
                    f64::from(x - bx0) + 0.5,
                    f64::from(y - by0) + 0.5,
                );
                output.record(x, y, sx, sy, fill_is_none);
            }
        }
    }
}

fn gpu_transform_regular_geometry_words(
    output: &mut GpuTransformGeometryOutput,
    method: &TransformMethod,
    data: &[f64],
) {
    let c = if matches!(method, TransformMethod::Quad) {
        Some(gpu_transform_geometry_coefficients(
            data,
            f64::from(output.width),
            f64::from(output.height),
        ))
    } else {
        None
    };
    for y in 0..output.height {
        for x in 0..output.width {
            let dx = f64::from(x) + 0.5;
            let dy = f64::from(y) + 0.5;
            let (sx, sy) = if let Some(c) = c {
                gpu_transform_geometry_quad(c, dx, dy)
            } else {
                // Geometry.c perspective_transform: fused first two products,
                // then translation and division at destination pixel centers.
                let denominator = data[6].mul_add(dx, data[7] * dy) + 1.0;
                (
                    (data[0].mul_add(dx, data[1] * dy) + data[2]) / denominator,
                    (data[3].mul_add(dx, data[4] * dy) + data[5]) / denominator,
                )
            };
            output.record(x, y, sx, sy, false);
        }
    }
}
