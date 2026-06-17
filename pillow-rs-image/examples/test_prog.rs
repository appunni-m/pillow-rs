use std::fs;
fn main() {
    let data = fs::read("tests/fixtures/input/images/jpeg/progressive.jpg").unwrap();
    eprintln!("RESULT: {:?}", pillow_rs_image::decode(&data).is_some());
}
