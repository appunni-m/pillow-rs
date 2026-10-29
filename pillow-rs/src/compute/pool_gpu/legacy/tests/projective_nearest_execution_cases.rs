use super::TransformData;

type ByteNearestTransformCase = ((u32, u32), (u32, u32), [f64; 8], u8);
type ByteQuadMeshTransformCase = (i32, TransformData, (u32, u32));

pub(super) fn byte_projective_nearest_byte_modes() -> [(&'static str, usize); 4] {
    [
        ("L", 1usize),
        ("LA", 2usize),
        ("RGB", 3usize),
        ("RGBA", 4usize),
    ]
}

pub(super) fn byte_projective_nearest_palette_modes() -> [(&'static str, usize); 5] {
    [
        ("L", 1usize),
        ("LA", 2usize),
        ("RGB", 3usize),
        ("RGBA", 4usize),
        ("P", 1usize),
    ]
}

fn byte_projective_nearest_affine_cases() -> Vec<(i32, TransformData)> {
    vec![
        (
            2,
            TransformData::Affine(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![1.0, 0.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![-1.0, 0.0, 8.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![1.0, 0.0, 0.0, 0.0, -1.0, 8.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![-1.0, 0.0, 8.0, 0.0, -1.0, 8.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![0.0, -1.0, 8.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![0.0, 1.0, 0.0, -1.0, 0.0, 8.0, 0.0, 0.0]),
        ),
        (
            2,
            TransformData::Affine(vec![0.0, -1.0, 8.0, -1.0, 0.0, 8.0, 0.0, 0.0]),
        ),
        (
            3,
            TransformData::Affine(vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0]),
        ),
        (
            3,
            TransformData::Affine(vec![0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0]),
        ),
        (
            3,
            TransformData::Affine(vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0]),
        ),
        (
            3,
            TransformData::Affine(vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0]),
        ),
    ]
}

fn byte_projective_nearest_mesh_cases() -> Vec<(i32, TransformData)> {
    vec![
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 8.0, 8.0],
                vec![0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0],
            )]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 8.0, 8.0],
                vec![1.0, 1.0, 1.0, 9.0, 9.0, 9.0, 9.0, 1.0],
            )]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 8.0, 8.0],
                vec![-1.0, -1.0, -1.0, 7.0, 7.0, 7.0, 7.0, -1.0],
            )]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 8.0, 8.0],
                vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0],
            )]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 8.0, 8.0],
                vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
            )]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 8.0, 8.0],
                vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0],
            )]),
        ),
    ]
}

pub(super) fn byte_projective_nearest_proven_cases() -> Vec<(i32, TransformData)> {
    let mut cases = byte_projective_nearest_affine_cases();
    cases.extend(byte_projective_nearest_mesh_cases());
    cases
}

pub(super) fn byte_projective_integer_affine_cases() -> [ByteNearestTransformCase; 4] {
    [
        (
            (9u32, 7u32),
            (4u32, 4u32),
            [2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0],
            251u8,
        ),
        (
            (9u32, 7u32),
            (7u32, 5u32),
            [-2.0, 1.0, 8.0, 1.0, 2.0, -3.0, 0.0, 0.0],
            251u8,
        ),
        (
            (9u32, 7u32),
            (6u32, 5u32),
            [1.0, 2.0, -3.0, 2.0, -1.0, 6.0, 0.0, 0.0],
            17u8,
        ),
        (
            (13u32, 11u32),
            (9u32, 7u32),
            [3.0, -1.0, 5.0, 1.0, 2.0, -4.0, 0.0, 0.0],
            199u8,
        ),
    ]
}

pub(super) fn byte_projective_fractional_cases() -> [ByteNearestTransformCase; 4] {
    [
        (
            (9u32, 8u32),
            (8u32, 7u32),
            [1.0, 0.0, 0.25, 0.0, 1.0, 0.25, 0.0, 0.0],
            251u8,
        ),
        (
            (16u32, 16u32),
            (13u32, 11u32),
            [0.5, 0.25, 0.125, -0.25, 0.5, 0.375, 0.0, 0.0],
            17u8,
        ),
        (
            (5u32, 7u32),
            (9u32, 6u32),
            [0.75, -0.125, 0.25, 0.125, 0.625, 0.375, 0.0, 0.0],
            199u8,
        ),
        (
            (32u32, 32u32),
            (16u32, 16u32),
            [1.0, 0.125, 0.25, -0.125, 1.0, 0.375, 0.0, 0.0],
            61u8,
        ),
    ]
}

pub(super) fn byte_projective_nonconstant_denominator_cases() -> [ByteNearestTransformCase; 3] {
    [
        (
            (9u32, 8u32),
            (8u32, 7u32),
            [1.0, 0.0, 8.0, 0.0, 1.0, 4.0, -0.125, 0.015625],
            251u8,
        ),
        (
            (16u32, 16u32),
            (8u32, 7u32),
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0 / 1024.0, 1.0 / 1024.0],
            17u8,
        ),
        (
            (16u32, 16u32),
            (8u32, 7u32),
            [1.0, 0.0, -2.0, 0.0, 1.0, -1.0, 1.0 / 32.0, -1.0 / 32.0],
            83u8,
        ),
    ]
}

pub(super) fn byte_projective_quad_mesh_cases() -> [ByteQuadMeshTransformCase; 8] {
    [
        (
            3,
            TransformData::Affine(vec![0.0, 0.0, 9.0, 0.0, 9.0, 6.0, 0.0, 6.0]),
            (9, 6),
        ),
        (
            3,
            TransformData::Affine(vec![1.25, 1.0, 1.25, 7.0, 10.25, 7.0, 10.0, 1.0]),
            (9, 6),
        ),
        (
            3,
            TransformData::Affine(vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0]),
            (9, 6),
        ),
        (
            3,
            TransformData::Affine(vec![2.0, 1.0, 2.0, 7.0, 8.0, 7.0, 8.0, 1.0]),
            (9, 6),
        ),
        (
            3,
            TransformData::Affine(vec![2.0, 1.0, 8.0, 1.0, 8.0, 10.0, 2.0, 10.0]),
            (9, 6),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0],
            )]),
            (9, 6),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![1.25, 1.0, 1.25, 7.0, 10.25, 7.0, 10.0, 1.0],
            )]),
            (9, 6),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0],
            )]),
            (9, 6),
        ),
    ]
}
