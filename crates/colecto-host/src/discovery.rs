//! Finds IMAP, SMTP and JMAP settings for an email address, for the account setup wizard.
//!
//! Methods, in the order their answers are trusted:
//! 1. Colecto's built-in list of large providers (no network request).
//! 2. Thunderbird Autoconfig served by the domain itself.
//! 3. Thunderbird's central provider database (ISPDB), which learns the domain.
//! 4. DNS SRV records (RFC 6186 and RFC 8314).
//! 5. The domain's MX record, matched against 1 and 3 (custom domains on big hosts).
//!
//! JMAP comes from `/.well-known/jmap` (RFC 8620) or a `_jmap._tcp` SRV record and is
//! added to whichever result wins. Every HTTP request is HTTPS only, size-capped and
//! short; nothing is guessed. The owner reviews every value before it is saved, which
//! is also the answer to spoofed DNS records: a lookup only ever pre-fills a form.
use hickory_resolver::{TokioResolver, proto::rr::RData};
use serde::Serialize;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const DNS_TIMEOUT: Duration = Duration::from_secs(3);
/// The whole lookup gives up after this, keeping whatever already arrived.
pub const DEADLINE: Duration = Duration::from_secs(9);
const MAX_BODY: usize = 64 * 1024;
const ISPDB: &str = "https://autoconfig.thunderbird.net/v1.1/";

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    /// TLS from the first byte.
    Tls,
    Starttls,
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub security: Security,
}

/// Where a value came from; the UI names the standard behind each one.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    ProviderList,
    Autoconfig,
    Ispdb,
    Srv,
    Mx,
    JmapWellKnown,
}

/// A provider Colecto cannot fully use with a password.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Limitation {
    /// Only OAuth sign-in, which Colecto does not support yet.
    OauthOnly,
    /// Reachable only through a local bridge app (Proton Mail Bridge).
    Bridge,
    /// No IMAP, SMTP or JMAP at all.
    NoAccess,
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Discovery {
    pub sources: Vec<Source>,
    pub provider: Option<String>,
    pub imap: Option<Server>,
    pub smtp: Option<Server>,
    pub jmap: Option<String>,
    pub username: Option<String>,
    /// The provider wants an app password instead of the normal one.
    pub app_password: bool,
    pub limitation: Option<Limitation>,
}

impl Discovery {
    fn mail(&self) -> bool {
        self.imap.is_some() && self.smtp.is_some()
    }
    fn found(&self) -> bool {
        self.mail() || self.jmap.is_some() || self.limitation.is_some()
    }
}

/// The address's domain in ASCII (IDNA) form, or `None` for anything that is not a
/// public-looking host name: IP literals, single labels and `localhost` never leave.
pub fn domain_of(address: &str) -> Option<String> {
    let (local, domain) = address.trim().rsplit_once('@')?;
    if local.is_empty() || domain.is_empty() || domain.len() > 253 {
        return None;
    }
    let url = reqwest::Url::parse(&format!("https://{domain}/")).ok()?;
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if host.starts_with('[') || host.parse::<std::net::Ipv4Addr>().is_ok() {
        return None;
    }
    let reserved = [
        "localhost",
        "local",
        "internal",
        "test",
        "invalid",
        "example",
    ];
    let tld = host.rsplit('.').next()?;
    (host.contains('.') && !reserved.contains(&tld)).then_some(host)
}

// ---- Built-in providers -----------------------------------------------------------

struct Preset {
    name: &'static str,
    domains: &'static [&'static str],
    /// MX host suffixes that identify the provider for custom domains.
    mx: &'static [&'static str],
    imap: Option<(&'static str, u16, Security)>,
    smtp: Option<(&'static str, u16)>,
    jmap: Option<&'static str>,
    app_password: bool,
    limitation: Option<Limitation>,
}

const fn preset(
    name: &'static str,
    domains: &'static [&'static str],
    mx: &'static [&'static str],
    imap: &'static str,
    smtp: (&'static str, u16),
    app_password: bool,
) -> Preset {
    Preset {
        name,
        domains,
        mx,
        imap: Some((imap, 993, Security::Tls)),
        smtp: Some(smtp),
        jmap: None,
        app_password,
        limitation: None,
    }
}

