//! Bounded `.colectoext` ZIP packages: a manifest, the client part's files under `client/`,
//! an optional Wasm service module and optional raster assets.
//!
//! Shared by the Host (installation) and `cargo colecto` (packaging), so both apply the
//! same limits and manifest rules.
use colecto_extension_api::ExtensionManifest;
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

pub mod manifest;

pub const MANIFEST: &str = "manifest.toml";
pub const CLIENT_DIR: &str = "client/";
pub const MAX_MANIFEST: usize = 8 * 1024;
/// The service module, and each file under `client/`.
pub const MAX_MODULE: usize = 16 * 1024 * 1024;
pub const MAX_CLIENT_FILES: usize = 256;
pub const MAX_PACKAGE: usize = 16 * 1024 * 1024;
pub const MAX_UNCOMPRESSED: usize = 48 * 1024 * 1024;
pub const MAX_ASSET: usize = 8 * 1024 * 1024;
pub const MAX_ASSETS: usize = 256;
pub const MAX_INSTALLED: usize = 32;
pub const MAX_RATIO: u64 = 100;
/// JSON encodes each upload byte as at most three digits and a comma.
pub const MAX_UPLOAD_BODY: usize = MAX_PACKAGE * 4 + 1024;

/// Files by their full path in the package, such as `client/main.js`.
pub type Files = BTreeMap<String, Vec<u8>>;
pub type Assets = BTreeMap<String, Vec<u8>>;

/// A package's contents. Whether they match the manifest is [`check`]'s job.
#[derive(Clone, Debug, Default)]
pub struct Package {
    pub manifest: String,
    pub client: Files,
    /// The service module's name at the package root, and its bytes.
    pub service: Option<(String, Vec<u8>)>,
    pub assets: Assets,
}

/// A path under `client/` the runtime may load: up to eight segments of `[A-Za-z0-9_.-]`, none
/// starting with `.`, ending in a type the client part can use.
pub fn safe_client_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix(CLIENT_DIR) else {
        return false;
    };
    let segments: Vec<&str> = rest.split('/').collect();
    segments.len() <= 8
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.len() <= 64
                && !segment.starts_with('.')
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        })
        && rest.rsplit_once('.').is_some_and(|(_, extension)| {
            matches!(extension, "js" | "wasm" | "json" | "bin" | "html" | "css")
        })
}

