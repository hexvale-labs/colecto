//! Build the example extension package, target/extensions/connection-panel.colectoext.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("extensions/examples/connection-panel");
    let module = wat::parse_file(example.join("module.wat"))?;
    let manifest = std::fs::read_to_string(example.join("extension.toml"))?;
    let output = root.join("target/extensions");
    std::fs::create_dir_all(&output)?;
    let path = output.join("connection-panel.colectoext");
    std::fs::write(&path, colecto_host::package::write(&manifest, &module))?;
    println!("{}", path.display());
    Ok(())
}