/// Large providers that offer IMAP/SMTP with a password, plus a few that cannot be
/// used that way so the wizard can say so up front.
const PRESETS: &[Preset] = &[
    preset(
        "Gmail",
        &["gmail.com", "googlemail.com"],
        &["google.com", "googlemail.com"],
        "imap.gmail.com",
        ("smtp.gmail.com", 465),
        true,
    ),
    preset(
        "Yahoo Mail",
        &[
            "yahoo.com",
            "ymail.com",
            "rocketmail.com",
            "yahoo.co.uk",
            "yahoo.de",
            "yahoo.fr",
            "yahoo.co.jp",
        ],
        &[],
        "imap.mail.yahoo.com",
        ("smtp.mail.yahoo.com", 465),
        true,
    ),
    Preset {
        smtp: Some(("smtp.mail.me.com", 587)),
        ..preset(
            "iCloud Mail",
            &["icloud.com", "me.com", "mac.com"],
            &["icloud.com"],
            "imap.mail.me.com",
            ("", 0),
            true,
        )
    },
    Preset {
        jmap: Some("https://api.fastmail.com/jmap/session"),
        ..preset(
            "Fastmail",
            &[
                "fastmail.com",
                "fastmail.fm",
                "fastmail.org",
                "fastmail.net",
                "fastmail.us",
            ],
            &["messagingengine.com"],
            "imap.fastmail.com",
            ("smtp.fastmail.com", 465),
            true,
        )
    },
    Preset {
        smtp: Some(("mail.gmx.net", 587)),
        ..preset(
            "GMX",
            &["gmx.net", "gmx.de", "gmx.at", "gmx.ch"],
            &["gmx.net"],
            "imap.gmx.net",
            ("", 0),
            false,
        )
    },
    Preset {
        smtp: Some(("mail.gmx.com", 587)),
        ..preset(
            "GMX",
            &["gmx.com", "gmx.us", "gmx.co.uk", "gmx.fr"],
            &["gmx.com"],
            "imap.gmx.com",
            ("", 0),
            false,
        )
    },
    preset(
        "mailbox.org",
        &["mailbox.org"],
        &["mailbox.org"],
        "imap.mailbox.org",
        ("smtp.mailbox.org", 465),
        false,
    ),
    preset(
        "Posteo",
        &[
            "posteo.de",
            "posteo.net",
            "posteo.org",
            "posteo.eu",
            "posteo.at",
            "posteo.ch",
        ],
        &["posteo.de"],
        "posteo.de",
        ("posteo.de", 465),
        false,
    ),
    preset(
        "Zoho Mail",
        &["zoho.com", "zohomail.com"],
        &["zoho.com"],
        "imap.zoho.com",
        ("smtp.zoho.com", 465),
        false,
    ),
    preset(
        "Yandex Mail",
        &["yandex.com", "yandex.ru", "ya.ru"],
        &["yandex.net", "yandex.ru"],
        "imap.yandex.com",
        ("smtp.yandex.com", 465),
        true,
    ),
    preset(
        "AOL Mail",
        &["aol.com", "aim.com"],
        &[],
        "imap.aol.com",
        ("smtp.aol.com", 465),
        true,
    ),
    Preset {
        name: "Outlook.com",
        domains: &[
            "outlook.com",
            "hotmail.com",
            "live.com",
            "msn.com",
            "hotmail.co.uk",
            "hotmail.fr",
            "outlook.de",
        ],
        mx: &["outlook.com"],
        imap: None,
        smtp: None,
        jmap: None,
        app_password: false,
        limitation: Some(Limitation::OauthOnly),
    },
    // Proton Mail Bridge's defaults; Bridge shows its own password and certificate.
    Preset {
        name: "Proton Mail",
        domains: &["proton.me", "protonmail.com", "protonmail.ch", "pm.me"],
        mx: &["protonmail.ch"],
        imap: Some(("127.0.0.1", 1143, Security::Starttls)),
        smtp: Some(("127.0.0.1", 1025)),
        jmap: None,
        app_password: false,
        limitation: Some(Limitation::Bridge),
    },
    Preset {
        name: "Tuta",
        domains: &[
            "tuta.com",
            "tutanota.com",
            "tutanota.de",
            "tuta.io",
            "keemail.me",
        ],
        mx: &["tutanota.de"],
        imap: None,
        smtp: None,
        jmap: None,
        app_password: false,
        limitation: Some(Limitation::NoAccess),
    },
];

