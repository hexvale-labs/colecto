//! `.colectoext` extension packages: a zip with exactly `manifest.toml` and `module.wasm`.
use std::io::{Cursor, Read, Write};

pub const MANIFEST: &str = "manifest.toml";
pub const MODULE: &str = "module.wasm";
const MAX_MANIFEST: u64 = 8 * 1024;
const MAX_MODULE: u64 = 64 * 1024;
/// Both files, plus generous room for zip headers.
pub const MAX_PACKAGE: usize = 96 * 1024;

/// Read a package's manifest source and Wasm module, bounding sizes before and while decompressing.
pub fn read(bytes: &[u8]) -> Result<(String, Vec<u8>), String> {
    let invalid =
        || format!("Choose a .colectoext package containing only {MANIFEST} and {MODULE}");
    if bytes.len() > MAX_PACKAGE {
        return Err("Extension package must be at most 96 KiB".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| invalid())?;
    if archive.len() != 2 {
        return Err(invalid());
    }
    let mut entry = |name: &str, limit: u64| -> Result<Vec<u8>, String> {
        let file = archive.by_name(name).map_err(|_| invalid())?;
        if !file.is_file() || file.size() > limit {
            return Err(format!("{name} must be at most {} KiB", limit / 1024));
        }
        // Declared sizes can lie; the reader is capped too.
        let mut data = Vec::new();
        file.take(limit + 1)
            .read_to_end(&mut data)
            .map_err(|_| invalid())?;
        if data.len() as u64 > limit {
            return Err(format!("{name} must be at most {} KiB", limit / 1024));
        }
        Ok(data)
    };
    let manifest = String::from_utf8(entry(MANIFEST, MAX_MANIFEST)?)
        .map_err(|_| format!("{MANIFEST} must be UTF-8 text"))?;
    let module = entry(MODULE, MAX_MODULE)?;
    Ok((manifest, module))
}

/// Build a package, for the example extension and tests.
pub fn write(manifest: &str, module: &[u8]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, data) in [(MANIFEST, manifest.as_bytes()), (MODULE, module)] {
        writer.start_file(name, options).expect("in-memory zip");
        writer.write_all(data).expect("in-memory zip");
    }
    writer.finish().expect("in-memory zip").into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_with(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in files {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn packages_round_trip_and_reject_anything_else() {
        let (manifest, module) = read(&write("id = \"x\"", b"\0asm")).unwrap();
        assert_eq!(
            (manifest.as_str(), module.as_slice()),
            ("id = \"x\"", &b"\0asm"[..])
        );
        let big = vec![0; 70 * 1024];
        for bad in [
            b"not a zip".to_vec(),
            zip_with(&[(MANIFEST, b"")]),
            zip_with(&[(MANIFEST, b""), (MODULE, b""), ("extra.js", b"")]),
            zip_with(&[(MANIFEST, b""), ("../module.wasm", b"")]),
            zip_with(&[(MANIFEST, &[0xff]), (MODULE, b"")]),
            zip_with(&[(MANIFEST, b""), (MODULE, &big)]),
        ] {
            assert!(read(&bad).is_err());
        }
    }

    #[test]
    fn compressed_entries_cannot_expand_past_their_limit() {
        // 1 MiB of zeros deflates to a few KiB: a small package must not unpack into a big module.
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer.start_file(MANIFEST, deflated).unwrap();
        writer.start_file(MODULE, deflated).unwrap();
        writer.write_all(&vec![0; 1024 * 1024]).unwrap();
        let bomb = writer.finish().unwrap().into_inner();
        assert!(bomb.len() < MAX_PACKAGE);
        assert!(read(&bomb).unwrap_err().contains("at most 64 KiB"));
    }
}
