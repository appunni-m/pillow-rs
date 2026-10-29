use super::super::*;
use super::*;

/// Prove a nearest projective/quad/mesh transform for raw byte samples.
///
/// Pillow's `src/libImaging/Geometry.c` `quad_transform` and mesh path evaluate
/// the inverse map in `f64`, while the projective shader evaluates destination
/// coordinates in `f32` and then rounds its selected source coordinate. Admit
/// only a bounded proof where every coefficient, destination coordinate,
/// intermediate arithmetic result, and final source coordinate select the
/// same source pixel in both domains.
/// This covers proof-certified f32-representable maps for ordinary packed byte
/// modes and raw indexed samples without claiming parity for arbitrary
/// homographies or mesh records. Nearest Quad and Mesh maps use the same
/// exhaustive source-selection proof, with explicit finite-intermediate
/// guards for their generic f32 bilinear arithmetic. Filtered projective
/// transforms remain host controlled unless an L/LA/RGB/RGBA/PA Perspective,
/// Quad, or complete one-record Mesh map is a unit-scale integer relocation or
/// a constant half-pixel source coordinate, or an interior integer coordinate
/// with the dedicated Bicubic tap proof. The Bilinear/relocation envelopes
/// land exactly on one source pixel, while the Bicubic envelope uses the
/// integer `[-1, 5, 5, -1] / 8` tap reduction; LA/RGBA also mirror Pillow's
/// premultiplied round trip.
// The comparison is intentional: the current shader receives raw destination
// indices and uses `floor(source + 0.5)`, whereas Pillow adds the destination
// pixel center before evaluating the map and then applies `COORD` truncation.
pub(in crate::compute::pool_gpu) fn gpu_projective_nearest_is_exact(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let image_layout_is_valid = match mode {
        Some("P" | "1" | "L") => matches!(image, DynamicImage::ImageLuma8(_)),
        Some("LA" | "PA") => matches!(image, DynamicImage::ImageLumaA8(_)),
        Some("RGB" | "HSV" | "YCbCr") => matches!(image, DynamicImage::ImageRgb8(_)),
        Some("RGBA" | "RGBX" | "RGBa" | "CMYK") => {
            matches!(image, DynamicImage::ImageRgba8(_))
        }
        _ => false,
    };
    image_layout_is_valid && gpu_projective_uniform_is_exact(op, mode, source_dimensions)
}

pub(in crate::compute::pool_gpu) fn gpu_projective_uniform_is_exact(
    op: &PipelineOp,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    ProjectiveUniformProof::new(op, mode, source_dimensions)
        .is_some_and(|proof| proof.proves_exact())
}

struct ProjectiveUniformProof<'a> {
    mode: Option<&'a str>,
    method: &'a TransformMethod,
    data: &'a [f64],
    filter: ResampleFilter,
    fill_is_none: bool,
    output_dimensions: (u32, u32),
    source_dimensions: (u32, u32),
    ordinary_byte_mode: bool,
    mode_is_supported: bool,
    mode_is_palette_alpha: bool,
    nearest_is_supported: bool,
}

