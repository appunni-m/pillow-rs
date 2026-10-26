use super::{PilError, PreparedColor3DLut, color3dlut_transform_table};

fn fixture() -> PreparedColor3DLut {
    PreparedColor3DLut {
        size: (2, 2, 2),
        table: (0..24).map(f64::from).collect(),
        channels: 3,
    }
}

#[test]
fn exact_width_results_preserve_the_preallocated_output_layout() {
    let input = fixture();
    let (actual, channels) = color3dlut_transform_table(
        &input,
        None,
        false,
        |values| Ok::<_, PilError>(values.to_vec()),
        |error| error,
    )
    .unwrap();

    assert_eq!(channels, 3);
    assert_eq!(actual, input.table);
}

#[test]
fn with_normals_keeps_pillow_cube_traversal_order() {
    let input = fixture();
    let (actual, channels) = color3dlut_transform_table(
        &input,
        None,
        true,
        |args| {
            assert_eq!(args.len(), 6);
            Ok::<_, PilError>(args[..3].to_vec())
        },
        |error| error,
    )
    .unwrap();

    let mut expected = Vec::with_capacity(24);
    for b in 0..2 {
        for g in 0..2 {
            for r in 0..2 {
                expected.extend([r as f64, g as f64, b as f64]);
            }
        }
    }
    assert_eq!(channels, 3);
    assert_eq!(actual, expected);
}

#[test]
fn wrong_callback_width_retains_pillow_slice_resize_behavior() {
    let input = fixture();
    let (actual, channels) = color3dlut_transform_table(
        &input,
        None,
        false,
        |values| Ok::<_, PilError>(values[..2].to_vec()),
        |error| error,
    )
    .unwrap();

    let mut expected = vec![0.0; 24];
    for entry in 0..8 {
        let start = (entry * 3).min(expected.len());
        let end = (entry * 3 + 3).min(expected.len());
        expected.splice(
            start..end,
            [input.table[entry * 3], input.table[entry * 3 + 1]],
        );
    }
    assert_eq!(channels, 3);
    assert_eq!(actual, expected);
}
