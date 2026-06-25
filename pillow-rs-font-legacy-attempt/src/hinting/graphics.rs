//! Graphics state, zones, and vector math.

#![allow(missing_docs)]

pub const ON_CURVE: u8 = 0x01;
pub const TOUCH_X: u8 = 0x02;
pub const TOUCH_Y: u8 = 0x04;

#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct F26Dot6Vector {
    pub x: i32,
    pub y: i32,
}

impl F26Dot6Vector {
    pub fn new(x: i32, y: i32) -> Self {
        F26Dot6Vector { x, y }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct GraphicsState {
    pub rp0: u16,
    pub rp1: u16,
    pub rp2: u16,
    pub gep0: u16,
    pub gep1: u16,
    pub gep2: u16,
    pub dual_vector: F26Dot6Vector,
    pub proj_vector: F26Dot6Vector,
    pub free_vector: F26Dot6Vector,
    pub loop_count: i32,
    pub round_state: i32,
    pub compensation: [i32; 4],
    pub minimum_distance: i32,
    pub control_value_cut_in: i32,
    pub single_width_cut_in: i32,
    pub single_width_value: i32,
    pub delta_base: u16,
    pub delta_shift: u16,
    pub auto_flip: bool,
    pub instruct_control: u8,
    pub scan_control: bool,
    pub scan_type: i32,
}

impl Default for GraphicsState {
    fn default() -> Self {
        GraphicsState {
            rp0: 0,
            rp1: 0,
            rp2: 0,
            gep0: 0,
            gep1: 0,
            gep2: 0,
            dual_vector: F26Dot6Vector::new(64, 0),
            proj_vector: F26Dot6Vector::new(64, 0),
            free_vector: F26Dot6Vector::new(64, 0),
            loop_count: 1,
            round_state: 1,
            compensation: [0, 0, 0, 0],
            minimum_distance: 64,
            control_value_cut_in: 17,
            single_width_cut_in: 0,
            single_width_value: 0,
            delta_base: 9,
            delta_shift: 3,
            auto_flip: true,
            instruct_control: 0,
            scan_control: false,
            scan_type: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Zone {
    pub points: Vec<F26Dot6Vector>,
    pub org: Vec<F26Dot6Vector>,
    pub tags: Vec<u8>,
    pub contours: Vec<u16>,
    pub n_points: u16,
    pub n_contours: u16,
}

impl Zone {
    pub fn new() -> Self {
        Zone {
            points: Vec::new(),
            org: Vec::new(),
            tags: Vec::new(),
            contours: Vec::new(),
            n_points: 0,
            n_contours: 0,
        }
    }

    pub fn allocate_twilight(&mut self, n: u16) {
        self.points = vec![F26Dot6Vector::new(0, 0); n as usize];
        self.org = vec![F26Dot6Vector::new(0, 0); n as usize];
        self.tags = vec![0u8; n as usize];
        self.contours = Vec::new();
        self.n_points = n;
        self.n_contours = 0;
    }

    pub fn is_touched_x(&self, idx: usize) -> bool {
        idx < self.tags.len() && (self.tags[idx] & TOUCH_X) != 0
    }
    pub fn is_touched_y(&self, idx: usize) -> bool {
        idx < self.tags.len() && (self.tags[idx] & TOUCH_Y) != 0
    }
    pub fn on_curve(&self, idx: usize) -> bool {
        idx < self.tags.len() && (self.tags[idx] & ON_CURVE) != 0
    }

    /// Move a point by `distance` F26Dot6 units along the freedom vector,
    /// marking touched axes.  Matches FreeType `Direct_Move`.
    pub fn direct_move(&mut self, fv: &F26Dot6Vector, point: usize, distance: i32) {
        if fv.x != 0 {
            self.points[point].x += (fv.x * distance) >> 6;
            self.tags[point] |= TOUCH_X;
        }
        if fv.y != 0 {
            self.points[point].y += (fv.y * distance) >> 6;
            self.tags[point] |= TOUCH_Y;
        }
    }
}
