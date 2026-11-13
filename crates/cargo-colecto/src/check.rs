//! Compares a built module with its manifest, catching mistakes the Host would only
//! reveal at runtime as silence (an event never delivered, a request never routed).

use crate::wasm::{Kind, Module};
use colecto_extension_api::{Capability, ExtensionManifest};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// A suggested fix, printed after the errors.
    pub help: Option<String>,
}

const IMPORTS: [&str; 3] = ["action", "request", "emit"];

pub fn check(manifest: &ExtensionManifest, module: &Module) -> Report {
    let mut report = Report::default();
    for (from, name) in &module.imports {
        if from != "colecto" || !IMPORTS.contains(&name.as_str()) {
            report.errors.push(format!(
                "the module imports `{from}.{name}`, but the Host only provides colecto.action, colecto.request and colecto.emit (no WASI)"
            ));
        }
    }
    for (name, kind, what) in [("run", 0, "function"), ("memory", 2, "memory")] {
        if !module.exports.contains(&(name.to_string(), kind)) {
            report.errors.push(format!(
                "the module does not export a {what} named `{name}`"
            ));
        }
    }
    let Some(handlers) = &module.handlers else {
        report.warnings.push(
            "no handler records found (the module was not built with colecto_extension::export!), so handlers were not checked against the manifest".into(),
        );
        return report;
    };

    let slot = manifest.slot.as_str();
    let has = |kind| handlers.iter().any(|h| h.kind == kind);
    for (kind, entry) in [(Kind::Panel, "panel"), (Kind::Overlay, "overlay")] {
        if has(kind) && slot != entry {
            report.errors.push(format!(
                "export! registers `{entry}:`, but extension.toml has slot = \"{slot}\""
            ));
        }
    }
    if !has(Kind::Panel) && !has(Kind::Overlay) {
        report.warnings.push(format!(
            "export! registers no `{slot}:` function, so the {slot} will show an error when displayed"
        ));
    }

    let bus_handlers = handlers
        .iter()
        .filter(|h| matches!(h.kind, Kind::Request | Kind::Event));
    if bus_handlers.clone().next().is_some()
        && !manifest.capabilities.contains(&Capability::ExtensionsBus)
    {
        report.errors.push(
            "export! registers bus handlers, but `extensions.bus` is not in capabilities".into(),
        );
    }

    let mut seen = Vec::new();
    for handler in handlers.iter().filter(|h| h.kind == Kind::Request) {
        if handler.bus != manifest.id {
            report.errors.push(format!(
                "`{}` is declared on bus `{}`, but requests only arrive on this extension's own bus `{}`",
                handler.name, handler.bus, manifest.id
            ));
        }
        if seen.contains(&&handler.name) {
            report.warnings.push(format!(
                "request `{}` is registered twice; only the first function runs",
                handler.name
            ));
        }
        seen.push(&handler.name);
    }

    let subscribed = manifest
        .bus
        .as_ref()
        .map_or(&[][..], |bus| &bus.subscribe[..]);
    let handled: Vec<String> = handlers
        .iter()
        .filter(|h| h.kind == Kind::Event)
        .map(|h| format!("{}/{}", h.bus, h.name))
        .collect();
    let missing: Vec<&String> = handled.iter().filter(|t| !subscribed.contains(t)).collect();
    for topic in &missing {
        report.errors.push(format!(
            "export! handles event `{topic}`, but extension.toml does not subscribe to it, so it would never arrive"
        ));
    }
    for topic in subscribed.iter().filter(|t| !handled.contains(t)) {
        report.errors.push(format!(
            "extension.toml subscribes to `{topic}`, but no function in export! handles it"
        ));
    }
    if !missing.is_empty() || subscribed.iter().any(|t| !handled.contains(t)) {
        let mut topics = handled.clone();
        topics.sort();
        topics.dedup();
        let list = topics
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ");
        report.help = Some(format!(
            "to match the code, use:\n\n[bus]\nsubscribe = [{list}]\n"
        ));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::Handler;

    fn manifest(extra: &str) -> ExtensionManifest {
        colecto_package::manifest::parse(&format!(
            "id = \"counter\"\nname = \"Counter\"\ndescription = \"Counts.\"\nauthor = \"Me\"\napi_version = 1\nslot = \"panel\"\n{extra}"
        ))
        .unwrap()
    }
    fn module(handlers: &[(Kind, &str, &str)]) -> Module {
        Module {
            imports: vec![("colecto".into(), "emit".into())],
            exports: vec![("run".into(), 0), ("memory".into(), 2)],
            handlers: Some(
                handlers
                    .iter()
                    .map(|&(kind, bus, name)| Handler {
                        kind,
                        bus: bus.into(),
                        name: name.into(),
                    })
                    .collect(),
            ),
        }
    }
    const BUS: &str = "capabilities = [\"extensions.bus\"]\n";

    #[test]
    fn matching_code_and_manifest_pass() {
        let manifest = manifest(&format!(
            "{BUS}[bus]\nsubscribe = [\"reader/v1/message_opened\"]"
        ));
        let module = module(&[
            (Kind::Panel, "", ""),
            (Kind::Request, "counter", "v1/count"),
            (Kind::Event, "reader", "v1/message_opened"),
        ]);
        assert_eq!(check(&manifest, &module), Report::default());
    }

    #[test]
    fn reports_subscription_drift_both_ways() {
        let manifest = manifest(&format!("{BUS}[bus]\nsubscribe = [\"inbox/v1/selected\"]"));
        let report = check(
            &manifest,
            &module(&[
                (Kind::Panel, "", ""),
                (Kind::Event, "reader", "v1/message_opened"),
            ]),
        );
        assert_eq!(report.errors.len(), 2, "{report:?}");
        assert!(report.errors[0].contains("does not subscribe to it"));
        assert!(report.errors[1].contains("no function in export! handles it"));
        assert!(
            report
                .help
                .unwrap()
                .contains("subscribe = [\"reader/v1/message_opened\"]")
        );
    }

    #[test]
    fn reports_slot_capability_bus_and_import_mistakes() {
        let mut module = module(&[
            (Kind::Overlay, "", ""),
            (Kind::Request, "someone-else", "v1/x"),
        ]);
        module
            .imports
            .push(("wasi_snapshot_preview1".into(), "fd_write".into()));
        module.exports.retain(|(name, _)| name != "memory");
        let report = check(&manifest("capabilities = []"), &module);
        let expected = [
            "wasi_snapshot_preview1.fd_write",
            "memory",
            "slot = \"panel\"",
            "`extensions.bus`",
            "own bus `counter`",
        ];
        assert_eq!(report.errors.len(), expected.len(), "{report:?}");
        for (error, fragment) in report.errors.iter().zip(expected) {
            assert!(error.contains(fragment), "{error}");
        }
        assert!(
            report.warnings.is_empty(),
            "an overlay function counts as a slot function"
        );
    }

    #[test]
    fn modules_without_records_are_only_warned_about() {
        let mut module = module(&[]);
        module.handlers = None;
        let report = check(&manifest("capabilities = []"), &module);
        assert!(report.errors.is_empty() && report.warnings.len() == 1);
    }
}
