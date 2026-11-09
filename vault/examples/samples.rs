// Writes the Vault sample files to a folder: cargo run --example samples -- <dir>
fn main() {
    let dir = std::env::args().nth(1).expect("folder");
    sompio_vault::samples::write_sample_files(std::path::Path::new(&dir)).unwrap();
}
