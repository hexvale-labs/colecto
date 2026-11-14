//! Extension API versions. `api-versions.json` and `api-support.json` next to this crate's
//! `Cargo.toml` are the source of truth; they are embedded at build time.
//! See `docs/extensions/api-versions/README.md` for when and how to add a version.
use crate::Capability;
use serde::Deserialize;
use std::sync::OnceLock;

/// One extension API version, as recorded in `api-versions.json`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiVersion {
    pub version: u32,
    /// Whether an extension must change code, not only `api_version`, to move to this version.
    pub breaking: bool,
    /// The first release that shipped this version; `null` until it ships.
    pub released_in: Option<String>,
    /// Capabilities first available in this version.
    pub capabilities: Vec<Capability>,
    /// Short summary of every other change; the docs page explains each one.
    pub changes: Vec<String>,
}

/// An API version a release could no longer serve, from `api-support.json`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discontinued {
    pub version: u32,
    /// The release whose notes explain why, under `## Breaking API changes`.
    pub release: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Versions {
    versions: Vec<ApiVersion>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Support {
    discontinued: Vec<Discontinued>,
}

pub fn api_versions() -> &'static [ApiVersion] {
    static VERSIONS: OnceLock<Vec<ApiVersion>> = OnceLock::new();
    VERSIONS.get_or_init(|| {
        serde_json::from_str::<Versions>(include_str!("../api-versions.json"))
            .expect("valid api-versions.json")
            .versions
    })
}

pub fn discontinued_api_versions() -> &'static [Discontinued] {
    static SUPPORT: OnceLock<Vec<Discontinued>> = OnceLock::new();
    SUPPORT.get_or_init(|| {
        serde_json::from_str::<Support>(include_str!("../api-support.json"))
            .expect("valid api-support.json")
            .discontinued
    })
}

/// The newest API version this build knows.
pub fn latest_api_version() -> u32 {
    api_versions().last().map_or(0, |v| v.version)
}

#[derive(Debug, PartialEq, Eq)]
pub enum ApiSupport {
    Supported,
    /// Dropped by this release tag.
    Discontinued(&'static str),
    /// Newer than this build: the extension needs a newer Colecto.
    Newer,
    Unknown,
}

pub fn api_support(version: u32) -> ApiSupport {
    if let Some(d) = discontinued_api_versions()
        .iter()
        .find(|d| d.version == version)
    {
        ApiSupport::Discontinued(&d.release)
    } else if version > latest_api_version() {
        ApiSupport::Newer
    } else if version == 0 {
        ApiSupport::Unknown
    } else {
        ApiSupport::Supported
    }
}

/// The API version that introduced a capability, or `None` if extensions never get it.
pub fn capability_since(capability: Capability) -> Option<u32> {
    api_versions()
        .iter()
        .find(|v| v.capabilities.contains(&capability))
        .map(|v| v.version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn registry_is_sequential_and_documented() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/extensions/api-versions");
        let mut pages: Vec<String> = std::fs::read_dir(&docs)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with("API-"))
            .collect();
        pages.sort();
        let mut expected: Vec<String> = api_versions()
            .iter()
            .map(|v| {
                let tag = if v.breaking { "-breaking" } else { "" };
                format!("API-{}{tag}.md", v.version)
            })
            .collect();
        expected.sort();
        assert_eq!(
            pages, expected,
            "one docs/extensions/api-versions/API-<n>[-breaking].md per version"
        );
        for (index, version) in api_versions().iter().enumerate() {
            assert_eq!(
                version.version as usize,
                index + 1,
                "versions count 1, 2, 3, …"
            );
        }
        // Unreleased versions come after every released one.
        let first_unreleased = api_versions().iter().position(|v| v.released_in.is_none());
        if let Some(first) = first_unreleased {
            assert!(
                api_versions()[first..]
                    .iter()
                    .all(|v| v.released_in.is_none())
            );
        }
    }

    #[test]
    fn every_extension_capability_has_one_version() {
        use Capability::*;
        // No wildcard: a new capability fails to compile here until it is given a version.
        let for_extensions = |c: Capability| match c {
            ExtensionsBus | AccountRead | MessagesList | MessagesRead | DraftsList | DraftsRead
            | DraftsWrite | MessagesSend | MessagesLabel | MessagesOrganize | WindowOverlay => true,
            AppNavigate => false,
        };
        let all = [
            ExtensionsBus,
            AccountRead,
            MessagesList,
            MessagesRead,
            DraftsList,
            DraftsRead,
            DraftsWrite,
            MessagesSend,
            MessagesLabel,
            MessagesOrganize,
            WindowOverlay,
            AppNavigate,
        ];
        for capability in all.into_iter().filter(|&c| for_extensions(c)) {
            let name = serde_json::to_string(&capability).unwrap();
            let count = api_versions()
                .iter()
                .filter(|v| v.capabilities.contains(&capability))
                .count();
            assert_eq!(
                count, 1,
                "{name} must be introduced by exactly one API version"
            );
        }
        assert_eq!(capability_since(AppNavigate), None);
    }

    #[test]
    fn discontinued_versions_were_released() {
        for d in discontinued_api_versions() {
            let version = api_versions().iter().find(|v| v.version == d.version);
            assert!(
                version.is_some_and(|v| v.released_in.is_some()),
                "API {} was never released",
                d.version
            );
            assert!(
                d.release.starts_with('v'),
                "release is a tag such as v1.2.3"
            );
        }
        assert_eq!(api_support(0), ApiSupport::Unknown);
        assert_eq!(api_support(latest_api_version() + 1), ApiSupport::Newer);
        assert_eq!(api_support(latest_api_version()), ApiSupport::Supported);
    }
}
