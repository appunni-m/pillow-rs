use pillow_rs_freetype::{BitmapBackend, Font};
use sha2::{Digest, Sha256};
use serde_json::json;
use std::fs;
use std::path::Path;

fn sha256_hex(d: &[u8]) -> String { Sha256::digest(d).iter().map(|b| format!("{:02x}",b)).collect() }

fn main() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fd = base.join("tests/fixtures/input/fonts_autohint");
    let fonts = [
        ("DejaVuSans","DejaVuSans.ttf","DejaVu Sans","Book"),
        ("LiberationSerif","LiberationSerif-Regular.ttf","Liberation Serif","Regular"),
    ];
    let sizes = [10f32,12.,16.,20.,24.];
    let chars: Vec<char> = (33..127).map(|c| char::from_u32(c).unwrap()).collect();
    let mut pil_rows: Vec<serde_json::Value> = Vec::new();
    let mut ft_rows: Vec<serde_json::Value> = Vec::new();
    for (fk,ff,fam,sty) in &fonts {
        let fb = fs::read(fd.join(ff)).unwrap();
        for &s in &sizes {
            let pf = Font::truetype(&fb,s,BitmapBackend::PIL).unwrap();
            let ff_ = Font::truetype(&fb,s,BitmapBackend::FreeType).unwrap();
            let (a,d) = pf.getmetrics();
            let gl = pf.getlength("Hello");
            for rows in [&mut pil_rows, &mut ft_rows] {
                rows.push(json!({"id":format!("{}_{}_getmetrics",fk,s as i32),"font":fk,"size_pt":s,"codepoint":0,"char":"","operation":"getmetrics","status":"active","ref_value":[a,d]}));
                rows.push(json!({"id":format!("{}_{}_getname",fk,s as i32),"font":fk,"size_pt":s,"codepoint":0,"char":"","operation":"getname","status":"active","ref_value":[fam,sty]}));
                rows.push(json!({"id":format!("{}_{}_getlength_hello",fk,s as i32),"font":fk,"size_pt":s,"codepoint":0,"char":"Hello","operation":"getlength","status":"active","ref_value":gl}));
            }
            for &c in &chars {
                let cs = c.to_string();
                let cp = c as u32;
                let pm = pf.getmask(&cs).unwrap();
                let fm_ = ff_.getmask(&cs).unwrap();
                let pb = pf.getbbox(&cs);
                let fb_ = ff_.getbbox(&cs);
                pil_rows.push(json!({"id":format!("{}_{}_{}_getmask",fk,s as i32,cp),"font":fk,"size_pt":s,"codepoint":cp,"char":cs,"operation":"getmask","status":"active","ref_sha256":sha256_hex(&pm.pixels),"ref_size":[pm.width,pm.height]}));
                pil_rows.push(json!({"id":format!("{}_{}_{}_getbbox",fk,s as i32,cp),"font":fk,"size_pt":s,"codepoint":cp,"char":cs,"operation":"getbbox","status":"active","ref_value":[pb.0,pb.1,pb.2,pb.3]}));
                ft_rows.push(json!({"id":format!("{}_{}_{}_getmask",fk,s as i32,cp),"font":fk,"size_pt":s,"codepoint":cp,"char":cs,"operation":"getmask","status":"active","ref_sha256":sha256_hex(&fm_.pixels),"ref_size":[fm_.width,fm_.height]}));
                ft_rows.push(json!({"id":format!("{}_{}_{}_getbbox",fk,s as i32,cp),"font":fk,"size_pt":s,"codepoint":cp,"char":cs,"operation":"getbbox","status":"active","ref_value":[fb_.0,fb_.1,fb_.2,fb_.3]}));
            }
        }
    }
    let pil_m = json!({"version":"0.1.0","font_source":"fonts_autohint","generator":"gen_refs","mode":"PIL","rows":pil_rows,"summary":{"total_rows":pil_rows.len(),"active_rows":pil_rows.len(),"fonts":2,"sizes":5,"glyphs":94}});
    let ft_m = json!({"version":"0.1.0","font_source":"fonts_autohint","generator":"gen_refs","mode":"FreeType-raw","rows":ft_rows,"summary":{"total_rows":ft_rows.len(),"active_rows":ft_rows.len(),"fonts":2,"sizes":5,"glyphs":94}});
    fs::write(base.join("tests/fixtures/coverage_matrix.json"),serde_json::to_string_pretty(&pil_m).unwrap()+"\n").unwrap();
    fs::write(base.join("tests/fixtures/coverage_matrix_ft.json"),serde_json::to_string_pretty(&ft_m).unwrap()+"\n").unwrap();
    eprintln!("gen_refs: {} PIL + {} FT rows", pil_rows.len(), ft_rows.len());
}
