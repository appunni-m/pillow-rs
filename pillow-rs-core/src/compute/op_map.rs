use crate::compute::registry::{self, OpDef, OpId};

macro_rules! register_gpu_ops {
    ($($variant:ident => $shader:literal, $inputs:literal, $has_params:literal $(, $passes:literal)?),* $(,)?) => {
        pub fn init() {
            let mut defs = Vec::new();
            $(defs.push(OpDef {
                id: OpId::$variant,
                variant_name: stringify!($variant),
                shader_source: include_str!(concat!("gpu_shaders/", $shader)),
                input_count: $inputs,
                has_params: $has_params,
                is_multi_pass: register_gpu_ops!(@multi $($passes)?),
                pass_count: register_gpu_ops!(@passes $($passes)?),
            });)*
            registry::build_registry(defs);
        }
    };
    (@multi) => { false };
    (@multi $p:literal) => { $p > 1 };
    (@passes) => { 1 };
    (@passes $p:literal) => { $p };
}

register_gpu_ops! {
    Invert => "invert.wgsl", 1, false,
    Grayscale => "grayscale.wgsl", 1, false,
    Solarize => "solarize.wgsl", 1, true,
    Posterize => "posterize.wgsl", 1, true,
    Brightness => "brightness.wgsl", 1, true,
    Contrast => "contrast.wgsl", 1, true,
    ColorSaturation => "color_saturation.wgsl", 1, true,
    Colorize => "colorize.wgsl", 1, true,
    Constant => "constant.wgsl", 1, true,
    Offset => "offset.wgsl", 1, true,
}
