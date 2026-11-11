//! Bounded `.colectoext` ZIP packages with a manifest, Wasm module and optional PNG assets.
use std::io::{Cursor, Read, Write};

pub const MANIFEST: &str = "manifest.toml";
pub const MODULE: &str = "module.wasm";
pub const MAX_MANIFEST: usize = 8 * 1024;
pub const MAX_MODULE: usize = 16 * 1024 * 1024;
pub const MAX_PACKAGE: usize = 16 * 1024 * 1024;
pub const MAX_UNCOMPRESSED: usize = 48 * 1024 * 1024;
pub const MAX_ASSET: usize = 8 * 1024 * 1024;
pub const MAX_ASSETS: usize = 256;
pub const MAX_INSTALLED: usize = 32;
pub const MAX_RATIO: u64 = 100;
/// JSON encodes each upload byte as at most three digits and a comma.
pub const MAX_UPLOAD_BODY: usize = MAX_PACKAGE * 4 + 1024;

/// Read a package's manifest source and Wasm module, bounding sizes before and while decompressing.
pub fn read(bytes: &[u8]) -> Result<(String, Vec<u8>), String> {
    let (manifest, module, _) = read_with_assets(bytes)?;
    Ok((manifest, module))
}
pub type Assets = std::collections::BTreeMap<String, Vec<u8>>;
pub fn read_with_assets(bytes: &[u8]) -> Result<(String, Vec<u8>, Assets), String> {
    let invalid = || {
        format!(
            "Choose a .colectoext package containing {MANIFEST}, {MODULE} and optional raster images under assets/"
        )
    };
    if bytes.len() > MAX_PACKAGE {
        return Err("Extension package must be at most 16 MiB".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| invalid())?;
    if !(2..=MAX_ASSETS + 2).contains(&archive.len()) {
        return Err(invalid());
    }
    let mut names = std::collections::HashSet::new();
    let mut asset_names = Vec::new();
    let mut declared_total = 0u64;
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|_| invalid())?;
        let name = file.name().map_err(|_| invalid())?;
        if !names.insert(name.to_string())
            || !file.is_file()
            || !matches!(
                file.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
        {
            return Err(invalid());
        }
        let limit = match name.as_ref() {
            MANIFEST => MAX_MANIFEST,
            MODULE => MAX_MODULE,
            _ => {
                let asset = name.strip_prefix("assets/").ok_or_else(invalid)?;
                if !safe_asset_name(asset) {
                    return Err(invalid());
                }
                asset_names.push(asset.to_owned());
                MAX_ASSET
            }
        };
        declared_total = declared_total
            .checked_add(file.size())
            .ok_or_else(invalid)?;
        if file.size() > limit as u64 || declared_total > MAX_UNCOMPRESSED as u64 {
            return Err("Extension entry or total content exceeds size limits".into());
        }
        if file.size() > file.compressed_size().saturating_mul(MAX_RATIO) {
            return Err("Extension compression ratio exceeds 100:1".into());
        }
    }
    let mut actual_total = 0usize;
    let mut entry = |name: &str, limit: usize| -> Result<Vec<u8>, String> {
        let file = archive.by_name(name).map_err(|_| invalid())?;
        if !file.is_file() || file.size() > limit as u64 {
            return Err(format!("{name} must be at most {} KiB", limit / 1024));
        }
        let ratio_limit = file.compressed_size().saturating_mul(MAX_RATIO);
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
    let module = entry(MODULE, MAX_MODULE)?;
    let mut assets = Assets::new();
    for name in asset_names {
        let data = entry(&format!("assets/{name}"), MAX_ASSET)?;
        asset_content_type(&name, &data).ok_or("Asset type does not match its name")?;
        assets.insert(name, data);
    }
    Ok((manifest, module, assets))
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

/// Build a package, for the example extension and tests.
pub fn write(manifest: &str, module: &[u8]) -> Vec<u8> {
    write_with_assets(manifest, module, &Assets::new())
}
pub fn write_with_assets(manifest: &str, module: &[u8], assets: &Assets) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, data) in [(MANIFEST, manifest.as_bytes()), (MODULE, module)] {
        writer.start_file(name, options).expect("in-memory zip");
        writer.write_all(data).expect("in-memory zip");
    }
    for (name, data) in assets {
        writer
            .start_file(format!("assets/{name}"), options)
            .expect("in-memory zip");
        writer.write_all(data).expect("in-memory zip");
    }
    writer.finish().expect("in-memory zip").into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
