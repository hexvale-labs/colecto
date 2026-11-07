fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let module = wat::parse_file(root.join("extensions/examples/connection-panel/module.wat"))?;
    let output = root.join("target/extensions");
    std::fs::create_dir_all(&output)?;
    std::fs::write(output.join("connection-panel.wasm"), module)?;
    println!("{}", output.join("connection-panel.wasm").display());
    Ok(())
}
