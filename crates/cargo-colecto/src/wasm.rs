//! Reads just enough of a Wasm binary: imports, exports and the handler records that
//! `colecto_extension::export!` leaves in the `colecto.handlers.v1` custom section.

/// Custom section written by `colecto_extension_sys::handler_record!`.
pub const HANDLERS_SECTION: &str = "colecto.handlers.v1";

#[derive(Debug, Default)]
pub struct Module {
    /// `(module, name)` pairs.
    pub imports: Vec<(String, String)>,
    /// `(name, kind)` pairs; kind 0 is a function, 2 a memory.
    pub exports: Vec<(String, u8)>,
    /// `None` when the module has no handler section (not built with `export!`).
    pub handlers: Option<Vec<Handler>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Panel,
    Overlay,
    Request,
    Event,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handler {
    pub kind: Kind,
    pub bus: String,
    pub name: String,
}

pub fn parse(bytes: &[u8]) -> Result<Module, String> {
    let invalid = || "module.wasm is not a valid WebAssembly module".to_string();
    if !bytes.starts_with(b"\0asm\x01\0\0\0") {
        return Err(invalid());
    }
    let mut module = Module::default();
    let mut reader = Reader { bytes, at: 8 };
    while reader.at < bytes.len() {
        let id = reader.byte().ok_or_else(invalid)?;
        let size = reader.leb().ok_or_else(invalid)? as usize;
        let end = reader
            .at
            .checked_add(size)
            .filter(|&end| end <= bytes.len())
            .ok_or_else(invalid)?;
        let mut section = Reader {
            bytes: &bytes[..end],
            at: reader.at,
        };
        match id {
            0 => {
                let name = section.name().ok_or_else(invalid)?;
                if name == HANDLERS_SECTION {
                    let records = decode_handlers(&bytes[section.at..end])?;
                    module.handlers.get_or_insert_with(Vec::new).extend(records);
                }
            }
            2 => {
                for _ in 0..section.leb().ok_or_else(invalid)? {
                    let from = section.name().ok_or_else(invalid)?;
                    let name = section.name().ok_or_else(invalid)?;
                    section.import_descriptor().ok_or_else(invalid)?;
                    module.imports.push((from, name));
                }
            }
            7 => {
                for _ in 0..section.leb().ok_or_else(invalid)? {
                    let name = section.name().ok_or_else(invalid)?;
                    let kind = section.byte().ok_or_else(invalid)?;
                    section.leb().ok_or_else(invalid)?;
                    module.exports.push((name, kind));
                }
            }
            _ => {}
        }
        reader.at = end;
    }
    Ok(module)
}

/// Inverse of `colecto_extension_sys::record`: a kind byte, then bus and name, each
/// prefixed by a one-byte length.
pub fn decode_handlers(mut bytes: &[u8]) -> Result<Vec<Handler>, String> {
    let invalid = || {
        format!(
            "the {HANDLERS_SECTION} section is malformed; rebuild with a matching colecto-extension"
        )
    };
    let mut handlers = Vec::new();
    while let Some((&kind, rest)) = bytes.split_first() {
        let kind = match kind {
            b'p' => Kind::Panel,
            b'o' => Kind::Overlay,
            b'r' => Kind::Request,
            b'e' => Kind::Event,
            _ => return Err(invalid()),
        };
        fn text(rest: &mut &[u8]) -> Option<String> {
            let (&len, tail) = rest.split_first()?;
            let (value, tail) = tail.split_at_checked(len as usize)?;
            *rest = tail;
            String::from_utf8(value.to_vec()).ok()
        }
        let mut rest = rest;
        let bus = text(&mut rest).ok_or_else(invalid)?;
        let name = text(&mut rest).ok_or_else(invalid)?;
        handlers.push(Handler { kind, bus, name });
        bytes = rest;
    }
    Ok(handlers)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(byte)
    }
    fn leb(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte < 0x80 {
                return Some(value);
            }
        }
        None
    }
    fn name(&mut self) -> Option<String> {
        let len = self.leb()? as usize;
        let end = self.at.checked_add(len)?;
        let value = std::str::from_utf8(self.bytes.get(self.at..end)?)
            .ok()?
            .to_string();
        self.at = end;
        Some(value)
    }
    fn limits(&mut self) -> Option<()> {
        let flags = self.byte()?;
        self.leb()?;
        if flags & 1 != 0 {
            self.leb()?;
        }
        Some(())
    }
    fn import_descriptor(&mut self) -> Option<()> {
        match self.byte()? {
            0 => self.leb().map(drop),                    // function type index
            1 => self.byte().and_then(|_| self.limits()), // table: element type, limits
            2 => self.limits(),                           // memory
            3 => self.byte().and_then(|_| self.byte()).map(drop), // global: type, mutability
            4 => self.byte().and_then(|_| self.leb()).map(drop), // tag
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_what_the_sdk_encodes() {
        let event: [u8; 3 + 6 + 17] =
            colecto_extension_sys::record(b'e', "reader", "v1/message_opened");
        let panel: [u8; 3] = colecto_extension_sys::record(b'p', "", "");
        let handlers = decode_handlers(&[&event[..], &panel[..]].concat()).unwrap();
        assert_eq!(
            handlers,
            [
                Handler {
                    kind: Kind::Event,
                    bus: "reader".into(),
                    name: "v1/message_opened".into()
                },
                Handler {
                    kind: Kind::Panel,
                    bus: String::new(),
                    name: String::new()
                },
            ]
        );
        assert!(decode_handlers(b"x\0\0").is_err());
        assert!(decode_handlers(b"e\x09read").is_err(), "truncated");
    }

    #[test]
    fn reads_imports_exports_and_handlers() {
        let module = wat::parse_str(
            r#"(module
                (import "colecto" "emit" (func (param i32 i32) (result i32)))
                (import "env" "table" (table 1 funcref))
                (import "env" "glob" (global i32))
                (memory (export "memory") 1)
                (func (export "run"))
                (@custom "colecto.handlers.v1" "p\00\00")
                (@custom "colecto.handlers.v1" "r\03abc\05v1/go"))"#,
        )
        .unwrap();
        let module = parse(&module).unwrap();
        assert_eq!(
            module.imports,
            [
                ("colecto".into(), "emit".into()),
                ("env".into(), "table".into()),
                ("env".into(), "glob".into())
            ]
        );
        assert!(
            module.exports.contains(&("run".into(), 0))
                && module.exports.contains(&("memory".into(), 2))
        );
        let kinds: Vec<_> = module.handlers.unwrap().iter().map(|h| h.kind).collect();
        assert_eq!(kinds, [Kind::Panel, Kind::Request]);
        assert!(
            parse(b"\0asm\x01\0\0\0\x07\x05").is_err(),
            "truncated section"
        );
        assert!(
            parse(&wat::parse_str("(module)").unwrap())
                .unwrap()
                .handlers
                .is_none()
        );
    }
}
