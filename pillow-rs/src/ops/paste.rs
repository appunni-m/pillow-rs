//! Paste operations — image overlay, color fill, and mask-based alpha blending.

mod color;
mod context_execution;
mod input;
mod operations;
mod source;
mod source_plan;
mod types;

pub use operations::PasteBoxPlan;
pub use source_plan::PasteSourcePlan;
pub use types::{
    AlphaCompositeBox, AlphaCompositeCropPlan, PasteBoxInput, PasteColorComponent, PasteMaskInput,
    PasteSource, PasteSourceInput,
};
