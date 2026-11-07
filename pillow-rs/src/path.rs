//! Rust-owned two-dimensional paths used by Pillow's ImagePath and Draw APIs.

/// A sequence of two-dimensional points.
///
/// The Python binding owns conversion from Python sequences and callbacks;
/// coordinate storage and geometric operations stay in the core.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImagePath {
    points: Vec<[f64; 2]>,
}

impl ImagePath {
    /// Creates a path from already converted point pairs.
    pub fn from_points(mut points: Vec<[f64; 2]>) -> Self {
        // Pillow keeps owned coordinate storage even for an empty path, so
        // its public `id` is not a dangling storage sentinel.
        if points.is_empty() && points.capacity() == 0 {
            points.reserve_exact(1);
        }
        Self { points }
    }

    /// Returns the number of points in the path.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns whether the path contains no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns a point by index.
    pub fn point(&self, index: usize) -> Option<[f64; 2]> {
        self.points.get(index).copied()
    }

    /// Replaces a point by index.
    pub fn set_point(&mut self, index: usize, point: [f64; 2]) -> bool {
        let Some(slot) = self.points.get_mut(index) else {
            return false;
        };
        *slot = point;
        true
    }

    /// Borrows the path's points for host-side conversion without copying.
    pub fn as_points(&self) -> &[[f64; 2]] {
        &self.points
    }

    /// Returns the smallest axis-aligned box containing every point.
    ///
    /// An empty path has a zero-sized box at the origin.
    pub fn bounding_box(&self) -> (f64, f64, f64, f64) {
        let Some(first) = self.points.first() else {
            return (0.0, 0.0, 0.0, 0.0);
        };
        let (mut x0, mut y0, mut x1, mut y1) = (first[0], first[1], first[0], first[1]);
        for point in self.points.iter().skip(1) {
            if point[0] < x0 {
                x0 = point[0];
            }
            if point[1] < y0 {
                y0 = point[1];
            }
            if point[0] > x1 {
                x1 = point[0];
            }
            if point[1] > y1 {
                y1 = point[1];
            }
        }
        (x0, y0, x1, y1)
    }

    /// Applies a six-value affine transform and optional horizontal wrapping.
    pub fn transform(&mut self, matrix: [f64; 6], wrap: f64) {
        let [a, b, c, d, e, f] = matrix;
        if b == 0.0 && d == 0.0 {
            for point in &mut self.points {
                point[0] = a * point[0] + c;
                point[1] = e * point[1] + f;
            }
        } else {
            for point in &mut self.points {
                let [x, y] = *point;
                point[0] = a * x + b * y + c;
                point[1] = d * x + e * y + f;
            }
        }
        if wrap != 0.0 {
            for point in &mut self.points {
                point[0] %= wrap;
            }
        }
    }

    /// Removes points closer than `distance` to their previous retained point.
    ///
    /// Returns the number of removed points. The first point is always kept.
    pub fn compact(&mut self, distance: f64) -> isize {
        if self.points.is_empty() {
            // Pillow 12.2's Path.compact starts its retained count at one even
            // for an empty path, exposing one zero point and returning -1.
            self.points.push([0.0, 0.0]);
            return -1;
        }
        if self.points.len() == 1 {
            return 0;
        }
        let mut retained = 1;
        for index in 1..self.points.len() {
            let previous = self.points[retained - 1];
            let current = self.points[index];
            let city_block = (previous[0] - current[0]).abs() + (previous[1] - current[1]).abs();
            if city_block >= distance {
                self.points[retained] = current;
                retained += 1;
            }
        }
        let removed = self.points.len() - retained;
        self.points.truncate(retained);
        removed as isize
    }
}