impl<'a> ProjectiveUniformProof<'a> {
    fn new(
        op: &'a PipelineOp,
        mode: Option<&'a str>,
        source_dimensions: (u32, u32),
    ) -> Option<Self> {
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
            return None;
        };
        Some(Self {
            mode,
            method,
            data,
            filter: *filter,
            fill_is_none: *fill_is_none,
            output_dimensions: (*w, *h),
            source_dimensions,
            ordinary_byte_mode: matches!(
                mode,
                Some("L" | "LA" | "RGB" | "RGBA" | "RGBX" | "RGBa" | "CMYK" | "HSV" | "YCbCr")
            ),
            mode_is_supported: matches!(
                mode,
                Some(
                    "P" | "1"
                        | "L"
                        | "LA"
                        | "PA"
                        | "RGB"
                        | "HSV"
                        | "YCbCr"
                        | "RGBA"
                        | "RGBX"
                        | "RGBa"
                        | "CMYK"
                )
            ),
            mode_is_palette_alpha: mode == Some("PA"),
            nearest_is_supported: gpu_transform_uses_nearest(mode, *filter),
        })
    }

    fn proves_exact(&self) -> bool {
        let filtered_constant = self.filtered_constant_is_admitted();
        let filtered_relocation = filtered_constant || self.filtered_relocation_is_admitted();
        if !self.is_admitted(filtered_relocation) {
            return false;
        }
        if filtered_constant {
            return true;
        }
        self.all_source_pixels_match()
    }

    fn filtered_constant_is_admitted(&self) -> bool {
        gpu_projective_filtered_constant_is_admitted(
            self.method.clone(),
            self.data,
            self.filter,
            self.mode,
            self.source_dimensions,
            self.output_dimensions,
        )
    }

    fn filtered_relocation_is_admitted(&self) -> bool {
        gpu_projective_filtered_relocation_is_admitted(
            self.method.clone(),
            self.data,
            self.filter,
            self.mode,
            self.output_dimensions,
        )
    }

    fn is_admitted(&self, filtered_relocation: bool) -> bool {
        let palette_alpha_relocation = self.mode_is_palette_alpha
            && (gpu_palette_alpha_projective_relocation_is_admitted(
                self.method.clone(),
                self.data,
                self.filter,
                self.output_dimensions,
            ) || filtered_relocation);
        // Perspective, Quad, and Mesh maps use the shader's raw-gid f32
        // arithmetic, while the exhaustive source-selection proof below compares
        // it with Pillow's centered f64/COORD result for every output pixel.
        // Finite-intermediate guards in the proof keep generic Quad/Mesh
        // bilinear overflow from bypassing the shader sampler's bounds checks.
        // Filtered transforms intentionally remain limited to zero-weight
        // relocations, constant half-pixel maps, and the interior integer
        // Bilinear/Bicubic envelopes proven above.
        let ordinary_projective_geometry_is_admitted = matches!(
            self.method,
            TransformMethod::Perspective | TransformMethod::Quad | TransformMethod::Mesh
        );
        self.mode_is_supported
            && (!self.mode_is_palette_alpha || palette_alpha_relocation)
            && ordinary_projective_geometry_is_admitted
            && (self.nearest_is_supported || filtered_relocation)
            && self.source_dimensions.0 != 0
            && self.source_dimensions.1 != 0
            && self.output_dimensions.0 != 0
            && self.output_dimensions.1 != 0
            && self.source_dimensions.0 <= (1 << 24)
            && self.source_dimensions.1 <= (1 << 24)
            && self.output_dimensions.0 <= (1 << 24)
            && self.output_dimensions.1 <= (1 << 24)
            && u64::from(self.output_dimensions.0)
                .saturating_mul(u64::from(self.output_dimensions.1))
                <= 1_048_576
            && self.coefficients_are_exact()
            && self.mesh_record_is_admitted()
    }

    fn coefficients_are_exact(&self) -> bool {
        let required_data = match self.method {
            TransformMethod::Perspective | TransformMethod::Quad => 8,
            TransformMethod::Mesh => 12,
            TransformMethod::Affine => return false,
        };
        if self.data.len() < required_data
            || (matches!(self.method, TransformMethod::Mesh) && self.data.len() != 12)
        {
            return false;
        }
        // The transform uniform stores all coefficients as f32 bit patterns.
        // Do not admit a value that changes when it crosses that ABI boundary.
        self.data[..required_data].iter().copied().all(|value| {
            let narrowed = value as f32;
            narrowed.is_finite() && f64::from(narrowed) == value
        })
    }

    fn mesh_record_is_admitted(&self) -> bool {
        let mesh_is_full_output = !matches!(self.method, TransformMethod::Mesh)
            || self.data[..4]
                == [
                    0.0,
                    0.0,
                    f64::from(self.output_dimensions.0),
                    f64::from(self.output_dimensions.1),
                ];
        let mesh_is_partial_relocation = matches!(self.method, TransformMethod::Mesh)
            && gpu_mesh_unit_relocation_is_admitted(
                self.data,
                self.output_dimensions.0,
                self.output_dimensions.1,
            );
        match self.method {
            TransformMethod::Mesh => {
                mesh_is_full_output || (mesh_is_partial_relocation && !self.fill_is_none)
            }
            TransformMethod::Perspective | TransformMethod::Quad | TransformMethod::Affine => true,
        }
    }

    #[inline]
    fn all_source_pixels_match(&self) -> bool {
        for dy in 0..self.output_dimensions.1 {
            for dx in 0..self.output_dimensions.0 {
                if !self.shader_arithmetic_is_finite(dx, dy)
                    || !self.source_selection_matches(dx, dy)
                {
                    return false;
                }
            }
        }
        true
    }

    #[inline]
    fn shader_arithmetic_is_finite(&self, dx: u32, dy: u32) -> bool {
        let (w, h) = self.output_dimensions;
        if matches!(self.method, TransformMethod::Perspective)
            && (self.data[6] != 0.0 || self.data[7] != 0.0)
        {
            let f = |index: usize| self.data[index] as f32;
            let dx = dx as f32;
            let dy = dy as f32;
            let denominator = f(6) * dx + f(7) * dy + 1.0;
            if denominator != 0.0
                && (!denominator.is_finite()
                    || !(f(0) * dx + f(1) * dy + f(2)).is_finite()
                    || !(f(3) * dx + f(4) * dy + f(5)).is_finite())
            {
                return false;
            }
        }
        if matches!(self.method, TransformMethod::Quad)
            && !gpu_quad_constant_map_is_admitted(self.data)
            && !gpu_quad_shader_arithmetic_is_finite(self.data, w, h, dx as f32, dy as f32)
        {
            return false;
        }
        !matches!(self.method, TransformMethod::Mesh)
            || gpu_mesh_constant_map_is_admitted(self.data, w, h)
            || gpu_mesh_unit_relocation_is_admitted(self.data, w, h)
            || gpu_mesh_shader_arithmetic_is_finite(self.data, w, h, dx as f32, dy as f32)
    }

    #[inline]
    fn source_selection_matches(&self, dx: u32, dy: u32) -> bool {
        let host = self.host_source_at(f64::from(dx), f64::from(dy));
        let device = self.shader_source_at(dx as f32, dy as f32);
        if matches!(self.method, TransformMethod::Quad | TransformMethod::Mesh)
            && device.is_some_and(|(x, y)| {
                !gpu_projective_shader_coordinate_is_safe(x)
                    || !gpu_projective_shader_coordinate_is_safe(y)
            })
        {
            return false;
        }
        let host_sample = host.and_then(|(x, y)| {
            let x = self.host_coordinate(x)?;
            let y = self.host_coordinate(y)?;
            (x >= 0
                && x < i64::from(self.source_dimensions.0)
                && y >= 0
                && y < i64::from(self.source_dimensions.1))
            .then_some((x, y))
        });
        let device_sample = device.and_then(|(x, y)| {
            let x = Self::device_coordinate(x)?;
            let y = Self::device_coordinate(y)?;
            (x >= 0
                && x < i64::from(self.source_dimensions.0)
                && y >= 0
                && y < i64::from(self.source_dimensions.1))
            .then_some((x, y))
        });
        host_sample == device_sample
    }

    #[inline]
    fn host_coordinate(&self, value: f64) -> Option<i64> {
        if !value.is_finite() {
            return None;
        }
        Some(
            if matches!(
                self.method,
                TransformMethod::Perspective | TransformMethod::Quad | TransformMethod::Mesh
            ) {
                if value < 0.0 { -1 } else { value as i64 }
            } else {
                (value + 0.5).floor() as i64
            },
        )
    }

    #[inline]
    fn device_coordinate(value: f32) -> Option<i64> {
        value.is_finite().then(|| (value + 0.5).floor() as i64)
    }

    #[inline]
    fn host_source_at(&self, dx: f64, dy: f64) -> Option<(f64, f64)> {
        match self.method {
            TransformMethod::Perspective => self.host_perspective_source(dx, dy),
            TransformMethod::Quad => self.host_quad_source(dx, dy),
            TransformMethod::Mesh => self.host_mesh_source(dx, dy),
            TransformMethod::Affine => None,
        }
    }

    #[inline]
    fn host_perspective_source(&self, dx: f64, dy: f64) -> Option<(f64, f64)> {
        let dx = dx + 0.5;
        let dy = dy + 0.5;
        let denominator = self.data[6] * dx + self.data[7] * dy + 1.0;
        if denominator == 0.0 || !denominator.is_finite() {
            return None;
        }
        let sx = (self.data[0] * dx + self.data[1] * dy + self.data[2]) / denominator;
        let sy = (self.data[3] * dx + self.data[4] * dy + self.data[5]) / denominator;
        (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    }

    #[inline]
    fn host_quad_source(&self, dx: f64, dy: f64) -> Option<(f64, f64)> {
        if gpu_quad_constant_map_is_admitted(self.data) {
            return Some((self.data[0], self.data[1]));
        }
        let dx = dx + 0.5;
        let dy = dy + 0.5;
        let sw = f64::from(self.output_dimensions.0);
        let sh = f64::from(self.output_dimensions.1);
        let u = dx / sw;
        let v = dy / sh;
        let x0 = self.data[0];
        let y0 = self.data[1];
        let sx = x0
            + (self.data[6] - x0) * u
            + (self.data[2] - x0) * v
            + (self.data[4] - self.data[2] - self.data[6] + x0) * u * v;
        let sy = y0
            + (self.data[7] - y0) * u
            + (self.data[3] - y0) * v
            + (self.data[5] - self.data[3] - self.data[7] + y0) * u * v;
        (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    }

    #[inline]
    fn host_mesh_source(&self, dx: f64, dy: f64) -> Option<(f64, f64)> {
        // The shader emits the fill word outside a record's bbox. Compare
        // source selection only inside the record, like Geometry.c's clip.
        if dx < self.data[0] || dx >= self.data[2] || dy < self.data[1] || dy >= self.data[3] {
            return None;
        }
        let (w, h) = self.output_dimensions;
        if gpu_mesh_constant_map_is_admitted(self.data, w, h) {
            return Some((self.data[4], self.data[5]));
        }
        if self.ordinary_byte_mode && gpu_mesh_unit_relocation_is_admitted(self.data, w, h) {
            // Mirror CPU quad_transform FMA order for admitted relocations;
            // a four-weight reconstruction has a different rounding shape.
            let local_x = dx + 0.5 - self.data[0];
            let local_y = dy + 0.5 - self.data[1];
            let box_width = self.data[2] - self.data[0];
            let direct = self.data[10] == self.data[4] + box_width && self.data[11] == self.data[5];
            let (sx, sy) = if direct {
                (self.data[4] + local_x, self.data[5] + local_y)
            } else {
                (self.data[4] + local_y, self.data[5] + local_x)
            };
            return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
        }
        let bbox_width = self.data[2] - self.data[0];
        let bbox_height = self.data[3] - self.data[1];
        if bbox_width <= 0.0 || bbox_height <= 0.0 {
            return None;
        }
        let u = (dx + 0.5 - self.data[0]) / bbox_width;
        let v = (dy + 0.5 - self.data[1]) / bbox_height;
        let x0 = self.data[4];
        let y0 = self.data[5];
        let sx = (1.0 - u) * (1.0 - v) * x0
            + u * (1.0 - v) * self.data[10]
            + u * v * self.data[8]
            + (1.0 - u) * v * self.data[6];
        let sy = (1.0 - u) * (1.0 - v) * y0
            + u * (1.0 - v) * self.data[11]
            + u * v * self.data[9]
            + (1.0 - u) * v * self.data[7];
        (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    }

    #[inline]
    fn shader_source_at(&self, dx: f32, dy: f32) -> Option<(f32, f32)> {
        match self.method {
            TransformMethod::Perspective => self.shader_perspective_source(dx, dy),
            TransformMethod::Quad => self.shader_quad_source(dx, dy),
            TransformMethod::Mesh => self.shader_mesh_source(dx, dy),
            TransformMethod::Affine => None,
        }
    }

    #[inline]
    fn shader_coefficient(&self, index: usize) -> f32 {
        self.data[index] as f32
    }

    #[inline]
    fn shader_perspective_source(&self, dx: f32, dy: f32) -> Option<(f32, f32)> {
        let f = |index: usize| self.shader_coefficient(index);
        if matches!(
            self.filter,
            ResampleFilter::Bilinear | ResampleFilter::Bicubic
        ) && f(6) == 0.0
            && f(7) == 0.0
            && f(0) == 0.0
            && f(1) == 0.0
            && f(3) == 0.0
            && f(4) == 0.0
        {
            return Some((f(2) - 0.5, f(5) - 0.5));
        }
        if gpu_perspective_signed_unit_relocation_is_admitted(self.data) {
            let a = f(0);
            let b = f(1);
            let d = f(3);
            let e = f(4);
            let sx = if a == 1.0 {
                f(2) + dx
            } else if a == -1.0 {
                f(2) - dx - 1.0
            } else if b == 1.0 {
                f(2) + dy
            } else {
                f(2) - dy - 1.0
            };
            let sy = if d == 1.0 {
                f(5) + if b.abs() == 1.0 { dx } else { dy }
            } else if d == -1.0 {
                f(5) - if b.abs() == 1.0 { dx } else { dy } - 1.0
            } else if e == 1.0 {
                f(5) + dy
            } else {
                f(5) - dy - 1.0
            };
            return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
        }
        if f(6) == 0.0 && f(7) == 0.0 && matches!(self.filter, ResampleFilter::Nearest) {
            let center_x = dx + 0.5;
            let center_y = dy + 0.5;
            let sx = (f(0) * center_x + f(1) * center_y + f(2)).floor();
            let sy = (f(3) * center_x + f(4) * center_y + f(5)).floor();
            return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
        }
        let denominator = f(6) * dx + f(7) * dy + 1.0;
        if denominator == 0.0 || !denominator.is_finite() {
            return None;
        }
        let sx = (f(0) * dx + f(1) * dy + f(2)) / denominator;
        let sy = (f(3) * dx + f(4) * dy + f(5)) / denominator;
        (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    }

    #[inline]
    fn shader_quad_source(&self, dx: f32, dy: f32) -> Option<(f32, f32)> {
        let f = |index: usize| self.shader_coefficient(index);
        let x0 = f(0);
        let y0 = f(1);
        if x0 == f(2) && x0 == f(4) && x0 == f(6) && y0 == f(3) && y0 == f(5) && y0 == f(7) {
            // Geometry.c filters after subtracting 0.5. The shader expects
            // the already-shifted source coordinate for filtered sampling.
            if !matches!(self.filter, ResampleFilter::Nearest) {
                return Some((x0 - 0.5, y0 - 0.5));
            }
            return Some((x0, y0));
        }
        let width = self.output_dimensions.0 as f32;
        let height = self.output_dimensions.1 as f32;
        let u = dx / width;
        let v = dy / height;
        let sx = x0 + (f(6) - x0) * u + (f(2) - x0) * v + (f(4) - f(2) - f(6) + x0) * u * v;
        let sy = y0 + (f(7) - y0) * u + (f(3) - y0) * v + (f(5) - f(3) - f(7) + y0) * u * v;
        (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    }

    #[inline]
    fn shader_mesh_source(&self, dx: f32, dy: f32) -> Option<(f32, f32)> {
        let f = |index: usize| self.shader_coefficient(index);
        let bx0 = f(0);
        let by0 = f(1);
        let bx1 = f(2);
        let by1 = f(3);
        if dx < bx0 || dx >= bx1 || dy < by0 || dy >= by1 {
            return None;
        }
        let width = self.output_dimensions.0 as f32;
        let height = self.output_dimensions.1 as f32;
        let x0 = f(4);
        let y0 = f(5);
        if x0 == f(6) && x0 == f(8) && x0 == f(10) && y0 == f(7) && y0 == f(9) && y0 == f(11) {
            if !matches!(self.filter, ResampleFilter::Nearest) {
                return Some((x0 - 0.5, y0 - 0.5));
            }
            return Some((x0, y0));
        }
        let direct_relocation = bx0 == 0.0
            && by0 == 0.0
            && bx1 == width
            && by1 == height
            && f(6) == x0
            && f(7) == y0 + height
            && f(8) == x0 + width
            && f(9) == y0 + height
            && f(10) == x0 + width
            && f(11) == y0;
        let swapped_relocation = bx0 == 0.0
            && by0 == 0.0
            && bx1 == width
            && by1 == height
            && f(6) == x0 + height
            && f(7) == y0
            && f(8) == x0 + height
            && f(9) == y0 + width
            && f(10) == x0
            && f(11) == y0 + width;
        if direct_relocation {
            return Some((x0 + dx, y0 + dy));
        }
        if swapped_relocation {
            return Some((x0 + dy, y0 + dx));
        }
        let bw = (bx1 - bx0).max(1.0);
        let bh = (by1 - by0).max(1.0);
        let u = (dx - bx0) / bw;
        let v = (dy - by0) / bh;
        let sx = (1.0 - u) * (1.0 - v) * x0
            + u * (1.0 - v) * f(10)
            + u * v * f(8)
            + (1.0 - u) * v * f(6);
        let sy = (1.0 - u) * (1.0 - v) * y0
            + u * (1.0 - v) * f(11)
            + u * v * f(9)
            + (1.0 - u) * v * f(7);
        (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
    }
}
