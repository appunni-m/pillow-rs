use super::convert_lab_impl::convert_rgb_bytes;

#[test]
fn matches_pillow_lab_byte_examples() {
    let colors = [
        ([0, 0, 0], [0, 0, 0]),
        ([255, 255, 255], [255, 0, 0]),
        ([128, 128, 128], [137, 0, 0]),
        ([255, 0, 0], [138, 81, 70]),
        ([0, 255, 0], [224, 177, 81]),
        ([0, 0, 255], [75, 68, 144]),
        ([12, 34, 56], [32, 254, 239]),
        ([255, 0, 255], [153, 94, 195]),
        ([0, 255, 255], [231, 205, 241]),
        ([255, 255, 0], [249, 240, 93]),
    ];
    for (source, expected) in colors {
        assert_eq!(convert_rgb_bytes(&source, 1).unwrap(), expected);
    }
}