/// A service module name at the package root: `[a-z0-9-]+.wasm`.
pub fn safe_service_name(name: &str) -> bool {
    name.strip_suffix(".wasm").is_some_and(|stem| {
        !stem.is_empty()
            && stem.len() <= 64
            && stem
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

/// Read a package, bounding sizes before and while decompressing.
pub fn read(bytes: &[u8]) -> Result<Package, String> {
    let invalid = || {
        format!(
            "Choose a .colectoext package containing {MANIFEST}, the client part under {CLIENT_DIR}, an optional service .wasm module and optional raster images under assets/"
        )
    };
    if bytes.len() > MAX_PACKAGE {
        return Err("Extension package must be at most 16 MiB".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| invalid())?;
    if !(1..=MAX_ASSETS + MAX_CLIENT_FILES + 2).contains(&archive.len()) {
        return Err(invalid());
    }
    let mut names = std::collections::HashSet::new();
    let (mut client_names, mut asset_names, mut service_name) = (Vec::new(), Vec::new(), None);
    let mut declared_total = 0u64;
    let archive_len = bytes.len() as u64;
    let archive_ratio_limit = archive_len.saturating_mul(MAX_RATIO);
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|_| invalid())?;
        let name = file.name().map_err(|_| invalid())?.to_string();
        if !names.insert(name.clone())
            || !file.is_file()
            || !matches!(
                file.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
        {
            return Err(invalid());
        }
        let limit = if name == MANIFEST {
            MAX_MANIFEST
        } else if safe_client_path(&name) {
            client_names.push(name.clone());
            MAX_MODULE
        } else if let Some(asset) = name.strip_prefix("assets/") {
            if !safe_asset_name(asset) {
                return Err(invalid());
            }
            asset_names.push(asset.to_owned());
            MAX_ASSET
        } else if safe_service_name(&name) && service_name.is_none() {
            service_name = Some(name.clone());
            MAX_MODULE
        } else {
            return Err(invalid());
        };
        if file.compressed_size() > archive_len {
            return Err(invalid());
        }
        declared_total = declared_total
            .checked_add(file.size())
            .ok_or_else(invalid)?;
        if file.size() > limit as u64 || declared_total > MAX_UNCOMPRESSED as u64 {
            return Err("Extension entry or total content exceeds size limits".into());
        }
        if file.size() > file.compressed_size().saturating_mul(MAX_RATIO)
            || declared_total > archive_ratio_limit
        {
            return Err("Extension compression ratio exceeds 100:1".into());
        }
    }
    if client_names.len() > MAX_CLIENT_FILES || asset_names.len() > MAX_ASSETS {
        return Err(invalid());
    }
    let mut actual_total = 0usize;
    let mut entry = |name: &str, limit: usize| -> Result<Vec<u8>, String> {
        let file = archive.by_name(name).map_err(|_| invalid())?;
        if !file.is_file() || file.size() > limit as u64 {
            return Err(format!("{name} must be at most {} KiB", limit / 1024));
        }
        let ratio_limit = file
            .compressed_size()
            .min(archive_len)
            .saturating_mul(MAX_RATIO)
            .min(archive_ratio_limit.saturating_sub(actual_total as u64));
        // Declared sizes can lie; bound actual inflation by all three budgets.
        let mut data = Vec::new();
        file.take((limit.min(MAX_UNCOMPRESSED - actual_total) as u64).min(ratio_limit) + 1)
            .read_to_end(&mut data)
            .map_err(|_| invalid())?;
        if data.len() as u64 > ratio_limit {
            return Err("Extension compression ratio exceeds 100:1".into());
        }
        if data.len() > limit {
            return Err(format!("{name} must be at most {} KiB", limit / 1024));
        }
        actual_total = actual_total.checked_add(data.len()).ok_or_else(invalid)?;
        if actual_total > MAX_UNCOMPRESSED {
            return Err("Extension total content exceeds size limit".into());
        }
        Ok(data)
    };
    let manifest = String::from_utf8(entry(MANIFEST, MAX_MANIFEST)?)
        .map_err(|_| format!("{MANIFEST} must be UTF-8 text"))?;
    let mut package = Package {
        manifest,
        ..Package::default()
    };
    for name in client_names {
        let data = entry(&name, MAX_MODULE)?;
        package.client.insert(name, data);
    }
    if let Some(name) = service_name {
        let data = entry(&name, MAX_MODULE)?;
        package.service = Some((name, data));
    }
    for name in asset_names {
        let data = entry(&format!("assets/{name}"), MAX_ASSET)?;
        asset_content_type(&name, &data).ok_or("Asset type does not match its name")?;
        package.assets.insert(name, data);
    }
    Ok(package)
}

/// Whether a package holds exactly what its manifest names: the client entry, and a service
/// module only when `service` is declared.
pub fn check(manifest: &ExtensionManifest, package: &Package) -> Result<(), String> {
    let client = manifest
        .client
        .as_deref()
        .ok_or("Extensions need a client part: set client = \"client/main.js\"")?;
    if !package.client.contains_key(client) {
        return Err(format!("The package has no {client}"));
    }
    match (&manifest.service, &package.service) {
        (None, None) => Ok(()),
        (Some(declared), Some((name, _))) if declared == name => Ok(()),
        (Some(declared), _) => Err(format!("The package has no {declared}")),
        (None, Some((name, _))) => Err(format!("{name} is not declared as service")),
    }
}

pub fn safe_asset_name(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && stem
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && matches!(extension, "png" | "jpg" | "jpeg" | "webp" | "gif")
    })
}
pub fn asset_content_type(name: &str, data: &[u8]) -> Option<&'static str> {
    if !safe_asset_name(name) {
        return None;
    }
    match name.rsplit_once('.')?.1 {
        "png" if data.starts_with(b"\x89PNG\r\n\x1a\n") => Some("image/png"),
        "jpg" | "jpeg" if data.starts_with(&[0xff, 0xd8, 0xff]) => Some("image/jpeg"),
        "gif" if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") => Some("image/gif"),
        "webp" if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") => {
            Some("image/webp")
        }
        _ => None,
    }
}

