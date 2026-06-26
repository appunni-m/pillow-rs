//! Compare raw raster output with the mask for the '|' glyph.

use pillow_rs_freetype::font::Font;
use pillow_rs_freetype::grays;
use pillow_rs_freetype::scaler;

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let font_path = format!(
        "{}/tests/fixtures/input/fonts_nohint/DejaVuSans.ttf",
        dir
    );
    let data = std::fs::read(&font_path).unwrap();
    let font = Font::truetype(&data, 10.0).unwrap();

    // Get the mask through the full pipeline
    let mask = font.getmask("|").unwrap();
    let bbox = font.getbbox("|");

    println!("=== Full pipeline (getmask) ===");
    println!("bbox: {:?}", bbox);
    println!("mask: {}x{}", mask.width, mask.height);
    println!("pixels:");
    for y in 0..mask.height {
        let off = (y * mask.width) as usize;
        let row = &mask.pixels[off..off + mask.width as usize];
        println!(
            "y{:2}: {}",
            y,
            row.iter().map(|b| format!("{:3}", b)).collect::<Vec<_>>().join(" ")
        );
    }

    // Compute sha256 of the mask
    let sha = {
        use sha2::{Sha256, Digest};
        let mut h = Sha256::new();
        h.update(&mask.pixels);
        h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>()
    };
    println!("\nmask sha256: {}", sha);
    println!("expected:    7e29fe874291b6404d8a94ba80560bd5bc074adb386d369cab6c4acc34dcf00f");

    // Now get the raw raster by manually replicating scaler + raster
    let gid = {
        // Parse tables
        use pillow_rs_freetype::tt;
        let table_dir = tt::parse_table_directory(&data).unwrap();
        let cmap_bytes = table_dir.find(&data, tt::tag(b"cmap")).unwrap();
        let cmap = tt::cmap::parse_cmap(cmap_bytes).unwrap();
        cmap.char_index(124).unwrap_or(0)
    };

    let parsed = {
        use pillow_rs_freetype::tt;
        let table_dir = tt::parse_table_directory(&data).unwrap();
        let head = tt::head::parse_head(table_dir.find(&data, tt::tag(b"head")).unwrap()).unwrap();
        let hhea = tt::hhea::parse_hhea(table_dir.find(&data, tt::tag(b"hhea")).unwrap()).unwrap();
        let maxp = tt::maxp::parse_maxp(table_dir.find(&data, tt::tag(b"maxp")).unwrap()).unwrap();
        let hmtx = tt::hmtx::parse_hmtx(
            table_dir.find(&data, tt::tag(b"hmtx")).unwrap(),
            hhea.num_hmetrics,
            maxp.num_glyphs,
        ).unwrap();
        let os2 = table_dir.find(&data, tt::tag(b"OS/2")).and_then(tt::os2::parse_os2);
        let loca_data = table_dir.find(&data, tt::tag(b"loca")).unwrap().to_vec();
        let glyf_data = table_dir.find(&data, tt::tag(b"glyf")).unwrap().to_vec();
        let cmap = tt::cmap::parse_cmap(table_dir.find(&data, tt::tag(b"cmap")).unwrap()).unwrap();

        pillow_rs_freetype::tables::FontData {
            cmap,
            head,
            hhea,
            hmtx,
            maxp,
            name: tt::name::NameTable { family: "DejaVuSans".into(), subfamily: "Book".into() },
            os2,
            loca_data,
            glyf_data,
            size_pt: 10.0,
        }
    };

    let scaled = scaler::scale_glyph(&parsed, gid, None).unwrap();
    let raster = grays::rasterize(scaled.outline).unwrap();

    println!("\n=== Raw raster ({}x{}) ===", raster.width, raster.height);
    for y in 0..raster.height {
        let off = y * raster.width;
        let row = &raster.pixels[off..off + raster.width];
        println!(
            "y{:2}: {}",
            y,
            row.iter().map(|b| format!("{:3}", b)).collect::<Vec<_>>().join(" ")
        );
    }
}
