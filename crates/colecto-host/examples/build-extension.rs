//! Build the example extension package, target/extensions/hello-pane.colectoext.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = root.join("docs/extensions/examples/hello-pane");
    let mut package = colecto_host::package::Package {
        manifest: std::fs::read_to_string(example.join("manifest.toml"))?,
        ..Default::default()
    };
    package.client.insert(
        "client/main.js".into(),
        std::fs::read(example.join("client/main.js"))?,
    );
    let output = root.join("target/extensions");
    std::fs::create_dir_all(&output)?;
    let path = output.join("hello-pane.colectoext");
    std::fs::write(&path, colecto_host::package::write(&package))?;
    println!("{}", path.display());
    Ok(())
}