/// Build an uncompressed package, for examples and tests.
pub fn write(package: &Package) -> Vec<u8> {
    write_with(package, zip::CompressionMethod::Stored)
}
/// Like [`write`], but Deflate-compressed, for packages people download.
pub fn write_deflated(package: &Package) -> Vec<u8> {
    write_with(package, zip::CompressionMethod::Deflated)
}
fn write_with(package: &Package, method: zip::CompressionMethod) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(method);
    let mut entries: Vec<(String, &[u8])> = vec![(MANIFEST.into(), package.manifest.as_bytes())];
    entries.extend(
        package
            .client
            .iter()
            .map(|(n, d)| (n.clone(), d.as_slice())),
    );
    entries.extend(
        package
            .service
            .iter()
            .map(|(n, d)| (n.clone(), d.as_slice())),
    );
    entries.extend(
        package
            .assets
            .iter()
            .map(|(n, d)| (format!("assets/{n}"), d.as_slice())),
    );
    for (name, data) in entries {
        writer.start_file(name, options).expect("in-memory zip");
        writer.write_all(data).expect("in-memory zip");
    }
    writer.finish().expect("in-memory zip").into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;
    // Size and ratio rules are the same for every client file and the service module.
    const MODULE: &str = "client/main.js";
    fn zip_with(files: &[(&str, &[u8])], method: zip::CompressionMethod) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in files {
            writer
                .start_file(
                    *name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(method)
                        .compression_level(if method == zip::CompressionMethod::Deflated {
                            Some(1)
                        } else {
                            None
                        }),
                )
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        zip_with(files, zip::CompressionMethod::Stored)
    }
    fn patterned(size: usize) -> Vec<u8> {
        let mut state = 17u32;
        (0..size)
            .map(|i| {
                if i % 8 == 0 {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    state as u8
                } else {
                    0
                }
            })
            .collect()
    }
    #[test]
    fn package_size_boundaries_and_entry_budgets() {
        let overhead = archive(&[(MANIFEST, b""), (MODULE, b"")]).len();
        for delta in [-1isize, 0, 1] {
            let module = vec![0; (MAX_PACKAGE as isize + delta) as usize - overhead];
            let bytes = archive(&[(MANIFEST, b""), (MODULE, &module)]);
            assert_eq!(bytes.len(), (MAX_PACKAGE as isize + delta) as usize);
            assert_eq!(read(&bytes).is_ok(), delta <= 0);
        }
        let data = patterned(MAX_MODULE + 1);
        for delta in [-1isize, 0, 1] {
            let size = (MAX_MODULE as isize + delta) as usize;
            assert_eq!(
                read(&zip_with(
                    &[(MANIFEST, b""), (MODULE, &data[..size])],
                    zip::CompressionMethod::Deflated
                ))
                .is_ok(),
                delta <= 0
            );
            let manifest = vec![b' '; (MAX_MANIFEST as isize + delta) as usize];
            assert_eq!(
                read(&archive(&[(MANIFEST, &manifest), (MODULE, b"x")])).is_ok(),
                delta <= 0
            );
            let mut image = data[..(MAX_ASSET as isize + delta) as usize].to_vec();
            image[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
            assert_eq!(
                read(&zip_with(
                    &[(MANIFEST, b""), (MODULE, b""), ("assets/cat.png", &image)],
                    zip::CompressionMethod::Deflated
                ))
                .is_ok(),
                delta <= 0
            );
        }
    }
    #[test]
    fn total_uncompressed_boundary_is_checked_before_inflating() {
        let mut data = patterned(MAX_ASSET);
        data[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        let names: Vec<_> = (0..6).map(|i| format!("assets/cat-{i}.png")).collect();
        for delta in [-1isize, 0, 1] {
            let mut files = vec![
                (MANIFEST, b"".as_slice()),
                (
                    MODULE,
                    if delta > 0 {
                        b"x".as_slice()
                    } else {
                        b"".as_slice()
                    },
                ),
            ];
            files.extend(names.iter().enumerate().map(|(i, name)| {
                (
                    name.as_str(),
                    &data[..if i == 0 && delta < 0 {
                        data.len() - 1
                    } else {
                        data.len()
                    }],
                )
            }));
            let bytes = zip_with(&files, zip::CompressionMethod::Deflated);
            assert!(bytes.len() <= MAX_PACKAGE);
            assert_eq!(read(&bytes).is_ok(), delta <= 0);
        }
    }
    #[test]
    fn asset_count_types_and_names_are_bounded() {
        let png = b"\x89PNG\r\n\x1a\n";
        for count in [MAX_ASSETS - 1, MAX_ASSETS, MAX_ASSETS + 1] {
            let names: Vec<_> = (0..count).map(|i| format!("assets/cat-{i}.png")).collect();
            let mut files = vec![(MANIFEST, b"".as_slice()), (MODULE, b"".as_slice())];
            files.extend(names.iter().map(|n| (n.as_str(), png.as_slice())));
            assert_eq!(read(&archive(&files)).is_ok(), count <= MAX_ASSETS);
        }
        for (name, magic) in [
            ("cat.png", png.as_slice()),
            ("cat.jpg", b"\xff\xd8\xff".as_slice()),
            ("cat.jpeg", b"\xff\xd8\xff".as_slice()),
            ("cat.webp", b"RIFF0000WEBP".as_slice()),
            ("cat.gif", b"GIF89a".as_slice()),
        ] {
            for size in [magic.len() - 1, magic.len(), magic.len() + 1] {
                let mut data = magic.to_vec();
                data.resize(size, 0);
                let bytes = archive(&[
                    (MANIFEST, b""),
                    (MODULE, b""),
                    (&format!("assets/{name}"), &data),
                ]);
                assert_eq!(read(&bytes).is_ok(), size >= magic.len());
            }
        }
        for name in [
            "client/../x.js",
            "client/.hidden.js",
            "client/main.exe",
            "Client/main.js",
            "other/main.js",
            "assets/cat.svg",
            "assets/Cat.png",
            "assets/../cat.png",
            "assets/a/cat.png",
            "assets/.png",
            "assets/cat.png.js",
        ] {
            assert!(read(&archive(&[(MANIFEST, b""), (MODULE, b""), (name, png)])).is_err());
        }
        assert!(
            read(&archive(&[
                (MANIFEST, b""),
                (MODULE, b""),
                ("assets/cat.png", b"GIF89a")
            ]))
            .is_err()
        );
    }
    #[test]
    fn compression_ratio_boundaries_and_bombs() {
        // Padding a completed Deflate stream makes the ratio exact across compressor versions.
        for delta in [-1isize, 0, 1] {
            let data = vec![0; (20000isize + delta) as usize];
            let mut bytes = zip_with(
                &[(MANIFEST, b""), (MODULE, &data)],
                zip::CompressionMethod::Deflated,
            );
            let (compressed, local, central) = {
                let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
                let central = archive.central_directory_start() as usize;
                let file = archive.by_name(MODULE).unwrap();
                (
                    file.compressed_size() as usize,
                    file.header_start() as usize,
                    central,
                )
            };
            let padding = 200 - compressed;
            bytes.splice(central..central, vec![0; padding]);
            bytes[local + 18..local + 22].copy_from_slice(&200u32.to_le_bytes());
            let central = central + padding;
            let mut offset = central;
            while &bytes[offset..offset + 4] == b"PK\x01\x02" {
                let name_len =
                    u16::from_le_bytes(bytes[offset + 28..offset + 30].try_into().unwrap())
                        as usize;
                let extra = u16::from_le_bytes(bytes[offset + 30..offset + 32].try_into().unwrap())
                    as usize;
                let comment =
                    u16::from_le_bytes(bytes[offset + 32..offset + 34].try_into().unwrap())
                        as usize;
                if &bytes[offset + 46..offset + 46 + name_len] == MODULE.as_bytes() {
                    bytes[offset + 20..offset + 24].copy_from_slice(&200u32.to_le_bytes());
                }
                offset += 46 + name_len + extra + comment;
            }
            bytes[offset + 16..offset + 20].copy_from_slice(&(central as u32).to_le_bytes());
            assert_eq!(read(&bytes).is_ok(), delta <= 0);
        }
        let bomb = zip_with(
            &[(MANIFEST, b""), (MODULE, &vec![0; MAX_MODULE + 1])],
            zip::CompressionMethod::Deflated,
        );
        assert!(read(&bomb).is_err());
    }
    #[test]
    fn forged_compressed_size_exceeding_archive_length_is_rejected() {
        let data = vec![0; 50_000];
        let mut bytes = zip_with(
            &[(MANIFEST, b""), (MODULE, &data)],
            zip::CompressionMethod::Deflated,
        );
        let (local, central) = {
            let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
            let central = archive.central_directory_start() as usize;
            let file = archive.by_name(MODULE).unwrap();
            (file.header_start() as usize, central)
        };
        // Forge a compressed_size larger than the actual zip file so per-entry ratio looks <= 100:1.
        let forged = (bytes.len() as u32) + 1000;
        bytes[local + 18..local + 22].copy_from_slice(&forged.to_le_bytes());
        let mut offset = central;
        while &bytes[offset..offset + 4] == b"PK\x01\x02" {
            let name_len =
                u16::from_le_bytes(bytes[offset + 28..offset + 30].try_into().unwrap()) as usize;
            let extra =
                u16::from_le_bytes(bytes[offset + 30..offset + 32].try_into().unwrap()) as usize;
            let comment =
                u16::from_le_bytes(bytes[offset + 32..offset + 34].try_into().unwrap()) as usize;
            if &bytes[offset + 46..offset + 46 + name_len] == MODULE.as_bytes() {
                bytes[offset + 20..offset + 24].copy_from_slice(&forged.to_le_bytes());
            }
            offset += 46 + name_len + extra + comment;
        }
        assert!(read(&bytes).is_err());
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    fn manifest(client: &str, service: Option<&str>) -> ExtensionManifest {
        manifest::parse(&format!(
            "id = \"x\"\nname = \"X\"\ndescription = \"d\"\nauthor = \"a\"\napi_version = 1\nclient = \"{client}\"\n{}capabilities = []\n",
            service.map(|s| format!("service = \"{s}\"\n")).unwrap_or_default()
        ))
        .unwrap()
    }
    #[test]
    fn packages_round_trip_and_must_match_their_manifest() {
        let mut package = Package {
            manifest: "x".into(),
            ..Package::default()
        };
        package
            .client
            .insert("client/main.js".into(), b"1".to_vec());
        package
            .client
            .insert("client/lib/parser.wasm".into(), b"\0asm".to_vec());
        let read_back = read(&write(&package)).unwrap();
        assert_eq!(read_back.client, package.client);
        assert!(check(&manifest("client/main.js", None), &read_back).is_ok());
        assert!(check(&manifest("client/other.js", None), &read_back).is_err());
        assert!(
            check(
                &manifest("client/main.js", Some("service.wasm")),
                &read_back
            )
            .is_err()
        );
        package.service = Some(("service.wasm".into(), b"\0asm".to_vec()));
        let read_back = read(&write_deflated(&package)).unwrap();
        assert!(
            check(
                &manifest("client/main.js", Some("service.wasm")),
                &read_back
            )
            .is_ok()
        );
        assert!(check(&manifest("client/main.js", None), &read_back).is_err());
        // Two service modules are one too many.
        package.client.clear();
        let two = {
            let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for name in [MANIFEST, "a.wasm", "b.wasm"] {
                writer
                    .start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(b"x").unwrap();
            }
            writer.finish().unwrap().into_inner()
        };
        assert!(read(&two).is_err());
    }
    #[test]
    fn client_file_count_is_bounded() {
        for count in [MAX_CLIENT_FILES, MAX_CLIENT_FILES + 1] {
            let mut package = Package::default();
            for i in 0..count {
                package
                    .client
                    .insert(format!("client/f{i}.json"), b"{}".to_vec());
            }
            assert_eq!(read(&write(&package)).is_ok(), count <= MAX_CLIENT_FILES);
        }
    }
}
