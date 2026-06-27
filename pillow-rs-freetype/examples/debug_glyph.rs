use pillow_rs_freetype::Font;
use std::io::Write;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let font_path = format!("{}/tests/fixtures/input/fonts_autohint/DejaVuSans.ttf", dir);
    let data = std::fs::read(&font_path).unwrap();
    let mut stdout = std::io::stdout();

    eprintln!("loading font...");
    let font = Font::truetype(&data, 10.0, Default::default()).unwrap();

    eprintln!("metrics: {:?}", font.getmetrics());

    for ch in &['|', '!', '.'] {
        eprintln!("rendering '{}'...", ch);
        let mask = font.getmask(&ch.to_string()).unwrap();
        let bbox = font.getbbox(&ch.to_string());
        writeln!(stdout, "═══ U+{:04X} '{}' ═══ ════", *ch as u32, ch).ok();
        writeln!(stdout, "  bbox: {:?}  mask: {}×{}, {} nonzero", bbox, mask.width, mask.height,
                 mask.pixels.iter().filter(|&&b| b > 0).count()).ok();

        for y in 0..mask.height {
            let off = (y * mask.width) as usize;
            if off + mask.width as usize <= mask.pixels.len() {
                let row: Vec<String> = mask.pixels[off..off + mask.width as usize]
                    .iter()
                    .map(|b| format!("{:3}", b))
                    .collect();
                writeln!(stdout, "  {}", row.join(" ")).ok();
            }
        }
    }
}
