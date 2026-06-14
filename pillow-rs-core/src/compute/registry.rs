use crate::pipeline::PipelineOp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum OpId {
    Invert = 0,
    Grayscale = 1,
    Solarize = 2,
    Posterize = 3,
    Brightness = 4,
    Contrast = 5,
    ColorSaturation = 6,
    Colorize = 7,
    Constant = 8,
    Offset = 9,
}

#[derive(Debug, Clone)]
pub struct OpDef {
    pub id: OpId,
    pub variant_name: &'static str,
    pub shader_source: &'static str,
    pub input_count: u8,
    pub has_params: bool,
    pub is_multi_pass: bool,
    pub pass_count: u8,
}

static OP_REGISTRY: std::sync::OnceLock<Vec<OpDef>> = std::sync::OnceLock::new();

pub fn get_registry() -> &'static [OpDef] {
    OP_REGISTRY.get_or_init(|| Vec::new())
}

pub fn build_registry(defs: Vec<OpDef>) {
    let _ = OP_REGISTRY.set(defs);
}

pub fn op_id(op: &PipelineOp) -> Option<OpId> {
    match op {
        PipelineOp::Invert => Some(OpId::Invert),
        PipelineOp::Grayscale => Some(OpId::Grayscale),
        PipelineOp::Solarize { .. } => Some(OpId::Solarize),
        PipelineOp::Posterize { .. } => Some(OpId::Posterize),
        PipelineOp::Brightness { .. } => Some(OpId::Brightness),
        PipelineOp::Contrast { .. } => Some(OpId::Contrast),
        PipelineOp::ColorSaturation { .. } => Some(OpId::ColorSaturation),
        PipelineOp::Colorize { .. } => Some(OpId::Colorize),
        PipelineOp::Constant { .. } => Some(OpId::Constant),
        PipelineOp::Offset { .. } => Some(OpId::Offset),
        _ => None,
    }
}

pub fn extract_params(op: &PipelineOp) -> Vec<u32> {
    match op {
        PipelineOp::Invert | PipelineOp::Grayscale => vec![],
        PipelineOp::Solarize { threshold } => vec![*threshold as u32],
        PipelineOp::Posterize { bits } => vec![*bits as u32],
        PipelineOp::Brightness { factor } => vec![(*factor * 1000.0) as u32],
        PipelineOp::Contrast { factor } => vec![(*factor * 1000.0) as u32],
        PipelineOp::ColorSaturation { factor } => vec![(*factor * 1000.0) as u32],
        PipelineOp::Colorize { black, white } => vec![
            (black.0 as u32) << 24 | (black.1 as u32) << 16 | (black.2 as u32) << 8,
            (white.0 as u32) << 24 | (white.1 as u32) << 16 | (white.2 as u32) << 8,
        ],
        PipelineOp::Constant { value } => vec![*value as u32],
        PipelineOp::Offset { x, y } => vec![*x as u32, *y as u32],
        _ => vec![],
    }
}

#[allow(dead_code)]
pub fn resize_dims(op: &PipelineOp) -> Option<(u32, u32)> {
    match op {
        PipelineOp::Resize { w, h, .. } => Some((*w, *h)),
        _ => None,
    }
}
