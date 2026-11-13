//! Checks GitHub for a newer stable Colecto release on behalf of the UI, whose content
//! security policy only allows its own origin. One answer is cached for the whole process,
//! so opening the app many times makes at most one request every few hours.
use serde::{Deserialize, Serialize};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

const LATEST: &str = "https://api.github.com/repos/hexvale-labs/colecto/releases/latest";
const RELEASE_PAGES: &str = "https://github.com/hexvale-labs/colecto/releases/";
const FRESH: Duration = Duration::from_secs(6 * 60 * 60);
const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(30 * 60);
const TIMEOUT: Duration = Duration::from_secs(8);
const MAX_BODY: usize = 1024 * 1024;
/// Set to `off` to never contact GitHub (tests, air-gapped servers).
pub const DISABLE_ENV: &str = "COLECTO_UPDATE_CHECK";

/// A stable release newer than the running build.
#[derive(Clone, Serialize, PartialEq, Eq, Debug)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

type Cached = (Instant, Result<Option<Release>, String>);
static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

/// `major.minor.patch`, with an optional leading `v`. Pre-release and build suffixes
/// are dropped from the running version and rejected in a published tag.
fn parse(text: &str, allow_suffix: bool) -> Option<(u64, u64, u64)> {
    let text = text.strip_prefix('v').unwrap_or(text);
    let core = match text.find(['-', '+']) {
        Some(_) if !allow_suffix => return None,
        Some(index) => &text[..index],
        None => text,
    };
    let mut parts = core.split('.').map(|part| part.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// The release, when it is stable, newer than `current` and links to this repository.
fn newer(current: &str, release: GitHubRelease) -> Option<Release> {
    if release.draft || release.prerelease || !release.html_url.starts_with(RELEASE_PAGES) {
        return None;
    }
    let latest = parse(&release.tag_name, false)?;
    (latest > parse(current, true)?).then(|| Release {
        version: format!("{}.{}.{}", latest.0, latest.1, latest.2),
        url: release.html_url,
    })
}

fn fetch(current: &str) -> Result<Option<Release>, String> {
    let failed = |_| "Could not check for updates".to_string();
    let response = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(format!("colecto/{current}"))
        .build()
        .map_err(failed)?
        .get(LATEST)
        .header("accept", "application/vnd.github+json")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(failed)?;
    let body = response.bytes().map_err(failed)?;
    if body.len() > MAX_BODY {
        return Err("Could not check for updates".into());
    }
    let release: GitHubRelease =
        serde_json::from_slice(&body).map_err(|_| "Could not check for updates".to_string())?;
    Ok(newer(current, release))
}

/// The newest release if it is newer than `current`, from the cache when it is recent.
/// Blocks for up to the request timeout; call it off the async runtime.
pub fn latest(current: &str) -> Result<Option<Release>, String> {
    if std::env::var(DISABLE_ENV).is_ok_and(|value| value == "off") {
        return Ok(None);
    }
    if let Some((at, result)) = CACHE.lock().map_err(|_| "Update cache poisoned")?.as_ref() {
        let keep = if result.is_ok() {
            FRESH
        } else {
            RETRY_AFTER_FAILURE
        };
        if at.elapsed() < keep {
            return result.clone();
        }
    }
    let result = fetch(current);
    if let Err(error) = &result {
        tracing::debug!(%error, "Release check failed");
    }
    *CACHE.lock().map_err(|_| "Update cache poisoned")? = Some((Instant::now(), result.clone()));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> GitHubRelease {
        GitHubRelease {
            tag_name: tag.into(),
            html_url: format!("{RELEASE_PAGES}tag/{tag}"),
            draft: false,
            prerelease: false,
        }
    }

    #[test]
    fn only_newer_stable_releases_count() {
        let found = newer("0.3.1", release("v0.4.0")).unwrap();
        assert_eq!(found.version, "0.4.0");
        assert_eq!(found.url, format!("{RELEASE_PAGES}tag/v0.4.0"));
        assert!(newer("0.3.1", release("v0.3.10")).is_some());
        assert!(newer("0.3.1", release("1.0.0")).is_some());
        assert!(newer("0.3.1-dev", release("v0.3.1")).is_none());
        for tag in [
            "v0.3.1",
            "v0.3.0",
            "v0.2.9",
            "v0.4.0-rc.1",
            "v0.4",
            "latest",
            "v0.4.0.1",
        ] {
            assert!(newer("0.3.1", release(tag)).is_none(), "{tag}");
        }
        let mut draft = release("v0.4.0");
        draft.draft = true;
        assert!(newer("0.3.1", draft).is_none());
        let mut pre = release("v0.4.0");
        pre.prerelease = true;
        assert!(newer("0.3.1", pre).is_none());
        let mut elsewhere = release("v0.4.0");
        elsewhere.html_url = "https://example.com/colecto".into();
        assert!(newer("0.3.1", elsewhere).is_none());
    }
}
