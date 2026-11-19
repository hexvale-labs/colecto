//! `extension.toml` / `manifest.toml` rules, shared by the Host and `cargo colecto`.
use crate::MAX_MANIFEST;
use colecto_extension_api::{
    ApiSupport, Capability, ExtensionManifest, api_support, capability_since, latest_api_version,
};

/// A bus request or event name: 1–64 bytes of lowercase ASCII, digits, `-`, `_`, `.` and
/// `/`.
pub fn valid_bus_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_./".contains(&b))
}

/// Why this build cannot run an extension that targets `api_version`, or `None` if it can.
pub fn unsupported(api_version: u32) -> Option<String> {
    explain(api_version, api_support(api_version))
}

/// The message for an extension that targets `api_version` with the given support.
pub fn explain(api_version: u32, support: ApiSupport) -> Option<String> {
    match support {
        ApiSupport::Supported => None,
        ApiSupport::Discontinued(release) => Some(format!(
            "This extension uses extension API {api_version}, which Colecto stopped supporting in {release}."
        )),
        ApiSupport::Newer => Some(format!(
            "This extension needs extension API {api_version}, but this Colecto supports up to API {}. Update Colecto to use it.",
            latest_api_version()
        )),
        ApiSupport::Unknown => Some("Unsupported extension API version".into()),
    }
}

/// Parses and checks a manifest against the rules shared by bundled and third-party
/// extensions, including that this build supports its API version.
pub fn parse(source: &str) -> Result<ExtensionManifest, String> {
    let manifest = parse_any_version(source)?;
    match unsupported(manifest.api_version) {
        Some(error) => Err(error),
        None => Ok(manifest),
    }
}

/// [`parse`], but also accepting API versions this build discontinued, so installed
/// packages that target one can still be listed and removed.
pub fn parse_any_version(source: &str) -> Result<ExtensionManifest, String> {
    let manifest: ExtensionManifest = toml::from_str(source).map_err(|e| e.to_string())?;
    colecto_extension_api::validate_settings(&manifest.settings)?;
    let version = manifest.api_version;
    if let ApiSupport::Newer | ApiSupport::Unknown = api_support(version) {
        return Err(unsupported(version).unwrap_or_default());
    }
    if manifest.id.is_empty()
        || !manifest
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("Invalid extension identifier".into());
    }
    if [&manifest.name, &manifest.description, &manifest.author]
        .iter()
        .any(|s| s.trim().is_empty())
    {
        return Err("Extension metadata cannot be empty".into());
    }
    if let Some(client) = &manifest.client
        && !(crate::safe_client_path(client)
            && (client.ends_with(".js") || client.ends_with(".wasm")))
    {
        return Err("client must name a .js or .wasm file under client/".into());
    }
    if let Some(service) = &manifest.service
        && !crate::safe_service_name(service)
    {
        return Err("service must name a .wasm module at the package root".into());
    }
    if let Some(pane) = &manifest.pane
        && !valid_icon(&pane.icon)
    {
        return Err("Unknown pane icon".into());
    }
    if let Some(overlay) = &manifest.overlay
        && !(manifest.capabilities.contains(&Capability::UiCanvas)
            && (1..=10000).contains(&overlay.z_index))
    {
        return Err("[overlay] needs ui.canvas and a z_index from 1 to 10000".into());
    }
    if version < 2 && manifest.settings.iter().any(|s| s.section.is_some()) {
        return Err("settings.section needs api_version = 2 or later".into());
    }
    if let Some(page) = &manifest.settings_page {
        if page.title.trim().is_empty() || page.title.len() > 32 {
            return Err("settings_page.title must be 1–32 bytes".into());
        }
        if CORE_SETTINGS_TITLES
            .iter()
            .any(|core| core.eq_ignore_ascii_case(page.title.trim()))
        {
            return Err("settings_page.title must not match a core Settings section".into());
        }
        if !valid_icon(&page.icon) {
            return Err("Unknown settings_page icon".into());
        }
    }
    for (index, target) in manifest.navigation.iter().enumerate() {
        if !NAVIGATION_OWNED.contains(&target.as_str())
            || manifest.navigation[..index].contains(target)
        {
            return Err(
                "navigation lists destinations from inbox, drafts, sent and compose".into(),
            );
        }
    }
    if manifest.capabilities.contains(&Capability::AppNavigate) {
        return Err("app.navigate is reserved for API keys".into());
    }
    for (index, &capability) in manifest.capabilities.iter().enumerate() {
        if manifest.capabilities[..index].contains(&capability) {
            return Err("Duplicate capability".into());
        }
        if capability_since(capability).is_none_or(|since| since > version) {
            let name = toml::Value::try_from(capability)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            return Err(match capability_since(capability) {
                Some(since) => format!("{name} needs api_version = {since} or later"),
                None => format!("{name} is not available to extensions"),
            });
        }
    }
    if let Some(bus) = &manifest.bus {
        if !manifest.capabilities.contains(&Capability::ExtensionsBus) {
            return Err("bus subscriptions require extensions.bus".into());
        }
        if bus.subscribe.len() > 32
            || bus.subscribe.iter().any(|entry| {
                entry.split_once('/').is_none_or(|(bus, name)| {
                    bus.is_empty()
                        || !bus
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                        || !valid_bus_name(name)
                })
            })
        {
            return Err("Invalid bus subscriptions (maximum 32)".into());
        }
    }
    Ok(manifest)
}

