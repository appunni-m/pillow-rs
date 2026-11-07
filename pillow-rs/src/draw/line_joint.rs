//! Geometry shared by native and Python-facing thick-line joint routes.

/// Geometry for one Pillow-style curved line joint.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct DrawCurveJointPlan {
    angles: [f64; 2],
    flipped: bool,
}

impl DrawCurveJointPlan {
    pub(crate) fn from_points(
        start: (f64, f64),
        center: (f64, f64),
        end: (f64, f64),
    ) -> Option<Self> {
        let angles = [
            (center.0 - start.0)
                .atan2(start.1 - center.1)
                .to_degrees()
                .rem_euclid(360.0),
            (end.0 - center.0)
                .atan2(center.1 - end.1)
                .to_degrees()
                .rem_euclid(360.0),
        ];
        Self::from_angles(angles)
    }

    pub(crate) fn from_angles(angles: [f64; 2]) -> Option<Self> {
        if angles[0] == angles[1] {
            return None;
        }
        let flipped = (angles[1] > angles[0] && angles[1] - 180.0 > angles[0])
            || (angles[1] < angles[0] && angles[1] + 180.0 > angles[0]);
        Some(Self { angles, flipped })
    }

    /// Returns Pillow pieslice bounds and angles for this joint.
    pub fn pieslice(&self, center: (f64, f64), width: u32) -> ([(f64, f64); 2], f64, f64) {
        let width = f64::from(width);
        let bounds = [
            (
                center.0 - width / 2.0 + 1.0,
                center.1 - width / 2.0 + 1.0,
            ),
            (
                center.0 + width / 2.0 - 1.0,
                center.1 + width / 2.0 - 1.0,
            ),
        ];
        let (start, end) = self.angles();
        (bounds, start, end)
    }

    /// Returns Pillow pieslice start and end angles for this joint.
    pub fn angles(&self) -> (f64, f64) {
        let (start, end) = if self.flipped {
            (self.angles[1] + 90.0, self.angles[0] + 90.0)
        } else {
            (self.angles[0] - 90.0, self.angles[1] - 90.0)
        };
        (start - 90.0, end - 90.0)
    }

    /// Returns the integer offsets for the two optional gap-cover points.
    pub fn gap_offsets(&self, distance: f64) -> [(i32, i32); 2] {
        [
            self.gap_offset(true, distance),
            self.gap_offset(false, distance),
        ]
    }

    /// Returns the integer offset for one optional gap-cover point.
    pub fn gap_offset(&self, first: bool, distance: f64) -> (i32, i32) {
        let angle = (self.gap_angle(first) - 90.0).to_radians();
        let dx = distance * angle.cos();
        let dy = distance * angle.sin();
        (pillow_joint_offset(dx), pillow_joint_offset(dy))
    }

    /// Returns the angle passed to Pillow's gap-coordinate calculation.
    pub fn gap_angle(&self, first: bool) -> f64 {
        match (self.flipped, first) {
            (true, true) => self.angles[0] + 90.0,
            (true, false) => self.angles[1] + 90.0,
            (false, true) => self.angles[0] - 90.0,
            (false, false) => self.angles[1] - 90.0,
        }
    }
}

fn pillow_joint_offset(delta: f64) -> i32 {
    if delta > 0.0 {
        delta.floor() as i32
    } else {
        delta.ceil() as i32
    }
}
