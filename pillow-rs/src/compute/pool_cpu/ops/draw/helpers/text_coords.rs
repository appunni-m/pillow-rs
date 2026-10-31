/// Return the native-image coordinate for a text-mask pixel when it is visible.
#[inline]
pub(crate) fn destination(
    x: i32,
    y: i32,
    text_x: u32,
    text_y: u32,
    image_width: u32,
    image_height: u32,
) -> Option<(u32, u32)> {
    let image_x = i64::from(x) + i64::from(text_x);
    let image_y = i64::from(y) + i64::from(text_y);
    if image_x < 0
        || image_y < 0
        || image_x >= i64::from(image_width)
        || image_y >= i64::from(image_height)
    {
        return None;
    }
    Some((image_x as u32, image_y as u32))
}
