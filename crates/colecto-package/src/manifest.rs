//! `extension.toml` / `manifest.toml` rules, shared by the Host and `cargo colecto`.
use crate::MAX_MANIFEST;
use colecto_extension_api::{
    ApiSupport, Capability, ExtensionManifest, api_support, capability_since, latest_api_version,
};

/// The API version that first allows `.` in bus names, as in `v0.1.2/add_button`.
pub const DOTTED_BUS_NAMES_SINCE: u32 = 3;

/// A bus request or event name: 1–64 bytes of lowercase ASCII, digits, `-`, `_`, `.` and
/// `/`. Names with `.` also need [`DOTTED_BUS_NAMES_SINCE`]; see [`bus_name_allowed`].
pub fn valid_bus_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_./".contains(&b))
}

/// [`valid_bus_name`] for an extension that targets `api_version`.
pub fn bus_name_allowed(name: &str, api_version: u32) -> bool {
    valid_bus_name(name) && (api_version >= DOTTED_BUS_NAMES_SINCE || !name.contains('.'))
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
    if ![
        "shell", "mailbox", "reader", "composer", "accounts", "panel", "overlay",
    ]
    .contains(&manifest.slot.as_str())
    {
        return Err("Unknown extension slot".into());
    }
    if manifest.capabilities.contains(&Capability::AppNavigate) {
        return Err("app.navigate is reserved for API keys".into());
    }
    if manifest.capabilities.contains(&Capability::WindowOverlay) {
        if !manifest.z_index.is_some_and(|z| (1..=10000).contains(&z)) {
            return Err("window.overlay requires z_index between 1 and 10000".into());
        }
    } else if manifest.z_index.is_some() {
        return Err("z_index requires window.overlay".into());
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
                        || !bus_name_allowed(name, version)
                })
            })
        {
            return Err("Invalid bus subscriptions (maximum 32)".into());
        }
    }
    Ok(manifest)
}

/// The extra rules for third-party (Wasm) packages.
/// Like [`parse_any_version`], it accepts discontinued API versions.
pub fn validate(manifest: &ExtensionManifest) -> Result<(), String> {
    parse_any_version(&toml::to_string(manifest).map_err(|e| e.to_string())?)?;
    // Guests can only emit declarative panels or sprites.
    if !["panel", "overlay"].contains(&manifest.slot.as_str()) {
        return Err("Wasm extensions require slot = panel or overlay".into());
    }
    if toml::to_string(manifest).map_err(|e| e.to_string())?.len() > MAX_MANIFEST {
        return Err("Manifest too large".into());
    }
    Ok(())
}
