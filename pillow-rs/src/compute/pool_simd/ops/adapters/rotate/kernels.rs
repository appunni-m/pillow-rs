//! Rotate-family native kernel entry points.

mod nearest;

pub(crate) use self::nearest::copy_contiguous_rgb_runs;
