use std::fs;

fn main() {
    let files = [
        ("422", "baseline_422.jpg"),
        ("420", "baseline_420.jpg"),
        ("q100", "q100.jpg"),
    ];
    for (label, fname) in &files {
        let path = format!("/home/appunni/work/pil-wasm/pillow-rs-image/test-assets/input/jpeg/{fname}");
        let data = fs::read(&path).unwrap();
        let decoded = pillow_rs_image::decode(&data);
        match decoded {
            Some(img) => {
                let bytes = &img.pixels;
                let ref_label = fname.replace(".jpg", "");
                let ref_path = format!("/home/appunni/work/pil-wasm/pillow-rs-image/tests/fixtures/outputs/raws/Decode.jpeg_{ref_label}_jpg.bin");
                if let Ok(ref_data) = fs::read(&ref_path) {
                    if ref_data.len() == bytes.len() {
                        let diffs: usize = bytes.iter().zip(ref_data.iter()).filter(|(a,b)| a!=b).count();
                        println!("{label}: {diffs}/{} bytes differ", bytes.len());
                        // Show all pixel differences for first row
                        println!("  First row pixel diffs (x, our_RGB, exp_RGB, our_YCbCr, exp_YCbCr):");
                        let w = img.width as usize;
                        for x in 0..w.min(16) {
                            let or = bytes[x*3] as i32;
                            let og = bytes[x*3+1] as i32;
                            let ob = bytes[x*3+2] as i32;
                            let er = ref_data[x*3] as i32;
                            let eg = ref_data[x*3+1] as i32;
                            let eb = ref_data[x*3+2] as i32;
                            if or != er || og != eg || ob != eb {
                                // Convert to YCbCr for comparison
                                let oy = (0.299*or as f64 + 0.587*og as f64 + 0.114*ob as f64).round() as i32;
                                let ocb = (128.0 - 0.169*or as f64 - 0.331*og as f64 + 0.500*ob as f64).round() as i32;
                                let ocr = (128.0 + 0.500*or as f64 - 0.419*og as f64 - 0.081*ob as f64).round() as i32;
                                let ey = (0.299*er as f64 + 0.587*eg as f64 + 0.114*eb as f64).round() as i32;
                                let ecb = (128.0 - 0.169*er as f64 - 0.331*eg as f64 + 0.500*eb as f64).round() as i32;
                                let ecr = (128.0 + 0.500*er as f64 - 0.419*eg as f64 - 0.081*eb as f64).round() as i32;
                                println!("    [{x}] our=({or},{og},{ob}) exp=({er},{eg},{eb})  YCbCr=({oy},{ocb},{ocr}) vs ({ey},{ecb},{ecr})");
                            }
                        }
                    }
                }
            }
            None => println!("{label}: decompress returned None"),
        }
    }
}
