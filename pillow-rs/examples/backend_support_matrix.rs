//! Emit the compute registry's declared backend support as deterministic JSON.
#![allow(unused_crate_dependencies)]

fn main() {
    println!(
        "{}",
        pillow_rs::backend_support_matrix_json().expect("backend support matrix builds")
    );
}
