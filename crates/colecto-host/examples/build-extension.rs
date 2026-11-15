//! Package the example extensions in docs/extensions/examples (hello-pane and Oneko) as
//! target/extensions/<id>.colectoext.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/extensions");
    std::fs::create_dir_all(&output)?;
    for name in ["hello-pane", "oneko"] {
        let example = root.join("docs/extensions/examples").join(name);
        let mut package = colecto_host::package::Package {
            manifest: std::fs::read_to_string(example.join("manifest.toml"))?,
            ..Default::default()
        };
        for entry in std::fs::read_dir(example.join("client"))? {
            let entry = entry?;
            let path = format!("client/{}", entry.file_name().to_string_lossy());
            package.client.insert(path, std::fs::read(entry.path())?);
        }
        if let Ok(entries) = std::fs::read_dir(example.join("assets")) {
            for entry in entries {
                let entry = entry?;
                package.assets.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read(entry.path())?,
                );
            }
        }
        let path = output.join(format!("{name}.colectoext"));
        std::fs::write(&path, colecto_host::package::write_deflated(&package))?;
        println!("{}", path.display());
    }
    Ok(())
}
