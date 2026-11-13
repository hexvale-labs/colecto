//! `cargo colecto`: builds a Rust extension for `wasm32-unknown-unknown`, checks the
//! module against `extension.toml` and writes the `.colectoext` package.
//! See `docs/extensions/rust-sdk.md`.

mod check;
mod wasm;

use colecto_package::{Assets, manifest};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
Build, check and package a Rust Colecto extension.

Usage: cargo colecto <COMMAND> [OPTIONS]

Commands:
  build   Build for wasm32-unknown-unknown, check against extension.toml, write the .colectoext
  check   Build and check, without writing a package

Options:
  --manifest-path <PATH>  The extension's Cargo.toml (default: the package in this directory)
  --out <PATH>            Where to write the package (default: <target-dir>/colecto/<id>.colectoext)
  --locked                Pass --locked to cargo build
  -h, --help              Show this help

The extension's manifest is extension.toml and its images are in assets/, both next to
Cargo.toml.";

struct Options {
    package: bool,
    manifest_path: Option<PathBuf>,
    out: Option<PathBuf>,
    locked: bool,
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Cargo runs `cargo-colecto colecto build …` for `cargo colecto build …`.
    if args.first().is_some_and(|a| a == "colecto") {
        args.remove(0);
    }
    let options = match parse_args(args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("error: {error}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: Vec<String>) -> Result<Option<Options>, String> {
    let mut args = args.into_iter();
    let package = match args.next().as_deref() {
        Some("build") => true,
        Some("check") => false,
        Some("-h" | "--help" | "help") | None => return Ok(None),
        Some(other) => return Err(format!("unknown command `{other}`")),
    };
    let mut options = Options {
        package,
        manifest_path: None,
        out: None,
        locked: false,
    };
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .map(PathBuf::from)
                .ok_or(format!("{flag} needs a value"))
        };
        match arg.as_str() {
            "--manifest-path" => options.manifest_path = Some(value("--manifest-path")?),
            "--out" => options.out = Some(value("--out")?),
            "--locked" => options.locked = true,
            "-h" | "--help" => return Ok(None),
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    if !package && options.out.is_some() {
        return Err("--out only applies to `build`".into());
    }
    Ok(Some(options))
}

fn run(options: &Options) -> Result<(), String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let cargo_toml = locate_project(&cargo, options.manifest_path.as_deref())?;
    let (target_dir, library) = find_library(&cargo, &cargo_toml)?;
    let root = cargo_toml
        .parent()
        .ok_or("Cargo.toml has no parent directory")?;

    // Read the manifest first, so its mistakes show before a long build.
    let source = std::fs::read_to_string(root.join("extension.toml"))
        .map_err(|e| format!("cannot read {}: {e}", root.join("extension.toml").display()))?;
    let parsed = manifest::parse(&source)
        .and_then(|m| manifest::validate(&m).map(|()| m))
        .map_err(|e| format!("extension.toml: {e}"))?;

    let mut build = Command::new(&cargo);
    build.args([
        "build",
        "--release",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
        "--manifest-path",
    ]);
    build.arg(&cargo_toml);
    if options.locked {
        build.arg("--locked");
    }
    let status = build
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !status.success() {
        return Err("cargo build failed (is the target installed? `rustup target add wasm32-unknown-unknown`)".into());
    }
    let path = target_dir
        .join("wasm32-unknown-unknown/release")
        .join(format!("{library}.wasm"));
    let module =
        std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    let report = check::check(&parsed, &wasm::parse(&module)?);
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    if !report.errors.is_empty() {
        for error in &report.errors {
            eprintln!("error: {error}");
        }
        if let Some(help) = &report.help {
            eprintln!("help: {help}");
        }
        return Err(format!(
            "{} problem(s) between the code and extension.toml",
            report.errors.len()
        ));
    }
    let assets = read_assets(&root.join("assets"))?;
    let package = colecto_package::write_deflated(&source, &module, &assets);
    // The same reader the Host uses at installation, so every package limit applies.
    colecto_package::read_with_assets(&package).map_err(|e| format!("package rejected: {e}"))?;
    if !options.package {
        eprintln!(
            "Checked {} ({} KiB module)",
            parsed.id,
            module.len().div_ceil(1024)
        );
        return Ok(());
    }
    let out = match &options.out {
        Some(out) => out.clone(),
        None => target_dir
            .join("colecto")
            .join(format!("{}.colectoext", parsed.id)),
    };
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(&out, &package).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    eprintln!(
        "Wrote {} ({} KiB)",
        out.display(),
        package.len().div_ceil(1024)
    );
    Ok(())
}

fn locate_project(
    cargo: &std::ffi::OsStr,
    manifest_path: Option<&Path>,
) -> Result<PathBuf, String> {
    let mut command = Command::new(cargo);
    command.args(["locate-project", "--message-format", "plain"]);
    if let Some(path) = manifest_path {
        command.arg("--manifest-path").arg(path);
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

/// The target directory and the Wasm file stem of the package's `cdylib` library.
fn find_library(cargo: &std::ffi::OsStr, cargo_toml: &Path) -> Result<(PathBuf, String), String> {
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(cargo_toml)
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let target_dir = metadata["target_directory"]
        .as_str()
        .ok_or("cargo metadata has no target directory")?;
    let package = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["manifest_path"].as_str().map(Path::new) == Some(cargo_toml))
        .ok_or_else(|| format!("no package at {}", cargo_toml.display()))?;
    let library = package["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| {
            t["crate_types"]
                .as_array()
                .is_some_and(|k| k.iter().any(|k| k == "cdylib"))
        })
        .and_then(|t| t["name"].as_str())
        .ok_or("the package needs a library with crate-type = [\"cdylib\"]")?;
    Ok((PathBuf::from(target_dir), library.replace('-', "_")))
}

fn read_assets(dir: &Path) -> Result<Assets, String> {
    let mut assets = Assets::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(assets);
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let data = std::fs::read(entry.path()).map_err(|e| format!("assets/{name}: {e}"))?;
        if colecto_package::asset_content_type(&name, &data).is_none() {
            return Err(format!(
                "assets/{name}: use a flat name of lowercase letters, digits and hyphens ending in .png, .jpg, .jpeg, .webp or .gif, with matching content"
            ));
        }
        assets.insert(name, data);
    }
    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_commands_and_options() {
        let options = parse_args(args(&[
            "build",
            "--manifest-path",
            "x/Cargo.toml",
            "--out",
            "a.colectoext",
            "--locked",
        ]))
        .unwrap()
        .unwrap();
        assert!(options.package && options.locked);
        assert_eq!(
            options.manifest_path.as_deref(),
            Some(Path::new("x/Cargo.toml"))
        );
        assert!(!parse_args(args(&["check"])).unwrap().unwrap().package);
        assert!(parse_args(args(&[])).unwrap().is_none());
        assert!(parse_args(args(&["check", "--out", "a"])).is_err());
        assert!(parse_args(args(&["build", "--out"])).is_err());
        assert!(parse_args(args(&["publish"])).is_err());
    }
}
