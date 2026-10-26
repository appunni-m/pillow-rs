use super::{TransposeMethod, TransposeTransform};

fn forward(method: &TransposeMethod, width: u32, height: u32, x: u32, y: u32) -> (u32, u32) {
    match method {
        TransposeMethod::FlipLeftRight => (width - 1 - x, y),
        TransposeMethod::FlipTopBottom => (x, height - 1 - y),
        TransposeMethod::Rotate90 => (y, width - 1 - x),
        TransposeMethod::Rotate180 => (width - 1 - x, height - 1 - y),
        TransposeMethod::Rotate270 => (height - 1 - y, x),
        TransposeMethod::Transpose => (y, x),
        TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
    }
}

fn dimensions(method: &TransposeMethod, width: u32, height: u32) -> (u32, u32) {
    match method {
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => (height, width),
        _ => (width, height),
    }
}

fn assert_composition(methods: &[&TransposeMethod], width: u32, height: u32) {
    let composed = methods
        .iter()
        .fold(TransposeTransform::Identity, |state, next| state.then(next));
    let expected_dimensions = methods
        .iter()
        .fold((width, height), |(w, h), method| dimensions(method, w, h));
    let actual_dimensions = match &composed {
        TransposeTransform::Identity => (width, height),
        TransposeTransform::Method(method) => dimensions(method, width, height),
    };
    assert_eq!(actual_dimensions, expected_dimensions, "{methods:?}");
    for y in 0..height {
        for x in 0..width {
            let (mut px, mut py) = (x, y);
            let (mut w, mut h) = (width, height);
            for method in methods {
                (px, py) = forward(method, w, h, px, py);
                (w, h) = dimensions(method, w, h);
            }
            let actual = match &composed {
                TransposeTransform::Identity => (x, y),
                TransposeTransform::Method(method) => forward(method, width, height, x, y),
            };
            assert_eq!(
                actual,
                (px, py),
                "{methods:?} at ({x},{y}) on {width}x{height}"
            );
        }
    }
}

#[test]
fn transpose_d4_all_pairs_and_triples_match_forward_coordinates() {
    let methods = [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ];
    for (width, height) in [(0, 0), (0, 7), (7, 0), (1, 1), (1, 7), (9, 1), (13, 19)] {
        for first in &methods {
            assert_composition(&[first], width, height);
            for second in &methods {
                assert_composition(&[first, second], width, height);
                for third in &methods {
                    assert_composition(&[first, second, third], width, height);
                }
            }
        }
    }
}