/// The extra rules for third-party packages. Like [`parse_any_version`], it accepts
/// discontinued API versions.
pub fn validate(manifest: &ExtensionManifest) -> Result<(), String> {
    let source = toml::to_string(manifest).map_err(|e| e.to_string())?;
    parse_any_version(&source)?;
    if manifest.client.is_none() {
        return Err("Extensions need a client part: set client = \"client/main.js\"".into());
    }
    if source.len() > MAX_MANIFEST {
        return Err("Manifest too large".into());
    }
    Ok(())
}

/// Core Settings section titles an extension's own section may not copy.
pub const CORE_SETTINGS_TITLES: &[&str] = &[
    "Accounts",
    "Connection",
    "Extensions",
    "API keys",
    "Send approvals",
    "Appearance",
    "Notifications",
];

/// Destinations an extension can own with `navigation`; Settings pages stay core.
pub const NAVIGATION_OWNED: &[&str] = &["inbox", "drafts", "sent", "compose"];

/// Icon names extensions may use, the closed set in `ui/src/Icon.svelte` (API 1).
pub const ICONS: &[&str] = &[
    "account",
    "alert",
    "attach",
    "back",
    "bell",
    "check",
    "close",
    "compose",
    "copy",
    "cpu",
    "download",
    "drafts",
    "extension",
    "forward",
    "image",
    "inbox",
    "info",
    "key",
    "layout",
    "mail",
    "next",
    "palette",
    "pencil",
    "plus",
    "refresh",
    "reply",
    "replyAll",
    "search",
    "sent",
    "server",
    "settings",
    "shield",
    "sidebar",
    "sort",
    "sparkles",
    "text",
    "trash",
];

pub fn valid_icon(name: &str) -> bool {
    ICONS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icons_match_the_ui() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src");
        let svelte = std::fs::read_to_string(root.join("Icon.svelte")).unwrap();
        let renderer = std::fs::read_to_string(root.join("runtime/protocol.ts")).unwrap();
        for icon in ICONS {
            assert!(
                svelte.contains(&format!("    {icon}: '")),
                "Icon.svelte has no {icon}"
            );
            assert!(
                renderer.contains(&format!("'{icon}'")),
                "protocol.ts does not list {icon}"
            );
        }
        let listed = renderer
            .split_once("export const ICONS = [")
            .and_then(|(_, rest)| rest.split_once(']'))
            .unwrap()
            .0
            .matches('\'')
            .count()
            / 2;
        assert_eq!(
            listed,
            ICONS.len(),
            "protocol.ts and manifest.rs list the same icons"
        );
    }
    #[test]
    fn settings_sections_need_api_2() {
        let manifest = |version: u32, section: &str| {
            format!(
                "id = \"x\"\nname = \"X\"\napi_version = {version}\ncapabilities = []\ndescription = \"d\"\nauthor = \"a\"\n\n[[settings]]\nkey = \"k\"\nlabel = \"K\"\ntype = \"toggle\"\ndefault = true\n{section}"
            )
        };
        assert_eq!(parse(&manifest(1, "")).err(), None);
        assert_eq!(
            parse(&manifest(1, "section = \"appearance\"")).unwrap_err(),
            "settings.section needs api_version = 2 or later"
        );
        assert!(parse(&manifest(2, "section = \"notifications\"")).is_ok());
        // Sections that hold credentials, connections or keys take no extension settings.
        for core in ["accounts", "servers", "api-keys", "extensions"] {
            assert!(parse(&manifest(2, &format!("section = \"{core}\""))).is_err());
        }
    }
}
