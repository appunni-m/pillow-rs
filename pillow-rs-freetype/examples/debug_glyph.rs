use pillow_rs_freetype::Font;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/tests/fixtures/input/fonts_nohint/DejaVuSans.ttf", dir);
    let data = std::fs::read(&path).unwrap();
    let font = Font::truetype(&data[..], 10.0).unwrap();
    println!("metrics: {:?}", font.getmetrics());
    println!("bbox '!': {:?}", font.getbbox("!"));
    let mask = font.getmask("!").unwrap();
    println!(
        "mask: {}x{}, {} nonzero",
        mask.width,
        mask.height,
        mask.pixels.iter().filter(|&&b| b > 0).count()
    );
    for y in 0..mask.height.min(10) {
        let off = (y * mask.width) as usize;
        let row: Vec<String> = mask.pixels[off..off + mask.width as usize]
            .iter()
            .map(|b| format!("{:3}", b))
            .collect();
        println!("  {}", row.join(" "));
    }
}
