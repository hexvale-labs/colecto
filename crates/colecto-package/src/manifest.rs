//! `extension.toml` / `manifest.toml` rules, shared by the Host and `cargo colecto`.
use crate::MAX_MANIFEST;
use colecto_extension_api::{API_VERSION, Capability, ExtensionManifest};

/// A bus request or event name: 1–64 bytes of lowercase ASCII, digits, `-`, `_` and `/`.
pub fn valid_bus_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_/".contains(&b))
}

/// Parses and checks a manifest against the rules shared by bundled and third-party
/// extensions.
pub fn parse(source: &str) -> Result<ExtensionManifest, String> {
    let manifest: ExtensionManifest = toml::from_str(source).map_err(|e| e.to_string())?;
    colecto_extension_api::validate_settings(&manifest.settings)?;
    if manifest.api_version != API_VERSION {
        return Err("Unsupported extension API version".into());
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
    for (index, capability) in manifest.capabilities.iter().enumerate() {
        if manifest.capabilities[..index].contains(capability) {
            return Err("Duplicate capability".into());
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

/// The extra rules for third-party (Wasm) packages.
pub fn validate(manifest: &ExtensionManifest) -> Result<(), String> {
    parse(&toml::to_string(manifest).map_err(|e| e.to_string())?)?;
    // Guests can only emit declarative panels or sprites.
    if !["panel", "overlay"].contains(&manifest.slot.as_str()) {
        return Err("Wasm extensions require slot = panel or overlay".into());
    }
    if toml::to_string(manifest).map_err(|e| e.to_string())?.len() > MAX_MANIFEST {
        return Err("Manifest too large".into());
    }
    Ok(())
}
