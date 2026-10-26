use super::JsValue;
use crate::{js_float_array, js_integer, js_integer_array, js_value_type_name};
pub(crate) fn js_reduce_factor(value: &JsValue) -> pillow_rs::ReduceFactor {
    if let Some(value) = js_integer(value) {
        return pillow_rs::ReduceFactor::Scalar(value);
    }
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::ReduceFactor::Sequence(values);
    }
    if let Some(values) = js_float_array(value) {
        return pillow_rs::ReduceFactor::FloatingSequence(values);
    }
    pillow_rs::ReduceFactor::Invalid(js_value_type_name(value))
}

pub(crate) fn js_reduce_box(value: &JsValue) -> pillow_rs::ReduceBox {
    if let Some(values) = js_integer_array(value) {
        return pillow_rs::ReduceBox::Sequence(values);
    }
    pillow_rs::ReduceBox::InvalidType(js_value_type_name(value))
}