fn from_preset(preset: &Preset, address: &str) -> Discovery {
    let smtp = preset.smtp.map(|(host, port)| Server {
        host: host.into(),
        port,
        security: if port == 465 {
            Security::Tls
        } else {
            Security::Starttls
        },
    });
    Discovery {
        sources: vec![Source::ProviderList],
        provider: Some(preset.name.into()),
        imap: preset.imap.map(|(host, port, security)| Server {
            host: host.into(),
            port,
            security,
        }),
        smtp,
        jmap: preset.jmap.map(Into::into),
        username: preset.imap.map(|_| address.trim().into()),
        app_password: preset.app_password,
        limitation: preset.limitation,
    }
}

fn by_domain(domain: &str) -> Option<&'static Preset> {
    PRESETS
        .iter()
        .find(|preset| preset.domains.contains(&domain))
}

fn by_mx(host: &str) -> Option<&'static Preset> {
    let host = host.trim_end_matches('.');
    PRESETS.iter().find(|preset| {
        preset
            .mx
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
    })
}

// ---- Thunderbird Autoconfig ---------------------------------------------------------

fn expand(template: &str, address: &str) -> String {
    let address = address.trim();
    let (local, domain) = address.rsplit_once('@').unwrap_or((address, ""));
    template
        .replace("%EMAILADDRESS%", address)
        .replace("%EMAILLOCALPART%", local)
        .replace("%EMAILDOMAIN%", domain)
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// Parses a `config-v1.1.xml` document. Takes the first IMAP and SMTP server Colecto
/// can use: encrypted, with password sign-in, and for SMTP a port whose TLS mode
/// matches Colecto's rule (465 is TLS, every other port STARTTLS).
pub fn parse_autoconfig(xml: &str, address: &str, source: Source) -> Option<Discovery> {
    let document = roxmltree::Document::parse(xml).ok()?;
    let provider = document
        .descendants()
        .find(|n| n.has_tag_name("emailProvider"))?;
    let text = |node: roxmltree::Node, tag: &str| {
        node.children()
            .find(|n| n.has_tag_name(tag))
            .and_then(|n| n.text())
            .map(str::trim)
            .unwrap_or("")
            .to_string()
    };
    let mut result = Discovery {
        sources: vec![source],
        ..Default::default()
    };
    let name = text(provider, "displayName");
    if !name.is_empty() && name.chars().count() <= 80 {
        result.provider = Some(name);
    }
    let mut oauth_only = false;
    for (tag, kind) in [("incomingServer", "imap"), ("outgoingServer", "smtp")] {
        for server in provider
            .children()
            .filter(|n| n.has_tag_name(tag) && n.attribute("type") == Some(kind))
        {
            let auth: Vec<String> = server
                .children()
                .filter(|n| n.has_tag_name("authentication"))
                .filter_map(|n| n.text())
                .map(|t| t.trim().to_string())
                .collect();
            if !auth.is_empty() && !auth.iter().any(|a| a == "password-cleartext") {
                oauth_only |= auth.iter().any(|a| a == "OAuth2");
                continue;
            }
            let host = expand(&text(server, "hostname"), address).to_ascii_lowercase();
            let Ok(port) = text(server, "port").parse::<u16>() else {
                continue;
            };
            let security = match (text(server, "socketType").as_str(), kind, port) {
                ("SSL", "imap", _) | ("SSL", "smtp", 465) => Security::Tls,
                ("STARTTLS", "imap", _) => Security::Starttls,
                ("STARTTLS", "smtp", port) if port != 465 => Security::Starttls,
                _ => continue,
            };
            if !valid_host(&host) || port == 0 {
                continue;
            }
            let found = Server {
                host,
                port,
                security,
            };
            if kind == "imap" {
                let username = expand(&text(server, "username"), address);
                result.username =
                    (!username.is_empty() && username.len() <= 320).then_some(username);
                result.imap = Some(found);
            } else {
                result.smtp = Some(found);
            }
            break;
        }
    }
    if !result.mail() {
        result.imap = None;
        result.smtp = None;
        result.username = None;
        if !oauth_only {
            return None;
        }
        result.limitation = Some(Limitation::OauthOnly);
    }
    Some(result)
}

// ---- HTTP ----------------------------------------------------------------------------

fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent(concat!("colecto/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()
}

async fn capped(mut response: reqwest::Response) -> Option<String> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if body.len() + chunk.len() > MAX_BODY {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).ok()
}

async fn fetch_autoconfig(
    client: &reqwest::Client,
    url: String,
    address: &str,
    source: Source,
) -> Option<Discovery> {
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    parse_autoconfig(&capped(response).await?, address, source)
}

/// The domain's own autoconfig: the subdomain first, then the well-known path.
async fn provider_autoconfig(
    client: &reqwest::Client,
    domain: &str,
    address: &str,
) -> Option<Discovery> {
    let query = format!("?emailaddress={}", encode(address.trim()));
    let first = fetch_autoconfig(
        client,
        format!("https://autoconfig.{domain}/mail/config-v1.1.xml{query}"),
        address,
        Source::Autoconfig,
    );
    let second = fetch_autoconfig(
        client,
        format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml{query}"),
        address,
        Source::Autoconfig,
    );
    let (first, second) = tokio::join!(first, second);
    first.or(second)
}

/// Thunderbird's database is asked about the domain only, never the full address.
async fn ispdb(client: &reqwest::Client, domain: &str, address: &str) -> Option<Discovery> {
    fetch_autoconfig(client, format!("{ISPDB}{domain}"), address, Source::Ispdb).await
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// A JMAP session answers with its capabilities, or asks for credentials.
async fn jmap_well_known(client: &reqwest::Client, origin: &str) -> Option<String> {
    let response = client
        .get(format!("{origin}/.well-known/jmap"))
        .send()
        .await
        .ok()?;
    let url = response.url().to_string();
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return response
            .headers()
            .contains_key(reqwest::header::WWW_AUTHENTICATE)
            .then_some(url);
    }
    if !response.status().is_success() {
        return None;
    }
    let session: serde_json::Value = serde_json::from_str(&capped(response).await?).ok()?;
    session
        .get("capabilities")?
        .get("urn:ietf:params:jmap:core")?;
    Some(url)
}

// ---- DNS -----------------------------------------------------------------------------

fn resolver() -> Option<TokioResolver> {
    let mut builder = TokioResolver::builder_tokio().ok()?;
    builder.options_mut().timeout = DNS_TIMEOUT;
    builder.options_mut().attempts = 1;
    builder.build().ok()
}

/// SRV targets as (priority, weight, host, port), with "." (service absent) removed.
async fn srv(resolver: &TokioResolver, name: String) -> Vec<(u16, u16, String, u16)> {
    let Ok(lookup) = resolver.srv_lookup(name).await else {
        return Vec::new();
    };
    lookup
        .answers()
        .iter()
        .filter_map(|record| match &record.data {
            RData::SRV(srv) => Some((
                srv.priority,
                srv.weight,
                srv.target
                    .to_ascii()
                    .trim_end_matches('.')
                    .to_ascii_lowercase(),
                srv.port,
            )),
            _ => None,
        })
        .filter(|(_, _, host, port)| valid_host(host) && *port != 0)
        .collect()
}

/// The preferred record: lowest priority, then highest weight.
fn best(mut records: Vec<(u16, u16, String, u16)>) -> Option<(String, u16)> {
    records.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    records
        .into_iter()
        .next()
        .map(|(_, _, host, port)| (host, port))
}

async fn srv_mail(resolver: &TokioResolver, domain: &str, address: &str) -> Option<Discovery> {
    let (imaps, imap, submissions, submission) = tokio::join!(
        srv(resolver, format!("_imaps._tcp.{domain}.")),
        srv(resolver, format!("_imap._tcp.{domain}.")),
        srv(resolver, format!("_submissions._tcp.{domain}.")),
        srv(resolver, format!("_submission._tcp.{domain}.")),
    );
    let imap = best(imaps)
        .map(|(host, port)| Server {
            host,
            port,
            security: Security::Tls,
        })
        .or_else(|| {
            best(imap).map(|(host, port)| Server {
                host,
                port,
                security: Security::Starttls,
            })
        })?;
    // Colecto speaks implicit TLS for SMTP only on 465.
    let smtp = best(submissions.into_iter().filter(|r| r.3 == 465).collect())
        .map(|(host, port)| Server {
            host,
            port,
            security: Security::Tls,
        })
        .or_else(|| {
            best(submission.into_iter().filter(|r| r.3 != 465).collect()).map(|(host, port)| {
                Server {
                    host,
                    port,
                    security: Security::Starttls,
                }
            })
        })?;
    Some(Discovery {
        sources: vec![Source::Srv],
        imap: Some(imap),
        smtp: Some(smtp),
        username: Some(address.trim().into()),
        ..Default::default()
    })
}

async fn srv_jmap(resolver: &TokioResolver, domain: &str) -> Option<String> {
    let (host, port) = best(srv(resolver, format!("_jmap._tcp.{domain}.")).await)?;
    Some(if port == 443 {
        format!("https://{host}/.well-known/jmap")
    } else {
        format!("https://{host}:{port}/.well-known/jmap")
    })
}

async fn mx_host(resolver: &TokioResolver, domain: &str) -> Option<String> {
    let lookup = resolver.mx_lookup(format!("{domain}.")).await.ok()?;
    let mut records: Vec<(u16, String)> = lookup
        .answers()
        .iter()
        .filter_map(|record| match &record.data {
            RData::MX(mx) => Some((
                mx.preference,
                mx.exchange
                    .to_ascii()
                    .trim_end_matches('.')
                    .to_ascii_lowercase(),
            )),
            _ => None,
        })
        .filter(|(_, host)| valid_host(host))
        .collect();
    records.sort();
    records.into_iter().next().map(|(_, host)| host)
}

/// The registrable part of a mail exchanger, e.g. `aspmx.l.google.com` → `google.com`.
/// A short heuristic, not the public suffix list: two labels, or three under a
/// two-letter country code with a generic second level (`example.co.uk`).
pub fn base_domain(host: &str) -> String {
    let labels: Vec<&str> = host.trim_end_matches('.').split('.').collect();
    let keep = match labels.as_slice() {
        [.., second, tld]
            if tld.len() == 2
                && ["co", "com", "net", "org", "ac", "gov", "edu", "ne", "or"].contains(second) =>
        {
            3
        }
        _ => 2,
    };
    labels[labels.len().saturating_sub(keep)..].join(".")
}

// ---- Putting it together ---------------------------------------------------------------

/// Looks up settings for `address`. `None` means nothing usable was found.
pub async fn discover(address: &str) -> Option<Discovery> {
    let domain = domain_of(address)?;
    if let Some(preset) = by_domain(&domain) {
        return Some(from_preset(preset, address));
    }
    let client = client()?;
    let resolver = resolver();
    let origin = format!("https://{domain}");
    let srv_mail = async {
        match &resolver {
            Some(resolver) => srv_mail(resolver, &domain, address).await,
            None => None,
        }
    };
    let jmap = async {
        let srv = async {
            match &resolver {
                Some(resolver) => srv_jmap(resolver, &domain).await,
                None => None,
            }
        };
        let (well_known, srv) = tokio::join!(jmap_well_known(&client, &origin), srv);
        well_known
            .map(|url| (url, Source::JmapWellKnown))
            .or(srv.map(|url| (url, Source::Srv)))
    };
    let mx = async {
        let host = mx_host(resolver.as_ref()?, &domain).await?;
        if let Some(preset) = by_mx(&host) {
            let mut found = from_preset(preset, address);
            found.sources = vec![Source::Mx, Source::ProviderList];
            return Some(found);
        }
        let base = base_domain(&host);
        if base == domain {
            return None;
        }
        let mut found = ispdb(&client, &base, address).await?;
        found.sources = vec![Source::Mx, Source::Ispdb];
        Some(found)
    };
    let (own, central, srv_mail, jmap, mx) = tokio::join!(
        provider_autoconfig(&client, &domain, address),
        ispdb(&client, &domain, address),
        srv_mail,
        jmap,
        mx
    );
    let mut result = [own, central, srv_mail, mx]
        .into_iter()
        .flatten()
        .find(Discovery::mail)
        .unwrap_or_default();
    if let Some((url, source)) = jmap {
        result.jmap = Some(url);
        result.sources.push(source);
    }
    result.found().then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FASTMAIL_LIKE: &str = r#"<?xml version="1.0"?>
<clientConfig version="1.1">
  <emailProvider id="example.org">
    <domain>example.org</domain>
    <displayName>Example Mail</displayName>
    <incomingServer type="pop3"><hostname>pop.example.org</hostname><port>995</port><socketType>SSL</socketType></incomingServer>
    <incomingServer type="imap"><hostname>imap.example.org</hostname><port>143</port><socketType>plain</socketType><authentication>password-cleartext</authentication></incomingServer>
    <incomingServer type="imap"><hostname>imap.%EMAILDOMAIN%</hostname><port>993</port><socketType>SSL</socketType><username>%EMAILLOCALPART%</username><authentication>password-cleartext</authentication></incomingServer>
    <outgoingServer type="smtp"><hostname>smtp.example.org</hostname><port>587</port><socketType>SSL</socketType><authentication>password-cleartext</authentication></outgoingServer>
    <outgoingServer type="smtp"><hostname>smtp.example.org</hostname><port>587</port><socketType>STARTTLS</socketType><username>%EMAILADDRESS%</username><authentication>password-cleartext</authentication></outgoingServer>
  </emailProvider>
</clientConfig>"#;

    #[test]
    fn autoconfig_skips_plain_and_mismatched_tls() {
        let found = parse_autoconfig(FASTMAIL_LIKE, "Jo@Example.org", Source::Autoconfig).unwrap();
        assert_eq!(found.provider.as_deref(), Some("Example Mail"));
        assert_eq!(
            found.imap,
            Some(Server {
                host: "imap.example.org".into(),
                port: 993,
                security: Security::Tls
            })
        );
        // Implicit TLS on 587 is not something Colecto can speak, so STARTTLS wins.
        assert_eq!(
            found.smtp,
            Some(Server {
                host: "smtp.example.org".into(),
                port: 587,
                security: Security::Starttls
            })
        );
        assert_eq!(found.username.as_deref(), Some("Jo"));
    }

    #[test]
    fn autoconfig_reports_oauth_only_providers() {
        let xml = r#"<clientConfig><emailProvider id="x"><incomingServer type="imap"><hostname>imap.x.com</hostname><port>993</port><socketType>SSL</socketType><authentication>OAuth2</authentication></incomingServer><outgoingServer type="smtp"><hostname>smtp.x.com</hostname><port>587</port><socketType>STARTTLS</socketType><authentication>OAuth2</authentication></outgoingServer></emailProvider></clientConfig>"#;
        let found = parse_autoconfig(xml, "a@x.com", Source::Ispdb).unwrap();
        assert_eq!(found.limitation, Some(Limitation::OauthOnly));
        assert!(found.imap.is_none());
    }

    #[test]
    fn autoconfig_rejects_dtds_and_bad_hosts() {
        assert!(
            parse_autoconfig(
                r#"<!DOCTYPE x [<!ENTITY a "b">]><clientConfig/>"#,
                "a@b.c",
                Source::Ispdb
            )
            .is_none()
        );
        let xml = FASTMAIL_LIKE.replace("imap.%EMAILDOMAIN%", "imap example/org");
        assert!(parse_autoconfig(&xml, "a@example.org", Source::Ispdb).is_none());
    }

    #[test]
    fn domains_are_checked_before_any_request() {
        assert_eq!(
            domain_of(" jo@Mail.Example.org "),
            Some("mail.example.org".into())
        );
        assert_eq!(domain_of("jo@bücher.de"), Some("xn--bcher-kva.de".into()));
        for bad in [
            "jo@localhost",
            "jo@127.0.0.1",
            "jo@[::1]",
            "jo@intranet",
            "@example.org",
            "jo@",
            "jo@host.local",
            "jo",
        ] {
            assert_eq!(domain_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn presets_match_by_domain_and_mx() {
        let gmail = from_preset(by_domain("gmail.com").unwrap(), "a@gmail.com");
        assert_eq!(gmail.imap.unwrap().port, 993);
        assert_eq!(gmail.smtp.unwrap().security, Security::Tls);
        assert!(gmail.app_password);
        assert_eq!(by_mx("aspmx.l.google.com.").unwrap().name, "Gmail");
        assert_eq!(
            by_mx("example-com.mail.protection.outlook.com")
                .unwrap()
                .limitation,
            Some(Limitation::OauthOnly)
        );
        assert!(by_mx("notgoogle.com").is_none());
        let icloud = from_preset(by_domain("me.com").unwrap(), "a@me.com");
        assert_eq!(icloud.smtp.unwrap().security, Security::Starttls);
        assert!(PRESETS.iter().all(|p| {
            p.domains
                .iter()
                .all(|d| by_domain(d).is_some_and(|q| q.name == p.name))
        }));
    }

    #[test]
    fn mx_base_domain_heuristic() {
        assert_eq!(base_domain("aspmx.l.google.com."), "google.com");
        assert_eq!(base_domain("mx.example.co.uk"), "example.co.uk");
        assert_eq!(base_domain("mx1.mail.de"), "mail.de");
        assert_eq!(base_domain("com"), "com");
    }

    #[test]
    fn srv_prefers_low_priority_then_high_weight() {
        let records = vec![
            (10, 5, "b".into(), 993),
            (0, 1, "c".into(), 993),
            (0, 9, "a".into(), 993),
        ];
        assert_eq!(best(records), Some(("a".into(), 993)));
    }

    /// Real lookups against the internet: `cargo test -p colecto-host discovery::tests::live -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "needs the internet"]
    async fn live() {
        for address in [
            "someone@riseup.net",
            "someone@disroot.org",
            "someone@kernel.org",
            "someone@stalw.art",
            "someone@purelymail.com",
            "someone@web.de",
            "someone@nonexistent-domain-colecto.example.com",
        ] {
            let found = tokio::time::timeout(DEADLINE, discover(address))
                .await
                .ok()
                .flatten();
            println!("{address}: {found:?}");
        }
    }
}
