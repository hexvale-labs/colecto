use colecto_core::{JmapAccount, MailAccount, MailStore};
use std::{env, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = env::var("COLECTO_PORT")
        .unwrap_or_else(|_| "4173".into())
        .parse()?;
    let directory = env::var_os("COLECTO_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".colecto-demo"));
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist");
    if !assets.join("index.html").is_file() {
        return Err("Frontend build missing; run just ui-build first".into());
    }
    let mut store = MailStore::open(&directory)?;
    let keys = [
        "COLECTO_IMAP_HOST",
        "COLECTO_IMAP_PORT",
        "COLECTO_SMTP_HOST",
        "COLECTO_SMTP_PORT",
        "COLECTO_MAIL_USERNAME",
        "COLECTO_MAIL_PASSWORD",
        "COLECTO_MAIL_ADDRESS",
    ];
    let jmap_keys = ["COLECTO_JMAP_SESSION_URL", "COLECTO_JMAP_TOKEN"];
    let jmap = jmap_keys.iter().any(|key| env::var_os(key).is_some());
    let imap = keys[..6].iter().any(|key| env::var_os(key).is_some());
    if jmap && imap {
        return Err("Configure either JMAP or IMAP/SMTP, not both".into());
    }
    if jmap {
        let required = |key: &str| env::var(key).map_err(|_| format!("Missing {key}"));
        store.connect_jmap(JmapAccount {
            session_url: required("COLECTO_JMAP_SESSION_URL")?,
            token: required("COLECTO_JMAP_TOKEN")?,
            address: required("COLECTO_MAIL_ADDRESS")?,
        })?;
    } else if keys.iter().any(|key| env::var_os(key).is_some()) {
        let required = |key: &str| env::var(key).map_err(|_| format!("Missing {key}"));
        store.connect_account(MailAccount {
            imap_host: required("COLECTO_IMAP_HOST")?,
            imap_port: env::var("COLECTO_IMAP_PORT")
                .unwrap_or_else(|_| "993".into())
                .parse()?,
            smtp_host: required("COLECTO_SMTP_HOST")?,
            smtp_port: env::var("COLECTO_SMTP_PORT")
                .unwrap_or_else(|_| "587".into())
                .parse()?,
            username: required("COLECTO_MAIL_USERNAME")?,
            password: required("COLECTO_MAIL_PASSWORD")?,
            address: required("COLECTO_MAIL_ADDRESS")?,
        })?;
    }
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let authority = listener.local_addr()?.to_string();
    println!("Colecto local demo: http://{authority}");
    println!("Draft directory: {}", directory.display());
    axum::serve(listener, colecto_host::app(store, authority, assets))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
