//! Shared native-byte vector kernels for the Draw architecture strategies.

mod admission;
mod arithmetic;
mod execute;
mod kernels;
mod layout;
mod strategy;

use admission::{has_visible_draw_point, has_visible_draw_rectangle, valid_draw_rectangle};
use layout::DrawLayout;
pub(crate) use strategy::{execute as vector_execute, is_eligible};
