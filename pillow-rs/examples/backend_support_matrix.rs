//! Emit the compute registry's declared backend support as deterministic JSON.
#![allow(unused_crate_dependencies)]

fn main() -> Result<(), pillow_rs::PilError> {
    println!("{}", pillow_rs::backend_support_matrix_json()?);
    Ok(())
}
